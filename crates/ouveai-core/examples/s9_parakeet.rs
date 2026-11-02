//! Spike S9: variantes do Parakeet TDT 0.6b v3 no formato sherpa-onnx (base int8 × fine-tune
//! pt-BR) com os pedaços do app (corte no silêncio medido, até `--chunk` s; padrão 60).
//! Grava o texto e os pedaços usados num JSON, para o script de WER e para rodar os mesmos
//! pedaços em outros runtimes.
//!
//! Uso:
//!   cargo run --release -p ouveai-core --example s9_parakeet -- <dir_modelo> <wav 16 kHz mono>
//!       [--chunk 60] --out saida.json

use ouveai_core::{asr, media, words};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, Wave};
use std::time::Instant;

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (dir, wav_path) = (&args[1], &args[2]);
    let chunk: f64 = arg(&args, "--chunk").map_or(media::CHUNK_MAX, |c| c.parse().expect("--chunk N"));
    let out = arg(&args, "--out").expect("--out saida.json");
    let threads = asr::file_threads();

    let wave = Wave::read(wav_path).expect("falha ao ler o WAV");
    assert_eq!(wave.sample_rate(), asr::SAMPLE_RATE, "o WAV precisa ser 16 kHz mono");
    let samples = wave.samples().to_vec();
    let sr = asr::SAMPLE_RATE as f64;
    let duration = samples.len() as f64 / sr;

    let ff = media::find().expect("FFmpeg não encontrado");
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let pauses = media::silences(&ff.path, std::path::Path::new(wav_path), duration, &cancel).expect("silencedetect");
    let plan = if chunk == media::CHUNK_MAX { media::ChunkPlan::default() } else { media::ChunkPlan::with_max(chunk) };
    let chunks = media::plan_chunks_with(&samples, asr::SAMPLE_RATE as u32, &pauses, plan);

    let f = |name: &str| Some(format!("{dir}/{name}"));
    let mut cfg = OfflineRecognizerConfig::default();
    cfg.model_config.transducer = OfflineTransducerModelConfig { encoder: f("encoder.int8.onnx"), decoder: f("decoder.int8.onnx"), joiner: f("joiner.int8.onnx") };
    cfg.model_config.tokens = f("tokens.txt");
    cfg.model_config.model_type = Some("nemo_transducer".into());
    cfg.model_config.num_threads = threads;
    cfg.model_config.provider = Some("cpu".into());
    cfg.decoding_method = Some("greedy_search".into());
    let t = Instant::now();
    let rec = OfflineRecognizer::create(&cfg).expect("falha ao carregar o modelo");
    let load_s = t.elapsed().as_secs_f64();

    let t = Instant::now();
    let mut texts = Vec::new();
    let mut starts: Vec<(f64, String)> = Vec::new();
    for c in &chunks {
        let s = &samples[(c.start * sr) as usize..((c.end * sr) as usize).min(samples.len())];
        let stream = rec.create_stream();
        stream.accept_waveform(asr::SAMPLE_RATE, s);
        rec.decode(&stream);
        let r = stream.get_result().expect("sem resultado");
        let tok = words::Tokens { timestamps: r.timestamps.unwrap_or_default(), durations: r.durations, tokens: r.tokens };
        let ws = words::assemble(&tok, c.start, c.end, &pauses);
        starts.extend(ws.iter().map(|w| (w.start, w.word.clone())));
        texts.push(ws.iter().map(|w| w.word.as_str()).collect::<Vec<_>>().join(" "));
    }
    let secs = t.elapsed().as_secs_f64();
    println!("{} pedaços · carga {load_s:.1} s · {duration:.1} s em {secs:.2} s ({:.1}× tempo real)", chunks.len(), duration / secs);
    let text = texts.join(" ");
    let doc = serde_json::json!({
        "modelo": dir, "chunk": chunk, "secs": secs, "audio_s": duration,
        "pedacos": chunks.iter().map(|c| [c.start, c.end]).collect::<Vec<_>>(),
        "texto": text,
        "palavras": starts,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap()).expect("gravar o JSON");
    println!("{text}");
}
