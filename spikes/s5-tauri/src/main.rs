//! Spike S5 (Fase 0): linha de base de um app Tauri vazio.
//! S5_WINDOWS = número de janelas abertas no início (0 = só o processo, como na bandeja).
//! S5_OUT = arquivo onde grava `ready <ms desde a criação do processo>`.
#![windows_subsystem = "windows"]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::webview::PageLoadEvent;
use tauri::{RunEvent, WebviewUrl, WebviewWindowBuilder};

/// Milissegundos desde a criação do processo (inclui o carregamento do exe, antes do `main`).
fn ms_since_process_start() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let ft = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64;
    let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exit, mut kernel, mut user, mut now) = (z, z, z, z, z);
    unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut created, &mut exit, &mut kernel, &mut user);
        GetSystemTimeAsFileTime(&mut now);
    }
    (ft(now) - ft(created)) / 10_000.0
}

fn report(out: &str, what: &str) {
    let line = format!("{what} {:.0}\n", ms_since_process_start());
    let _ = std::fs::OpenOptions::new().create(true).append(true).open(out).and_then(|mut f| {
        use std::io::Write;
        f.write_all(line.as_bytes())
    });
}

fn main() {
    let n: usize = std::env::var("S5_WINDOWS").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let out = std::env::var("S5_OUT").unwrap_or_else(|_| "s5.log".into());
    report(&out, "main");

    // Com a feature `sherpa`, força o link do sherpa-onnx (CRT estático) junto com o Tauri (CRT híbrido).
    #[cfg(feature = "sherpa")]
    if let Ok(dir) = std::env::var("S5_MODEL") {
        let mut cfg = sherpa_onnx::OfflineRecognizerConfig::default();
        cfg.model_config.transducer = sherpa_onnx::OfflineTransducerModelConfig {
            encoder: Some(format!("{dir}/encoder.int8.onnx")),
            decoder: Some(format!("{dir}/decoder.int8.onnx")),
            joiner: Some(format!("{dir}/joiner.int8.onnx")),
        };
        cfg.model_config.tokens = Some(format!("{dir}/tokens.txt"));
        cfg.model_config.model_type = Some("nemo_transducer".into());
        let ok = sherpa_onnx::OfflineRecognizer::create(&cfg).is_some();
        report(&out, if ok { "sherpa_ok" } else { "sherpa_falhou" });
    }

    let loaded = Arc::new(AtomicUsize::new(0));
    let out_load = out.clone();
    let out_setup = out.clone();

    tauri::Builder::default()
        .setup(move |app| {
            for i in 0..n {
                WebviewWindowBuilder::new(app, format!("w{i}"), WebviewUrl::App("index.html".into()))
                    .title(format!("S5 janela {i}"))
                    .inner_size(800.0, 600.0)
                    .build()?;
            }
            if n == 0 {
                report(&out_setup, "ready");
            }
            Ok(())
        })
        .on_page_load(move |_webview, payload| {
            if payload.event() == PageLoadEvent::Finished && loaded.fetch_add(1, Ordering::SeqCst) + 1 == n {
                report(&out_load, "ready");
            }
        })
        .build(tauri::generate_context!())
        .expect("falha ao iniciar o Tauri")
        .run(|_app, event| {
            // Sem janelas o Tauri tenta sair; aqui o processo fica vivo, como na bandeja.
            if let RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
