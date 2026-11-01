//! Transcrição ao vivo (6.4 do plano), por canal:
//! áudio 16 kHz → Silero VAD → segmento (~800 ms de silêncio, ou 20 s cortado no
//! ponto de menor energia, L17) → fila única → Parakeet → palavras com o offset
//! da sessão (regra 2) → filtro de alucinação → eco (marca, não apaga) → saída.
//!
//! A gravação começa na hora: enquanto o modelo carrega, os segmentos esperam
//! na fila (L10). Quem usa o motor (o app ou a CLI) grava cada segmento no
//! banco assim que ele chega.

use crate::asr::Asr;
use crate::words::{self, Word};
use serde::Serialize;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

pub const RATE: u64 = 16_000;
const MIN_SILENCE: f32 = 0.8;
const MIN_SPEECH: f32 = 0.25;
/// Fala corrida: depois de 10 s, a primeira pausa curta (120 ms bem abaixo do
/// nível da fala) fecha o segmento na hora. Sem pausa, aos 20 s corta no ponto
/// de menor energia dos últimos 3 s (cortar mais para trás atrasava o texto).
pub const MAX_SEGMENT: u64 = 20 * RATE;
const SOFT_FROM: u64 = 10 * RATE;
const SOFT_LOOK: u64 = RATE;
const SOFT_FRAMES: usize = 6;
const SOFT_RATIO: f32 = 0.15;
const HARD_LOOK: u64 = 3 * RATE;
const CUT_GUARD: u64 = RATE / 5;
/// Margem antes/depois da fala detectada: o Silero marca o início com atraso
/// (≈0,7 s em áudio com ruído de fundo), e cortar no meio da palavra perde texto.
const PAD_BEFORE: u64 = RATE * 6 / 10;
const PAD_AFTER: u64 = RATE / 5;
/// No corte forçado, quanto olhar para trás de quando o VAD percebeu a fala.
const LOOKBACK: u64 = 2 * RATE;
const MIN_SEG: u64 = RATE / 4;
/// Áudio guardado por canal (corte forçado e eco).
const RING: u64 = 60 * RATE;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Channel {
    Eu,
    Outros,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Eu => "EU",
            Channel::Outros => "OUTROS",
        }
    }
}

// ---------- VAD ----------

pub trait Vad: Send {
    fn accept(&mut self, samples: &[f32]);
    /// Próximo segmento fechado: (início em amostras desde o último reset, tamanho).
    fn pop(&mut self) -> Option<(u64, u64)>;
    fn speaking(&self) -> bool;
    fn flush(&mut self);
    fn reset(&mut self);
}

pub struct SileroVad(sherpa_onnx::VoiceActivityDetector);

impl SileroVad {
    pub fn new(model: &Path) -> Result<Self, String> {
        let cfg = sherpa_onnx::VadModelConfig {
            silero_vad: sherpa_onnx::SileroVadModelConfig {
                model: Some(model.to_string_lossy().into_owned()),
                threshold: 0.5,
                min_silence_duration: MIN_SILENCE,
                min_speech_duration: MIN_SPEECH,
                window_size: 512,
                // Acima do nosso corte de 20 s (quem corta é o segmentador).
                max_speech_duration: 30.0,
            },
            sample_rate: RATE as i32,
            num_threads: 1,
            provider: Some("cpu".into()),
            ..Default::default()
        };
        sherpa_onnx::VoiceActivityDetector::create(&cfg, 60.0).map(SileroVad).ok_or_else(|| "não foi possível carregar o Silero VAD".into())
    }

    /// O Silero embutido no app (gravado na pasta de modelos se faltar).
    pub fn embedded() -> Result<Self, String> {
        let spec = crate::models::silero();
        crate::models::ensure_embedded(&spec).map_err(|e| e.to_string())?;
        Self::new(&spec.file("silero_vad.onnx"))
    }
}

impl Vad for SileroVad {
    fn accept(&mut self, samples: &[f32]) {
        self.0.accept_waveform(samples);
    }
    fn pop(&mut self) -> Option<(u64, u64)> {
        let s = self.0.front()?;
        let r = (s.start().max(0) as u64, s.n().max(0) as u64);
        self.0.pop();
        Some(r)
    }
    fn speaking(&self) -> bool {
        self.0.detected()
    }
    fn flush(&mut self) {
        self.0.flush();
    }
    fn reset(&mut self) {
        self.0.reset();
    }
}

// ---------- áudio recente ----------

#[derive(Default)]
pub struct Ring {
    start: u64,
    data: VecDeque<f32>,
}

impl Ring {
    pub fn reset(&mut self, start: u64) {
        self.start = start;
        self.data.clear();
    }
    pub fn end(&self) -> u64 {
        self.start + self.data.len() as u64
    }
    pub fn push(&mut self, pos: u64, s: &[f32]) {
        if pos != self.end() {
            self.reset(pos);
        }
        self.data.extend(s);
        let keep_from = self.end().saturating_sub(RING);
        if keep_from > self.start {
            self.data.drain(..(keep_from - self.start) as usize);
            self.start = keep_from;
        }
    }
    /// Amostras em `[a, b)` (só o que ainda está guardado).
    pub fn slice(&self, a: u64, b: u64) -> Vec<f32> {
        let (a, b) = (a.max(self.start), b.min(self.end()));
        if b <= a {
            return Vec::new();
        }
        self.data.range((a - self.start) as usize..(b - self.start) as usize).copied().collect()
    }
}

/// O quadro de 20 ms com menor energia em `[a, b)`.
fn quietest(r: &Ring, a: u64, b: u64) -> u64 {
    const F: u64 = RATE / 50;
    let s = r.slice(a, b);
    let mut best = (f32::MAX, b);
    for (k, fr) in s.chunks(F as usize).enumerate() {
        if fr.len() < F as usize {
            break;
        }
        let e: f32 = fr.iter().map(|x| x * x).sum();
        if e < best.0 {
            best = (e, a.max(r.start) + k as u64 * F + F / 2);
        }
    }
    best.1
}

/// Meio da primeira pausa curta em `[a, b)`: `SOFT_FRAMES` quadros seguidos com
/// RMS abaixo de `SOFT_RATIO` × a mediana da fala em `[from, b)`.
fn short_pause(r: &Ring, from: u64, a: u64, b: u64) -> Option<u64> {
    const F: usize = (RATE / 50) as usize;
    let rms = |fr: &[f32]| (fr.iter().map(|x| x * x).sum::<f32>() / fr.len() as f32).sqrt();
    let mut all: Vec<f32> = r.slice(from, b).as_chunks::<F>().0.iter().map(|f| rms(f)).collect();
    if all.len() < 50 {
        return None;
    }
    all.sort_by(|x, y| x.total_cmp(y));
    let limit = all[all.len() / 2] * SOFT_RATIO;
    let a = a.max(r.start);
    let mut run = 0;
    for (k, fr) in r.slice(a, b).as_chunks::<F>().0.iter().enumerate() {
        run = if rms(fr) < limit { run + 1 } else { 0 };
        if run == SOFT_FRAMES {
            return Some(a + ((k + 1) * F - SOFT_FRAMES * F / 2) as u64);
        }
    }
    None
}

// ---------- segmentador ----------

#[derive(Debug)]
pub struct Seg {
    pub ch: Channel,
    /// Posição na sessão (amostras) da primeira amostra.
    pub start: u64,
    pub samples: Vec<f32>,
    pub forced: bool,
}

impl Seg {
    pub fn end(&self) -> u64 {
        self.start + self.samples.len() as u64
    }
}

pub struct Segmenter {
    ch: Channel,
    vad: Box<dyn Vad>,
    /// Posição da sessão que corresponde ao índice 0 do VAD.
    base: u64,
    fed: u64,
    started: bool,
    ring: Ring,
    /// Nenhum segmento começa antes daqui (fim do anterior / corte forçado).
    open_from: u64,
    last_end: u64,
    speech_since: Option<u64>,
}

impl Segmenter {
    pub fn new(ch: Channel, vad: Box<dyn Vad>) -> Self {
        Segmenter { ch, vad, base: 0, fed: 0, started: false, ring: Ring::default(), open_from: 0, last_end: 0, speech_since: None }
    }

    fn restart(&mut self, pos: u64) {
        self.vad.reset();
        self.base = pos;
        self.fed = 0;
        self.ring.reset(pos);
        self.open_from = pos;
        self.last_end = pos;
        self.speech_since = None;
        self.started = true;
    }

    pub fn push(&mut self, pos: u64, samples: &[f32], out: &mut Vec<Seg>) {
        if !self.started {
            self.restart(pos);
        } else if pos != self.base + self.fed {
            // Pausa/retomada ou fonte reaberta: fecha o que estava aberto e recomeça.
            self.flush(out);
            self.restart(pos);
        }
        self.ring.push(pos, samples);
        self.vad.accept(samples);
        self.fed += samples.len() as u64;
        self.collect(out);
        let now = self.base + self.fed;
        if self.vad.speaking() {
            let since = *self.speech_since.get_or_insert(now.saturating_sub(LOOKBACK).max(self.last_end).max(self.open_from));
            let from = since.max(self.open_from);
            let len = now - from;
            let cut = if len >= MAX_SEGMENT {
                Some(quietest(&self.ring, now - HARD_LOOK, now - CUT_GUARD))
            } else if len >= SOFT_FROM + CUT_GUARD {
                short_pause(&self.ring, from, (from + SOFT_FROM).max(now - SOFT_LOOK), now - CUT_GUARD)
            } else {
                None
            };
            if let Some(cut) = cut {
                self.emit(from, cut, true, out);
                self.open_from = cut;
                self.speech_since = Some(cut);
            }
        }
    }

    fn collect(&mut self, out: &mut Vec<Seg>) {
        while let Some((s, n)) = self.vad.pop() {
            let (a, b) = ((self.base + s).max(self.open_from), self.base + s + n);
            if b > a {
                self.emit(a, b, false, out);
            }
            self.open_from = self.open_from.max(b);
            self.speech_since = None;
        }
    }

    fn emit(&mut self, a: u64, b: u64, forced: bool, out: &mut Vec<Seg>) {
        if b.saturating_sub(a) < MIN_SEG {
            return;
        }
        let pa = a.saturating_sub(PAD_BEFORE).max(self.last_end);
        let pb = if forced { b } else { (b + PAD_AFTER).min(self.ring.end()) };
        let samples = self.ring.slice(pa, pb);
        if samples.is_empty() {
            return;
        }
        self.last_end = pb;
        out.push(Seg { ch: self.ch, start: pa.max(self.ring.start), samples, forced });
    }

    /// Fecha a fala em andamento (pausa, parada).
    pub fn flush(&mut self, out: &mut Vec<Seg>) {
        if !self.started {
            return;
        }
        self.vad.flush();
        self.collect(out);
    }
}

// ---------- filtro de alucinação ----------

fn norm(w: &str) -> String {
    w.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Tira repetições em laço (mais de 3 iguais seguidas) e descarta o segmento sem
/// palavra de verdade ou com mais palavras por segundo do que alguém fala.
pub fn clean(mut ws: Vec<Word>, duration: f64) -> Vec<Word> {
    ws.retain(|w| !w.word.trim().is_empty());
    let mut out: Vec<Word> = Vec::with_capacity(ws.len());
    let mut run = 0;
    for w in ws {
        let same = out.last().is_some_and(|p| !norm(&p.word).is_empty() && norm(&p.word) == norm(&w.word));
        run = if same { run + 1 } else { 1 };
        if run <= 3 {
            out.push(w);
        }
    }
    let real = out.iter().filter(|w| !norm(&w.word).is_empty()).count();
    if real == 0 || real as f64 > 7.0 * duration.max(0.5) {
        return Vec::new();
    }
    out
}

// ---------- eco ----------

const FRAME: usize = (RATE / 50) as usize; // 20 ms

/// Envelope de energia (RMS por quadro de 20 ms).
pub fn envelope(s: &[f32]) -> Vec<f32> {
    s.chunks(FRAME).map(|f| (f.iter().map(|x| x * x).sum::<f32>() / f.len() as f32).sqrt()).collect()
}

fn pearson(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n < 10 {
        return 0.0;
    }
    let (ma, mb) = (a[..n].iter().sum::<f32>() / n as f32, b[..n].iter().sum::<f32>() / n as f32);
    let (mut sab, mut saa, mut sbb) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..n {
        let (x, y) = (a[i] - ma, b[i] - mb);
        sab += x * y;
        saa += x * x;
        sbb += y * y;
    }
    if saa < 1e-12 || sbb < 1e-12 { 0.0 } else { sab / (saa.sqrt() * sbb.sqrt()) }
}

/// Maior correlação do envelope do mic com o do alto-falante atrasado de 0 a `max_lag` quadros
/// (o som sai da caixa e chega ao mic depois).
pub fn best_corr(mic: &[f32], spk: &[f32], max_lag: usize) -> f32 {
    (0..=max_lag.min(mic.len().saturating_sub(10))).map(|lag| pearson(&mic[lag..], spk)).fold(0.0, f32::max)
}

/// Fração das palavras do EU (2+ letras) que aparecem no texto de OUTROS.
pub fn text_similarity(eu: &str, outros: &str) -> f32 {
    let ws: Vec<String> = eu.split_whitespace().map(norm).filter(|w| w.chars().count() >= 2).collect();
    if ws.len() < 2 {
        return 0.0;
    }
    let other: std::collections::HashSet<String> = outros.split_whitespace().map(norm).collect();
    ws.iter().filter(|w| other.contains(*w)).count() as f32 / ws.len() as f32
}

/// Limiar de "OUTROS tocando" no quadro (≈ -45 dBFS).
const SPK_ON: f32 = 0.0056;
pub const ECHO_OVERLAP: f32 = 0.5;
pub const ECHO_CORR: f32 = 0.6;
pub const ECHO_TEXT: f32 = 0.5;

/// Sobreposição (fração dos quadros do EU com OUTROS tocando) e correlação dos envelopes.
pub fn echo_audio(mic: &[f32], spk: &[f32]) -> (f32, f32) {
    let (em, es) = (envelope(mic), envelope(spk));
    if em.is_empty() || es.is_empty() {
        return (0.0, 0.0);
    }
    let on = es.iter().take(em.len()).filter(|&&x| x > SPK_ON).count() as f32 / em.len() as f32;
    (on, best_corr(&em, &es, 15))
}

// ---------- motor ----------

#[derive(Clone, Debug, Serialize)]
pub struct LiveSegment {
    pub ch: Channel,
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<Word>,
    pub echo: bool,
    pub forced: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "state", content = "error", rename_all = "lowercase")]
pub enum ModelState {
    Loading,
    Ready,
    Error(String),
}

/// Quem recebe o resultado (o app grava no banco e avisa a UI).
pub trait Output: Send {
    /// Devolve o id do segmento (para marcar eco depois).
    fn segment(&mut self, s: &LiveSegment) -> i64;
    fn echo(&mut self, id: i64, echo: bool);
    fn model(&mut self, st: ModelState);
}

pub type Loader = Box<dyn FnOnce() -> Result<Arc<Asr>, String> + Send>;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Stats {
    pub segments: usize,
    pub echoes: usize,
    pub forced: usize,
    pub dropped: usize,
    /// Fim da fala → texto pronto (s); só faz sentido ao vivo.
    pub latencies: Vec<f64>,
}

impl Stats {
    pub fn percentile(&self, p: f64) -> Option<f64> {
        if self.latencies.is_empty() {
            return None;
        }
        let mut v = self.latencies.clone();
        v.sort_by(f64::total_cmp);
        Some(v[((v.len() - 1) as f64 * p).round() as usize])
    }
}

enum Msg {
    Audio(Channel, u64, Vec<f32>),
    Flush,
}

pub struct Engine {
    tx: Option<Sender<Msg>>,
    seg: Option<JoinHandle<()>>,
    rec: Option<JoinHandle<Stats>>,
    headphones: Arc<AtomicBool>,
}

struct Recent {
    id: i64,
    ch: Channel,
    start: f64,
    end: f64,
    text: String,
    overlap: f32,
    echo: bool,
}

impl Engine {
    /// `started` = o instante da posição 0 da sessão (para medir o atraso).
    pub fn start(headphones: bool, vads: [Box<dyn Vad>; 2], loader: Loader, out: Box<dyn Output>, started: Instant) -> Engine {
        let (tx, rx) = mpsc::channel::<Msg>();
        let (stx, srx) = mpsc::channel::<Seg>();
        let spk = Arc::new(Mutex::new(Ring::default()));
        let hp = Arc::new(AtomicBool::new(headphones));
        let [ve, vo] = vads;
        let seg = {
            let spk = spk.clone();
            thread::Builder::new().name("live-vad".into()).spawn(move || segment_loop(rx, stx, ve, vo, spk)).ok()
        };
        let rec = {
            let hp = hp.clone();
            thread::Builder::new().name("live-asr".into()).spawn(move || recognize_loop(srx, loader, out, spk, hp, started)).ok()
        };
        Engine { tx: Some(tx), seg, rec, headphones: hp }
    }

    /// Nunca bloqueia (a captura não espera o reconhecedor).
    pub fn audio(&self, ch: Channel, pos: u64, samples: Vec<f32>) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Audio(ch, pos, samples));
        }
    }

    /// Fecha as falas em andamento (pausa).
    pub fn flush(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Flush);
        }
    }

    pub fn set_headphones(&self, on: bool) {
        self.headphones.store(on, Ordering::Relaxed);
    }

    /// Fecha tudo, espera a fila esvaziar e devolve as estatísticas.
    pub fn finish(mut self) -> Stats {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Msg::Flush);
        }
        if let Some(h) = self.seg.take() {
            let _ = h.join();
        }
        self.rec.take().and_then(|h| h.join().ok()).unwrap_or_default()
    }
}

fn segment_loop(rx: Receiver<Msg>, stx: Sender<Seg>, ve: Box<dyn Vad>, vo: Box<dyn Vad>, spk: Arc<Mutex<Ring>>) {
    let mut segs = [Segmenter::new(Channel::Eu, ve), Segmenter::new(Channel::Outros, vo)];
    let mut out = Vec::new();
    for m in rx {
        match m {
            Msg::Audio(ch, pos, s) => {
                if ch == Channel::Outros {
                    spk.lock().unwrap().push(pos, &s);
                }
                segs[ch as usize].push(pos, &s, &mut out);
            }
            Msg::Flush => segs.iter_mut().for_each(|g| g.flush(&mut out)),
        }
        for s in out.drain(..) {
            let _ = stx.send(s);
        }
    }
    segs.iter_mut().for_each(|g| g.flush(&mut out));
    for s in out.drain(..) {
        let _ = stx.send(s);
    }
}

fn recognize_loop(srx: Receiver<Seg>, loader: Loader, mut out: Box<dyn Output>, spk: Arc<Mutex<Ring>>, hp: Arc<AtomicBool>, started: Instant) -> Stats {
    let mut st = Stats::default();
    out.model(ModelState::Loading);
    let asr = match loader() {
        Ok(a) => {
            out.model(ModelState::Ready);
            Some(a)
        }
        Err(e) => {
            out.model(ModelState::Error(e));
            None
        }
    };
    let mut recent: VecDeque<Recent> = VecDeque::new();
    for seg in srx {
        let Some(asr) = &asr else {
            st.dropped += 1;
            continue;
        };
        let (a, b) = (seg.start as f64 / RATE as f64, seg.end() as f64 / RATE as f64);
        if std::env::var_os("OUVEAI_LIVE_DEBUG").is_some() {
            eprintln!("  [seg] {} {:.2}..{:.2} forced={}", seg.ch.as_str(), a, b, seg.forced);
        }
        let toks = match asr.recognize(&seg.samples) {
            Ok(t) => t,
            Err(e) => {
                log::error!("reconhecedor ao vivo: {e}");
                st.dropped += 1;
                continue;
            }
        };
        let ws = clean(words::assemble(&toks, a, b, &[]), b - a);
        if ws.is_empty() {
            continue;
        }
        let text = ws.iter().map(|w| w.word.as_str()).collect::<Vec<_>>().join(" ");
        let (start, end) = (ws[0].start, ws[ws.len() - 1].end);

        // Eco (só sem fone): EU que é a voz de OUTROS saindo da caixa.
        let check = !hp.load(Ordering::Relaxed);
        let mut overlap = 0.0;
        let mut echo = false;
        if check && seg.ch == Channel::Eu {
            let s = spk.lock().unwrap().slice(seg.start, seg.end());
            let (ov, corr) = echo_audio(&seg.samples, &s);
            overlap = ov;
            let others: String = recent.iter().filter(|r| r.ch == Channel::Outros && r.start < end && r.end > start).map(|r| r.text.as_str()).collect::<Vec<_>>().join(" ");
            let sim = text_similarity(&text, &others);
            echo = ov >= ECHO_OVERLAP && corr >= ECHO_CORR || ov >= 0.3 && sim >= ECHO_TEXT;
        }
        let ls = LiveSegment { ch: seg.ch, start, end, text: text.clone(), words: ws, echo, forced: seg.forced };
        let id = out.segment(&ls);
        st.segments += 1;
        st.forced += seg.forced as usize;
        st.echoes += echo as usize;
        st.latencies.push(started.elapsed().as_secs_f64() - end);
        if std::env::var_os("OUVEAI_LIVE_DEBUG").is_some() {
            eprintln!("  [lat] {} seg {:.2}..{:.2} fala {:.2}..{:.2} atraso {:.2} s", seg.ch.as_str(), a, b, start, end, started.elapsed().as_secs_f64() - end);
        }

        // OUTROS que chega depois: confere o texto dos EU recentes que ele cobre.
        if check && seg.ch == Channel::Outros {
            for r in recent.iter_mut().filter(|r| r.ch == Channel::Eu && !r.echo && r.start < end && r.end > start) {
                if r.overlap >= 0.3 && text_similarity(&r.text, &text) >= ECHO_TEXT {
                    r.echo = true;
                    out.echo(r.id, true);
                    st.echoes += 1;
                }
            }
        }
        recent.push_back(Recent { id, ch: seg.ch, start, end, text, overlap, echo });
        while recent.front().is_some_and(|r| end - r.end > 60.0) {
            recent.pop_front();
        }
    }
    st
}

#[cfg(test)]
mod tests {
    use super::*;

    /// VAD de energia, determinístico: fala = janela de 512 com RMS > 0,05.
    struct EnergyVad {
        pos: u64,
        pending: Vec<f32>,
        speech: Option<u64>,
        last_voice: u64,
        done: VecDeque<(u64, u64)>,
    }

    impl EnergyVad {
        fn boxed() -> Box<dyn Vad> {
            Box::new(EnergyVad { pos: 0, pending: Vec::new(), speech: None, last_voice: 0, done: VecDeque::new() })
        }
    }

    impl Vad for EnergyVad {
        fn accept(&mut self, s: &[f32]) {
            self.pending.extend_from_slice(s);
            while self.pending.len() >= 512 {
                let w: Vec<f32> = self.pending.drain(..512).collect();
                let rms = (w.iter().map(|x| x * x).sum::<f32>() / 512.0).sqrt();
                if rms > 0.05 {
                    self.speech.get_or_insert(self.pos);
                    self.last_voice = self.pos + 512;
                } else if let Some(s) = self.speech
                    && self.pos + 512 - self.last_voice >= (MIN_SILENCE * RATE as f32) as u64
                {
                    self.done.push_back((s, self.last_voice - s));
                    self.speech = None;
                }
                self.pos += 512;
            }
        }
        fn pop(&mut self) -> Option<(u64, u64)> {
            self.done.pop_front()
        }
        fn speaking(&self) -> bool {
            self.speech.is_some()
        }
        fn flush(&mut self) {
            if let Some(s) = self.speech.take() {
                self.done.push_back((s, self.last_voice - s));
            }
        }
        fn reset(&mut self) {
            *self = EnergyVad { pos: 0, pending: Vec::new(), speech: None, last_voice: 0, done: VecDeque::new() };
        }
    }

    fn tone(secs: f64, amp: f32) -> Vec<f32> {
        (0..(secs * RATE as f64) as usize).map(|i| amp * (i as f32 * 0.3).sin()).collect()
    }

    fn feed(g: &mut Segmenter, pos: u64, audio: &[f32], out: &mut Vec<Seg>) -> u64 {
        let mut p = pos;
        for c in audio.chunks(1600) {
            g.push(p, c, out);
            p += c.len() as u64;
        }
        p
    }

    #[test]
    fn fecha_no_silencio_com_margem() {
        let mut g = Segmenter::new(Channel::Eu, EnergyVad::boxed());
        let mut audio = vec![0.0; RATE as usize];
        audio.extend(tone(1.0, 0.5));
        audio.extend(vec![0.0; 2 * RATE as usize]);
        audio.extend(tone(1.5, 0.5));
        audio.extend(vec![0.0; 2 * RATE as usize]);
        let mut out = Vec::new();
        feed(&mut g, 32_000, &audio, &mut out);
        assert_eq!(out.len(), 2, "{out:?}");
        // 1ª fala em 32 000 + 1 s (± a janela de 512), com 200 ms de margem antes.
        let s0 = out[0].start as i64 - (32_000 + RATE as i64 - PAD_BEFORE as i64);
        assert!(s0.abs() <= 512, "{s0}");
        assert!(!out[0].forced);
        let dur1 = out[1].samples.len() as f64 / RATE as f64;
        assert!((dur1 - 2.3).abs() < 0.1, "{dur1}");
    }

    #[test]
    fn fala_corrida_corta_no_ponto_mais_baixo() {
        let mut g = Segmenter::new(Channel::Outros, EnergyVad::boxed());
        let mut audio = tone(5.0, 0.5);
        // Respiro de 200 ms aos 5 s (cedo demais: só vale depois de 10 s).
        audio.extend(tone(0.2, 0.02));
        audio.extend(tone(9.8, 0.5));
        // Respiro de 200 ms aos 15 s (curto demais para o VAD fechar).
        audio.extend(tone(0.2, 0.02));
        // Depois, 25 s sem respiro: corte duro aos 20 s, nos últimos 3 s.
        audio.extend(tone(25.0, 0.5));
        let mut out = Vec::new();
        let end = feed(&mut g, 0, &audio, &mut out);
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out.iter().all(|s| s.forced));
        let cut = out[0].end() as f64 / RATE as f64;
        assert!((cut - 15.1).abs() < 0.08, "{cut}");
        let hard = out[1].end() as f64 / RATE as f64 - cut;
        assert!((17.0..=19.8).contains(&hard), "{hard}");
        assert_eq!(out[1].start, out[0].end()); // sem buraco nem sobreposição
        g.flush(&mut out);
        assert_eq!(out.len(), 3);
        assert_eq!(out[2].start, out[1].end());
        assert!(out[2].end() <= end);
    }

    #[test]
    fn retomada_depois_da_pausa_fecha_e_recomeca() {
        let mut g = Segmenter::new(Channel::Eu, EnergyVad::boxed());
        let mut out = Vec::new();
        feed(&mut g, 0, &tone(1.0, 0.5), &mut out);
        assert!(out.is_empty());
        // Pausa: a próxima posição pula 60 s.
        feed(&mut g, 60 * RATE + 16_000, &tone(0.5, 0.0), &mut out);
        assert_eq!(out.len(), 1);
        assert!(out[0].end() <= RATE + PAD_AFTER);
    }

    #[test]
    fn filtro_tira_laco_e_lixo() {
        let w = |s: &str, t: f64| Word { word: s.into(), start: t, end: t + 0.2 };
        let laco: Vec<Word> = (0..8).map(|i| w("obrigado", i as f64 * 0.3)).collect();
        assert_eq!(clean(laco, 3.0).len(), 3);
        assert!(clean(vec![w(".", 0.0), w("?", 0.2)], 1.0).is_empty());
        let rapido: Vec<Word> = (0..30).map(|i| w(&format!("p{i}"), i as f64 * 0.03)).collect();
        assert!(clean(rapido, 1.0).is_empty());
        let ok = vec![w("não", 0.0), w("não", 0.3), w("não,", 0.6), w("pode.", 0.9)];
        assert_eq!(clean(ok, 1.5).len(), 4);
    }

    #[test]
    fn eco_correlaciona_com_atraso_e_ignora_fala_propria() {
        // OUTROS: fala com envelope variando; o mic pega a mesma coisa 120 ms depois, mais baixo.
        let spk: Vec<f32> = (0..3 * RATE as usize).map(|i| ((i as f32 / 2000.0).sin().abs() + 0.05) * 0.3 * (i as f32 * 0.7).sin()).collect();
        let delay = (RATE as usize) * 12 / 100;
        let mut mic = vec![0.0; delay];
        mic.extend(spk.iter().map(|x| x * 0.4));
        mic.truncate(spk.len());
        let (ov, corr) = echo_audio(&mic, &spk);
        assert!(ov > 0.9 && corr > 0.9, "{ov} {corr}");
        // Fala própria (outro envelope) com OUTROS tocando ao mesmo tempo: correlação baixa.
        let own: Vec<f32> = (0..3 * RATE as usize).map(|i| ((i as f32 / 700.0).cos().abs() + 0.05) * 0.3 * (i as f32 * 0.9).sin()).collect();
        let (_, c2) = echo_audio(&own, &spk);
        assert!(c2 < ECHO_CORR, "{c2}");
        assert!(text_similarity("a gente fecha o escopo na sexta", "então a gente fecha o escopo na sexta.") > 0.8);
        assert!(text_similarity("fechado, mando o resumo", "a gente fecha o escopo") < 0.3);
    }
}
