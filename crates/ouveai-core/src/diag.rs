//! "Exportar diagnóstico" (.zip): versão, máquina, dispositivos, configurações e logs.
//! Nunca leva texto transcrito, áudio, título de sessão nem nome de arquivo (regra 8):
//! os logs já seguem essa regra, e o resto é montado aqui só com contagens e nomes de
//! dispositivo.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Valor de configuração que pode ir para o relatório. O proxy perde usuário e senha.
pub fn redact_setting(key: &str, value: &str) -> String {
    if key == "net.proxy"
        && let Some((scheme, rest)) = value.split_once("://")
        && let Some((_, host)) = rest.rsplit_once('@')
    {
        return format!("{scheme}://***@{host}");
    }
    value.to_string()
}

/// "2026-11-01 14:03 UTC" a partir de segundos Unix.
pub fn utc_stamp(unix: i64) -> String {
    let (y, m, d) = crate::export::civil_from_days(unix.div_euclid(86_400));
    let s = unix.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", s / 3600, s % 3600 / 60)
}

/// Arquivos de log da pasta (o atual e os rotacionados), por nome.
pub fn log_files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.ends_with(".log").then(|| fs::read(e.path()).ok().map(|d| (format!("logs/{name}"), d)))?
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Grava o .zip em `dest`. Se falhar no meio, não deixa um arquivo pela metade.
pub fn write_zip(dest: &Path, entries: &[(String, Vec<u8>)]) -> io::Result<()> {
    let tmp = dest.with_extension("zip.part");
    let r = (|| -> io::Result<()> {
        let mut z = zip::ZipWriter::new(fs::File::create(&tmp)?);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in entries {
            z.start_file(name.as_str(), opts).map_err(io::Error::other)?;
            z.write_all(data)?;
        }
        z.finish().map_err(io::Error::other)?.sync_all()
    })();
    match r {
        Ok(()) => fs::rename(&tmp, dest),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn proxy_perde_usuario_e_senha() {
        assert_eq!(redact_setting("net.proxy", "http://joao:segredo@proxy.empresa:8080"), "http://***@proxy.empresa:8080");
        assert_eq!(redact_setting("net.proxy", "http://proxy.empresa:8080"), "http://proxy.empresa:8080");
        assert_eq!(redact_setting("audio.retention", "7"), "7");
        assert_eq!(utc_stamp(1_793_541_780), "2026-11-01 14:03 UTC");
    }

    #[test]
    fn zip_com_relatorio_e_logs() {
        let dir = std::env::temp_dir().join(format!("ouveai-diag-{}", std::process::id()));
        let logs = dir.join("logs");
        fs::create_dir_all(&logs).unwrap();
        fs::write(logs.join("ouveai.log"), "linha").unwrap();
        fs::write(logs.join("ouveai_2026-11-01.log"), "antiga").unwrap();
        fs::write(logs.join("outro.txt"), "não entra").unwrap();

        let mut entries = vec![("relatorio.txt".to_string(), b"versao 0.9.0".to_vec())];
        entries.extend(log_files(&logs));
        let dest = dir.join("d.zip");
        write_zip(&dest, &entries).unwrap();
        assert!(!dir.join("d.zip.part").exists());

        let mut z = zip::ZipArchive::new(fs::File::open(&dest).unwrap()).unwrap();
        let mut names: Vec<String> = z.file_names().filter_map(|n| n.ok().map(|n| n.into_owned())).collect();
        names.sort();
        assert_eq!(names, ["logs/ouveai.log", "logs/ouveai_2026-11-01.log", "relatorio.txt"]);
        let mut s = String::new();
        z.by_name("logs/ouveai.log").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "linha");
        fs::remove_dir_all(&dir).ok();
    }
}
