//! Atalhos globais (D3). Se o registro falhar (atalho já usado por outro programa),
//! o erro fica no estado e a tela de Configurações sugere trocar.

use crate::state::{Action, AppState};
use crate::{actions, windows};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let st = app.state::<AppState>();
            let Some(action) = Action::ALL.into_iter().find(|&a| parse(&st.accel(a)).is_ok_and(|s| s == *shortcut)) else {
                return;
            };
            match action {
                Action::Mini => windows::toggle_mini(app),
                Action::Record => actions::toggle_record(app),
                Action::Mark => actions::mark(app),
            }
        })
        .build()
}

pub fn parse(accel: &str) -> Result<Shortcut, String> {
    accel.parse::<Shortcut>().map_err(|e| format!("atalho inválido: {e}"))
}

/// (Re)registra os três atalhos a partir das configurações.
pub fn register_all(app: &AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let st = app.state::<AppState>();
    let mut errors = Vec::new();
    let mut seen: Vec<Shortcut> = Vec::new();
    for a in Action::ALL {
        let accel = st.accel(a);
        let r = parse(&accel).and_then(|s| {
            if seen.contains(&s) {
                return Err("o mesmo atalho já está em outra ação".to_string());
            }
            seen.push(s);
            gs.register(s).map_err(|e| format!("não foi possível registrar (já usado por outro programa?): {e}"))
        });
        if let Err(e) = r {
            log::warn!("atalho {} ({accel}): {e}", a.id());
            errors.push((a, e));
        }
    }
    *st.shortcut_errors.lock().unwrap() = errors;
}

/// Troca o atalho de uma ação. Valida antes de gravar; se o novo falhar, volta ao anterior.
pub fn set(app: &AppHandle, action: Action, accel: &str) -> Result<(), String> {
    parse(accel)?;
    let st = app.state::<AppState>();
    let key = format!("shortcut.{}", action.id());
    let old = st.accel(action);
    st.set_setting(&key, accel);
    register_all(app);
    let failed = st.shortcut_errors.lock().unwrap().iter().find(|(a, _)| *a == action).map(|(_, e)| e.clone());
    if let Some(e) = failed {
        st.set_setting(&key, &old);
        register_all(app);
        return Err(e);
    }
    Ok(())
}
