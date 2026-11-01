//! Parte 4: "Comparar com o Whisper" numa sessão salva.
//!
//! - Arquivo: relê o arquivo original pelo FFmpeg e compara janelas de 20 s com as
//!   palavras atuais da sessão (inclusive as já corrigidas).
//! - Reunião: compara cada segmento com o trecho do `eu.wav`/`outros.wav` salvo. Ecos
//!   ficam de fora. Sem áudio salvo, não há o que comparar.
//! - Uma comparação por vez. Não começa com gravação em andamento nem com a fila de
//!   arquivos rodando (os dois modelos juntos passam de 1,5 GB de RAM), e para se uma
//!   gravação começar no meio (a reunião tem prioridade, L18).
//! - O resultado fica em memória até ser aplicado, descartado ou o app fechar.
//! - Regra 8: o log leva só números, nunca texto nem nome de arquivo.

use crate::state::AppState;
use ouveai_core::compare::{self, Divergence, Piece};
use ouveai_core::models::{self, Presence};
use ouveai_core::store;
use ouveai_core::{asr, media, wav};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

/// Erro que a UI trata à parte: falta baixar o Whisper.
pub const NEED_WHISPER: &str = "whisper";

#[derive(Clone, serde::Serialize)]
pub struct CompareResult {
    pub divergences: Vec<Divergence>,
    /// `(início, fim)` dos trechos em que o Whisper falhou e que foram descartados.
    pub unreliable: Vec<(f64, f64)>,
    /// Trechos comparados e pulados (sem fala, eco ou longos demais).
    pub compared: usize,
    pub skipped: usize,
    pub whisper_s: f64,
    pub audio_s: f64,
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Phase {
    Loading,
    Converting,
    Comparing { done: usize, total: usize },
    /// A pessoa pediu para cancelar; o trecho em andamento ainda termina (até ~10 s).
    Cancelling,
    Done { result: CompareResult },
    Failed { error: String },
}

#[derive(Clone, serde::Serialize)]
pub struct CompareView {
    pub session_id: i64,
    #[serde(flatten)]
    pub phase: Phase,
}

#[derive(Default)]
pub struct Compare {
    view: Mutex<Option<CompareView>>,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
}

fn set(app: &AppHandle, id: i64, phase: Phase) {
    let c = app.state::<Compare>();
    let cancelling = c.cancel.lock().unwrap().as_ref().is_some_and(|f| f.load(Ordering::SeqCst));
    let phase = match phase {
        Phase::Loading | Phase::Converting | Phase::Comparing { .. } if cancelling => Phase::Cancelling,
        p => p,
    };
    let v = CompareView { session_id: id, phase };
    *app.state::<Compare>().view.lock().unwrap() = Some(v.clone());
    let _ = app.emit("compare", Some(v));
}

pub fn view(app: &AppHandle) -> Option<CompareView> {
    app.state::<Compare>().view.lock().unwrap().clone()
}

pub fn running(app: &AppHandle) -> bool {
    app.state::<Compare>().cancel.lock().unwrap().is_some()
}

pub fn start(app: &AppHandle, id: i64) -> Result<(), String> {
    if models::presence(&models::whisper()) != Presence::Ready {
        return Err(NEED_WHISPER.into());
    }
    let st = app.state::<AppState>();
    if st.is_recording() {
        return Err("termine a gravação antes de comparar".into());
    }
    if crate::files::busy(app) {
        return Err("espere a importação de arquivos terminar antes de comparar".into());
    }
    let c = app.state::<Compare>();
    let cancel = {
        let mut slot = c.cancel.lock().unwrap();
        if slot.is_some() {
            return Err("já há uma comparação em andamento".into());
        }
        let flag = Arc::new(AtomicBool::new(false));
        *slot = Some(flag.clone());
        flag
    };
    set(app, id, Phase::Loading);
    let app = app.clone();
    std::thread::spawn(move || {
        let t = Instant::now();
        let r = run(&app, id, &cancel);
        *app.state::<Compare>().cancel.lock().unwrap() = None;
        match r {
            Ok(result) => {
                log::info!(
                    "comparação: {:.0} s de áudio em {:.1} s; {} trechos divergentes, {} descartados",
                    result.audio_s,
                    t.elapsed().as_secs_f64(),
                    result.divergences.len(),
                    result.unreliable.len()
                );
                set(&app, id, Phase::Done { result });
            }
            Err(_) if cancel.load(Ordering::SeqCst) && !app.state::<AppState>().is_recording() => {
                log::info!("comparação cancelada após {:.1} s", t.elapsed().as_secs_f64());
                *app.state::<Compare>().view.lock().unwrap() = None;
                let _ = app.emit("compare", None::<CompareView>);
            }
            Err(error) => {
                log::warn!("comparação não concluída após {:.1} s", t.elapsed().as_secs_f64());
                set(&app, id, Phase::Failed { error });
            }
        }
    });
    Ok(())
}

pub fn cancel(app: &AppHandle) {
    let c = app.state::<Compare>();
    let Some(flag) = c.cancel.lock().unwrap().clone() else { return };
    flag.store(true, Ordering::SeqCst);
    let id = c.view.lock().unwrap().as_ref().map(|v| v.session_id);
    if let Some(id) = id {
        set(app, id, Phase::Cancelling);
    }
}

/// Descarta o resultado (ou o erro) guardado.
pub fn dismiss(app: &AppHandle) {
    if !running(app) {
        *app.state::<Compare>().view.lock().unwrap() = None;
        let _ = app.emit("compare", None::<CompareView>);
    }
}

fn run(app: &AppHandle, id: i64, cancel: &AtomicBool) -> Result<CompareResult, String> {
    let st = app.state::<AppState>();
    let session = st.store.lock().unwrap().session(id).map_err(|e| e.to_string())?.ok_or("sessão não encontrada")?;
    // Vigia a gravação: se ela começar, a comparação para.
    let stop = || cancel.load(Ordering::Relaxed) || st.is_recording();
    let stopped = || if st.is_recording() { "comparação interrompida: a gravação começou".to_string() } else { "comparação cancelada".to_string() };

    let whisper = asr::Whisper::load(asr::file_threads())?;
    if session.kind == "file" {
        let src = PathBuf::from(session.source_path.ok_or("esta sessão não guarda o caminho do arquivo")?);
        if !src.is_file() {
            return Err("o arquivo original não foi encontrado (foi movido ou apagado?)".into());
        }
        let t = st.store.lock().unwrap().transcript(id).map_err(|e| e.to_string())?.ok_or("sessão não encontrada")?;
        let ff = media::find().ok_or("o FFmpeg não foi encontrado (veja Modelos › Ferramentas)")?;
        set(app, id, Phase::Converting);
        let samples = compare::file_samples(&ff.path, &src, cancel).map_err(|e| if stop() { stopped() } else { e.to_string() })?;
        let ranges = compare::file_ranges(&samples, &t.pauses, compare::WHISPER_CHUNK);
        let watch = AtomicBool::new(false);
        let pass = compare::whisper_pass(&whisper, &samples, &ranges, &t.words, &watch, &mut |done, total| {
            if stop() {
                watch.store(true, Ordering::Relaxed);
            }
            set(app, id, Phase::Comparing { done, total });
        })
        .map_err(|e| if stop() { stopped() } else { e })?;
        let o = compare::diverge_pass(&t.words, &pass);
        let skipped = pass.skipped();
        Ok(CompareResult {
            divergences: o.divergences,
            unreliable: o.unreliable,
            compared: pass.windows.len() - skipped,
            skipped,
            whisper_s: pass.secs,
            audio_s: samples.len() as f64 / asr::SAMPLE_RATE as f64,
        })
    } else {
        let dir = st.store.lock().unwrap().audio(id).map_err(|e| e.to_string())?.ok_or("esta reunião não tem áudio salvo")?.0;
        let dir = PathBuf::from(dir);
        let (segs, words) = {
            let s = st.store.lock().unwrap();
            (s.segments(id).map_err(|e| e.to_string())?, s.segment_words(id).map_err(|e| e.to_string())?)
        };
        meeting(app, id, &whisper, &dir, &segs, &words, &stop, &stopped)
    }
}

#[allow(clippy::too_many_arguments)]
fn meeting(
    app: &AppHandle,
    id: i64,
    whisper: &asr::Whisper,
    dir: &Path,
    segs: &[store::SegmentRow],
    rows: &[(i64, ouveai_core::words::Word)],
    stop: &dyn Fn() -> bool,
    stopped: &dyn Fn() -> String,
) -> Result<CompareResult, String> {
    const RATE: u32 = 16_000;
    const PAD: f64 = 0.25;
    let words: Vec<ouveai_core::words::Word> = rows.iter().map(|(_, w)| w.clone()).collect();
    let t0 = Instant::now();
    let (mut pieces, mut skipped, mut audio_s) = (Vec::new(), 0, 0.0);
    for (k, seg) in segs.iter().enumerate() {
        if stop() {
            return Err(stopped());
        }
        set(app, id, Phase::Comparing { done: k, total: segs.len() });
        // As palavras de um segmento são contíguas na ordem de `idx`.
        let Some(first) = rows.iter().position(|(s, _)| *s == seg.id) else { continue };
        let n = rows[first..].iter().take_while(|(s, _)| *s == seg.id).count();
        let span = seg.end_s - seg.start_s + 2.0 * PAD;
        if seg.flags & store::FLAG_ECHO != 0 || span > asr::WHISPER_MAX_SECONDS {
            skipped += 1;
            continue;
        }
        let file = dir.join(if seg.channel == "EU" { "eu.wav" } else { "outros.wav" });
        let clip = wav::read_clip(&file, RATE, seg.start_s - PAD, seg.end_s + PAD).map_err(|_| "o áudio salvo desta reunião não pôde ser lido".to_string())?;
        if clip.is_empty() {
            skipped += 1;
            continue;
        }
        audio_s += clip.len() as f64 / RATE as f64;
        let samples: Vec<f32> = clip.iter().map(|&v| v as f32 / 32768.0).collect();
        let text = whisper.recognize(&samples)?;
        pieces.push((first, n, compare::split_words(&text)));
    }
    set(app, id, Phase::Comparing { done: segs.len(), total: segs.len() });
    let pieces: Vec<Piece> = pieces.into_iter().map(|(first, n, ws)| Piece { words: &words[first..first + n], offset: first, whisper: Some(ws) }).collect();
    let o = compare::diverge_pieces(&pieces);
    Ok(CompareResult { divergences: o.divergences, unreliable: o.unreliable, compared: pieces.len(), skipped, whisper_s: t0.elapsed().as_secs_f64(), audio_s })
}

/// Aplica os trechos escolhidos (índices em `divergences` do resultado guardado). Confere
/// antes se o texto da sessão ainda é o da comparação. Devolve um aviso (ex.: não deu
/// para atualizar os arquivos ao lado do original).
pub fn apply(app: &AppHandle, id: i64, chosen: Vec<usize>) -> Result<Option<String>, String> {
    let result = match view(app) {
        Some(CompareView { session_id, phase: Phase::Done { result } }) if session_id == id => result,
        _ => return Err("não há comparação concluída para esta sessão".into()),
    };
    let st = app.state::<AppState>();
    let mut db = st.store.lock().unwrap();
    let words: Vec<ouveai_core::words::Word> = db.segment_words(id).map_err(|e| e.to_string())?.into_iter().map(|(_, w)| w).collect();
    let edits = edits(&words, &result.divergences, &chosen)?;
    let kind = db.session(id).map_err(|e| e.to_string())?.map(|s| s.kind).unwrap_or_default();
    db.replace_words(id, &edits).map_err(|e| e.to_string())?;
    drop(db);
    log::info!("comparação aplicada: {} trechos", edits.len());
    dismiss(app);
    if kind == "file" { crate::files::refresh_outputs(app, id) } else { Ok(None) }
}

fn edits(words: &[ouveai_core::words::Word], divs: &[Divergence], chosen: &[usize]) -> Result<Vec<(usize, usize, Vec<ouveai_core::words::Word>)>, String> {
    let changed = || "a transcrição mudou desde a comparação; compare de novo".to_string();
    let mut out = Vec::with_capacity(chosen.len());
    for &i in chosen {
        let d = divs.get(i).ok_or("trecho inválido")?;
        if d.to > words.len() {
            return Err(changed());
        }
        let now = words[d.from..d.to].iter().map(|w| w.word.as_str()).collect::<Vec<_>>().join(" ");
        if now != d.parakeet {
            return Err(changed());
        }
        out.push((d.from, d.to, compare::whisper_words(words, d)));
    }
    Ok(out)
}
