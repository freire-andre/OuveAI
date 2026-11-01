//! Downloader (S4): certificados do Windows, proxy do sistema ou manual, retomada por
//! `Range` + `If-Range` a partir de `<arquivo>.part`, novas tentativas com espera crescente.
//! A conferência de tamanho e SHA-256 fica com quem chama (`core::models`).

use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use ureq::ResponseExt;
use ureq::tls::{RootCerts, TlsConfig};

#[derive(Clone, Debug, Default)]
pub struct NetConfig {
    /// Proxy manual das Configurações (`http://host:porta`). Vazio = proxy do sistema.
    pub proxy: Option<String>,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
    pub bytes_per_sec: f64,
    /// Preenchido enquanto espera a próxima tentativa.
    pub retry: Option<Retry>,
}

/// Falhou e vai tentar de novo daqui a `wait_secs`.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Retry {
    pub attempt: u32,
    pub max: u32,
    pub wait_secs: u64,
    /// Status HTTP do servidor; 0 = sem conexão (DNS, recusa, tempo esgotado).
    pub status: u16,
}

#[derive(Debug)]
pub enum NetError {
    Cancelled,
    Status(u16),
    Io(std::io::Error),
    Net(String),
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetError::Cancelled => write!(f, "cancelado"),
            NetError::Status(s) => write!(f, "o servidor respondeu {s}"),
            NetError::Io(e) => write!(f, "erro de disco: {e}"),
            NetError::Net(e) => write!(f, "erro de rede: {e}"),
        }
    }
}

impl From<std::io::Error> for NetError {
    fn from(e: std::io::Error) -> Self {
        NetError::Io(e)
    }
}

const RETRIES: u32 = 5;

pub(crate) fn agent(cfg: &NetConfig) -> ureq::Agent {
    let proxy = match cfg.proxy.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => ureq::Proxy::new(p).map_err(|e| log::warn!("proxy manual inválido ({e}); usando o do sistema")).ok(),
        None => None,
    }
    .or_else(ureq::Proxy::try_from_env);
    ureq::Agent::config_builder()
        .proxy(proxy)
        .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_per_read(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .into()
}

/// Baixa `url` para `part`, retomando o que já existe. `total` é o tamanho esperado (do catálogo).
pub fn download(
    cfg: &NetConfig,
    url: &str,
    part: &Path,
    total: u64,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<(), NetError> {
    if let Some(dir) = part.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let agent = agent(cfg);
    let mut attempt = 0;
    loop {
        match fetch_once(&agent, url, part, total, cancel, on_progress) {
            Ok(()) => return Ok(()),
            Err(e @ NetError::Cancelled) => return Err(e),
            // 4xx (exceto 408/429) não melhora tentando de novo.
            Err(NetError::Status(s)) if (400..500).contains(&s) && s != 408 && s != 429 => return Err(NetError::Status(s)),
            Err(e) => {
                attempt += 1;
                if attempt > RETRIES {
                    return Err(e);
                }
                let wait = Duration::from_secs((2u64 << attempt).min(30));
                log::warn!("download falhou ({e}); tentativa {attempt}/{RETRIES} em {} s", wait.as_secs());
                let status = if let NetError::Status(s) = e { s } else { 0 };
                let done = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0).min(total);
                let retry = Retry { attempt, max: RETRIES, wait_secs: wait.as_secs(), status };
                on_progress(Progress { done, total, bytes_per_sec: 0.0, retry: Some(retry) });
                let until = Instant::now() + wait;
                while Instant::now() < until {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(NetError::Cancelled);
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }
}

fn fetch_once(
    agent: &ureq::Agent,
    url: &str,
    part: &Path,
    total: u64,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<(), NetError> {
    let etag_path = part.with_extension("part.etag");
    let mut have = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if have > total {
        have = 0; // .part maior que o esperado: recomeça
    }
    if total > 0 && have == total {
        return Ok(());
    }

    let mut req = agent.get(url);
    if have > 0 {
        req = req.header("Range", format!("bytes={have}-"));
        if let Ok(e) = std::fs::read_to_string(&etag_path) {
            req = req.header("If-Range", e.trim());
        }
    }
    let resp = req.call().map_err(|e| NetError::Net(e.to_string()))?;
    let status = resp.status().as_u16();
    let header = |n: &str| resp.headers().get(n).and_then(|v| v.to_str().ok()).map(str::to_string);
    // Só host: a query das URLs assinadas é credencial temporária (S4).
    log::info!("download {status} de {} (retomando de {have})", resp.get_uri().host().unwrap_or("?"));

    match status {
        206 => {
            let start = header("content-range")
                .and_then(|cr| cr.trim_start_matches("bytes ").split('-').next()?.parse::<u64>().ok());
            if start != Some(have) {
                return Err(NetError::Net(format!("content-range inesperado: {start:?}, esperado {have}")));
            }
        }
        200 => have = 0, // servidor ignorou o Range, ou o arquivo mudou (If-Range)
        416 => {
            std::fs::remove_file(part).ok();
            return Err(NetError::Net("416: recomeçando do zero".into()));
        }
        s => return Err(NetError::Status(s)),
    }
    if let Some(e) = header("etag") {
        std::fs::write(&etag_path, e)?;
    }

    let mut out = OpenOptions::new().create(true).write(true).truncate(false).open(part)?;
    out.set_len(have)?;
    out.seek(SeekFrom::Start(have))?;

    let mut body = resp.into_body();
    let mut reader = body.as_reader();
    let mut buf = vec![0u8; 256 * 1024];
    let t0 = Instant::now();
    let (mut got, mut last) = (0u64, Instant::now());
    loop {
        if cancel.load(Ordering::Relaxed) {
            out.flush()?;
            return Err(NetError::Cancelled);
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                out.flush()?;
                return Err(NetError::Net(e.to_string()));
            }
        };
        out.write_all(&buf[..n])?;
        got += n as u64;
        if last.elapsed() >= Duration::from_millis(250) {
            on_progress(Progress { done: have + got, total, bytes_per_sec: got as f64 / t0.elapsed().as_secs_f64(), retry: None });
            last = Instant::now();
        }
    }
    out.flush()?;
    let done = have + got;
    on_progress(Progress { done, total, bytes_per_sec: got as f64 / t0.elapsed().as_secs_f64().max(0.001), retry: None });
    if total > 0 && done < total {
        return Err(NetError::Net(format!("conexão terminou antes do fim ({done} de {total} bytes)")));
    }
    Ok(())
}
