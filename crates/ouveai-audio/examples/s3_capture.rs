//! Spike S3 (Fase 0): captura do microfone (EU) e do loopback (OUTROS) ao mesmo tempo.
//!
//! Grava um WAV por fonte (48 kHz mono, PCM16), cada pacote posicionado pelo seu QPC
//! (relógio da sessão) e as lacunas preenchidas com silêncio. Registra trocas do
//! dispositivo padrão, invalidações e reaberturas, o tipo do dispositivo (fone ou
//! alto-falante) e o nível por segundo de cada fonte.
//!
//! Uso:
//!   s3_capture [--secs 120] [--out DIR] [--no-mic] [--mic-role console|comm] [--mic-tol-ms 10] [--proc ms-teams.exe]
//!
//! Fontes:
//!   mic       microfone padrão (EU); `--mic-role comm` usa o padrão de comunicação
//!   loop_mm   loopback do padrão multimídia (OUTROS)
//!   loop_comm loopback do padrão de comunicação (OUTROS; fica ocioso se for o mesmo dispositivo)
//!   proc      loopback só do processo indicado em --proc (Win11, opcional)

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use wasapi::{AudioClient, Device, DeviceEnumerator, DeviceEventCallbacks, Direction, Role, SampleType, StreamMode, WaveFormat};

const RATE: u64 = 48_000;
/// Diferença entre o QPC e o que já foi escrito acima da qual vira lacuna (preenchida com silêncio).
const GAP_TOL_FRAMES: u64 = RATE / 100; // 10 ms

// ---------- relógio da sessão (QPC em 100 ns) ----------

fn qpc_100ns() -> u64 {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let (mut c, mut f) = (0i64, 0i64);
    unsafe {
        let _ = QueryPerformanceCounter(&mut c);
        let _ = QueryPerformanceFrequency(&mut f);
    }
    (c as u128 * 10_000_000 / f as u128) as u64
}

// ---------- log de eventos ----------

#[derive(Clone)]
struct Events {
    t0: u64,
    file: Arc<Mutex<BufWriter<File>>>,
}

impl Events {
    fn log(&self, source: &str, event: &str, detail: &str) {
        let t = (qpc_100ns().saturating_sub(self.t0)) as f64 / 1e7;
        let line = format!("{t:.3};{source};{event};{detail}");
        println!("  [{t:>7.2}s] {source:<9} {event}: {detail}");
        let mut f = self.file.lock().unwrap();
        let _ = writeln!(f, "{line}");
        let _ = f.flush();
    }
}

// ---------- WAV (cabeçalho regravado a cada segundo, para sobreviver a um kill) ----------

struct Track {
    w: BufWriter<File>,
    /// Desvio entre o QPC e o já escrito abaixo do qual a faixa segue contínua (jitter não vira lacuna).
    tol: u64,
    written: u64,
    from_packets: u64,
    filled_gap: u64,
    gaps: u64,
    max_gap_ms: f64,
    overlaps: u64,
    last_header: Instant,
}

impl Track {
    fn create(path: &Path, tol: u64) -> std::io::Result<Self> {
        let mut w = BufWriter::new(File::create(path)?);
        w.write_all(&[0u8; 44])?;
        let mut t = Track { w, tol, written: 0, from_packets: 0, filled_gap: 0, gaps: 0, max_gap_ms: 0.0, overlaps: 0, last_header: Instant::now() };
        t.header()?;
        Ok(t)
    }

    fn header(&mut self) -> std::io::Result<()> {
        let data = (self.written * 2) as u32;
        let mut h = Vec::with_capacity(44);
        h.extend_from_slice(b"RIFF");
        h.extend_from_slice(&(36 + data).to_le_bytes());
        h.extend_from_slice(b"WAVEfmt ");
        h.extend_from_slice(&16u32.to_le_bytes());
        h.extend_from_slice(&1u16.to_le_bytes()); // PCM
        h.extend_from_slice(&1u16.to_le_bytes()); // mono
        h.extend_from_slice(&(RATE as u32).to_le_bytes());
        h.extend_from_slice(&((RATE * 2) as u32).to_le_bytes());
        h.extend_from_slice(&2u16.to_le_bytes());
        h.extend_from_slice(&16u16.to_le_bytes());
        h.extend_from_slice(b"data");
        h.extend_from_slice(&data.to_le_bytes());
        self.w.flush()?;
        let f = self.w.get_mut();
        let end = f.seek(SeekFrom::End(0))?;
        f.seek(SeekFrom::Start(0))?;
        f.write_all(&h)?;
        f.seek(SeekFrom::Start(end))?;
        self.last_header = Instant::now();
        Ok(())
    }

    fn zeros(&mut self, n: u64) {
        for _ in 0..n {
            let _ = self.w.write_all(&0i16.to_le_bytes());
        }
        self.written += n;
    }

    /// Escreve `samples` na posição `pos` (em frames desde o início da sessão).
    /// `None` = sem timestamp confiável: anexa em sequência.
    fn put(&mut self, pos: Option<u64>, samples: &[f32]) -> Option<f64> {
        let mut gap_ms = None;
        let mut skip = 0usize;
        if let Some(p) = pos {
            if p > self.written + self.tol {
                let n = p - self.written;
                let ms = n as f64 * 1000.0 / RATE as f64;
                self.zeros(n);
                self.filled_gap += n;
                self.gaps += 1;
                self.max_gap_ms = self.max_gap_ms.max(ms);
                gap_ms = Some(ms);
            } else if p + (samples.len() as u64) + self.tol <= self.written {
                self.overlaps += 1;
                return None;
            } else if p + self.tol < self.written {
                skip = ((self.written - p) as usize).min(samples.len());
                self.overlaps += 1;
            }
        }
        for s in &samples[skip..] {
            let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            let _ = self.w.write_all(&v.to_le_bytes());
        }
        let n = (samples.len() - skip) as u64;
        self.written += n;
        self.from_packets += n;
        if self.last_header.elapsed() >= Duration::from_secs(1) {
            let _ = self.header();
        }
        gap_ms
    }
}

// ---------- nível por segundo (medidor) ----------

#[derive(Default)]
struct Meter {
    sum_sq: f64,
    n: u64,
    packets: u64,
    device: String,
}

fn dbfs(m: &Meter) -> f64 {
    if m.n == 0 {
        return f64::NEG_INFINITY;
    }
    let rms = (m.sum_sq / m.n as f64).sqrt();
    if rms <= 1e-9 { -120.0 } else { 20.0 * rms.log10() }
}

// ---------- dispositivo: tipo (form factor) ----------

fn form_factor(dev: &Device) -> String {
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::STGM_READ;
    use windows::core::GUID;
    // PKEY_AudioEndpoint_FormFactor {1DA5D803-D492-4EDD-8C23-E0C0FFEE7F0E}, 0
    let key = PROPERTYKEY { fmtid: GUID::from_u128(0x1da5d803_d492_4edd_8c23_e0c0ffee7f0e), pid: 0 };
    let names = ["RemoteNetwork", "Speakers", "LineLevel", "Headphones", "Microphone", "Headset", "Handset", "DigitalPassthrough", "SPDIF", "HDMI/DisplayPort", "Unknown"];
    let r: windows::core::Result<u32> = (|| unsafe {
        let store = dev.as_immdevice().OpenPropertyStore(STGM_READ)?;
        let pv = store.GetValue(&key)?;
        u32::try_from(&pv)
    })();
    match r {
        Ok(v) => names.get(v as usize).map_or(format!("{v}"), |s| s.to_string()),
        Err(_) => "?".into(),
    }
}

fn describe(dev: &Device) -> String {
    format!("{} [{}]", dev.get_friendlyname().unwrap_or_else(|_| "?".into()), form_factor(dev))
}

// ---------- processo (para o loopback por aplicativo) ----------

/// (pid, pid do pai, nome do exe) de todos os processos.
fn list_processes() -> Vec<(u32, u32, String)> {
    use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
    let mut procs: Vec<(u32, u32, String)> = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return procs };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            procs.push((e.th32ProcessID, e.th32ParentProcessID, String::from_utf16_lossy(&e.szExeFile[..len])));
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
    }
    procs
}

/// Quem está captando de cada microfone agora (sessões ativas), ex.: "USB PnP: Zoom.exe".
fn capture_sessions(enumr: &DeviceEnumerator) -> String {
    use wasapi::SessionState;
    let procs = list_processes();
    let me = std::process::id();
    let mut parts = Vec::new();
    if let Ok(coll) = enumr.get_device_collection(&Direction::Capture) {
        for i in 0..coll.get_nbr_devices().unwrap_or(0) {
            let Ok(d) = coll.get_device_at_index(i) else { continue };
            let Ok(sessions) = d.get_iaudiosessionmanager().and_then(|m| m.get_audiosessionenumerator()) else { continue };
            let mut users = Vec::new();
            for s in 0..sessions.get_count().unwrap_or(0) {
                let Ok(c) = sessions.get_session(s) else { continue };
                if c.get_state().ok() != Some(SessionState::Active) {
                    continue;
                }
                let pid = c.get_process_id().unwrap_or(0);
                if pid == me {
                    continue;
                }
                users.push(procs.iter().find(|p| p.0 == pid).map_or(format!("pid {pid}"), |p| p.2.clone()));
            }
            if !users.is_empty() {
                users.sort();
                users.dedup();
                parts.push(format!("{}: {}", d.get_friendlyname().unwrap_or_default(), users.join(", ")));
            }
        }
    }
    if parts.is_empty() { "nenhum outro app captando".into() } else { parts.join(" | ") }
}

fn find_process_root(exe: &str) -> Option<u32> {
    let procs = list_processes();
    let same: Vec<&(u32, u32, String)> = procs.iter().filter(|p| p.2.eq_ignore_ascii_case(exe)).collect();
    // A raiz é o processo com esse nome cujo pai não tem o mesmo nome.
    same.iter()
        .find(|p| !same.iter().any(|q| q.0 == p.1))
        .or(same.first())
        .map(|p| p.0)
}

// ---------- uma fonte de captura ----------

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Mic(MicRole),
    LoopMm,
    LoopComm,
    Proc(u32),
}

/// Papel do microfone: `Console` = "dispositivo padrão"; `Comm` = "dispositivo de comunicação padrão".
#[derive(Clone, Copy, PartialEq)]
enum MicRole {
    Console,
    Comm,
}

struct Shared {
    stop: AtomicBool,
    /// Incrementado pelo callback quando muda o padrão de saída (multimídia/console ou comunicação).
    render_default_gen: AtomicU64,
    capture_default_gen: AtomicU64,
}

fn open(kind: Kind, enumr: &DeviceEnumerator) -> Result<(AudioClient, String, Option<String>), wasapi::WasapiError> {
    let fmt = WaveFormat::new(32, 32, &SampleType::Float, RATE as usize, 1, None);
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 200_000 };
    let (mut client, desc, id) = match kind {
        Kind::Mic(r) => {
            let role = if r == MicRole::Comm { Role::Communications } else { Role::Console };
            let d = enumr.get_default_device_for_role(&Direction::Capture, &role)?;
            (d.get_iaudioclient()?, describe(&d), d.get_id().ok())
        }
        Kind::LoopMm | Kind::LoopComm => {
            let role = if kind == Kind::LoopMm { Role::Multimedia } else { Role::Communications };
            let t = Instant::now();
            let d = enumr.get_default_device_for_role(&Direction::Render, &role)?;
            let t_dev = t.elapsed();
            let c = d.get_iaudioclient()?;
            let t_cli = t.elapsed();
            let desc = describe(&d);
            let t_desc = t.elapsed();
            if std::env::var_os("S3_TIMING").is_some() {
                eprintln!("    [timing] device {:?} · iaudioclient {:?} · describe {:?}", t_dev, t_cli - t_dev, t_desc - t_cli);
            }
            (c, desc, d.get_id().ok())
        }
        Kind::Proc(pid) => {
            let mode0 = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: 0 };
            let mut c = AudioClient::new_application_loopback_client(pid, true)?;
            c.initialize_client(&fmt, &Direction::Capture, &mode0)?;
            return Ok((c, format!("processo {pid}"), None));
        }
    };
    let t = Instant::now();
    if std::env::var("S3_FMT").as_deref() == Ok("mix") {
        let mix = client.get_mixformat()?;
        eprintln!("    [timing] mixformat {} Hz · {} canais · {} bits", mix.get_samplespersec(), mix.get_nchannels(), mix.get_bitspersample());
        let m = StreamMode::EventsShared { autoconvert: false, buffer_duration_hns: 200_000 };
        client.initialize_client(&mix, &Direction::Capture, &m)?;
    } else {
        client.initialize_client(&fmt, &Direction::Capture, &mode)?;
    }
    if std::env::var_os("S3_TIMING").is_some() {
        eprintln!("    [timing] initialize_client {:?}", t.elapsed());
    }
    Ok((client, desc, id))
}

fn capture_thread(name: &'static str, kind: Kind, out: PathBuf, t0: u64, tol: u64, ev: Events, sh: Arc<Shared>, meter: Arc<Mutex<Meter>>, mm_id: Arc<Mutex<Option<String>>>) {
    let _ = wasapi::initialize_mta();
    unsafe {
        let mut idx = 0u32;
        let _ = windows::Win32::System::Threading::AvSetMmThreadCharacteristicsW(windows::core::w!("Pro Audio"), &mut idx);
    }
    let enumr = DeviceEnumerator::new().expect("enumerator");
    let mut track = Track::create(&out, tol).expect("criar WAV");
    let mut buf = vec![0u8; 4 * RATE as usize]; // 1 s de float mono
    let mut reopen_count = 0u32;
    let gen_of = |k: Kind| if matches!(k, Kind::Mic(_)) { &sh.capture_default_gen } else { &sh.render_default_gen };

    'outer: while !sh.stop.load(Ordering::Relaxed) {
        let seen_gen = gen_of(kind).load(Ordering::SeqCst);
        let (client, desc, id) = match open(kind, &enumr) {
            Ok(x) => x,
            Err(e) => {
                ev.log(name, "falha_abrir", &e.to_string());
                thread::sleep(Duration::from_millis(500));
                continue;
            }
        };
        // loop_comm fica ocioso enquanto for o mesmo dispositivo do loop_mm.
        if kind == Kind::LoopComm && id.is_some() && *mm_id.lock().unwrap() == id {
            ev.log(name, "ocioso", &format!("mesmo dispositivo do loop_mm: {desc}"));
            meter.lock().unwrap().device = "(= loop_mm)".into();
            while !sh.stop.load(Ordering::Relaxed) && gen_of(kind).load(Ordering::SeqCst) == seen_gen {
                thread::sleep(Duration::from_millis(100));
            }
            continue;
        }
        if kind == Kind::LoopMm {
            *mm_id.lock().unwrap() = id.clone();
        }
        meter.lock().unwrap().device = desc.clone();
        ev.log(name, if reopen_count == 0 { "aberto" } else { "reaberto" }, &desc);
        reopen_count += 1;

        let h = match client.set_get_eventhandle() { Ok(h) => h, Err(e) => { ev.log(name, "erro", &e.to_string()); continue; } };
        let cap = match client.get_audiocaptureclient() { Ok(c) => c, Err(e) => { ev.log(name, "erro", &e.to_string()); continue; } };
        if let Err(e) = client.start_stream() {
            ev.log(name, "erro_start", &e.to_string());
            thread::sleep(Duration::from_millis(500));
            continue;
        }
        let mut last_packet = Instant::now();
        let mut silent_reported = false;

        loop {
            if sh.stop.load(Ordering::Relaxed) {
                let _ = client.stop_stream();
                break 'outer;
            }
            if !matches!(kind, Kind::Proc(_)) && gen_of(kind).load(Ordering::SeqCst) != seen_gen {
                ev.log(name, "troca_padrao", "dispositivo padrão mudou, reabrindo");
                let _ = client.stop_stream();
                continue 'outer;
            }
            match h.wait_for_event(200) {
                Ok(()) => {}
                Err(wasapi::WasapiError::EventTimeout) => {
                    if last_packet.elapsed() > Duration::from_secs(2) && !silent_reported {
                        ev.log(name, "sem_pacotes", "nenhum pacote há 2 s (normal no loopback em silêncio)");
                        silent_reported = true;
                    }
                    continue;
                }
                Err(e) => {
                    ev.log(name, "erro_wait", &e.to_string());
                    let _ = client.stop_stream();
                    continue 'outer;
                }
            }
            loop {
                let (frames, info) = match cap.read_from_device(&mut buf) {
                    Ok(x) => x,
                    Err(e) => {
                        ev.log(name, "invalidado", &e.to_string());
                        let _ = client.stop_stream();
                        thread::sleep(Duration::from_millis(300));
                        continue 'outer;
                    }
                };
                if frames == 0 {
                    break;
                }
                if silent_reported {
                    ev.log(name, "pacotes_voltaram", &format!("após {:.1} s", last_packet.elapsed().as_secs_f64()));
                    silent_reported = false;
                }
                last_packet = Instant::now();
                let samples: Vec<f32> = buf[..frames as usize * 4]
                    .chunks_exact(4)
                    .map(|b| if info.flags.silent { 0.0 } else { f32::from_le_bytes([b[0], b[1], b[2], b[3]]) })
                    .collect();
                if info.flags.data_discontinuity {
                    ev.log(name, "descontinuidade", "AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY");
                }
                let pos = if info.flags.timestamp_error || info.timestamp == 0 || info.timestamp < t0 {
                    None
                } else {
                    Some((info.timestamp - t0) * RATE / 10_000_000)
                };
                if let Some(ms) = track.put(pos, &samples) {
                    if ms >= 100.0 {
                        ev.log(name, "lacuna", &format!("{ms:.0} ms preenchidos com silêncio"));
                    }
                }
                let mut m = meter.lock().unwrap();
                m.packets += 1;
                m.n += samples.len() as u64;
                m.sum_sq += samples.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>();
            }
        }
    }

    // Completa até o fim da sessão, para todas as faixas terminarem juntas.
    let end = (qpc_100ns() - t0) * RATE / 10_000_000;
    if end > track.written {
        let n = end - track.written;
        track.zeros(n);
        track.filled_gap += n;
    }
    let _ = track.header();
    let dur = track.written as f64 / RATE as f64;
    ev.log(
        name,
        "resumo",
        &format!(
            "faixa {dur:.2} s · de pacotes {:.2} s · silêncio preenchido {:.2} s · lacunas>10ms {} (máx {:.0} ms) · sobreposições {} · aberturas {reopen_count}",
            track.from_packets as f64 / RATE as f64,
            track.filled_gap as f64 / RATE as f64,
            track.gaps,
            track.max_gap_ms,
            track.overlaps
        ),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let secs: u64 = arg("--secs").and_then(|s| s.parse().ok()).unwrap_or(120);
    let no_mic = args.iter().any(|a| a == "--no-mic");
    // S3 (f1 × f2): no mic Bluetooth em chamada o QPC tem jitter; com 10 ms a faixa picota, com 200 ms fica limpa.
    let mic_tol: u64 = arg("--mic-tol-ms").and_then(|s| s.parse::<u64>().ok()).unwrap_or(200) * RATE / 1000;
    let proc_name = arg("--proc");
    let stamp = chrono_like_stamp();
    let out = PathBuf::from(arg("--out").unwrap_or_else(|| format!("s3_{stamp}")));
    std::fs::create_dir_all(&out).expect("criar pasta de saída");

    let _ = wasapi::initialize_mta();
    let enumr = DeviceEnumerator::new().expect("enumerator");

    println!("== dispositivos ==");
    for (dir, label) in [(Direction::Render, "saída"), (Direction::Capture, "entrada")] {
        if let Ok(coll) = enumr.get_device_collection(&dir) {
            for i in 0..coll.get_nbr_devices().unwrap_or(0) {
                if let Ok(d) = coll.get_device_at_index(i) {
                    println!("  {label:<7} {}", describe(&d));
                }
            }
        }
        for role in [Role::Console, Role::Multimedia, Role::Communications] {
            if let Ok(d) = enumr.get_default_device_for_role(&dir, &role) {
                println!("  padrão {label} ({role}): {}", describe(&d));
            }
        }
    }

    let t0 = qpc_100ns();
    let ev = Events { t0, file: Arc::new(Mutex::new(BufWriter::new(File::create(out.join("eventos.csv")).unwrap()))) };
    ev.log("sessão", "início", &format!("{secs} s · saída em {}", out.display()));

    let sh = Arc::new(Shared { stop: AtomicBool::new(false), render_default_gen: AtomicU64::new(0), capture_default_gen: AtomicU64::new(0) });

    let mut cb = DeviceEventCallbacks::new();
    {
        let (sh, ev) = (sh.clone(), ev.clone());
        cb.set_default_device_callback(move |dir, role, id| {
            ev.log("windows", "padrao_mudou", &format!("{dir} {role} -> {}", id.as_deref().unwrap_or("nenhum")));
            if dir == Direction::Capture { &sh.capture_default_gen } else { &sh.render_default_gen }.fetch_add(1, Ordering::SeqCst);
        });
    }
    {
        let ev = ev.clone();
        cb.set_device_state_callback(move |id, state| ev.log("windows", "estado", &format!("{id} -> {state}")));
    }
    let _reg = enumr.register_notification_callback(cb).expect("registrar notificações");

    let mut sources: Vec<(&'static str, Kind)> = vec![("loop_mm", Kind::LoopMm), ("loop_comm", Kind::LoopComm)];
    if !no_mic {
        let role = if arg("--mic-role").as_deref() == Some("comm") { MicRole::Comm } else { MicRole::Console };
        sources.insert(0, ("mic", Kind::Mic(role)));
    }
    if let Some(p) = &proc_name {
        match find_process_root(p) {
            Some(pid) => sources.push(("proc", Kind::Proc(pid))),
            None => ev.log("proc", "nao_encontrado", p),
        }
    }

    let mm_id = Arc::new(Mutex::new(None));
    let mut meters = Vec::new();
    let mut handles = Vec::new();
    for (name, kind) in sources {
        let m = Arc::new(Mutex::new(Meter::default()));
        meters.push((name, m.clone()));
        let (ev, sh, mm_id, path) = (ev.clone(), sh.clone(), mm_id.clone(), out.join(format!("{name}.wav")));
        let tol = if matches!(kind, Kind::Mic(_)) { mic_tol } else { GAP_TOL_FRAMES };
        handles.push(thread::Builder::new().name(name.into()).spawn(move || capture_thread(name, kind, path, t0, tol, ev, sh, m, mm_id)).unwrap());
        thread::sleep(Duration::from_millis(150)); // loop_mm abre antes do loop_comm
    }

    let mut levels = BufWriter::new(File::create(out.join("niveis.csv")).unwrap());
    let _ = writeln!(levels, "t_s;{}", meters.iter().map(|(n, _)| format!("{n}_dbfs;{n}_pacotes")).collect::<Vec<_>>().join(";"));
    println!("\n(gravando {secs} s; Ctrl+C encerra também, os WAVs ficam válidos até o último segundo)\n");
    let start = Instant::now();
    let mut next = 1u64;
    let mut last_sessions = String::new();
    while start.elapsed() < Duration::from_secs(secs) {
        thread::sleep(Duration::from_millis(50));
        if start.elapsed() < Duration::from_secs(next) {
            continue;
        }
        if next % 3 == 1 {
            let s = capture_sessions(&enumr);
            if s != last_sessions {
                ev.log("windows", "mics_em_uso", &s);
                last_sessions = s;
            }
        }
        let mut line = format!("{next:>4}s");
        let mut csv = format!("{next}");
        for (name, m) in &meters {
            let mut g = m.lock().unwrap();
            let d = dbfs(&g);
            let bar = if d.is_finite() { ((d + 60.0) / 6.0).clamp(0.0, 10.0) as usize } else { 0 };
            line.push_str(&format!(" | {name} {:>6} {:<10}", if d.is_finite() { format!("{d:.0}dB") } else { "--".into() }, "▮".repeat(bar)));
            csv.push_str(&format!(";{};{}", if d.is_finite() { format!("{d:.1}") } else { String::new() }, g.packets));
            g.sum_sq = 0.0;
            g.n = 0;
            g.packets = 0;
        }
        println!("{line}");
        let _ = writeln!(levels, "{csv}");
        let _ = levels.flush();
        next += 1;
    }
    sh.stop.store(true, Ordering::SeqCst);
    for h in handles {
        let _ = h.join();
    }
    ev.log("sessão", "fim", &format!("{:.2} s de relógio", (qpc_100ns() - t0) as f64 / 1e7));
}

fn chrono_like_stamp() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{s}")
}
