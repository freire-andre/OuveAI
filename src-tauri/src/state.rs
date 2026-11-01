//! Estado do app: configurações (SQLite) + flags em memória.

use ouveai_core::store::Store;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use std::sync::atomic::{AtomicBool, Ordering};

/// Ações com atalho global (D3). A chave vira `shortcut.<id>` na tabela `settings`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Mini,
    Record,
    Mark,
}

impl Action {
    pub const ALL: [Action; 3] = [Action::Mini, Action::Record, Action::Mark];

    pub fn id(self) -> &'static str {
        match self {
            Action::Mini => "mini",
            Action::Record => "record",
            Action::Mark => "mark",
        }
    }

    pub fn default_accel(self) -> &'static str {
        match self {
            Action::Mini => "Ctrl+Alt+Shift+O",
            Action::Record => "Ctrl+Alt+Shift+R",
            Action::Mark => "Ctrl+Alt+Shift+M",
        }
    }
}

pub struct AppState {
    pub store: Mutex<Store>,
    pub stealth: AtomicBool,
    /// `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` só existe a partir do build 19041 (Win10 2004).
    pub stealth_supported: bool,
    pub build: u32,
    pub recording: AtomicBool,
    /// Erro do último registro de cada atalho (ex.: já usado por outro programa).
    pub shortcut_errors: Mutex<Vec<(Action, String)>>,
    /// Última posição da mini (física), gravada no banco quando ela fecha.
    pub mini_pos: Mutex<Option<(i32, i32)>>,
    /// Janelas criadas e ainda carregando a página (rótulo → início), para medir a abertura.
    pub opening: Mutex<HashMap<String, Instant>>,
}

impl AppState {
    pub fn setting(&self, key: &str) -> Option<String> {
        self.store.lock().unwrap().get(key).unwrap_or_else(|e| {
            log::error!("falha ao ler a configuração {key}: {e}");
            None
        })
    }

    pub fn set_setting(&self, key: &str, value: &str) {
        if let Err(e) = self.store.lock().unwrap().set(key, value) {
            log::error!("falha ao gravar a configuração {key}: {e}");
        }
    }

    pub fn accel(&self, a: Action) -> String {
        self.setting(&format!("shortcut.{}", a.id())).unwrap_or_else(|| a.default_accel().to_string())
    }

    pub fn is_stealth(&self) -> bool {
        self.stealth.load(Ordering::Relaxed)
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::Relaxed)
    }
}
