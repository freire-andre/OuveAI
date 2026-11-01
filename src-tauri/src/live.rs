//! Gravação de reunião (Fase 4): captura (EU + OUTROS) → motor ao vivo → banco,
//! um segmento por vez, com a mini e a principal avisadas por eventos.
//!
//! - A gravação começa na hora; o modelo carrega em paralelo (L10).
//! - O modelo fica carregado por 5 min depois de parar (idle TTL), para a
//!   próxima reunião começar sem espera; depois disso a memória é liberada.
//! - Regra 8: o log tem contagens, tempos e nomes de dispositivo; nunca texto.

use crate::state::AppState;
use crate::tray::TrayItems;
use crate::windows;
use ouveai_audio::{Capture, Ch, Config, Event, MicMode, OutrosMode};
use ouveai_core::asr::Asr;
use ouveai_core::live::{Channel, Engine, LiveSegment, ModelState, Output, RATE, SileroVad, Vad};
use ouveai_core::{paths, store, wav};
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const IDLE_TTL: Duration = Duration::from_secs(5 * 60);
/// OUTROS sem sinal por esse tempo, com a sessão ativa → dica na mini.
const OUTROS_SILENT: Duration = Duration::from_secs(60);
const SIGNAL_DB: f32 = -50.0;
const KEEP_SEGMENTS: usize = 40;

#[derive(Clone, Serialize, Default)]
pub struct ChannelView {
    /// opening · waiting (antivírus?) · stuck · open · bt_idle · off · error · app_missing
    pub state: &'static str,
    pub device: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct SegView {
    pub id: i64,
    pub ch: Channel,
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub echo: bool,
}

#[derive(Clone, Serialize)]
pub struct MicElsewhere {
    pub device: String,
    pub device_id: String,
    pub app: String,
}

#[derive(Clone, Serialize)]
pub struct LiveView {
    /// idle · recording · paused · finishing
    pub state: &'static str,
    pub session_id: Option<i64>,
    /// loading · ready · error
    pub model: &'static str,
    pub model_error: Option<String>,
    pub eu: ChannelView,
    pub outros: ChannelView,
    pub headphones: bool,
    pub mic_elsewhere: Option<MicElsewhere>,
    pub outros_silent: bool,
    pub mic_denied: bool,
    pub marks: usize,
    pub notice: Option<String>,
    pub segments: VecDeque<SegView>,
}

impl Default for LiveView {
    fn default() -> Self {
        LiveView {
            state: "idle",
            session_id: None,
            model: "loading",
            model_error: None,
            eu: ChannelView { state: "opening", ..Default::default() },
            outros: ChannelView { state: "opening", ..Default::default() },
            headphones: false,
            mic_elsewhere: None,
            outros_silent: false,
            mic_denied: false,
            marks: 0,
            notice: None,
            segments: VecDeque::new(),
        }
    }
}

#[derive(Clone, Serialize)]
struct Tick {
    elapsed: f64,
    eu: f32,
    outros: f32,
}

type Wavs = Arc<Mutex<Option<[wav::Writer; 2]>>>;

struct Session {
    id: i64,
    t0: u64,
    started: Instant,
    cfg: Config,
    capture: Option<Capture>,
    engine: Arc<Engine>,
    wavs: Wavs,
    audio_dir: Option<PathBuf>,
    keep_days: Option<i64>,
    alive: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct Live {
    session: Mutex<Option<Session>>,
    view: Mutex<LiveView>,
    /// Modelo carregado e quando foi usado pela última vez.
    asr: Arc<Mutex<Option<Arc<Asr>>>>,
    asr_gen: Arc<AtomicU64>,
    test: Mutex<Option<(Capture, Arc<AtomicBool>)>>,
    /// Um gravar/parar por vez (atalho apertado duas vezes seguidas).
    toggling: AtomicBool,
}

fn emit_view(app: &AppHandle) {
    let v = app.state::<Live>().view.lock().unwrap().clone();
    let _ = app.emit("live", v);
}

fn update(app: &AppHandle, f: impl FnOnce(&mut LiveView)) {
    f(&mut app.state::<Live>().view.lock().unwrap());
    emit_view(app);
}

pub fn view(app: &AppHandle) -> LiveView {
    app.state::<Live>().view.lock().unwrap().clone()
}

// ---------- configurações de áudio ----------

pub fn config(st: &AppState) -> Config {
    fn parse<T: serde::de::DeserializeOwned>(st: &AppState, k: &str) -> Option<T> {
        st.setting(k).and_then(|v| serde_json::from_str(&v).ok())
    }
    Config { mic: parse(st, "audio.mic").unwrap_or(MicMode::Auto), outros: parse(st, "audio.outros").unwrap_or(OutrosMode::Auto), mic_tol_ms: 200 }
}

/// "Estou usando fone": automático pelo tipo do dispositivo de saída, ou manual.
pub fn headphones(st: &AppState) -> bool {
    match st.setting("audio.headphones").as_deref() {
        Some("yes") => true,
        Some("no") => false,
        _ => ouveai_audio::default_output_is_headphone().unwrap_or(false),
    }
}

/// D2: não salvar (padrão) · 7 · 30 dias · sempre (0).
fn retention(st: &AppState) -> Option<i64> {
    match st.setting("audio.retention").as_deref() {
        Some("7") => Some(7),
        Some("30") => Some(30),
        Some("always") => Some(0),
        _ => None,
    }
}

fn threads(st: &AppState) -> i32 {
    st.setting("live.threads").and_then(|v| v.parse().ok()).filter(|n| (1..=8).contains(n)).unwrap_or(2)
}

// ---------- saída do motor ----------

struct AppOut {
    app: AppHandle,
    session: i64,
    loading: Instant,
}

impl Output for AppOut {
    fn segment(&mut self, s: &LiveSegment) -> i64 {
        let st = self.app.state::<AppState>();
        let flags = if s.echo { store::FLAG_ECHO } else { 0 };
        let id = match st.store.lock().unwrap().add_segment(self.session, s.ch.as_str(), &s.text, flags, &s.words) {
            Ok(id) => id,
            Err(e) => {
                log::error!("falha ao gravar segmento: {e}");
                -1
            }
        };
        let seg = SegView { id, ch: s.ch, start: s.start, end: s.end, text: s.text.clone(), echo: s.echo };
        {
            let live = self.app.state::<Live>();
            let mut v = live.view.lock().unwrap();
            v.segments.push_back(seg.clone());
            while v.segments.len() > KEEP_SEGMENTS {
                v.segments.pop_front();
            }
        }
        let _ = self.app.emit("live-segment", seg);
        id
    }

    fn echo(&mut self, id: i64, echo: bool) {
        if let Err(e) = self.app.state::<AppState>().store.lock().unwrap().set_segment_flag(id, store::FLAG_ECHO, echo) {
            log::error!("falha ao marcar eco: {e}");
        }
        if let Some(s) = self.app.state::<Live>().view.lock().unwrap().segments.iter_mut().find(|s| s.id == id) {
            s.echo = echo;
        }
        let _ = self.app.emit("live-echo", (id, echo));
    }

    fn model(&mut self, m: ModelState) {
        match &m {
            ModelState::Loading => self.loading = Instant::now(),
            ModelState::Ready => log::info!("modelo ao vivo pronto em {:.2} s", self.loading.elapsed().as_secs_f64()),
            ModelState::Error(e) => log::error!("modelo ao vivo: {e}"),
        }
        update(&self.app, |v| match m {
            ModelState::Loading => v.model = "loading",
            ModelState::Ready => v.model = "ready",
            ModelState::Error(e) => {
                v.model = "error";
                v.model_error = Some(e);
            }
        });
    }
}

// ---------- captura ----------

fn mark_device(app: &AppHandle, note: &str) {
    let live = app.state::<Live>();
    let g = live.session.lock().unwrap();
    if let Some(s) = g.as_ref() {
        let t = ouveai_audio::pos_of(ouveai_audio::qpc_100ns(), s.t0) as f64 / RATE as f64;
        let _ = app.state::<AppState>().store.lock().unwrap().add_mark(s.id, t, "dispositivo", Some(note));
    }
}

fn chan(v: &mut LiveView, ch: Ch) -> &mut ChannelView {
    if ch == Ch::Eu { &mut v.eu } else { &mut v.outros }
}

/// Eventos da captura → estado da mini + marcadores "troca de dispositivo".
fn on_event(app: &AppHandle, ev: Event) {
    match ev {
        Event::Opened { ch, device, form, reopen } => {
            log::info!("{ch:?} aberto: {device} [{form}]{}", if reopen { " (reaberto)" } else { "" });
            if reopen {
                mark_device(app, &device);
            }
            update(app, |v| {
                *chan(v, ch) = ChannelView { state: "open", device: Some(device.clone()), error: None };
                if reopen {
                    v.notice = Some(format!("{} agora em: {device}", if ch == Ch::Eu { "EU" } else { "OUTROS" }));
                }
                if ch == Ch::Eu {
                    v.mic_elsewhere = None;
                }
            });
        }
        Event::Waiting { ch } => {
            log::warn!("{ch:?} sem abrir há 2 s (antivírus?)");
            update(app, |v| {
                if chan(v, ch).state == "opening" {
                    chan(v, ch).state = "waiting";
                }
            });
        }
        Event::Stuck { ch } => {
            log::warn!("{ch:?} sem abrir há 60 s");
            update(app, |v| {
                if chan(v, ch).state != "error" {
                    chan(v, ch).state = "stuck";
                }
            });
        }
        Event::OpenError { ch, error } => {
            log::warn!("{ch:?} não abriu: {error}");
            // 0x80070490: não existe dispositivo desse tipo (nenhum microfone ligado).
            let msg = if error.contains("0x80070490") {
                if ch == Ch::Eu { "nenhum microfone encontrado".to_string() } else { "nenhuma saída de som encontrada".to_string() }
            } else {
                error
            };
            update(app, |v| *chan(v, ch) = ChannelView { state: "error", device: None, error: Some(msg) });
        }
        Event::Lost { ch, error } => {
            log::warn!("{ch:?} perdido: {error}; reabrindo");
            update(app, |v| chan(v, ch).state = "opening");
        }
        Event::DefaultChanged { ch } => {
            log::info!("{ch:?}: dispositivo padrão mudou; reabrindo");
            update(app, |v| chan(v, ch).state = "opening");
        }
        Event::Gap { ch, ms } => log::warn!("{ch:?}: lacuna de {ms} ms preenchida com silêncio"),
        Event::BtMicIdle { device } => {
            log::info!("mic Bluetooth sem uso, não aberto: {device}");
            update(app, |v| v.eu = ChannelView { state: "bt_idle", device: Some(device), error: None });
        }
        Event::MicElsewhere { device, device_id, app: exe } => {
            log::info!("o app {exe} usa outro mic: {device}");
            update(app, |v| v.mic_elsewhere = Some(MicElsewhere { device, device_id, app: exe }));
        }
        Event::AppMissing { exe } => {
            log::info!("app {exe} não está aberto (modo por aplicativo)");
            update(app, |v| v.outros = ChannelView { state: "app_missing", device: Some(exe), error: None });
        }
    }
}

fn make_sink(engine: Arc<Engine>, wavs: Wavs) -> ouveai_audio::Sink {
    Box::new(move |ch: Ch, pos: u64, s: &[f32]| {
        let c = if ch == Ch::Eu { Channel::Eu } else { Channel::Outros };
        if let Some(w) = wavs.lock().unwrap().as_mut() {
            let _ = w[c as usize].put(pos, s);
        }
        engine.audio(c, pos, s.to_vec());
    })
}

fn start_capture(app: &AppHandle, cfg: &Config, t0: u64, engine: Arc<Engine>, wavs: Wavs) -> Capture {
    let a = app.clone();
    Capture::start(cfg.clone(), t0, make_sink(engine, wavs), Arc::new(move |ev| on_event(&a, ev)))
}

fn vads() -> Result<[Box<dyn Vad>; 2], String> {
    Ok([Box::new(SileroVad::embedded()?), Box::new(SileroVad::embedded()?)])
}

fn set_recording(app: &AppHandle, on: bool) {
    app.state::<AppState>().recording.store(on, Ordering::Relaxed);
    if let Some(t) = app.try_state::<TrayItems>() {
        let _ = t.record.set_text(if on { "Parar gravação" } else { "Gravar" });
    }
    crate::actions::broadcast(app);
}

// ---------- comandos ----------

pub fn start(app: &AppHandle) -> Result<(), String> {
    let live = app.state::<Live>();
    if live.session.lock().unwrap().is_some() || live.view.lock().unwrap().state == "finishing" {
        return Ok(());
    }
    stop_test(app);
    if ouveai_core::models::presence(&ouveai_core::models::parakeet()) != ouveai_core::models::Presence::Ready {
        windows::open_main(app, Some("models"));
        return Err("Baixe ou importe o modelo de reconhecimento antes de gravar.".into());
    }
    let st = app.state::<AppState>();
    let cfg = config(&st);
    let hp = headphones(&st);
    let keep = retention(&st);
    let threads = threads(&st);
    let vads = vads()?;
    let id = st.store.lock().unwrap().create_meeting("Reunião", &app.package_info().version.to_string()).map_err(|e| e.to_string())?;
    let t0 = ouveai_audio::qpc_100ns();
    let started = Instant::now();

    // Modelo: o já carregado (idle TTL) ou carrega agora, sem segurar a gravação.
    let cache = live.asr.clone();
    live.asr_gen.fetch_add(1, Ordering::SeqCst);
    let loader: ouveai_core::live::Loader = Box::new(move || {
        if let Some(a) = cache.lock().unwrap().clone() {
            return Ok(a);
        }
        let a = Arc::new(Asr::load(threads)?);
        *cache.lock().unwrap() = Some(a.clone());
        Ok(a)
    });
    let out = Box::new(AppOut { app: app.clone(), session: id, loading: Instant::now() });
    let engine = Arc::new(Engine::start(hp, vads, loader, out, started));

    let (wavs, audio_dir): (Wavs, Option<PathBuf>) = match keep {
        Some(_) => {
            let dir = paths::recordings_dir().join(id.to_string());
            let w = std::fs::create_dir_all(&dir).and_then(|_| Ok([wav::Writer::create(&dir.join("eu.wav"), RATE as u32)?, wav::Writer::create(&dir.join("outros.wav"), RATE as u32)?]));
            match w {
                Ok(w) => {
                    // Já no início: se o app cair, a sessão interrompida ainda acha o áudio.
                    if let Err(e) = st.store.lock().unwrap().set_audio_dir(id, &dir.to_string_lossy()) {
                        log::error!("falha ao registrar a pasta do áudio: {e}");
                    }
                    (Arc::new(Mutex::new(Some(w))), Some(dir))
                }
                Err(e) => {
                    log::error!("não foi possível salvar o áudio: {e}");
                    (Arc::new(Mutex::new(None)), None)
                }
            }
        }
        None => (Arc::new(Mutex::new(None)), None),
    };

    *live.view.lock().unwrap() = LiveView {
        state: "recording",
        session_id: Some(id),
        headphones: hp,
        mic_denied: crate::os::mic_permission() == Some(false) && cfg.mic != MicMode::Off,
        eu: ChannelView { state: if cfg.mic == MicMode::Off { "off" } else { "opening" }, ..Default::default() },
        ..Default::default()
    };
    let capture = start_capture(app, &cfg, t0, engine.clone(), wavs.clone());
    let alive = Arc::new(AtomicBool::new(true));
    *live.session.lock().unwrap() = Some(Session { id, t0, started, cfg: cfg.clone(), capture: Some(capture), engine, wavs, audio_dir, keep_days: keep, alive: alive.clone() });
    set_recording(app, true);
    windows::show_mini(app);
    emit_view(app);
    log::info!(
        "gravação iniciada · fone: {} · salvar áudio: {} · {} threads",
        if hp { "sim" } else { "não" },
        match keep {
            None => "não".to_string(),
            Some(0) => "sempre".into(),
            Some(d) => format!("{d} dias"),
        },
        threads
    );
    spawn_ticker(app.clone(), alive, t0);
    Ok(())
}

/// A cada 250 ms: tempo e níveis para a mini; vigia o OUTROS mudo.
fn spawn_ticker(app: AppHandle, alive: Arc<AtomicBool>, t0: u64) {
    std::thread::spawn(move || {
        let mut last_signal = Instant::now();
        while alive.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(250));
            let live = app.state::<Live>();
            let levels = live.session.lock().unwrap().as_ref().and_then(|s| s.capture.as_ref().map(|c| c.levels()));
            let paused = levels.is_none();
            let [eu, outros] = levels.unwrap_or([f32::NEG_INFINITY; 2]);
            let elapsed = ouveai_audio::pos_of(ouveai_audio::qpc_100ns(), t0) as f64 / RATE as f64;
            let _ = app.emit("live-tick", Tick { elapsed, eu: if eu.is_finite() { eu } else { -120.0 }, outros: if outros.is_finite() { outros } else { -120.0 } });
            if paused || outros > SIGNAL_DB {
                last_signal = Instant::now();
            }
            let silent = last_signal.elapsed() >= OUTROS_SILENT;
            let was = live.view.lock().unwrap().outros_silent;
            if silent != was {
                if silent {
                    log::info!("OUTROS sem sinal há 60 s");
                }
                update(&app, |v| v.outros_silent = silent);
            }
        }
    });
}

pub fn stop(app: &AppHandle) {
    let live = app.state::<Live>();
    let Some(s) = live.session.lock().unwrap().take() else { return };
    s.alive.store(false, Ordering::SeqCst);
    update(app, |v| v.state = "finishing");
    windows::hide_mini(app);
    let app = app.clone();
    std::thread::spawn(move || {
        let id = s.id;
        finalize(&app, s);
        *app.state::<Live>().view.lock().unwrap() = LiveView { state: "idle", ..Default::default() };
        set_recording(&app, false);
        emit_view(&app);
        let _ = app.emit("meeting-done", id);
        schedule_release(&app);
    });
}

/// Ao sair do app com a gravação ativa: fecha tudo antes de encerrar.
pub fn shutdown(app: &AppHandle) {
    stop_test(app);
    let Some(s) = app.state::<Live>().session.lock().unwrap().take() else { return };
    s.alive.store(false, Ordering::SeqCst);
    log::info!("saindo com a gravação ativa: encerrando a sessão");
    finalize(app, s);
}

/// Para a captura, espera o motor terminar a fila, fecha os WAVs e a sessão.
fn finalize(app: &AppHandle, mut s: Session) {
    if let Some(c) = s.capture.take() {
        c.stop();
    }
    let duration = s.started.elapsed().as_secs_f64();
    let stats = match Arc::try_unwrap(s.engine) {
        Ok(e) => Some(e.finish()),
        Err(_) => {
            log::error!("motor ao vivo ainda em uso ao parar");
            None
        }
    };
    if let Some([a, b]) = s.wavs.lock().unwrap().take() {
        let _ = (a.finish(), b.finish());
    }
    let dir = s.audio_dir.as_ref().map(|d| d.to_string_lossy().into_owned());
    let keep = s.keep_days.filter(|&d| d > 0);
    if let Err(e) = app.state::<AppState>().store.lock().unwrap().end_meeting(s.id, duration, dir.as_deref(), keep) {
        log::error!("falha ao encerrar a sessão: {e}");
    }
    let Some(stats) = stats else { return };
    let f = |x: Option<f64>| x.map_or("-".into(), |v| format!("{v:.2} s"));
    log::info!(
        "gravação encerrada: {:.0} s · {} segmentos · {} eco · {} cortes em fala corrida · {} descartados · atraso p50 {} p95 {}",
        duration,
        stats.segments,
        stats.echoes,
        stats.forced,
        stats.dropped,
        f(stats.percentile(0.5)),
        f(stats.percentile(0.95))
    );
}

/// Solta o modelo depois de 5 min sem gravar (idle TTL).
fn schedule_release(app: &AppHandle) {
    let live = app.state::<Live>();
    let mine = live.asr_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let (cache, g) = (live.asr.clone(), live.asr_gen.clone());
    std::thread::spawn(move || {
        std::thread::sleep(IDLE_TTL);
        if g.load(Ordering::SeqCst) == mine && cache.lock().unwrap().take().is_some() {
            log::info!("modelo ao vivo liberado (5 min sem gravar)");
        }
    });
}

pub fn toggle(app: &AppHandle) {
    let live = app.state::<Live>();
    if live.toggling.swap(true, Ordering::SeqCst) {
        return;
    }
    toggle_inner(app);
    live.toggling.store(false, Ordering::SeqCst);
}

fn toggle_inner(app: &AppHandle) {
    let recording = app.state::<Live>().session.lock().unwrap().is_some();
    if recording {
        stop(app);
    } else if let Err(e) = start(app) {
        log::warn!("gravação não iniciada: {e}");
        let _ = app.emit("live-error", e);
    }
}

pub fn pause(app: &AppHandle) {
    let live = app.state::<Live>();
    // A captura para fora do lock: as threads dela também usam a sessão (marcas).
    let (c, id, t0, engine) = {
        let mut g = live.session.lock().unwrap();
        let Some(s) = g.as_mut() else { return };
        let Some(c) = s.capture.take() else { return };
        (c, s.id, s.t0, s.engine.clone())
    };
    let t = ouveai_audio::pos_of(ouveai_audio::qpc_100ns(), t0) as f64 / RATE as f64;
    c.stop(); // solta o mic
    engine.flush();
    drop(engine);
    let st = app.state::<AppState>();
    let db = st.store.lock().unwrap();
    let _ = db.set_status(id, "paused");
    let _ = db.add_mark(id, t, "pausa", None);
    drop(db);
    log::info!("gravação pausada");
    update(app, |v| v.state = "paused");
}

pub fn resume(app: &AppHandle) {
    let live = app.state::<Live>();
    let mut g = live.session.lock().unwrap();
    let Some(s) = g.as_mut() else { return };
    if s.capture.is_some() {
        return;
    }
    s.capture = Some(start_capture(app, &s.cfg, s.t0, s.engine.clone(), s.wavs.clone()));
    let _ = app.state::<AppState>().store.lock().unwrap().set_status(s.id, "recording");
    drop(g);
    log::info!("gravação retomada");
    update(app, |v| v.state = "recording");
}

/// ⭐ no instante atual (relógio da sessão).
pub fn mark(app: &AppHandle) -> bool {
    let live = app.state::<Live>();
    let g = live.session.lock().unwrap();
    let Some(s) = g.as_ref() else { return false };
    let t = ouveai_audio::pos_of(ouveai_audio::qpc_100ns(), s.t0) as f64 / RATE as f64;
    let ok = app.state::<AppState>().store.lock().unwrap().add_mark(s.id, t, "estrela", None).is_ok();
    drop(g);
    update(app, |v| v.marks += 1);
    ok
}

/// "Usar este mic": troca o EU sem parar a sessão.
pub fn use_mic(app: &AppHandle, device_id: String) {
    let live = app.state::<Live>();
    if let Some(s) = live.session.lock().unwrap().as_mut() {
        s.cfg.mic = MicMode::Device(device_id.clone());
        if let Some(c) = &s.capture {
            c.set_mic(MicMode::Device(device_id));
        }
    }
    update(app, |v| v.mic_elsewhere = None);
}

/// "Tentar de novo" (fonte travada): reabre a captura inteira.
pub fn retry(app: &AppHandle) {
    let live = app.state::<Live>();
    let Some(c) = live.session.lock().unwrap().as_mut().and_then(|s| s.capture.take()) else { return };
    c.stop();
    let mut g = live.session.lock().unwrap();
    let Some(s) = g.as_mut() else { return };
    s.capture = Some(start_capture(app, &s.cfg, s.t0, s.engine.clone(), s.wavs.clone()));
    drop(g);
    log::info!("captura reaberta (tentar de novo)");
    update(app, |v| {
        v.eu.state = if v.eu.state == "off" { "off" } else { "opening" };
        v.outros.state = "opening";
    });
}

pub fn set_headphones(app: &AppHandle, on: bool) {
    if let Some(s) = app.state::<Live>().session.lock().unwrap().as_ref() {
        s.engine.set_headphones(on);
    }
    update(app, |v| v.headphones = on);
}

// ---------- Testar áudio ----------

#[derive(Clone, Serialize)]
struct TestTick {
    eu: f32,
    outros: f32,
    eu_open: bool,
    outros_open: bool,
}

pub fn start_test(app: &AppHandle) -> Result<(), String> {
    if app.state::<Live>().session.lock().unwrap().is_some() {
        return Err("Já está gravando: os medidores estão na mini.".into());
    }
    stop_test(app);
    let cfg = config(&app.state::<AppState>());
    let a = app.clone();
    let cap = Capture::start(cfg, ouveai_audio::qpc_100ns(), Box::new(|_, _, _| {}), Arc::new(move |ev| {
        let _ = a.emit("audio-test-event", ev);
    }));
    let alive = Arc::new(AtomicBool::new(true));
    *app.state::<Live>().test.lock().unwrap() = Some((cap, alive.clone()));
    let app = app.clone();
    std::thread::spawn(move || {
        while alive.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(100));
            let live = app.state::<Live>();
            let g = live.test.lock().unwrap();
            let Some((c, _)) = g.as_ref() else { break };
            let [eu, outros] = c.levels();
            let t = TestTick { eu: if eu.is_finite() { eu } else { -120.0 }, outros: if outros.is_finite() { outros } else { -120.0 }, eu_open: c.is_open(Ch::Eu), outros_open: c.is_open(Ch::Outros) };
            drop(g);
            let _ = app.emit("audio-test", t);
        }
    });
    Ok(())
}

pub fn stop_test(app: &AppHandle) {
    if let Some((c, alive)) = app.state::<Live>().test.lock().unwrap().take() {
        alive.store(false, Ordering::SeqCst);
        std::thread::spawn(move || c.stop());
    }
}
