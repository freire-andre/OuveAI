//! Diretório local único do app e da CLI (plano, seção 6.6; regra 9).
//!
//! É o mesmo caminho que o Tauri chama de `app_local_data_dir`:
//! `%LOCALAPPDATA%\<identifier>`. Nada vai para o Roaming.

use std::path::PathBuf;

/// Tem que ser igual ao `identifier` de `src-tauri/tauri.conf.json`.
pub const IDENTIFIER: &str = "br.com.ouveai";

/// `%LOCALAPPDATA%\br.com.ouveai`. A variável `OUVEAI_DATA_DIR` sobrepõe (testes e diagnóstico).
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("OUVEAI_DATA_DIR") {
        return PathBuf::from(d);
    }
    let base = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA não definido");
    PathBuf::from(base).join(IDENTIFIER)
}

pub fn db_path() -> PathBuf {
    data_dir().join("ouveai.db")
}

pub fn models_dir() -> PathBuf {
    data_dir().join("models")
}

pub fn bin_dir() -> PathBuf {
    data_dir().join("bin")
}

pub fn recordings_dir() -> PathBuf {
    data_dir().join("recordings")
}

/// O mesmo diretório que o `tauri-plugin-log` usa no Windows (`app_log_dir`).
pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

#[cfg(test)]
mod tests {
    #[test]
    fn fica_no_local_appdata() {
        let _g = crate::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: protegido pelo ENV_LOCK.
        unsafe { std::env::remove_var("OUVEAI_DATA_DIR") };
        let d = super::data_dir();
        let local = std::env::var("LOCALAPPDATA").unwrap();
        assert!(d.starts_with(&local), "{d:?}");
        assert!(d.ends_with(super::IDENTIFIER));
        assert!(!d.to_string_lossy().contains("Roaming"));
    }
}
