//! `ouveai-cli live` (Replay determinístico a partir de dois WAVs) e
//! `ouveai-cli capture` (captura real, sem UI).

use ouveai_core::asr::Asr;
use ouveai_core::live::{Channel, Engine, LiveSegment, ModelState, Output, RATE, SileroVad, Stats, Vad};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const LIVE_THREADS: i32 = 2;

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1))
}

fn clock(s: f64) -> String {
    let s = s.max(0.0);
    format!("{:02}:{:02}.{}", (s / 60.0) as u64, (s % 60.0) as u64, ((s * 10.0) as u64) % 10)
}

struct Print {
    n: i64,
    loaded: Instant,
}

impl Output for Print {
    fn segment(&mut self, s: &LiveSegment) -> i64 {
        self.n += 1;
        println!("[{:>3}] {} {:<6} {}{}{}", self.n, clock(s.start), s.ch.as_str(), if s.echo { "(eco) " } else { "" }, if s.forced { "(corte) " } else { "" }, s.text);
        self.n
    }
    fn echo(&mut self, id: i64, _echo: bool) {
        println!("      ↳ [{id:>3}] marcado como eco (o texto repete OUTROS)");
    }
    fn model(&mut self, st: ModelState) {
        match st {
            ModelState::Loading => self.loaded = Instant::now(),
            ModelState::Ready => println!("modelo carregado em {:.2} s ({LIVE_THREADS} threads)", self.loaded.elapsed().as_secs_f64()),
            ModelState::Error(e) => println!("modelo: {e}"),
        }
    }
}

fn vads() -> [Box<dyn Vad>; 2] {
    let v = || -> Box<dyn Vad> { Box::new(SileroVad::embedded().unwrap_or_else(|e| crate::fail(e))) };
    [v(), v()]
}

fn loader() -> ouveai_core::live::Loader {
    Box::new(|| Asr::load(LIVE_THREADS).map(Arc::new))
}

fn summary(st: &Stats, wall: f64, audio: f64) {
    println!("\nsegmentos: {} · eco: {} · cortes em fala corrida: {} · descartados: {}", st.segments, st.echoes, st.forced, st.dropped);
    println!("áudio {:.1} s processado em {:.1} s", audio, wall);
    if let (Some(p50), Some(p95)) = (st.percentile(0.5), st.percentile(0.95)) {
        println!("atraso fim da fala → texto: p50 {p50:.2} s · p95 {p95:.2} s (meta: p50 ≤ 3 s · p95 ≤ 6 s)");
    }
}

fn read16k(p: &Path) -> Vec<f32> {
    let w = sherpa_onnx::Wave::read(&p.to_string_lossy()).unwrap_or_else(|| crate::fail(format!("não foi possível ler {}", p.display())));
    if w.sample_rate() != RATE as i32 {
        crate::fail(format!("{}: precisa ser WAV 16 kHz mono (converta com ffmpeg -ar 16000 -ac 1)", p.display()));
    }
    w.samples().to_vec()
}

/// Replay: EU + OUTROS de dois WAVs. `--echo` soma OUTROS (atrasado 120 ms, -8 dB)
/// ao EU, como um alto-falante vazando no mic. `--realtime` mede o atraso de verdade.
pub fn replay_cmd(args: &[String]) {
    let eu = arg(args, "--eu").map(|p| read16k(Path::new(p))).unwrap_or_default();
    let outros = arg(args, "--outros").map(|p| read16k(Path::new(p))).unwrap_or_default();
    if eu.is_empty() && outros.is_empty() {
        crate::fail("uso: ouveai-cli live --eu a.wav --outros b.wav [--echo] [--headphones] [--realtime]");
    }
    let realtime = args.iter().any(|a| a == "--realtime");
    let headphones = args.iter().any(|a| a == "--headphones");
    let n = eu.len().max(outros.len());
    let mut eu = eu;
    eu.resize(n, 0.0);
    let mut outros = outros;
    outros.resize(n, 0.0);
    if args.iter().any(|a| a == "--echo") {
        let d = (RATE as usize) * 12 / 100;
        for i in d..n {
            eu[i] = (eu[i] + outros[i - d] * 0.4).clamp(-1.0, 1.0);
        }
        println!("eco simulado: OUTROS somado ao EU com 120 ms de atraso e -8 dB");
    }
    let t = Instant::now();
    let engine = Engine::start(headphones, vads(), loader(), Box::new(Print { n: 0, loaded: Instant::now() }), Instant::now());
    let step = (RATE / 10) as usize;
    for (k, a) in (0..n).step_by(step).enumerate() {
        let b = (a + step).min(n);
        engine.audio(Channel::Eu, a as u64, eu[a..b].to_vec());
        engine.audio(Channel::Outros, a as u64, outros[a..b].to_vec());
        if realtime {
            let due = Duration::from_millis(100 * (k as u64 + 1));
            if let Some(w) = due.checked_sub(t.elapsed()) {
                std::thread::sleep(w);
            }
        }
    }
    let st = engine.finish();
    summary(&st, t.elapsed().as_secs_f64(), n as f64 / RATE as f64);
}

/// Captura real (mic + loopback) por N segundos, com a transcrição no terminal.
pub fn capture_cmd(args: &[String]) {
    use ouveai_audio::{Capture, Ch, Config, Event, MicMode, OutrosMode};
    let secs: u64 = arg(args, "--secs").and_then(|s| s.parse().ok()).unwrap_or(30);
    let mut cfg = Config::default();
    if args.iter().any(|a| a == "--no-mic") {
        cfg.mic = MicMode::Off;
    }
    if let Some(exe) = arg(args, "--app") {
        cfg.outros = OutrosMode::App(exe.clone());
    }
    let headphones = if args.iter().any(|a| a == "--headphones") {
        true
    } else if args.iter().any(|a| a == "--speakers") {
        false
    } else {
        ouveai_audio::default_output_is_headphone().unwrap_or(false)
    };
    let save = arg(args, "--save").map(std::path::PathBuf::from);
    println!("dispositivos:");
    for d in ouveai_audio::list_devices() {
        println!("  {} {}{} [{}]{}", if d.input { "entrada" } else { "saída  " }, if d.default { "* " } else { "  " }, d.name, d.form, if d.bluetooth { " (Bluetooth)" } else { "" });
    }
    println!("fone: {} · {secs} s\n", if headphones { "sim (sem filtro de eco)" } else { "não (filtro de eco ligado)" });

    let t0 = ouveai_audio::qpc_100ns();
    let started = Instant::now();
    let engine = Arc::new(Engine::start(headphones, vads(), loader(), Box::new(Print { n: 0, loaded: Instant::now() }), started));
    let wavs = Arc::new(std::sync::Mutex::new(save.map(|d| {
        std::fs::create_dir_all(&d).ok();
        [ouveai_core::wav::Writer::create(&d.join("eu.wav"), RATE as u32).unwrap(), ouveai_core::wav::Writer::create(&d.join("outros.wav"), RATE as u32).unwrap()]
    })));
    let sink = {
        let (e, wavs) = (engine.clone(), wavs.clone());
        Box::new(move |ch: Ch, pos: u64, s: &[f32]| {
            let c = if ch == Ch::Eu { Channel::Eu } else { Channel::Outros };
            if let Some(w) = wavs.lock().unwrap().as_mut() {
                let _ = w[c as usize].put(pos, s);
            }
            e.audio(c, pos, s.to_vec());
        })
    };
    let events = Arc::new(move |ev: Event| {
        let t = started.elapsed().as_secs_f64();
        println!("      · {} {:?}", clock(t), ev);
    });
    let cap = Capture::start(cfg, t0, sink, events);
    for i in 1..=secs {
        std::thread::sleep(Duration::from_secs(1));
        if i % 5 == 0 {
            let [a, b] = cap.levels();
            let f = |x: f32| if x.is_finite() { format!("{x:.0} dB") } else { "--".into() };
            println!("      · {i:>3} s · nível EU {} · OUTROS {}", f(a), f(b));
        }
    }
    cap.stop();
    if let Some([a, b]) = wavs.lock().unwrap().take() {
        let _ = (a.finish(), b.finish());
    }
    let engine = Arc::try_unwrap(engine).unwrap_or_else(|_| crate::fail("motor ainda em uso"));
    let st = engine.finish();
    summary(&st, started.elapsed().as_secs_f64(), secs as f64);
}
