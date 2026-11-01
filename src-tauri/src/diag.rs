//! "Exportar diagnóstico" e "Verificar atualização" (tela Sobre).

use crate::state::AppState;
use ouveai_core::{diag, models, paths, sys, update};
use std::fmt::Write;
use tauri::{AppHandle, Manager};

fn version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn yes(b: bool) -> &'static str {
    if b { "sim" } else { "não" }
}

/// Relatório em texto: só máquina, dispositivos, contagens e configurações.
fn report(app: &AppHandle) -> String {
    let st = app.state::<AppState>();
    let m = sys::machine(&paths::data_dir());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut r = String::new();
    let _ = writeln!(r, "OuveAI {} · diagnóstico de {}", version(app), diag::utc_stamp(now));
    let _ = writeln!(r, "Sem texto transcrito, áudio, títulos ou nomes de arquivo.\n");

    let _ = writeln!(r, "[Máquina]");
    let _ = writeln!(r, "Windows: {}", sys::windows_version());
    let _ = writeln!(r, "Processador: {} · {} threads · AVX2 {}", sys::cpu_name(), m.cores, yes(m.avx2));
    let _ = writeln!(r, "RAM: {} MB no total, {} MB livres", m.ram_total_mb, m.ram_free_mb);
    let _ = writeln!(r, "Disco livre na pasta de dados: {} MB", m.disk_free_mb);
    let _ = writeln!(r, "Invisível no compartilhamento disponível: {}", yes(st.stealth_supported));
    let mic = match crate::os::mic_permission() {
        Some(true) => "permitido",
        Some(false) => "BLOQUEADO",
        None => "sem registro",
    };
    let _ = writeln!(r, "Permissão do microfone (Windows): {mic}\n");

    let _ = writeln!(r, "[OuveAI]");
    for spec in models::catalog() {
        let state = match models::presence(&spec) {
            models::Presence::Ready => "pronto".to_string(),
            models::Presence::Absent => "ausente".to_string(),
            models::Presence::Corrupt(why) => format!("com defeito ({why})"),
        };
        let _ = writeln!(r, "Modelo {}: {state}", spec.id);
    }
    let _ = writeln!(r, "FFmpeg: {}", if ouveai_core::media::find().is_some() { "encontrado" } else { "ausente" });
    let _ = writeln!(r, "Gravando agora: {}", yes(st.is_recording()));
    if let Ok((meetings, files)) = st.store.lock().unwrap().counts() {
        let _ = writeln!(r, "Sessões: {meetings} reuniões, {files} arquivos");
    }
    let mb = |b: u64| b / (1024 * 1024);
    let _ = writeln!(
        r,
        "Espaço: áudio {} MB, modelos {} MB",
        mb(crate::dir_size(&paths::recordings_dir())),
        mb(crate::dir_size(&paths::models_dir()))
    );
    for (a, e) in st.shortcut_errors.lock().unwrap().iter() {
        let _ = writeln!(r, "Atalho {a:?} com erro: {e}");
    }

    let _ = writeln!(r, "\n[Dispositivos de áudio]");
    for d in ouveai_audio::device::list_devices() {
        let _ = writeln!(
            r,
            "{} {}{} · {} · {}{}",
            if d.input { "entrada" } else { "saída  " },
            if d.default { "(padrão) " } else { "" },
            d.name,
            d.form,
            if d.bluetooth { "Bluetooth · " } else { "" },
            d.id
        );
    }

    let _ = writeln!(r, "\n[Configurações]");
    if let Ok(all) = st.store.lock().unwrap().all() {
        for (k, v) in all {
            let _ = writeln!(r, "{k} = {}", diag::redact_setting(&k, &v));
        }
    }
    r.replace('\n', "\r\n")
}

#[tauri::command]
pub async fn export_diagnostics(app: AppHandle, dest: String) -> Result<(), String> {
    let mut entries = vec![("relatorio.txt".to_string(), report(&app).into_bytes())];
    entries.extend(diag::log_files(&paths::data_dir().join("logs")));
    diag::write_zip(std::path::Path::new(&dest), &entries).map_err(|e| format!("não consegui salvar o arquivo: {e}"))?;
    log::info!("diagnóstico exportado ({} arquivos)", entries.len());
    Ok(())
}

#[derive(serde::Serialize)]
pub struct UpdateInfo {
    current: String,
    /// `None` = nenhuma versão publicada ainda.
    latest: Option<update::Release>,
    newer: bool,
}

/// Consulta o GitHub Releases. Só roda quando a pessoa clica (D5).
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateInfo, String> {
    let current = version(&app);
    let cfg = ouveai_core::net::NetConfig { proxy: app.state::<AppState>().setting("net.proxy") };
    let cur = current.clone();
    let latest = tauri::async_runtime::spawn_blocking(move || update::latest(&cfg, &cur))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| {
            log::warn!("verificação de atualização falhou: {e}");
            match e {
                ouveai_core::net::NetError::Net(_) => {
                    "Sem conexão com o GitHub. Confira a internet ou o proxy (Configurações › Rede).".to_string()
                }
                e => format!("Não consegui verificar: {e}."),
            }
        })?;
    let newer = latest.as_ref().is_some_and(|l| update::is_newer(&l.version, &current));
    Ok(UpdateInfo { current, latest, newer })
}

/// Abre a página da versão no navegador. Só aceita o repositório do OuveAI.
#[tauri::command]
pub fn open_release(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    if !url.starts_with(&format!("https://github.com/{}/", update::REPO)) {
        return Err("link inesperado".into());
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}
