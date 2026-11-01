//! Ícone e menu da bandeja.

use crate::{actions, state::AppState, windows};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Manager, Wry};

/// Itens cujo texto/estado muda em tempo de execução.
pub struct TrayItems {
    pub record: MenuItem<Wry>,
    pub stealth: CheckMenuItem<Wry>,
}

pub fn create(app: &App) -> tauri::Result<()> {
    let st = app.state::<AppState>();
    let open = MenuItem::with_id(app, "open", "Abrir", true, None::<&str>)?;
    let mini = MenuItem::with_id(app, "mini", "Mostrar/ocultar mini", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Gravar", true, None::<&str>)?;
    let stealth = CheckMenuItem::with_id(app, "stealth", "Stealth (oculto em capturas)", st.stealth_supported, st.is_stealth(), None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Configurações", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(app, &[&open, &mini, &record, &sep()?, &stealth, &settings, &sep()?, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("ícone padrão"))
        .tooltip("OuveAI")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "open" => windows::open_main(app, None),
            "mini" => windows::toggle_mini(app),
            "record" => actions::toggle_record(app),
            "stealth" => {
                let on = !app.state::<AppState>().is_stealth();
                actions::set_stealth(app, on);
            }
            "settings" => windows::open_main(app, Some("settings")),
            "quit" => {
                windows::save_mini_pos(app);
                log::info!("saindo pelo menu da bandeja");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                windows::open_main(tray.app_handle(), None);
            }
        })
        .build(app)?;

    app.manage(TrayItems { record, stealth });
    Ok(())
}
