//! CLI do OuveAI.
//!
//! ouveai-cli paths
//! ouveai-cli models
//! ouveai-cli models download <id> [--mirror] [--stop-after-secs N]
//! ouveai-cli models import <id> <arquivo.tar.bz2 | pasta>
//! ouveai-cli models verify <id>
//! ouveai-cli config [set <chave> <valor>]
//! ouveai-cli ffmpeg [download [--stop-after-secs N]]
//! ouveai-cli transcribe <arquivo> [--check] [--chunk N]
//! ouveai-cli transcribe --from-json <arquivo.json>
//! ouveai-cli live --eu a.wav --outros b.wav [--echo] [--headphones] [--realtime]
//! ouveai-cli capture [--secs N] [--no-mic] [--app exe] [--headphones|--speakers] [--save pasta]

mod live;

use ouveai_core::models::{self, Presence, Source, Step};
use ouveai_core::net::NetConfig;
use ouveai_core::paths;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn usage() -> ! {
    eprintln!(
        "uso:\n  ouveai-cli paths\n  ouveai-cli models\n  ouveai-cli models download <id> [--mirror] [--stop-after-secs N]\n  ouveai-cli models import <id> <arquivo|pasta>\n  ouveai-cli models verify <id>\n  ouveai-cli config [set <chave> <valor>]\n  ouveai-cli ffmpeg [download [--stop-after-secs N]]\n  ouveai-cli transcribe <arquivo> [--check]\n  ouveai-cli transcribe --from-json <arquivo.json>\n  ouveai-cli live --eu a.wav --outros b.wav [--echo] [--headphones] [--realtime]\n  ouveai-cli capture [--secs N] [--no-mic] [--app exe] [--headphones|--speakers] [--save pasta]"
    );
    std::process::exit(2);
}

fn mb(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

fn print_step(s: Step) {
    let line = match s {
        Step::Downloading { retry: Some(r), .. } => {
            let why = if r.status == 0 { "sem conexão".to_string() } else { format!("servidor respondeu {}", r.status) };
            format!("{why}; tentativa {}/{} em {} s", r.attempt, r.max, r.wait_secs)
        }
        Step::Downloading { done, total, bytes_per_sec, .. } => {
            format!("baixando   {:7.1} / {:.1} MB  {:5.1} MB/s", mb(done), mb(total), mb(bytes_per_sec as u64))
        }
        Step::Verifying { done, total } => format!("conferindo {:7.1} / {:.1} MB", mb(done), mb(total)),
        Step::Extracting { done, total } => format!("extraindo  {:7.1} / {:.1} MB", mb(done), mb(total)),
    };
    print!("\r{line:<60}");
    std::io::stdout().flush().ok();
}

fn spec(id: Option<&String>) -> models::ModelSpec {
    let id = id.unwrap_or_else(|| usage());
    models::by_id(id).unwrap_or_else(|| {
        eprintln!("modelo desconhecido: {id}");
        std::process::exit(2)
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("paths") => {
            println!("dados:     {}", paths::data_dir().display());
            println!("banco:     {}", paths::db_path().display());
            println!("modelos:   {}", paths::models_dir().display());
            println!("ffmpeg:    {}", paths::bin_dir().display());
            println!("gravações: {}", paths::recordings_dir().display());
            println!("logs:      {}", paths::logs_dir().display());
        }
        Some("models") => models_cmd(&args[1..]),
        Some("config") => config_cmd(&args[1..]),
        Some("ffmpeg") => ffmpeg_cmd(&args[1..]),
        Some("transcribe") => transcribe_cmd(&args[1..]),
        Some("live") => live::replay_cmd(&args[1..]),
        Some("capture") => live::capture_cmd(&args[1..]),
        _ => usage(),
    }
}

/// Cancela depois de N segundos (simula queda, para testar a retomada).
fn stop_after(args: &[String], cancel: &Arc<AtomicBool>) {
    if let Some(secs) = args.iter().position(|a| a == "--stop-after-secs").and_then(|i| args.get(i + 1)?.parse().ok()) {
        let c = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(secs));
            c.store(true, Ordering::Relaxed);
        });
    }
}

fn ffmpeg_cmd(args: &[String]) {
    use ouveai_core::media;
    match args.first().map(String::as_str) {
        None => match media::find() {
            Some(f) => println!("{} ({:?})\n{}", f.version, f.origin, f.path.display()),
            None => {
                eprintln!("FFmpeg não encontrado (PATH nem {})", media::local_path().display());
                std::process::exit(1);
            }
        },
        Some("download") => {
            let cancel = Arc::new(AtomicBool::new(false));
            stop_after(args, &cancel);
            let p = &media::PACKAGE;
            println!("FFmpeg {} ({}, {:.0} MB)", p.version, p.license, mb(p.size));
            let r = media::download(&NetConfig::default(), &cancel, &mut print_step);
            println!();
            match r {
                Ok(f) => println!("{}\n{}", f.version, f.path.display()),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(if matches!(e, models::InstallError::Cancelled) { 3 } else { 1 });
                }
            }
        }
        _ => usage(),
    }
}

fn fail(msg: impl std::fmt::Display) -> ! {
    eprintln!("{msg}");
    std::process::exit(1)
}

fn transcribe_cmd(args: &[String]) {
    use ouveai_core::transcribe::{self as tr, Stage};
    use ouveai_core::{asr, media, suspects};
    use std::path::Path;

    if args.first().map(String::as_str) == Some("--from-json") {
        let json = Path::new(args.get(1).unwrap_or_else(|| usage()));
        let t = tr::read_json(json).unwrap_or_else(|e| fail(e));
        let out = tr::outputs_for(json);
        tr::write_txt_srt(&t, &out).unwrap_or_else(|e| fail(e));
        println!("regenerados:\n  {}\n  {}", out.txt.display(), out.srt.display());
        return;
    }
    let input = Path::new(args.first().filter(|a| !a.starts_with("--")).unwrap_or_else(|| usage()));
    let check = args.iter().any(|a| a == "--check");
    // Só para testes de qualidade: pedaços de até N s (corte entre 3/4 e N).
    let plan = match args.iter().position(|a| a == "--chunk") {
        Some(i) => match args.get(i + 1).and_then(|v| v.parse::<f64>().ok()) {
            Some(n) if (10.0..=asr::MAX_SECONDS).contains(&n) => media::ChunkPlan::with_max(n),
            _ => fail(format!("--chunk precisa de um número entre 10 e {}", asr::MAX_SECONDS)),
        },
        None => media::ChunkPlan::default(),
    };
    if !input.is_file() {
        fail(format!("arquivo não encontrado: {}", input.display()));
    }
    let ff = media::find().unwrap_or_else(|| fail("FFmpeg não encontrado (instale ou rode: ouveai-cli ffmpeg download)"));

    let t = std::time::Instant::now();
    let asr = asr::Asr::load(asr::file_threads()).unwrap_or_else(|e| fail(e));
    let load_s = t.elapsed().as_secs_f64();
    println!("modelo carregado em {load_s:.2} s ({} threads)", asr::file_threads());

    let cancel = AtomicBool::new(false);
    let (tr_, rep) = tr::transcribe_with(input, &ff.path, &asr, &cancel, plan, &mut |s| {
        let line = match s {
            Stage::Converting => "convertendo o áudio (FFmpeg)…".to_string(),
            Stage::Analyzing => "medindo os silêncios (silencedetect)…".to_string(),
            Stage::Recognizing { chunk, chunks, .. } => format!("reconhecendo… pedaço {chunk} de {chunks}"),
            Stage::Saving => "salvando…".to_string(),
        };
        print!("\r{line:<60}");
        std::io::stdout().flush().ok();
    })
    .unwrap_or_else(|e| fail(format!("\n{e}")));
    println!("\r{:<60}", "");

    if check {
        let p = tr::check_path(input);
        std::fs::write(&p, tr::to_json(&tr_)).unwrap_or_else(|e| fail(e));
        println!("verificação: {}", p.display());
    } else {
        let out = tr::outputs_for(input);
        tr::write_all(&tr_, &out).unwrap_or_else(|e| fail(e));
        println!("gerados:\n  {}\n  {}\n  {}", out.json.display(), out.txt.display(), out.srt.display());
    }

    let fmt_t = |t: f64| format!("{:02}:{:02}:{:05.2}", (t / 3600.0) as u64, (t / 60.0) as u64 % 60, t % 60.0);
    println!("\nduração do áudio:     {:.1} s", rep.duration);
    println!("carga do modelo:      {load_s:.2} s");
    println!("conversão + silêncio: {:.2} s", rep.convert_s);
    println!("transcrição:          {:.2} s", rep.transcribe_s);
    println!("fator de tempo real:  {:.1}× (duração ÷ tempo de transcrição)", rep.rtf);
    println!("pedaços:              {}", rep.chunks);
    match rep.forced_cuts.len() {
        0 => {}
        1..=3 => {
            let at: Vec<String> = rep.forced_cuts.iter().map(|c| fmt_t(*c)).collect();
            println!("⚠ sem silêncio para cortar em {}: corte no ponto de menor volume (confira as palavras perto)", at.join(", "));
        }
        n => println!("⚠ {n} de {} cortes sem silêncio medido (áudio com ruído de fundo): cortes no ponto de menor volume", rep.chunks - 1),
    }
    println!("palavras: {} · pausas medidas: {}", rep.words, tr_.pauses.len());

    println!("\ncandidatos a erro: {} ({:.1} por minuto)", rep.suspects.len(), rep.suspects_per_min);
    for s in &rep.suspects {
        let why = match s.kind {
            suspects::Kind::PtPt => format!("grafia de Portugal → {}", s.hint.as_deref().unwrap_or("")),
            suspects::Kind::Name => "nome próprio? (maiúscula no meio da frase)".to_string(),
            suspects::Kind::Language => "inglês/espanhol no meio do português".to_string(),
        };
        println!("  {}  {:<18} {why}", fmt_t(s.start), s.word);
    }
    println!("  (confiança por token: o sherpa-onnx 2.0.1 não expõe; heurística 3 sem fonte)");
    if rep.suspects_per_min > suspects::MAX_PER_MIN {
        println!("→ mais de {} por minuto: vale comparar com o Whisper (Parte 4).", suspects::MAX_PER_MIN);
    }
}

fn config_cmd(args: &[String]) {
    let db = paths::db_path();
    if let Some(dir) = db.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let store = ouveai_core::store::Store::open(&db).unwrap_or_else(|e| {
        eprintln!("erro ao abrir o banco: {e}");
        std::process::exit(1)
    });
    let r = match args {
        [] => store.all().map(|all| all.iter().for_each(|(k, v)| println!("{k} = {v}"))),
        [cmd, k, v] if cmd == "set" => store.set(k, v),
        _ => usage(),
    };
    if let Err(e) = r {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}

fn models_cmd(args: &[String]) {
    let cancel = Arc::new(AtomicBool::new(false));
    let r = match args.first().map(String::as_str) {
        None => {
            for m in models::catalog() {
                let p = match models::presence(&m) {
                    Presence::Ready => "pronto".to_string(),
                    Presence::Absent => "ausente".to_string(),
                    Presence::Corrupt(d) => format!("corrompido ({d})"),
                };
                println!("{:<28} {:>8.1} MB  {p}", m.id, mb(m.files_size()));
            }
            return;
        }
        Some("download") => {
            let m = spec(args.get(1));
            let source = if args.iter().any(|a| a == "--mirror") { Source::Mirror } else { Source::Primary };
            stop_after(args, &cancel);
            models::download(&m, &NetConfig::default(), source, &cancel, &mut print_step)
        }
        Some("import") => {
            let m = spec(args.get(1));
            let path = args.get(2).unwrap_or_else(|| usage());
            models::import(&m, std::path::Path::new(path), &cancel, &mut print_step)
        }
        Some("verify") => {
            let m = spec(args.get(1));
            models::verify_dir(&m, &m.dir(), &mut print_step)
        }
        _ => usage(),
    };
    println!();
    match r {
        Ok(()) => println!("ok"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(match e {
                models::InstallError::Cancelled => 3,
                _ => 1,
            });
        }
    }
}
