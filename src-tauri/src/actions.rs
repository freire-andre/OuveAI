//! Ações compartilhadas pela bandeja, pelos atalhos e pelos comandos do frontend.

use crate::state::AppState;
use crate::tray::TrayItems;
use crate::windows;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, serde::Serialize)]
pub struct Status {
    pub stealth: bool,
    pub stealth_supported: bool,
    pub build: u32,
    pub recording: bool,
    pub shortcuts: Vec<ShortcutInfo>,
    pub data_dir: String,
    pub version: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ShortcutInfo {
    pub action: crate::state::Action,
    pub accel: String,
    pub error: Option<String>,
}

pub fn status(app: &AppHandle) -> Status {
    let st = app.state::<AppState>();
    let errors = st.shortcut_errors.lock().unwrap().clone();
    Status {
        stealth: st.is_stealth(),
        stealth_supported: st.stealth_supported,
        build: st.build,
        recording: st.is_recording(),
        shortcuts: crate::state::Action::ALL
            .iter()
            .map(|&a| ShortcutInfo {
                action: a,
                accel: st.accel(a),
                error: errors.iter().find(|(e, _)| *e == a).map(|(_, m)| m.clone()),
            })
            .collect(),
        data_dir: ouveai_core::paths::data_dir().display().to_string(),
        version: app.package_info().version.to_string(),
    }
}

/// Avisa todas as janelas de que o estado mudou.
pub fn broadcast(app: &AppHandle) {
    let _ = app.emit("status", status(app));
}

pub fn set_stealth(app: &AppHandle, on: bool) {
    let st = app.state::<AppState>();
    st.stealth.store(on, Ordering::Relaxed);
    st.set_setting("stealth", if on { "true" } else { "false" });
    windows::apply_stealth(app, on);
    if let Some(t) = app.try_state::<TrayItems>() {
        let _ = t.stealth.set_checked(on);
    }
    log::info!("stealth {}", if on { "ligado" } else { "desligado" });
    broadcast(app);
}

/// Grava / para a reunião (bandeja, atalho, mini).
/// Fora da thread principal: iniciar abre dispositivos, carrega o VAD e cria a mini
/// (criar janela na thread principal dentro de um comando trava o Tauri no Windows).
pub fn toggle_record(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || crate::live::toggle(&app));
}

pub fn mark(app: &AppHandle) {
    if !crate::live::mark(app) {
        log::info!("marca ignorada: não está gravando");
        return;
    }
    log::info!("marca");
    let _ = app.emit("mark", ());
}
