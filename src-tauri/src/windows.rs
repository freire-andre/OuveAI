//! Janelas sob demanda (plano 6.2): nenhuma WebView na bandeja; a principal e a mini
//! são criadas quando abertas e destruídas quando fechadas.

use crate::state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri::webview::{PageLoadEvent, PageLoadPayload};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Webview, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

pub const MAIN: &str = "main";
pub const MINI: &str = "mini";
pub const WIZARD: &str = "wizard";

/// Já há uma caixa de erro aberta (não empilha várias se a pessoa clicar de novo).
static SHOWING: AtomicBool = AtomicBool::new(false);

/// Checa o WebView2 antes de criar uma janela. Sem ele, o Tauri mostraria uma caixa
/// genérica em inglês; aqui a pessoa vê uma explicação em português e a janela não é
/// pedida.
fn webview_ok(app: &AppHandle, what: &str) -> bool {
    match wry::webview_version() {
        Ok(_) => true,
        Err(e) => {
            log::error!("WebView2 indisponível ao abrir {what}: {e}");
            error_box(app, None);
            false
        }
    }
}

/// Falha ao criar uma janela por outro motivo.
fn window_failed(app: &AppHandle, what: &str, e: &tauri::Error) {
    log::error!("falha ao criar {what}: {e}");
    let err = e.to_string();
    error_box(app, (!err.to_lowercase().contains("webview")).then_some(err));
}

/// Caixa nativa do Windows (não depende da WebView). `detail` = `None` é o caso do
/// WebView2. O erro do sistema não traz texto de reunião nem nome de arquivo (regra 8).
fn error_box(app: &AppHandle, detail: Option<String>) {
    if SHOWING.swap(true, Ordering::SeqCst) {
        return;
    }
    let webview = detail.is_none();
    let text = if let Some(err) = detail {
        format!(
            "O OuveAI não conseguiu abrir a janela.\n\nDetalhe: {err}\n\nFeche o OuveAI pela bandeja e abra de novo. \
             Se continuar, os detalhes ficam no log em {}.",
            ouveai_core::paths::logs_dir().display()
        )
    } else {
        "O OuveAI não conseguiu abrir a janela porque o Microsoft Edge WebView2, que o Windows usa para mostrar \
         as telas do app, não foi encontrado ou está com problema.\n\n\
         Para resolver, instale ou repare o \"Microsoft Edge WebView2 Runtime\" (em Configurações › Aplicativos, \
         ou pelo site da Microsoft) ou peça ajuda ao TI. Depois, abra o OuveAI de novo.\n\n\
         Nenhuma reunião ou transcrição foi perdida."
            .to_string()
    };
    app.dialog()
        .message(text)
        .title(if webview { "OuveAI: falta o WebView2" } else { "OuveAI" })
        .kind(MessageDialogKind::Error)
        .show(|_| SHOWING.store(false, Ordering::SeqCst));
}
const MINI_W: f64 = 420.0;
const MINI_H: f64 = 260.0;

/// Abre (ou foca) a janela principal. `view` escolhe a tela (`settings`, `sessions`).
pub fn open_main(app: &AppHandle, view: Option<&str>) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if let Some(v) = view {
            let _ = w.emit("navigate", v);
        }
        return;
    }
    if !webview_ok(app, "a janela principal") {
        return;
    }
    let st = app.state::<AppState>();
    st.opening.lock().unwrap().insert(MAIN.into(), Instant::now());
    let url = format!("index.html#{}", view.unwrap_or("sessions"));
    let r = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App(url.into()))
        .title("OuveAI")
        .inner_size(960.0, 640.0)
        .min_inner_size(720.0, 480.0)
        .visible(false)
        .content_protected(st.is_stealth())
        .build();
    if let Err(e) = r {
        window_failed(app, "a janela principal", &e);
    }
}

/// Assistente de primeiro uso (abre ao iniciar até ser concluído).
pub fn open_wizard(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(WIZARD) {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    if !webview_ok(app, "o assistente") {
        return;
    }
    let st = app.state::<AppState>();
    st.opening.lock().unwrap().insert(WIZARD.into(), Instant::now());
    let r = WebviewWindowBuilder::new(app, WIZARD, WebviewUrl::App("index.html#wizard".into()))
        .title("Bem-vindo ao OuveAI")
        .inner_size(720.0, 600.0)
        .resizable(false)
        .maximizable(false)
        .center()
        .visible(false)
        .content_protected(st.is_stealth())
        .build();
    if let Err(e) = r {
        window_failed(app, "o assistente", &e);
    }
}

pub fn toggle_mini(app: &AppHandle) {
    if app.get_webview_window(MINI).is_some() {
        hide_mini(app);
    } else {
        show_mini(app);
    }
}

pub fn show_mini(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MINI) {
        ensure_visible(app, &w);
        let _ = w.show();
        return;
    }
    if !webview_ok(app, "a mini") {
        return;
    }
    let st = app.state::<AppState>();
    st.opening.lock().unwrap().insert(MINI.into(), Instant::now());
    let r = WebviewWindowBuilder::new(app, MINI, WebviewUrl::App("index.html#mini".into()))
        .title("OuveAI · mini")
        .inner_size(MINI_W, MINI_H)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        // L20: a mini não rouba o foco do app de reunião (cliques funcionam, teclado não).
        .focused(false)
        .focusable(false)
        .visible(false)
        .content_protected(st.is_stealth())
        .build();
    let w = match r {
        Ok(w) => w,
        Err(e) => {
            window_failed(app, "a mini", &e);
            return;
        }
    };
    let saved = st.setting("mini.pos").and_then(|s| {
        let (x, y) = s.split_once(',')?;
        Some((x.parse::<i32>().ok()?, y.parse::<i32>().ok()?))
    });
    match saved {
        Some((x, y)) => {
            let _ = w.set_position(PhysicalPosition::new(x, y));
            ensure_visible(app, &w);
        }
        None => place_default(app, &w),
    }
    let app2 = app.clone();
    w.on_window_event(move |ev| {
        if let WindowEvent::Moved(p) = ev {
            *app2.state::<AppState>().mini_pos.lock().unwrap() = Some((p.x, p.y));
        }
    });
}

/// As janelas nascem ocultas e só aparecem com a página pronta (sem quadro vazio piscando).
pub fn on_page_load(webview: &Webview, payload: &PageLoadPayload<'_>) {
    if payload.event() != PageLoadEvent::Finished {
        return;
    }
    let app = webview.app_handle();
    let label = webview.label();
    let Some(t0) = app.state::<AppState>().opening.lock().unwrap().remove(label) else { return };
    let w = webview.window();
    let _ = w.show();
    if label == MAIN || label == WIZARD {
        // O 1º ShowWindow do processo pode herdar o modo de início (minimizado). O show() é
        // enfileirado na thread da janela; a checagem vai para a mesma fila, depois dele.
        let w2 = w.clone();
        let _ = app.run_on_main_thread(move || {
            if w2.is_minimized().unwrap_or(false) {
                log::info!("janela main abriu minimizada: restaurando");
                let _ = w2.unminimize();
            }
            let _ = w2.set_focus();
        });
    }
    log::info!("janela {label} pronta em {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
}

pub fn hide_mini(app: &AppHandle) {
    save_mini_pos(app);
    if let Some(w) = app.get_webview_window(MINI) {
        let _ = w.destroy();
    }
}

pub fn save_mini_pos(app: &AppHandle) {
    let st = app.state::<AppState>();
    if let Some((x, y)) = st.mini_pos.lock().unwrap().take() {
        st.set_setting("mini.pos", &format!("{x},{y}"));
    }
}

/// L19: se a mini ficou fora de todos os monitores (monitor desligado, resolução trocada), recentraliza.
fn ensure_visible(app: &AppHandle, w: &WebviewWindow) {
    let (Ok(pos), Ok(size)) = (w.outer_position(), w.outer_size()) else { return };
    let monitors = app.available_monitors().unwrap_or_default();
    // Considera visível se pelo menos 80×40 px da mini caem dentro da área útil de algum monitor.
    let visible = monitors.iter().any(|m| {
        let a = m.work_area();
        let (ax, ay) = (a.position.x, a.position.y);
        let (aw, ah) = (a.size.width as i32, a.size.height as i32);
        let ix = (pos.x + size.width as i32).min(ax + aw) - pos.x.max(ax);
        let iy = (pos.y + size.height as i32).min(ay + ah) - pos.y.max(ay);
        ix >= 80 && iy >= 40
    });
    if !visible {
        log::info!("mini fora dos monitores atuais: recentralizando");
        place_default(app, w);
    }
}

/// Canto inferior direito da área útil do monitor principal, com margem.
fn place_default(app: &AppHandle, w: &WebviewWindow) {
    let Ok(Some(m)) = app.primary_monitor() else { return };
    let a = m.work_area();
    let size = w.outer_size().unwrap_or_default();
    let margin = (24.0 * m.scale_factor()) as i32;
    let x = a.position.x + a.size.width as i32 - size.width as i32 - margin;
    let y = a.position.y + a.size.height as i32 - size.height as i32 - margin;
    let _ = w.set_position(PhysicalPosition::new(x, y));
    *app.state::<AppState>().mini_pos.lock().unwrap() = Some((x, y));
}

/// Aplica a proteção contra captura em todas as janelas abertas (as novas já nascem com ela).
pub fn apply_stealth(app: &AppHandle, on: bool) {
    for (label, w) in app.webview_windows() {
        if let Err(e) = w.set_content_protected(on) {
            log::warn!("stealth em {label}: {e}");
        }
    }
}
