//! Spike S1 (Fase 0): carrega o Parakeet TDT 0.6b v3 int8 pelo crate `sherpa-onnx`,
//! transcreve um WAV e relata os campos reais do resultado, o tempo de carga,
//! o fator de tempo real (duração ÷ tempo de transcrição) e a RAM de pico.
//!
//! Uso: cargo run --release -p ouveai-core --example s1_asr -- <modelo_dir> <wav> [--beam "hotword1/hotword2"]

use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig, Wave};
use std::time::Instant;

fn peak_ram_mb() -> f64 {
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    c.PeakWorkingSetSize as f64 / (1024.0 * 1024.0)
}

fn config(dir: &str, threads: i32, decoding: &str) -> OfflineRecognizerConfig {
    let mut cfg = OfflineRecognizerConfig::default();
    cfg.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(format!("{dir}/encoder.int8.onnx")),
        decoder: Some(format!("{dir}/decoder.int8.onnx")),
        joiner: Some(format!("{dir}/joiner.int8.onnx")),
    };
    cfg.model_config.tokens = Some(format!("{dir}/tokens.txt"));
    cfg.model_config.model_type = Some("nemo_transducer".into());
    cfg.model_config.num_threads = threads;
    cfg.model_config.provider = Some("cpu".into());
    cfg.decoding_method = Some(decoding.into());
    cfg
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (dir, wav_path) = (&args[1], &args[2]);
    let beam = args.iter().position(|a| a == "--beam").map(|i| args[i + 1].clone());

    let threads = std::thread::available_parallelism().map(|n| n.get().min(4) as i32).unwrap_or(4);
    println!("cpu: avx2={} threads_asr={threads}", std::arch::is_x86_feature_detected!("avx2"));

    let wave = Wave::read(wav_path).expect("falha ao ler o WAV");
    let duration = wave.num_samples() as f64 / wave.sample_rate() as f64;
    println!("audio: {wav_path} · {} Hz · {duration:.2} s", wave.sample_rate());

    let t0 = Instant::now();
    let recognizer = OfflineRecognizer::create(&config(dir, threads, "greedy_search"))
        .expect("falha ao criar o reconhecedor");
    let load_s = t0.elapsed().as_secs_f64();
    println!("carga do modelo: {load_s:.2} s · RAM pico até aqui: {:.0} MB", peak_ram_mb());

    let t1 = Instant::now();
    let stream = recognizer.create_stream();
    stream.accept_waveform(wave.sample_rate(), wave.samples());
    recognizer.decode(&stream);
    let result = stream.get_result().expect("sem resultado");
    let decode_s = t1.elapsed().as_secs_f64();

    println!(
        "transcrição: {decode_s:.2} s · fator de tempo real: {:.1}× mais rápido que o tempo real · RAM pico: {:.0} MB",
        duration / decode_s,
        peak_ram_mb()
    );

    let ts = result.timestamps.as_deref().unwrap_or(&[]);
    let du = result.durations.as_deref().unwrap_or(&[]);
    println!(
        "campos: tokens={} timestamps={} durations={}",
        result.tokens.len(),
        result.timestamps.as_ref().map_or("ausente".into(), |v| v.len().to_string()),
        result.durations.as_ref().map_or("ausente".into(), |v| v.len().to_string()),
    );
    let with_sp = result.tokens.iter().filter(|t| t.starts_with('▁')).count();
    let with_space = result.tokens.iter().filter(|t| t.starts_with(' ')).count();
    println!("marcador de palavra: '▁'={with_sp} · espaço inicial={with_space}");

    println!("\nprimeiros 25 tokens (token | início | duração):");
    for i in 0..result.tokens.len().min(25) {
        println!(
            "  {:>3} {:<14} {:>7.3} {:>6}",
            i,
            format!("{:?}", result.tokens[i]),
            ts.get(i).copied().unwrap_or(f32::NAN),
            du.get(i).map_or("-".into(), |d| format!("{d:.3}"))
        );
    }
    println!("\ntexto:\n{}\n", result.text);

    let out = std::path::Path::new(wav_path).with_extension("s1.json");
    let json = format!(
        "{{\"duration\":{duration:.3},\"load_s\":{load_s:.3},\"decode_s\":{decode_s:.3},\"text\":{:?},\"tokens\":{:?},\"timestamps\":{:?},\"durations\":{:?}}}",
        result.text, result.tokens, ts, du
    );
    std::fs::write(&out, json).expect("falha ao gravar o JSON");
    println!("resultado bruto: {}", out.display());

    // Hotwords só funcionam com modified_beam_search; aqui só verificamos se o TDT aceita.
    if let Some(hw) = beam {
        drop(recognizer);
        let mut cfg = config(dir, threads, "modified_beam_search");
        cfg.hotwords_score = 1.5;
        match OfflineRecognizer::create(&cfg) {
            None => println!("\nmodified_beam_search: reconhecedor NÃO foi criado"),
            Some(r) => {
                let s = r.create_stream_with_hotwords(&hw.replace('/', "\n"));
                let t = Instant::now();
                s.accept_waveform(wave.sample_rate(), wave.samples());
                r.decode(&s);
                let res = s.get_result().expect("sem resultado");
                println!(
                    "\nmodified_beam_search + hotwords [{hw}] ({:.2} s):\n{}",
                    t.elapsed().as_secs_f64(),
                    res.text
                );
            }
        }
    }
}
