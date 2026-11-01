//! OuveAI: app de bandeja (Tauri v2). Fase 1: esqueleto, janelas sob demanda,
//! stealth, atalhos globais, instância única, logs e configurações.
#![windows_subsystem = "windows"]

mod actions;
mod compare;
mod diag;
mod ffmpeg;
mod files;
mod live;
mod models;
mod os;
mod shortcuts;
mod state;
mod tray;
mod windows;

use ouveai_core::{paths, store::Store};
use state::{Action, AppState};
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use tauri::{AppHandle, Emitter, Manager, RunEvent};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

#[tauri::command]
fn get_status(app: AppHandle) -> actions::Status {
    actions::status(&app)
}

#[tauri::command]
fn set_stealth(app: AppHandle, on: bool) {
    actions::set_stealth(&app, on);
}

#[tauri::command]
fn set_shortcut(app: AppHandle, action: Action, accel: String) -> Result<(), String> {
    let r = shortcuts::set(&app, action, accel.trim());
    actions::broadcast(&app);
    r
}

#[tauri::command]
fn reset_shortcuts(app: AppHandle) {
    let st = app.state::<AppState>();
    for a in Action::ALL {
        st.set_setting(&format!("shortcut.{}", a.id()), a.default_accel());
    }
    shortcuts::register_all(&app);
    actions::broadcast(&app);
}

/// Enquanto a tela de Configurações captura uma combinação, os atalhos globais ficam desligados.
#[tauri::command]
fn suspend_shortcuts(app: AppHandle, on: bool) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    if on {
        let _ = app.global_shortcut().unregister_all();
    } else {
        shortcuts::register_all(&app);
        actions::broadcast(&app);
    }
}

#[tauri::command]
async fn toggle_record(app: AppHandle) {
    actions::toggle_record(&app);
}

#[tauri::command]
async fn mark(app: AppHandle) {
    actions::mark(&app);
}

#[tauri::command]
fn hide_mini(app: AppHandle) {
    windows::hide_mini(&app);
}

#[tauri::command]
async fn open_main(app: AppHandle, view: Option<String>) {
    windows::open_main(&app, view.as_deref());
}

#[tauri::command]
fn models_list(app: AppHandle) -> Vec<models::ModelView> {
    models::list(&app)
}

#[tauri::command]
fn model_download(app: AppHandle, id: String, source: ouveai_core::models::Source) -> Result<(), String> {
    models::download(&app, &id, source)
}

#[tauri::command]
fn model_import(app: AppHandle, id: String, path: String) -> Result<(), String> {
    models::import(&app, &id, path.into())
}

#[tauri::command]
fn model_pause(app: AppHandle, id: String) {
    models::pause(&app, &id);
}

#[tauri::command]
fn model_remove(app: AppHandle, id: String) -> Result<(), String> {
    models::remove(&app, &id)
}

#[tauri::command]
async fn ffmpeg_status(app: AppHandle) -> ffmpeg::FfmpegView {
    // async: o probe roda o ffmpeg.exe, não pode travar a thread principal.
    ffmpeg::status(&app)
}

#[tauri::command]
fn ffmpeg_download(app: AppHandle) -> Result<(), String> {
    ffmpeg::download(&app)
}

#[tauri::command]
fn ffmpeg_pause(app: AppHandle) {
    ffmpeg::pause(&app);
}

// --- Importar arquivo (Fase 3) ---
// async: rodam fora da thread principal (probe do FFmpeg, leitura do banco).

#[tauri::command]
async fn file_enqueue(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    files::enqueue(&app, paths)
}

#[tauri::command]
fn file_cancel(app: AppHandle, id: u64) {
    files::cancel(&app, id);
}

#[tauri::command]
fn file_dismiss(app: AppHandle, id: u64) {
    files::dismiss(&app, id);
}

#[tauri::command]
fn file_queue(app: AppHandle) -> files::QueueView {
    files::view(&app)
}

#[tauri::command]
async fn file_sessions(app: AppHandle) -> Result<Vec<ouveai_core::store::SessionRow>, String> {
    files::list(&app)
}

#[tauri::command]
async fn file_detail(app: AppHandle, id: i64) -> Result<files::Detail, String> {
    files::detail(&app, id)
}

#[tauri::command]
async fn file_update_word(app: AppHandle, id: i64, idx: i64, text: String) -> Result<Option<String>, String> {
    files::update_word(&app, id, idx, &text)
}

#[tauri::command]
async fn file_export(app: AppHandle, id: i64, dest: String) -> Result<(), String> {
    files::export(&app, id, &dest)
}

#[tauri::command]
fn compare_start(app: AppHandle, id: i64) -> Result<(), String> {
    compare::start(&app, id)
}

#[tauri::command]
fn compare_state(app: AppHandle) -> Option<compare::CompareView> {
    compare::view(&app)
}

#[tauri::command]
fn compare_cancel(app: AppHandle) {
    compare::cancel(&app)
}

#[tauri::command]
fn compare_dismiss(app: AppHandle) {
    compare::dismiss(&app)
}

#[tauri::command]
async fn compare_apply(app: AppHandle, id: i64, chosen: Vec<usize>) -> Result<Option<String>, String> {
    compare::apply(&app, id, chosen)
}

#[tauri::command]
fn file_reveal(app: AppHandle, id: i64) -> Result<(), String> {
    files::reveal(&app, id)
}

#[tauri::command]
async fn file_delete(app: AppHandle, id: i64) -> Result<(), String> {
    files::delete(&app, id)
}

// --- Reunião (Fase 4) ---

#[tauri::command]
fn live_state(app: AppHandle) -> live::LiveView {
    live::view(&app)
}

#[tauri::command]
async fn live_pause(app: AppHandle) {
    live::pause(&app);
}

#[tauri::command]
async fn live_resume(app: AppHandle) {
    live::resume(&app);
}

#[tauri::command]
fn live_use_mic(app: AppHandle, device_id: String) {
    live::use_mic(&app, device_id);
}

#[tauri::command]
async fn live_retry(app: AppHandle) {
    live::retry(&app);
}

#[tauri::command]
fn live_headphones(app: AppHandle, on: bool) {
    live::set_headphones(&app, on);
}

#[derive(serde::Serialize)]
struct AudioSetup {
    devices: Vec<ouveai_audio::DeviceInfo>,
    apps: Vec<String>,
    mic: ouveai_audio::MicMode,
    outros: ouveai_audio::OutrosMode,
    /// auto · yes · no
    headphones: String,
    headphones_auto: Option<bool>,
    /// none · 7 · 30 · always
    retention: String,
    mic_denied: bool,
    app_loopback: bool,
}

#[tauri::command]
async fn audio_setup(app: AppHandle) -> AudioSetup {
    let st = app.state::<AppState>();
    let cfg = live::config(&st);
    AudioSetup {
        devices: ouveai_audio::list_devices(),
        apps: ouveai_audio::playing_apps(),
        mic: cfg.mic,
        outros: cfg.outros,
        headphones: st.setting("audio.headphones").unwrap_or_else(|| "auto".into()),
        headphones_auto: ouveai_audio::default_output_is_headphone(),
        retention: st.setting("audio.retention").unwrap_or_else(|| "none".into()),
        mic_denied: os::mic_permission() == Some(false),
        // Captura por aplicativo: build 20348 ou mais novo.
        app_loopback: st.build >= 20348,
    }
}

#[tauri::command]
fn audio_set(app: AppHandle, key: String, value: String) -> Result<(), String> {
    let ok = match key.as_str() {
        "mic" => serde_json::from_str::<ouveai_audio::MicMode>(&value).is_ok(),
        "outros" => serde_json::from_str::<ouveai_audio::OutrosMode>(&value).is_ok(),
        "headphones" => ["auto", "yes", "no"].contains(&value.as_str()),
        "retention" => ["none", "7", "30", "always"].contains(&value.as_str()),
        _ => false,
    };
    if !ok {
        return Err("configuração inválida".into());
    }
    app.state::<AppState>().set_setting(&format!("audio.{key}"), &value);
    log::info!("áudio: {key} alterado");
    Ok(())
}

#[tauri::command]
async fn audio_test_start(app: AppHandle) -> Result<(), String> {
    live::start_test(&app)
}

#[tauri::command]
fn audio_test_stop(app: AppHandle) {
    live::stop_test(&app);
}

#[derive(serde::Serialize)]
struct MeetingDetail {
    session: ouveai_core::store::SessionRow,
    segments: Vec<ouveai_core::store::SegmentRow>,
    marks: Vec<ouveai_core::store::MarkRow>,
    audio_dir: Option<String>,
    audio_expires_at: Option<String>,
}

#[tauri::command]
async fn meeting_sessions(app: AppHandle) -> Result<Vec<ouveai_core::store::SessionRow>, String> {
    app.state::<AppState>().store.lock().unwrap().sessions(Some("meeting")).map_err(|e| e.to_string())
}

#[tauri::command]
async fn meeting_detail(app: AppHandle, id: i64) -> Result<MeetingDetail, String> {
    let st = app.state::<AppState>();
    let db = st.store.lock().unwrap();
    let session = db.session(id).map_err(|e| e.to_string())?.ok_or("sessão não encontrada")?;
    let (audio_dir, audio_expires_at) = db.audio(id).map_err(|e| e.to_string())?.map_or((None, None), |(d, e)| (Some(d), e));
    Ok(MeetingDetail {
        session,
        segments: db.segments(id).map_err(|e| e.to_string())?,
        marks: db.marks(id).map_err(|e| e.to_string())?,
        audio_dir,
        audio_expires_at,
    })
}

#[tauri::command]
async fn meeting_rename(app: AppHandle, id: i64, title: String) -> Result<(), String> {
    let t = title.trim();
    if t.is_empty() {
        return Err("o nome não pode ficar vazio".into());
    }
    app.state::<AppState>().store.lock().unwrap().rename_session(id, t).map_err(|e| e.to_string())
}

#[tauri::command]
async fn meeting_delete(app: AppHandle, id: i64) -> Result<(), String> {
    if live::view(&app).session_id == Some(id) {
        return Err("pare a gravação antes de apagar".into());
    }
    let st = app.state::<AppState>();
    let db = st.store.lock().unwrap();
    if let Some((dir, _)) = db.audio(id).map_err(|e| e.to_string())? {
        remove_recording(&dir);
    }
    db.delete_session(id).map_err(|e| e.to_string())
}

/// Só apaga pastas dentro de `recordings` (proteção contra um caminho estranho no banco).
fn remove_recording(dir: &str) {
    let d = std::path::Path::new(dir);
    if d.starts_with(paths::recordings_dir())
        && let Err(e) = std::fs::remove_dir_all(d)
    {
        log::warn!("não foi possível apagar o áudio de uma sessão: {e}");
    }
}

#[tauri::command]
fn meeting_reveal_audio(app: AppHandle, id: i64) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = app.state::<AppState>().store.lock().unwrap().audio(id).map_err(|e| e.to_string())?.ok_or("esta reunião não tem áudio salvo")?.0;
    app.opener().open_path(dir, None::<&str>).map_err(|e| e.to_string())
}

// --- Fase 5: busca, exportar/copiar, ouvir trecho, dados ---

#[tauri::command]
async fn search(app: AppHandle, q: String) -> Result<Vec<ouveai_core::store::Hit>, String> {
    app.state::<AppState>().store.lock().unwrap().search(&q, 200).map_err(|e| e.to_string())
}

/// Monta a reunião e entrega para `f` (que gera o texto no formato pedido).
fn with_meeting<T>(app: &AppHandle, id: i64, f: impl FnOnce(&ouveai_core::export::Meeting) -> T) -> Result<T, String> {
    let st = app.state::<AppState>();
    let db = st.store.lock().unwrap();
    let s = db.session(id).map_err(|e| e.to_string())?.ok_or("sessão não encontrada")?;
    let segments = db.segments(id).map_err(|e| e.to_string())?;
    let marks = db.marks(id).map_err(|e| e.to_string())?;
    let words = db.segment_words(id).map_err(|e| e.to_string())?;
    drop(db);
    Ok(f(&ouveai_core::export::Meeting {
        title: &s.title,
        started_at: &s.started_at,
        duration_s: s.duration_s,
        segments: &segments,
        marks: &marks,
        words: &words,
    }))
}

#[derive(serde::Serialize)]
struct Copied {
    markdown: String,
    html: String,
}

/// `clock`: minutos a leste de UTC para usar a hora do relógio; `None` = tempo relativo.
#[tauri::command]
async fn meeting_copy(app: AppHandle, id: i64, clock: Option<i64>) -> Result<Copied, String> {
    use ouveai_core::export::{Opts, html, markdown};
    let o = Opts { clock };
    with_meeting(&app, id, |m| Copied { markdown: markdown(m, o, false), html: html(m, o) })
}

#[tauri::command]
async fn meeting_export(app: AppHandle, id: i64, dest: String, clock: Option<i64>) -> Result<(), String> {
    use ouveai_core::export as ex;
    let o = ex::Opts { clock };
    let dest = std::path::PathBuf::from(dest);
    let body = match dest.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("md") => with_meeting(&app, id, |m| ex::markdown(m, o, true))?,
        Some("txt") => with_meeting(&app, id, |m| ex::txt(m, o))?,
        Some("srt") => with_meeting(&app, id, ex::srt)?,
        Some("json") => with_meeting(&app, id, ex::json)?,
        _ => return Err("use .md, .txt, .srt ou .json".into()),
    };
    std::fs::write(&dest, body).map_err(|e| e.to_string())
}

/// Trecho do áudio salvo (EU + OUTROS misturados) como WAV, para tocar na interface.
#[tauri::command]
async fn meeting_clip(app: AppHandle, id: i64, start: f64, end: f64) -> Result<tauri::ipc::Response, String> {
    use ouveai_core::wav;
    const RATE: u32 = 16_000;
    let dir = app.state::<AppState>().store.lock().unwrap().audio(id).map_err(|e| e.to_string())?.ok_or("esta reunião não tem áudio salvo")?.0;
    let dir = std::path::PathBuf::from(dir);
    let (a, b) = ((start - 0.25).max(0.0), end.clamp(start, start + 60.0) + 0.25);
    let tracks: Vec<Vec<i16>> = ["eu.wav", "outros.wav"].iter().filter_map(|n| wav::read_clip(&dir.join(n), RATE, a, b).ok()).collect();
    let mixed = wav::mix(&tracks);
    if mixed.is_empty() {
        return Err("o áudio deste trecho não está mais disponível".into());
    }
    Ok(tauri::ipc::Response::new(wav::encode(&mixed, RATE)))
}

fn dir_size(p: &std::path::Path) -> u64 {
    match std::fs::read_dir(p) {
        Ok(rd) => rd
            .flatten()
            .map(|e| match e.file_type() {
                Ok(t) if t.is_dir() => dir_size(&e.path()),
                Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
                Err(_) => 0,
            })
            .sum(),
        Err(_) => 0,
    }
}

#[derive(serde::Serialize)]
struct DataUsage {
    meetings: i64,
    files: i64,
    audio_bytes: u64,
    db_bytes: u64,
    models_bytes: u64,
}

#[tauri::command]
async fn data_usage(app: AppHandle) -> Result<DataUsage, String> {
    let (meetings, files) = app.state::<AppState>().store.lock().unwrap().counts().map_err(|e| e.to_string())?;
    let db = paths::db_path();
    let db_bytes = ["", "-wal", "-shm"].iter().filter_map(|s| std::fs::metadata(format!("{}{s}", db.display())).ok()).map(|m| m.len()).sum();
    Ok(DataUsage { meetings, files, audio_bytes: dir_size(&paths::recordings_dir()), db_bytes, models_bytes: dir_size(&paths::models_dir()) })
}

/// "Excluir tudo": todas as sessões (reuniões e arquivos) e todo o áudio salvo.
/// Configurações, modelos e os arquivos gerados ao lado dos vídeos ficam.
#[tauri::command]
async fn delete_all(app: AppHandle) -> Result<(), String> {
    if live::view(&app).session_id.is_some() {
        return Err("pare a gravação antes de excluir tudo".into());
    }
    if files::busy(&app) {
        return Err("espere a importação de arquivos terminar (ou cancele) antes de excluir tudo".into());
    }
    let dirs = app.state::<AppState>().store.lock().unwrap().delete_all().map_err(|e| e.to_string())?;
    for d in &dirs {
        remove_recording(d);
    }
    // Sobras sem sessão (ex.: áudio de uma gravação que nem chegou a ser registrada).
    let rec = paths::recordings_dir();
    if let Ok(rd) = std::fs::read_dir(&rec) {
        for e in rd.flatten() {
            let _ = if e.path().is_dir() { std::fs::remove_dir_all(e.path()) } else { std::fs::remove_file(e.path()) };
        }
    }
    files::clear_reports(&app);
    log::info!("excluir tudo: sessões apagadas, {} pasta(s) de áudio", dirs.len());
    let _ = app.emit("sessions-changed", ());
    Ok(())
}

/// D2: apaga os áudios vencidos (ao abrir o app e depois de hora em hora).
fn purge_expired(app: &AppHandle) {
    let expired = app.state::<AppState>().store.lock().unwrap().take_expired_audio().unwrap_or_default();
    for d in &expired {
        remove_recording(d);
    }
    if !expired.is_empty() {
        log::info!("{} áudio(s) de reunião vencido(s) apagado(s)", expired.len());
        let _ = app.emit("sessions-changed", ());
    }
}

#[derive(serde::Serialize)]
struct MachineCheck {
    #[serde(flatten)]
    machine: ouveai_core::sys::Machine,
    min_build: u32,
    min_ram_mb: u64,
    min_cores: usize,
    model_mb: u64,
}

#[tauri::command]
fn machine_check() -> MachineCheck {
    use ouveai_core::sys;
    MachineCheck {
        machine: sys::machine(&paths::data_dir()),
        min_build: sys::MIN_BUILD,
        min_ram_mb: sys::MIN_RAM_MB,
        min_cores: sys::MIN_CORES,
        model_mb: ouveai_core::models::space_needed(&ouveai_core::models::parakeet(), ouveai_core::models::Source::Primary)
            / (1024 * 1024),
    }
}

#[tauri::command]
fn get_proxy(app: AppHandle) -> String {
    app.state::<AppState>().setting("net.proxy").unwrap_or_default()
}

#[tauri::command]
fn set_proxy(app: AppHandle, proxy: String) -> Result<(), String> {
    let p = proxy.trim();
    if !p.is_empty() && ureq_proxy_ok(p).is_err() {
        return Err("use o formato http://servidor:porta".into());
    }
    app.state::<AppState>().set_setting("net.proxy", p);
    log::info!("proxy manual {}", if p.is_empty() { "removido" } else { "definido" });
    Ok(())
}

fn ureq_proxy_ok(p: &str) -> Result<(), ()> {
    let rest = p.strip_prefix("http://").or_else(|| p.strip_prefix("https://")).ok_or(())?;
    let (host, port) = rest.trim_end_matches('/').rsplit_once(':').ok_or(())?;
    (!host.is_empty() && port.parse::<u16>().is_ok()).then_some(()).ok_or(())
}

#[tauri::command]
fn open_data_dir(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = paths::data_dir();
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn wizard_finish(app: AppHandle) {
    app.state::<AppState>().set_setting("wizard.done", "true");
    log::info!("assistente de primeiro uso concluído");
    if let Some(w) = app.get_webview_window(windows::WIZARD) {
        let _ = w.destroy();
    }
}

fn main() {
    let build = ouveai_core::sys::windows_build();
    let store = Store::open(&paths::db_path()).expect("falha ao abrir o banco local");
    // Sessões que ficaram gravando quando o app caiu viram `interrupted` (texto preservado).
    // O áudio que ficou recebe a validade configurada (D2).
    let keep = match store.get("audio.retention").ok().flatten().as_deref() {
        Some("7") => Some(7),
        Some("30") => Some(30),
        _ => None,
    };
    let orphans = store.recover_orphans(keep).unwrap_or(0);
    let stealth_supported = build >= 19041;
    // Ligado por padrão; em builds antigos começa desligado (a tela de Configurações avisa).
    let stealth = stealth_supported && store.get("stealth").ok().flatten().is_none_or(|v| v == "true");
    let state = AppState {
        store: Mutex::new(store),
        stealth: AtomicBool::new(stealth),
        stealth_supported,
        build,
        recording: AtomicBool::new(false),
        shortcut_errors: Mutex::new(Vec::new()),
        mini_pos: Mutex::new(None),
        opening: Mutex::new(Default::default()),
    };

    tauri::Builder::default()
        // Tem que ser o primeiro plugin: a segunda instância só avisa a primeira e sai.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("segunda instância: focando a janela principal");
            windows::open_main(app, None);
        }))
        .plugin(
            // Regra 8: os logs nunca levam texto transcrito nem título de sessão.
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .target(Target::new(TargetKind::LogDir { file_name: Some("ouveai".into()) }))
                .level(log::LevelFilter::Info)
                .max_file_size(2 * 1024 * 1024)
                .rotation_strategy(RotationStrategy::KeepSome(5))
                .build(),
        )
        .plugin(shortcuts::plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .manage(models::Models::default())
        .manage(files::Files::default())
        .manage(live::Live::default())
        .manage(compare::Compare::default())
        .invoke_handler(tauri::generate_handler![
            diag::export_diagnostics,
            compare_start,
            compare_state,
            compare_cancel,
            compare_dismiss,
            compare_apply,
            diag::check_update,
            diag::open_release,
            get_status,
            set_stealth,
            set_shortcut,
            reset_shortcuts,
            suspend_shortcuts,
            toggle_record,
            mark,
            hide_mini,
            open_main,
            models_list,
            model_download,
            model_import,
            model_pause,
            model_remove,
            ffmpeg_status,
            ffmpeg_download,
            ffmpeg_pause,
            file_enqueue,
            file_cancel,
            file_dismiss,
            file_queue,
            file_sessions,
            file_detail,
            file_update_word,
            file_export,
            file_reveal,
            file_delete,
            live_state,
            live_pause,
            live_resume,
            live_use_mic,
            live_retry,
            live_headphones,
            audio_setup,
            audio_set,
            audio_test_start,
            audio_test_stop,
            meeting_sessions,
            meeting_detail,
            meeting_rename,
            meeting_delete,
            meeting_reveal_audio,
            meeting_copy,
            meeting_export,
            meeting_clip,
            search,
            data_usage,
            delete_all,
            machine_check,
            get_proxy,
            set_proxy,
            open_data_dir,
            wizard_finish
        ])
        .on_page_load(windows::on_page_load)
        .setup(move |app| {
            log::info!(
                "OuveAI {} · Windows build {build} · stealth {} (suportado: {stealth_supported})",
                app.package_info().version,
                if stealth { "ligado" } else { "desligado" }
            );
            tray::create(app)?;
            shortcuts::register_all(app.handle());
            std::thread::spawn(models::ensure_embedded);
            if app.state::<AppState>().setting("wizard.done").as_deref() != Some("true") {
                windows::open_wizard(app.handle());
            }
            purge_expired(app.handle());
            let h = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                    purge_expired(&h);
                }
            });
            if orphans > 0 {
                log::warn!("{orphans} sessão(ões) interrompida(s) recuperada(s)");
            }
            log::info!("pronto na bandeja em {:.0} ms", os::ms_since_process_start());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("falha ao iniciar o Tauri")
        .run(|app, event| match event {
            // Fechar a última janela não encerra: o app continua na bandeja. Só "Sair" encerra.
            RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            RunEvent::Exit => {
                windows::save_mini_pos(app);
                live::shutdown(app);
            }
            _ => {}
        });
}
