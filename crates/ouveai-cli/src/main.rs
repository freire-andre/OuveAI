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
//! ouveai-cli compare <arquivo> [--json saida.json]
//! ouveai-cli edl-subs <edl.json> [--out final.srt] [--check <vídeo final | final.check.json>]
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
        "uso:\n  ouveai-cli paths\n  ouveai-cli models\n  ouveai-cli models download <id> [--mirror] [--stop-after-secs N]\n  ouveai-cli models import <id> <arquivo|pasta>\n  ouveai-cli models verify <id>\n  ouveai-cli config [set <chave> <valor>]\n  ouveai-cli ffmpeg [download [--stop-after-secs N]]\n  ouveai-cli transcribe <arquivo> [--check]\n  ouveai-cli transcribe --from-json <arquivo.json>\n  ouveai-cli compare <arquivo> [--json saida.json]\n  ouveai-cli edl-subs <edl.json> [--out final.srt] [--check <vídeo final|final.check.json>]\n  ouveai-cli live --eu a.wav --outros b.wav [--echo] [--headphones] [--realtime]\n  ouveai-cli capture [--secs N] [--no-mic] [--app exe] [--headphones|--speakers] [--save pasta]"
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
        Some("compare") => compare_cmd(&args[1..]),
        Some("edl-subs") => edl_cmd(&args[1..]),
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

/// Parte 5: legenda do vídeo final a partir do `edl.json` e dos `.json` dos brutos.
fn edl_cmd(args: &[String]) {
    use ouveai_core::edl;
    use ouveai_core::transcribe as tr;
    use std::path::{Path, PathBuf};

    let path = Path::new(args.first().filter(|a| !a.starts_with("--")).unwrap_or_else(|| usage()));
    let opt = |name: &str| args.iter().position(|a| a == name).map(|i| PathBuf::from(args.get(i + 1).unwrap_or_else(|| usage())));
    let check = opt("--check");

    let cuts = edl::read(path).unwrap_or_else(|e| fail(e));
    let gaps = edl::validate(&cuts).unwrap_or_else(|errs| fail(format!("edl.json com erro:\n  {}", errs.join("\n  "))));
    let sources = edl::load_sources(path, &cuts).unwrap_or_else(|errs| fail(errs.join("\n")));
    println!("{} cortes", cuts.len());
    for (i, (c, t)) in cuts.iter().zip(&sources).enumerate() {
        println!("  {}. {}  {:.3}–{:.3} → {}  ({} palavras)", i + 1, c.file, c.in_, c.out, edl::clock(c.position), edl::map_cut(c, t).0.len());
    }
    if !gaps.is_empty() {
        println!("lacunas entre cortes (confira):");
        for g in &gaps {
            println!("  {} – {}", edl::clock(g.start), edl::clock(g.end));
        }
    }
    for w in edl::warnings(&cuts, &sources) {
        println!("⚠ {w}");
    }

    let built = edl::build(&cuts, &sources);
    // Com o vídeo final, a legenda leva o nome dele (o player carrega sozinho).
    let video = check.as_deref().filter(|c| !c.to_string_lossy().to_lowercase().ends_with(".json"));
    let out = opt("--out").unwrap_or_else(|| edl::srt_path(path, video));
    std::fs::write(&out, ouveai_core::subs::to_srt(&built.cues)).unwrap_or_else(|e| fail(e));
    println!("legenda: {} ({} blocos, {} palavras)", out.display(), built.cues.len(), built.words.len());

    let Some(check) = check else { return };
    let words = if check.to_string_lossy().to_lowercase().ends_with(".json") {
        tr::read_json(&check).unwrap_or_else(|e| fail(e)).words
    } else {
        verify_transcribe(&check)
    };
    let r = edl::check(&cuts, &built, &words);
    println!("\nverificação ({} pares casados)", r.matched);
    println!("  desvio do início: mediano {:.3} s · máximo {:.3} s (referência: {:.2} s mediano)", r.median, r.max, edl::DESVIO_REFERENCIA);
    println!("  sem par: {} da legenda · {} da verificação", r.unmatched_subs, r.unmatched_check);
    if r.issues.is_empty() {
        println!("  nenhum trecho com desvio > {:.1} s", edl::DESVIO_LISTAR);
    } else {
        println!("trechos com desvio > {:.1} s:", edl::DESVIO_LISTAR);
        for is in &r.issues {
            println!("  {}–{} (corte {}, {} {}, até {:+.2} s): {}", edl::clock(is.start), edl::clock(is.end), is.cut + 1, is.words, if is.words == 1 { "palavra" } else { "palavras" }, is.worst, is.why);
        }
    }
    if !r.fixes.is_empty() {
        println!("correção proposta no edl.json (aplique no app, em Legendas, ou edite o arquivo):");
        for f in &r.fixes {
            println!("  corte {}: {} {:.3} → {:.3}", f.cut, f.field, f.old, f.new);
        }
    }
}

/// Transcreve o vídeo final só para a verificação (`.check.json`, não toca em nenhum `.srt`).
fn verify_transcribe(video: &std::path::Path) -> Vec<ouveai_core::words::Word> {
    use ouveai_core::transcribe as tr;
    use ouveai_core::{asr, media};
    if !video.is_file() {
        fail(format!("arquivo não encontrado: {}", video.display()));
    }
    let ff = media::find().unwrap_or_else(|| fail("FFmpeg não encontrado"));
    let a = asr::Asr::load(asr::file_threads()).unwrap_or_else(|e| fail(e));
    let cancel = AtomicBool::new(false);
    print!("transcrevendo o vídeo final para a verificação…");
    std::io::stdout().flush().ok();
    let (t, rep) = tr::transcribe(video, &ff.path, &a, &cancel, &mut |_| {}).unwrap_or_else(|e| fail(format!("\n{e}")));
    let p = tr::check_path(video);
    std::fs::write(&p, tr::to_json(&t)).unwrap_or_else(|e| fail(e));
    println!(" {:.1} s de áudio em {:.1} s → {}", rep.duration, rep.transcribe_s, p.display());
    t.words
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

/// Parte 4: transcreve com o Parakeet e com o Whisper e lista os trechos divergentes.
fn compare_cmd(args: &[String]) {
    use ouveai_core::{asr, compare, media, transcribe as tr};
    use std::path::Path;

    let input = Path::new(args.first().filter(|a| !a.starts_with("--")).unwrap_or_else(|| usage()));
    let json_out = args.iter().position(|a| a == "--json").map(|i| args.get(i + 1).unwrap_or_else(|| usage()).clone());
    // Só para medir: janelas do Whisper de até N s.
    let chunk = match args.iter().position(|a| a == "--chunk") {
        Some(i) => match args.get(i + 1).and_then(|v| v.parse::<f64>().ok()) {
            Some(n) if (8.0..=asr::WHISPER_MAX_SECONDS).contains(&n) => n,
            _ => fail(format!("--chunk precisa de um número entre 8 e {}", asr::WHISPER_MAX_SECONDS)),
        },
        None => compare::WHISPER_CHUNK,
    };
    if !input.is_file() {
        fail(format!("arquivo não encontrado: {}", input.display()));
    }
    let ff = media::find().unwrap_or_else(|| fail("FFmpeg não encontrado (instale ou rode: ouveai-cli ffmpeg download)"));
    let threads = asr::file_threads();
    let cancel = AtomicBool::new(false);
    let progress = |what: &str, i: usize, n: usize| {
        print!("\r{:<60}", format!("{what}… {i} de {n}"));
        std::io::stdout().flush().ok();
    };

    let t = std::time::Instant::now();
    let parakeet = asr::Asr::load(threads).unwrap_or_else(|e| fail(e));
    let p_load = t.elapsed().as_secs_f64();
    let (tr_, rep) = tr::transcribe(input, &ff.path, &parakeet, &cancel, &mut |s| {
        if let tr::Stage::Recognizing { chunk, chunks, .. } = s {
            progress("Parakeet", chunk, chunks)
        }
    })
    .unwrap_or_else(|e| fail(format!("\n{e}")));
    drop(parakeet);

    let t = std::time::Instant::now();
    let whisper = asr::Whisper::load(threads).unwrap_or_else(|e| fail(format!("\r{e}")));
    let w_load = t.elapsed().as_secs_f64();
    let samples = compare::file_samples(&ff.path, input, &cancel).unwrap_or_else(|e| fail(format!("\n{e}")));
    let ranges = compare::file_ranges(&samples, &tr_.pauses, chunk);
    let pass = compare::whisper_pass(&whisper, &samples, &ranges, &tr_.words, &cancel, &mut |i, n| progress("Whisper", i, n))
        .unwrap_or_else(|e| fail(format!("\n{e}")));
    println!("\r{:<60}", "");

    let outcome = compare::diverge_pass(&tr_.words, &pass);
    let divs = &outcome.divergences;
    let changed: usize = divs.iter().map(|d| d.to - d.from).sum();
    let fmt_t = |t: f64| format!("{:02}:{:02}:{:04.1}", (t / 3600.0) as u64, (t / 60.0) as u64 % 60, t % 60.0);
    println!("duração do áudio: {:.1} s · {threads} threads", rep.duration);
    println!("Parakeet: carga {p_load:.1} s · {:.1} s ({:.1}× tempo real) · {} palavras", rep.transcribe_s, rep.rtf, rep.words);
    println!(
        "Whisper:  carga {w_load:.1} s · {:.1} s ({:.1}× tempo real) · {} palavras · {} trechos ({} sem fala, pulados)",
        pass.secs,
        rep.duration / pass.secs.max(1e-6),
        pass.word_count(),
        pass.windows.len(),
        pass.skipped()
    );
    if !outcome.unreliable.is_empty() {
        let at: Vec<String> = outcome.unreliable.iter().map(|(a, b)| format!("{}–{}", fmt_t(*a), fmt_t(*b))).collect();
        println!("⚠ o Whisper falhou em {} trecho(s), descartados: {}", at.len(), at.join(", "));
    }
    println!(
        "\n{} trechos divergentes ({} de {} palavras do Parakeet, {:.1}%):\n",
        divs.len(),
        changed,
        tr_.words.len(),
        100.0 * changed as f64 / tr_.words.len().max(1) as f64
    );
    for d in divs {
        println!("{}  P: {}", fmt_t(d.start), d.parakeet);
        println!("{:10}  W: {}\n", "", if d.whisper.is_empty() { "(nada)" } else { &d.whisper });
    }
    if let Some(path) = json_out {
        std::fs::write(&path, serde_json::to_string_pretty(&outcome).unwrap()).unwrap_or_else(|e| fail(e));
        println!("divergências: {path}");
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
