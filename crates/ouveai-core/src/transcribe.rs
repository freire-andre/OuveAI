//! Transcrição de arquivo (Partes 2 e 3 do prompt.txt):
//! FFmpeg → WAV 16 kHz mono (temporário) → `silencedetect` → pedaços de até
//! 60 s (corte num silêncio entre 45 s e 60 s; ver `media::CHUNK_MAX`) → reconhecedor → palavras (regra 2) → `.json` / `.txt` / `.srt`.

use crate::asr::{Asr, SAMPLE_RATE};
use crate::media::{self, MediaError};
use crate::suspects::{self, Suspect};
use crate::words::{self, Pause, Word};
use crate::subs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// O conteúdo do `.json` (formato do prompt.txt).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transcript {
    pub source: String,
    pub duration: f64,
    pub words: Vec<Word>,
    pub pauses: Vec<Pause>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "stage", rename_all = "lowercase")]
pub enum Stage {
    Converting,
    Analyzing,
    Recognizing { chunk: usize, chunks: usize, done: f64, total: f64 },
    Saving,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub duration: f64,
    pub convert_s: f64,
    /// Tempo só do reconhecedor (sem a conversão e a análise).
    pub transcribe_s: f64,
    /// Duração do áudio ÷ tempo de transcrição.
    pub rtf: f64,
    pub chunks: usize,
    /// Cortes sem silêncio na janela (feitos no ponto de menor volume).
    pub forced_cuts: Vec<f64>,
    pub words: usize,
    pub suspects: Vec<Suspect>,
    pub suspects_per_min: f64,
}

#[derive(Debug)]
pub enum Error {
    Media(MediaError),
    Cancelled,
    Invalid(String),
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Media(e) => write!(f, "{e}"),
            Error::Cancelled => write!(f, "cancelado"),
            Error::Invalid(e) => write!(f, "{e}"),
            Error::Io(e) => write!(f, "erro de disco: {e}"),
        }
    }
}

impl From<MediaError> for Error {
    fn from(e: MediaError) -> Self {
        match e {
            MediaError::Cancelled => Error::Cancelled,
            e => Error::Media(e),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Transcreve o arquivo. O reconhecedor já vem carregado (o tempo de carga é
/// medido por quem carrega). Nada é gravado em disco além do WAV temporário.
pub fn transcribe(input: &Path, ff: &Path, asr: &Asr, cancel: &AtomicBool, on: &mut dyn FnMut(Stage)) -> Result<(Transcript, Report), Error> {
    transcribe_with(input, ff, asr, cancel, media::ChunkPlan::default(), on)
}

pub fn transcribe_with(
    input: &Path,
    ff: &Path,
    asr: &Asr,
    cancel: &AtomicBool,
    plan: media::ChunkPlan,
    on: &mut dyn FnMut(Stage),
) -> Result<(Transcript, Report), Error> {
    let t0 = Instant::now();
    on(Stage::Converting);
    let wav = media::to_wav(ff, input, cancel)?;
    let wave = sherpa_onnx::Wave::read(&wav.path.to_string_lossy()).ok_or_else(|| Error::Invalid("não foi possível ler o áudio convertido".into()))?;
    if wave.sample_rate() != SAMPLE_RATE {
        return Err(Error::Invalid(format!("taxa inesperada: {} Hz", wave.sample_rate())));
    }
    let samples = wave.samples();
    let duration = samples.len() as f64 / SAMPLE_RATE as f64;
    if samples.is_empty() {
        return Err(Error::Invalid("o arquivo não tem áudio".into()));
    }

    on(Stage::Analyzing);
    let pauses = media::silences(ff, &wav.path, duration, cancel)?;
    let convert_s = t0.elapsed().as_secs_f64();
    let chunks = media::plan_chunks_with(samples, SAMPLE_RATE as u32, &pauses, plan);

    let t1 = Instant::now();
    let mut all: Vec<Word> = Vec::new();
    for (i, c) in chunks.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        on(Stage::Recognizing { chunk: i + 1, chunks: chunks.len(), done: c.start, total: duration });
        let (a, b) = ((c.start * SAMPLE_RATE as f64) as usize, ((c.end * SAMPLE_RATE as f64) as usize).min(samples.len()));
        let toks = asr.recognize(&samples[a..b]).map_err(Error::Invalid)?;
        // Offset do pedaço somado aos tempos das palavras (as pausas já estão no eixo do arquivo).
        all.extend(words::assemble(&toks, c.start, c.end, &pauses));
    }
    let transcribe_s = t1.elapsed().as_secs_f64();
    drop(wav); // apaga o WAV temporário

    let sus = suspects::find(&all);
    let report = Report {
        duration,
        convert_s,
        transcribe_s,
        rtf: duration / transcribe_s.max(1e-6),
        chunks: chunks.len(),
        forced_cuts: chunks.iter().filter(|c| c.forced).map(|c| c.end).collect(),
        words: all.len(),
        suspects_per_min: sus.len() as f64 / (duration / 60.0).max(1.0 / 60.0),
        suspects: sus,
    };
    let source = input.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok((Transcript { source, duration, words: all, pauses }, report))
}

/// `.json` com 3 casas e uma palavra por linha (fácil de corrigir à mão).
pub fn to_json(t: &Transcript) -> String {
    let q = |s: &str| serde_json::to_string(s).unwrap();
    let mut s = String::new();
    s.push_str(&format!("{{\n  \"source\": {},\n  \"duration\": {:.3},\n  \"words\": [", q(&t.source), t.duration));
    for (i, w) in t.words.iter().enumerate() {
        s.push_str(if i == 0 { "\n" } else { ",\n" });
        s.push_str(&format!("    {{\"word\": {}, \"start\": {:.3}, \"end\": {:.3}}}", q(&w.word), w.start, w.end));
    }
    s.push_str(if t.words.is_empty() { "],\n  \"pauses\": [" } else { "\n  ],\n  \"pauses\": [" });
    for (i, p) in t.pauses.iter().enumerate() {
        s.push_str(if i == 0 { "\n" } else { ",\n" });
        s.push_str(&format!("    {{\"start\": {:.3}, \"end\": {:.3}}}", p.start, p.end));
    }
    s.push_str(if t.pauses.is_empty() { "]\n}\n" } else { "\n  ]\n}\n" });
    s
}

pub fn read_json(path: &Path) -> Result<Transcript, Error> {
    let text = fs::read_to_string(path)?;
    let mut t: Transcript = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| Error::Invalid(format!("JSON inválido: {e}")))?;
    // Os tempos são do arquivo original e nunca mudam; só garante 3 casas.
    for w in &mut t.words {
        w.start = words::ms(w.start);
        w.end = words::ms(w.end);
    }
    Ok(t)
}

pub struct Outputs {
    pub json: PathBuf,
    pub txt: PathBuf,
    pub srt: PathBuf,
}

/// Mesmo nome-base da entrada, ao lado dela.
pub fn outputs_for(input: &Path) -> Outputs {
    Outputs { json: input.with_extension("json"), txt: input.with_extension("txt"), srt: input.with_extension("srt") }
}

pub fn check_path(input: &Path) -> PathBuf {
    input.with_extension("check.json")
}

/// Grava `.txt` e `.srt` a partir das palavras (usado depois de corrigir o texto).
pub fn write_txt_srt(t: &Transcript, out: &Outputs) -> Result<(), Error> {
    fs::write(&out.txt, subs::to_txt(&t.words, &t.pauses))?;
    fs::write(&out.srt, subs::to_srt(&subs::segment(&t.words, &t.pauses)))?;
    Ok(())
}

pub fn write_all(t: &Transcript, out: &Outputs) -> Result<(), Error> {
    fs::write(&out.json, to_json(t))?;
    write_txt_srt(t, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_ida_e_volta_com_3_casas() {
        let t = Transcript {
            source: "vídeo \"1\".mp4".into(),
            duration: 12.3456,
            words: vec![Word { word: "Olá,".into(), start: 0.08, end: 0.4000001 }],
            pauses: vec![Pause { start: 0.5, end: 1.25 }],
        };
        let s = to_json(&t);
        assert!(s.contains("\"duration\": 12.346"), "{s}");
        assert!(s.contains("{\"word\": \"Olá,\", \"start\": 0.080, \"end\": 0.400}"), "{s}");
        let dir = std::env::temp_dir().join(format!("ouveai-json-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join("a.json");
        fs::write(&p, &s).unwrap();
        let back = read_json(&p).unwrap();
        assert_eq!(back.source, t.source);
        assert_eq!(back.words[0].end, 0.4);
        let empty = Transcript { source: "x".into(), duration: 1.0, words: vec![], pauses: vec![] };
        assert!(serde_json::from_str::<Transcript>(&to_json(&empty)).is_ok());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn saidas_ao_lado_da_entrada() {
        let o = outputs_for(Path::new(r"C:\v\aula.final.mp4"));
        assert_eq!(o.json, Path::new(r"C:\v\aula.final.json"));
        assert_eq!(check_path(Path::new(r"C:\v\aula.mp4")), Path::new(r"C:\v\aula.check.json"));
    }
}
