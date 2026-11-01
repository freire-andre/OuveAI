//! Captura ao vivo: EU (mic) e OUTROS (loopback), cada fonte na sua thread
//! MMCSS, em 16 kHz mono float (`autoconvert` do WASAPI), posicionadas pelo QPC
//! numa linha do tempo por canal. Uma thread de entrega solta o áudio com
//! ~400 ms de atraso (tempo para os pacotes chegarem e as fontes se somarem).
//!
//! Regras do S3:
//! - cada fonte abre sozinha: o antivírus pode segurar a abertura por segundos,
//!   e EU nunca espera OUTROS (aviso em 2 s, alerta em 60 s);
//! - troca do padrão e `AUDCLNT_E_DEVICE_INVALIDATED` reabrem sem parar a sessão;
//! - nunca abrir sozinho o mic de um fone Bluetooth que nenhum app esteja
//!   usando (isso força o perfil de chamada e o som fica abafado);
//! - tolerância de 200 ms no mic (jitter do Bluetooth em chamada).

use crate::device::{self, SessionUser};
use crate::timeline::Timeline;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use wasapi::{AudioClient, DeviceEnumerator, DeviceEventCallbacks, Direction, Role, SampleType, StreamMode, WaveFormat};

pub const RATE: u64 = 16_000;
/// Atraso da entrega: os pacotes de todas as fontes chegam antes disso.
const LATENCY: u64 = RATE * 4 / 10;
const WAIT_WARN: Duration = Duration::from_secs(2);
const WAIT_STUCK: Duration = Duration::from_secs(60);

/// Relógio da sessão: QPC em unidades de 100 ns.
pub fn qpc_100ns() -> u64 {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let (mut c, mut f) = (0i64, 0i64);
    unsafe {
        let _ = QueryPerformanceCounter(&mut c);
        let _ = QueryPerformanceFrequency(&mut f);
    }
    (c as u128 * 10_000_000 / f.max(1) as u128) as u64
}

/// Posição (amostras a 16 kHz) de um instante QPC em relação a `t0`.
pub fn pos_of(qpc: u64, t0: u64) -> u64 {
    qpc.saturating_sub(t0) * RATE / 10_000_000
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Ch {
    Eu,
    Outros,
}

impl Ch {
    fn i(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", content = "id", rename_all = "lowercase")]
pub enum MicMode {
    /// O mic que o app de reunião usa; sem app, o dispositivo padrão (Console).
    #[default]
    Auto,
    Device(String),
    Off,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", content = "id", rename_all = "lowercase")]
pub enum OutrosMode {
    /// Padrão multimídia + padrão de comunicação, somados (sem duplicar).
    #[default]
    Auto,
    Device(String),
    /// Loopback só do processo (Windows 11 / build 20348+).
    App(String),
}

#[derive(Clone, Debug)]
pub struct Config {
    pub mic: MicMode,
    pub outros: OutrosMode,
    pub mic_tol_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config { mic: MicMode::Auto, outros: OutrosMode::Auto, mic_tol_ms: 200 }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Opened { ch: Ch, device: String, form: String, reopen: bool },
    /// Sem abrir há 2 s (antivírus pedindo permissão?).
    Waiting { ch: Ch },
    /// Sem abrir há 60 s.
    Stuck { ch: Ch },
    OpenError { ch: Ch, error: String },
    /// Dispositivo perdido (desconectado, invalidado); reabrindo.
    Lost { ch: Ch, error: String },
    /// O dispositivo padrão mudou; reabrindo no novo.
    DefaultChanged { ch: Ch },
    Gap { ch: Ch, ms: u64 },
    /// Mic de fone Bluetooth sem uso: não abrimos (forçaria o perfil de chamada).
    BtMicIdle { device: String },
    /// O app de reunião está usando outro mic.
    MicElsewhere { device: String, device_id: String, app: String },
    AppMissing { exe: String },
}

pub type Sink = Box<dyn FnMut(Ch, u64, &[f32]) + Send>;
pub type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Mic,
    LoopMm,
    LoopComm,
    LoopDevice,
    Proc,
}

impl Kind {
    fn ch(self) -> Ch {
        if self == Kind::Mic { Ch::Eu } else { Ch::Outros }
    }
    /// Posição na linha do tempo do canal (loopbacks somados).
    fn slot(self) -> usize {
        if self == Kind::LoopComm { 1 } else { 0 }
    }
}

const OPENING: u8 = 0;
const OPEN: u8 = 1;
const HELD: u8 = 2;

struct SourceState {
    kind: Kind,
    state: AtomicU8,
    since: Mutex<Instant>,
    warned: AtomicU8, // 0 nada, 1 aguardando, 2 travado
}

struct Shared {
    t0: u64,
    stop: AtomicBool,
    render_gen: AtomicU64,
    capture_gen: AtomicU64,
    lines: [Mutex<Timeline>; 2],
    acc: [Mutex<(f64, u64)>; 2],
    level: [AtomicU32; 2],
    mic_mode: Mutex<MicMode>,
    outros_mode: OutrosMode,
    mic_tol: u64,
    mic_id: Mutex<Option<String>>,
    sources: Vec<SourceState>,
    ev: EventSink,
}

impl Shared {
    fn emit(&self, e: Event) {
        (self.ev)(e)
    }
    fn set_state(&self, i: usize, s: u8) {
        let src = &self.sources[i];
        let prev = src.state.swap(s, Ordering::SeqCst);
        if s == OPENING && prev != OPENING {
            *src.since.lock().unwrap() = Instant::now();
            src.warned.store(0, Ordering::SeqCst);
        }
    }
}

pub struct Capture {
    sh: Arc<Shared>,
    sources: Vec<JoinHandle<()>>,
    flusher: Option<JoinHandle<()>>,
    flush_stop: Arc<AtomicBool>,
}

impl Capture {
    /// Começa a captar. As posições entregues ao `sink` são amostras a 16 kHz
    /// desde `t0` (QPC), a partir do instante atual.
    pub fn start(cfg: Config, t0: u64, sink: Sink, ev: EventSink) -> Capture {
        let mut kinds = Vec::new();
        if cfg.mic != MicMode::Off {
            kinds.push(Kind::Mic);
        }
        match &cfg.outros {
            OutrosMode::Auto => kinds.extend([Kind::LoopMm, Kind::LoopComm]),
            OutrosMode::Device(_) => kinds.push(Kind::LoopDevice),
            OutrosMode::App(_) => kinds.push(Kind::Proc),
        }
        let start = pos_of(qpc_100ns(), t0);
        let mic_tol = cfg.mic_tol_ms * RATE / 1000;
        let sh = Arc::new(Shared {
            t0,
            stop: AtomicBool::new(false),
            render_gen: AtomicU64::new(0),
            capture_gen: AtomicU64::new(0),
            lines: [Mutex::new(Timeline::new(start, mic_tol)), Mutex::new(Timeline::new(start, RATE / 100))],
            acc: [Mutex::new((0.0, 0)), Mutex::new((0.0, 0))],
            level: [AtomicU32::new(f32::NEG_INFINITY.to_bits()), AtomicU32::new(f32::NEG_INFINITY.to_bits())],
            mic_mode: Mutex::new(cfg.mic.clone()),
            outros_mode: cfg.outros.clone(),
            mic_tol,
            mic_id: Mutex::new(None),
            sources: kinds
                .iter()
                .map(|&k| SourceState { kind: k, state: AtomicU8::new(OPENING), since: Mutex::new(Instant::now()), warned: AtomicU8::new(0) })
                .collect(),
            ev,
        });
        let flush_stop = Arc::new(AtomicBool::new(false));
        let flusher = {
            let (sh, fs) = (sh.clone(), flush_stop.clone());
            thread::Builder::new().name("audio-entrega".into()).spawn(move || flusher(sh, fs, sink)).ok()
        };
        let mut sources = Vec::new();
        for (i, &kind) in kinds.iter().enumerate() {
            let sh = sh.clone();
            if let Ok(h) = thread::Builder::new().name(format!("audio-{i}")).spawn(move || source_thread(sh, i)) {
                sources.push(h);
            }
            if kind == Kind::LoopMm {
                thread::sleep(Duration::from_millis(100)); // o multimídia abre antes do comunicação (dedupe)
            }
        }
        Capture { sh, sources, flusher, flush_stop }
    }

    /// Níveis (dBFS) dos últimos ~100 ms: [EU, OUTROS].
    pub fn levels(&self) -> [f32; 2] {
        [f32::from_bits(self.sh.level[0].load(Ordering::Relaxed)), f32::from_bits(self.sh.level[1].load(Ordering::Relaxed))]
    }

    /// Troca o mic sem parar a sessão (ex.: "usar o mic do Teams").
    pub fn set_mic(&self, mode: MicMode) {
        *self.sh.mic_mode.lock().unwrap() = mode;
        self.sh.capture_gen.fetch_add(1, Ordering::SeqCst);
    }

    /// O canal já abriu algum dispositivo?
    pub fn is_open(&self, ch: Ch) -> bool {
        self.sh.sources.iter().any(|s| s.kind.ch() == ch && s.state.load(Ordering::SeqCst) == OPEN)
    }

    /// Para tudo e entrega o áudio até agora.
    pub fn stop(mut self) {
        self.sh.stop.store(true, Ordering::SeqCst);
        for h in self.sources.drain(..) {
            let _ = h.join();
        }
        self.flush_stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.flusher.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.sh.stop.store(true, Ordering::SeqCst);
        self.flush_stop.store(true, Ordering::SeqCst);
    }
}

fn db(sum_sq: f64, n: u64) -> f32 {
    if n == 0 {
        return f32::NEG_INFINITY;
    }
    let rms = (sum_sq / n as f64).sqrt();
    if rms <= 1e-9 { -120.0 } else { (20.0 * rms.log10()) as f32 }
}

fn flusher(sh: Arc<Shared>, flush_stop: Arc<AtomicBool>, mut sink: Sink) {
    let _ = wasapi::initialize_mta();
    let enumr = DeviceEnumerator::new().ok();
    // Troca de padrão → as fontes reabrem no novo dispositivo.
    let _reg = enumr.as_ref().and_then(|e| {
        let mut cb = DeviceEventCallbacks::new();
        let s = sh.clone();
        cb.set_default_device_callback(move |dir, _role, _id| {
            if dir == Direction::Capture { &s.capture_gen } else { &s.render_gen }.fetch_add(1, Ordering::SeqCst);
        });
        e.register_notification_callback(cb).ok()
    });
    let mut last_users = Instant::now() - Duration::from_secs(10);
    let mut warned_elsewhere: Option<(String, String)> = None;
    loop {
        let done = flush_stop.load(Ordering::SeqCst);
        if !done {
            thread::sleep(Duration::from_millis(100));
        }
        let now = pos_of(qpc_100ns(), sh.t0);
        let upto = if done { now } else { now.saturating_sub(LATENCY) };
        for ch in [Ch::Eu, Ch::Outros] {
            let chunk = sh.lines[ch.i()].lock().unwrap().drain(upto);
            if let Some((pos, samples)) = chunk {
                sink(ch, pos, &samples);
            }
            let mut a = sh.acc[ch.i()].lock().unwrap();
            sh.level[ch.i()].store(db(a.0, a.1).to_bits(), Ordering::Relaxed);
            *a = (0.0, 0);
        }
        if done {
            break;
        }
        // Fontes que não abrem (antivírus segurando o Initialize).
        for s in &sh.sources {
            if s.state.load(Ordering::SeqCst) != OPENING {
                continue;
            }
            let waited = s.since.lock().unwrap().elapsed();
            let w = s.warned.load(Ordering::SeqCst);
            if waited > WAIT_STUCK && w < 2 {
                s.warned.store(2, Ordering::SeqCst);
                sh.emit(Event::Stuck { ch: s.kind.ch() });
            } else if waited > WAIT_WARN && w < 1 {
                s.warned.store(1, Ordering::SeqCst);
                sh.emit(Event::Waiting { ch: s.kind.ch() });
            }
        }
        // O app de reunião usa outro mic? (a cada 3 s)
        if last_users.elapsed() >= Duration::from_secs(3) {
            last_users = Instant::now();
            let ours = sh.mic_id.lock().unwrap().clone();
            if let (Some(e), Some(ours)) = (&enumr, ours)
                && let Some(u) = device::meeting_mic(e) {
                    let key = (u.device_id.clone(), u.exe.clone());
                    if u.device_id != ours && warned_elsewhere.as_ref() != Some(&key) {
                        sh.emit(Event::MicElsewhere { device: u.device, device_id: u.device_id, app: u.exe });
                        warned_elsewhere = Some(key);
                    }
                }
        }
    }
}

fn open_client(e: &DeviceEnumerator, dev: &wasapi::Device) -> Result<AudioClient, wasapi::WasapiError> {
    let fmt = WaveFormat::new(32, 32, &SampleType::Float, RATE as usize, 1, None);
    let mut c = dev.get_iaudioclient()?;
    c.initialize_client(&fmt, &Direction::Capture, &StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 200_000 })?;
    let _ = e;
    Ok(c)
}

/// O que abrir agora. `Err(msg)` = esperar e tentar de novo (com o motivo).
enum Plan {
    Open { client: AudioClient, desc: String, form: String, id: Option<String>, pid: Option<u32> },
    Hold(Option<Event>),
}

fn plan(sh: &Shared, kind: Kind, e: &DeviceEnumerator) -> Result<Plan, String> {
    let werr = |x: wasapi::WasapiError| x.to_string();
    let dev = match kind {
        Kind::Mic => {
            let mode = sh.mic_mode.lock().unwrap().clone();
            match mode {
                MicMode::Off => return Ok(Plan::Hold(None)),
                MicMode::Device(id) => e.get_device(&id).map_err(werr)?,
                MicMode::Auto => {
                    // O mic do app de reunião; sem app, o dispositivo padrão (Console).
                    let used: Option<SessionUser> = device::meeting_mic(e);
                    let d = match &used {
                        Some(u) => e.get_device(&u.device_id).map_err(werr)?,
                        None => e.get_default_device_for_role(&Direction::Capture, &Role::Console).map_err(werr)?,
                    };
                    if used.is_none() && device::is_bluetooth(&d) {
                        return Ok(Plan::Hold(Some(Event::BtMicIdle { device: d.get_friendlyname().unwrap_or_default() })));
                    }
                    d
                }
            }
        }
        Kind::LoopMm => e.get_default_device_for_role(&Direction::Render, &Role::Multimedia).map_err(werr)?,
        Kind::LoopComm => {
            let d = e.get_default_device_for_role(&Direction::Render, &Role::Communications).map_err(werr)?;
            // Mesmo endpoint do multimídia: fica ocioso (sem duplicar).
            let id = d.get_id().ok();
            let mm = e.get_default_device_for_role(&Direction::Render, &Role::Multimedia).ok().and_then(|m| m.get_id().ok());
            if id.is_some() && mm == id {
                return Ok(Plan::Hold(None));
            }
            d
        }
        Kind::LoopDevice => {
            let OutrosMode::Device(id) = &sh.outros_mode else { unreachable!() };
            e.get_device(id).map_err(werr)?
        }
        Kind::Proc => {
            let OutrosMode::App(exe) = &sh.outros_mode else { unreachable!() };
            let Some(pid) = device::find_process_root(exe) else {
                return Ok(Plan::Hold(Some(Event::AppMissing { exe: exe.clone() })));
            };
            let fmt = WaveFormat::new(32, 32, &SampleType::Float, RATE as usize, 1, None);
            let mut c = AudioClient::new_application_loopback_client(pid, true).map_err(werr)?;
            c.initialize_client(&fmt, &Direction::Capture, &StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 0 }).map_err(werr)?;
            return Ok(Plan::Open { client: c, desc: exe.clone(), form: "App".into(), id: None, pid: Some(pid) });
        }
    };
    let desc = dev.get_friendlyname().unwrap_or_else(|_| "?".into());
    let form = device::form_factor(&dev);
    let id = dev.get_id().ok();
    let client = open_client(e, &dev).map_err(werr)?;
    Ok(Plan::Open { client, desc, form, id, pid: None })
}

fn source_thread(sh: Arc<Shared>, i: usize) {
    let _ = wasapi::initialize_mta();
    unsafe {
        let mut idx = 0u32;
        let _ = windows::Win32::System::Threading::AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut idx);
    }
    let kind = sh.sources[i].kind;
    let ch = kind.ch();
    let Ok(e) = DeviceEnumerator::new() else { return };
    let mut buf = vec![0u8; 4 * RATE as usize];
    let mut bad_clock_logged = false;
    let mut opened_before = false;
    let mut last_err = String::new();
    let mut held_event: Option<String> = None;
    let follows_default = matches!(kind, Kind::Mic | Kind::LoopMm | Kind::LoopComm);
    let gen_of = |k: Kind| if k == Kind::Mic { &sh.capture_gen } else { &sh.render_gen };
    let sleep_unless = |ms: u64, seen: u64| {
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(ms) && !sh.stop.load(Ordering::SeqCst) && gen_of(kind).load(Ordering::SeqCst) == seen {
            thread::sleep(Duration::from_millis(50));
        }
    };

    'outer: while !sh.stop.load(Ordering::SeqCst) {
        let seen = gen_of(kind).load(Ordering::SeqCst);
        if sh.sources[i].state.load(Ordering::SeqCst) == OPEN {
            sh.set_state(i, OPENING);
        }
        let (client, desc, form, id, pid) = match plan(&sh, kind, &e) {
            Ok(Plan::Open { client, desc, form, id, pid }) => (client, desc, form, id, pid),
            Ok(Plan::Hold(ev)) => {
                sh.set_state(i, HELD);
                if let Some(ev) = ev {
                    let key = format!("{ev:?}");
                    if held_event.as_deref() != Some(key.as_str()) {
                        sh.emit(ev);
                        held_event = Some(key);
                    }
                }
                if kind == Kind::Mic {
                    *sh.mic_id.lock().unwrap() = None;
                }
                sleep_unless(2000, seen);
                continue;
            }
            Err(err) => {
                if sh.sources[i].state.load(Ordering::SeqCst) == HELD {
                    sh.set_state(i, OPENING);
                }
                if err != last_err {
                    sh.emit(Event::OpenError { ch, error: err.clone() });
                    last_err = err;
                }
                sleep_unless(500, seen);
                continue;
            }
        };
        held_event = None;
        last_err.clear();
        if kind == Kind::Mic {
            *sh.mic_id.lock().unwrap() = id.clone();
        }
        let (h, cap) = match (client.set_get_eventhandle(), client.get_audiocaptureclient()) {
            (Ok(h), Ok(c)) => (h, c),
            _ => {
                sleep_unless(500, seen);
                continue;
            }
        };
        if let Err(err) = client.start_stream() {
            sh.emit(Event::OpenError { ch, error: err.to_string() });
            sleep_unless(500, seen);
            continue;
        }
        sh.set_state(i, OPEN);
        sh.lines[ch.i()].lock().unwrap().reset_source(kind.slot());
        sh.emit(Event::Opened { ch, device: desc, form, reopen: opened_before });
        opened_before = true;
        let tol = sh.mic_tol;
        let mut alive_check = Instant::now();

        loop {
            if sh.stop.load(Ordering::SeqCst) {
                let _ = client.stop_stream();
                break 'outer;
            }
            if (follows_default || kind == Kind::Mic) && gen_of(kind).load(Ordering::SeqCst) != seen {
                sh.emit(Event::DefaultChanged { ch });
                let _ = client.stop_stream();
                continue 'outer;
            }
            if let Some(pid) = pid
                && alive_check.elapsed() > Duration::from_secs(5) {
                    alive_check = Instant::now();
                    if !device::process_alive(pid) {
                        let _ = client.stop_stream();
                        continue 'outer;
                    }
                }
            match h.wait_for_event(200) {
                Ok(()) => {}
                Err(wasapi::WasapiError::EventTimeout) => continue,
                Err(err) => {
                    sh.emit(Event::Lost { ch, error: err.to_string() });
                    let _ = client.stop_stream();
                    sleep_unless(300, seen);
                    continue 'outer;
                }
            }
            loop {
                let (frames, info) = match cap.read_from_device(&mut buf) {
                    Ok(x) => x,
                    Err(err) => {
                        sh.emit(Event::Lost { ch, error: err.to_string() });
                        let _ = client.stop_stream();
                        sleep_unless(300, seen);
                        continue 'outer;
                    }
                };
                if frames == 0 {
                    break;
                }
                let samples: Vec<f32> = buf[..frames as usize * 4]
                    .as_chunks::<4>().0.iter()
                    .map(|b| if info.flags.silent { 0.0 } else { f32::from_le_bytes([b[0], b[1], b[2], b[3]]) })
                    .collect();
                // Um pacote não pode vir do futuro: alguns dispositivos virtuais (ex.: o
                // "Áudio Remoto" da Área de Trabalho Remota/Sandbox) entregam um tempo que
                // não é QPC. Nesse caso a faixa segue pela contagem de amostras.
                let now = pos_of(qpc_100ns(), sh.t0);
                let pos = if info.flags.timestamp_error || info.timestamp == 0 || info.timestamp < sh.t0 { None } else { Some(pos_of(info.timestamp, sh.t0)) };
                let pos = match pos {
                    Some(p) if p > now + RATE => {
                        if !bad_clock_logged {
                            bad_clock_logged = true;
                            log::warn!("{ch:?}: o dispositivo entrega um tempo fora do relógio ({} s à frente); seguindo pela contagem de amostras", (p - now) / RATE);
                        }
                        None
                    }
                    p => p,
                };
                let gap = sh.lines[ch.i()].lock().unwrap().put(kind.slot(), pos, &samples);
                // No loopback, lacuna é só silêncio (nada tocando); no mic, é perda.
                if let Some(g) = gap.filter(|&g| kind == Kind::Mic && g > tol) {
                    let ms = g * 1000 / RATE;
                    if ms >= 500 {
                        sh.emit(Event::Gap { ch, ms });
                    }
                }
                let mut a = sh.acc[ch.i()].lock().unwrap();
                a.0 += samples.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>();
                a.1 += samples.len() as u64;
            }
        }
    }
    sh.set_state(i, HELD);
}
