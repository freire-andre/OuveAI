//! Spike S4 (Fase 0): baixa um arquivo grande com o `ureq`, retomando com `Range`
//! a partir de `<dest>.part`. Relata o proxy usado, os redirecionamentos, a
//! velocidade e confere o SHA-256 e o tamanho (catálogo, L14).
//!
//! Uso: cargo run --release -p ouveai-core --example s4_download -- <url> <dest>
//!        [--sha256 HEX] [--size BYTES] [--stop-mb N] [--webpki]
//! `--webpki` usa a lista de raízes embutida (padrão do ureq) em vez do verificador do Windows.
//! `--stop-mb N` simula uma queda: para depois de N MB baixados nesta rodada (código 3).
//! Rodar de novo com o mesmo <dest> retoma de onde parou.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::{Duration, Instant};
use ureq::ResponseExt;
use ureq::tls::{RootCerts, TlsConfig};

/// SHA-256 incremental pelo CNG do Windows (BCrypt), sem crate extra.
struct Sha256 {
    alg: *mut core::ffi::c_void,
    hash: *mut core::ffi::c_void,
}

impl Sha256 {
    fn new() -> Self {
        use windows_sys::Win32::Security::Cryptography::*;
        let (mut alg, mut hash) = (std::ptr::null_mut(), std::ptr::null_mut());
        unsafe {
            assert_eq!(BCryptOpenAlgorithmProvider(&mut alg, BCRYPT_SHA256_ALGORITHM, std::ptr::null(), 0), 0);
            assert_eq!(BCryptCreateHash(alg, &mut hash, std::ptr::null_mut(), 0, std::ptr::null(), 0, 0), 0);
        }
        Self { alg, hash }
    }
    fn update(&mut self, data: &[u8]) {
        use windows_sys::Win32::Security::Cryptography::BCryptHashData;
        unsafe { assert_eq!(BCryptHashData(self.hash, data.as_ptr(), data.len() as u32, 0), 0) };
    }
    fn hex(self) -> String {
        use windows_sys::Win32::Security::Cryptography::*;
        let mut out = [0u8; 32];
        unsafe {
            assert_eq!(BCryptFinishHash(self.hash, out.as_mut_ptr(), 32, 0), 0);
            BCryptDestroyHash(self.hash);
            BCryptCloseAlgorithmProvider(self.alg, 0);
        }
        out.iter().map(|b| format!("{b:02x}")).collect()
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).map(|i| args[i + 1].clone())
}

fn mb(b: u64) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (url, dest) = (&args[1], &args[2]);
    let want_sha = arg(&args, "--sha256").map(|s| s.to_lowercase());
    let want_size = arg(&args, "--size").map(|s| s.parse::<u64>().unwrap());
    let stop_after = arg(&args, "--stop-mb").map(|s| s.parse::<u64>().unwrap() * 1024 * 1024);
    let roots = if args.iter().any(|a| a == "--webpki") { RootCerts::WebPki } else { RootCerts::PlatformVerifier };
    println!("tls: {}", if matches!(roots, RootCerts::WebPki) { "raízes embutidas (webpki)" } else { "verificador do Windows" });
    let part = format!("{dest}.part");
    let etag_path = format!("{dest}.part.etag");

    // Proxy: variáveis HTTPS_PROXY/HTTP_PROXY/ALL_PROXY ou o proxy fixo do Windows
    // (HKCU ...\Internet Settings\ProxyServer). PAC/WPAD não são lidos pelo ureq.
    let proxy = ureq::Proxy::try_from_env();
    match &proxy {
        Some(p) => println!("proxy: {}", p.uri()),
        None => println!("proxy: nenhum (variáveis de ambiente e ProxyServer do Windows vazios)"),
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(proxy)
        .tls_config(TlsConfig::builder().root_certs(roots).build())
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_per_read(Some(Duration::from_secs(30)))
        .save_redirect_history(true)
        .http_status_as_error(false)
        .build()
        .into();

    // Retomada: hash do que já existe no .part e pedido a partir do fim dele.
    let mut have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let etag = std::fs::read_to_string(&etag_path).ok();
    let mut sha = Sha256::new();
    if have > 0 {
        let mut f = File::open(&part).unwrap();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = f.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            sha.update(&buf[..n]);
        }
        println!("retomando: {have} bytes já no .part ({:.1} MB) · etag salvo: {}", mb(have), etag.as_deref().unwrap_or("-"));
    }

    let mut req = agent.get(url.as_str());
    if have > 0 {
        req = req.header("Range", format!("bytes={have}-"));
        if let Some(e) = &etag {
            // If-Range: se o arquivo mudou no servidor, ele devolve 200 com o arquivo inteiro.
            req = req.header("If-Range", e.trim());
        }
    }

    let t0 = Instant::now();
    let resp = match req.call() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("erro na requisição: {e}\n  detalhe: {e:?}");
            std::process::exit(2);
        }
    };
    let ttfb = t0.elapsed();
    let status = resp.status().as_u16();
    if let Some(h) = resp.get_redirect_history() {
        let hosts: Vec<String> = h.iter().map(|u| u.host().unwrap_or("?").to_string()).collect();
        println!("redirecionamentos: {}", hosts.join(" -> "));
    }
    // Só host + caminho: a query das URLs assinadas (GitHub, HF) é credencial temporária e não vai para log.
    let fin = resp.get_uri();
    println!("final: {}{}", fin.host().unwrap_or("?"), fin.path());
    let header = |n: &str| resp.headers().get(n).and_then(|v| v.to_str().ok()).map(str::to_string);
    println!(
        "status: {status} · resposta em {:.0} ms · content-length: {} · content-range: {} · accept-ranges: {} · etag: {}",
        ttfb.as_secs_f64() * 1000.0,
        header("content-length").unwrap_or("-".into()),
        header("content-range").unwrap_or("-".into()),
        header("accept-ranges").unwrap_or("-".into()),
        header("etag").unwrap_or("-".into()),
    );

    match status {
        206 => {
            // Content-Range: bytes <ini>-<fim>/<total>; o início tem que bater com o que temos.
            let cr = header("content-range").unwrap_or_default();
            let start: u64 = cr.trim_start_matches("bytes ").split('-').next().and_then(|s| s.parse().ok()).unwrap_or(u64::MAX);
            if start != have {
                eprintln!("content-range começa em {start}, esperado {have}: abortando");
                std::process::exit(2);
            }
        }
        200 => {
            if have > 0 {
                println!("servidor ignorou o Range (ou o arquivo mudou): recomeçando do zero");
                have = 0;
                sha = Sha256::new();
            }
        }
        416 => {
            println!("416: o .part já tem o arquivo inteiro (ou passou dele)");
        }
        _ => {
            eprintln!("status inesperado {status}");
            std::process::exit(2);
        }
    }
    if let Some(e) = header("etag") {
        std::fs::write(&etag_path, e).unwrap();
    }

    let mut out = OpenOptions::new().create(true).write(true).truncate(false).open(&part).unwrap();
    out.set_len(have).unwrap();
    out.seek(SeekFrom::Start(have)).unwrap();

    let mut got = 0u64;
    let mut stopped = false;
    if status != 416 {
        let mut body = resp.into_body();
        let mut reader = body.as_reader();
        let mut buf = vec![0u8; 256 * 1024];
        let mut last = Instant::now();
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => {
                    out.flush().unwrap();
                    eprintln!("\nconexão caiu após {got} bytes nesta rodada: {e}\n  rode de novo para retomar");
                    std::process::exit(4);
                }
            };
            out.write_all(&buf[..n]).unwrap();
            sha.update(&buf[..n]);
            got += n as u64;
            if last.elapsed() >= Duration::from_secs(2) {
                let s = t0.elapsed().as_secs_f64();
                print!("\r  {:.1} MB nesta rodada · {:.1} MB/s   ", mb(got), mb(got) / s);
                std::io::stdout().flush().ok();
                last = Instant::now();
            }
            if stop_after.is_some_and(|s| got >= s) {
                stopped = true;
                break;
            }
        }
    }
    out.flush().unwrap();
    drop(out);
    let secs = t0.elapsed().as_secs_f64();
    let total = have + got;
    println!("\nbaixado nesta rodada: {got} bytes ({:.1} MB) em {secs:.1} s · {:.1} MB/s · total no disco: {total}", mb(got), mb(got) / secs);

    if stopped {
        println!("parada simulada (--stop-mb): .part com {total} bytes; rode de novo para retomar");
        std::process::exit(3);
    }

    let hex = sha.hex();
    println!("sha256: {hex}");
    let size_ok = want_size.is_none_or(|w| w == total);
    let sha_ok = want_sha.as_ref().is_none_or(|w| *w == hex);
    if let Some(w) = want_size {
        println!("tamanho: {} (esperado {w})", if size_ok { "OK" } else { "DIFERENTE" });
    }
    if let Some(w) = &want_sha {
        println!("sha256: {} (esperado {w})", if sha_ok { "OK" } else { "DIFERENTE" });
    }
    if size_ok && sha_ok {
        std::fs::rename(&part, dest).unwrap();
        std::fs::remove_file(&etag_path).ok();
        println!("ok: {dest}");
    } else {
        eprintln!("verificação falhou: o .part fica para análise");
        std::process::exit(5);
    }
}
