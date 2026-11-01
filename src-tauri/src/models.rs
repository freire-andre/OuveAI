//! Tela de modelos: estado de cada modelo, download/importação em segundo plano com
//! progresso por evento, pausa (o `.part` fica para retomar).

use crate::state::AppState;
use ouveai_core::models::{self, InstallError, Presence, Source, Step};
use ouveai_core::net::NetConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

pub(crate) struct Job {
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) step: Option<Step>,
}

#[derive(Default)]
pub struct Models {
    pub(crate) jobs: Mutex<HashMap<String, Job>>,
    pub(crate) errors: Mutex<HashMap<String, String>>,
}

#[derive(Clone, serde::Serialize)]
pub struct ModelView {
    pub id: String,
    pub name: String,
    pub license: String,
    pub size: u64,
    pub embedded: bool,
    /// absent · downloading · paused · verifying · extracting · ready · corrupt
    pub state: &'static str,
    pub detail: Option<String>,
    pub step: Option<Step>,
    pub partial: u64,
    pub error: Option<String>,
    /// URL do pacote para o plano B "Baixar pelo navegador".
    pub browser_url: Option<String>,
    pub has_mirror: bool,
    /// Para que serve (modelos opcionais).
    pub note: Option<String>,
}

pub fn list(app: &AppHandle) -> Vec<ModelView> {
    let m = app.state::<Models>();
    let jobs = m.jobs.lock().unwrap();
    let errors = m.errors.lock().unwrap();
    models::catalog()
        .into_iter()
        .map(|spec| {
            let job = jobs.get(&spec.id);
            let partial = models::partial_bytes(&spec);
            let (state, detail) = match (job, models::presence(&spec)) {
                (Some(j), _) => (
                    match j.step {
                        Some(Step::Verifying { .. }) => "verifying",
                        Some(Step::Extracting { .. }) => "extracting",
                        _ => "downloading",
                    },
                    None,
                ),
                (None, Presence::Ready) => ("ready", None),
                (None, Presence::Corrupt(d)) => ("corrupt", Some(d)),
                (None, Presence::Absent) if partial > 0 => ("paused", None),
                (None, Presence::Absent) => ("absent", None),
            };
            ModelView {
                state,
                detail,
                step: job.and_then(|j| j.step),
                partial,
                error: errors.get(&spec.id).cloned(),
                browser_url: spec.archive.as_ref().map(|a| a.url.clone()),
                has_mirror: spec.mirror_base.is_some(),
                size: spec.files_size(),
                embedded: spec.embedded,
                id: spec.id,
                name: spec.name,
                license: spec.license,
                note: spec.note,
            }
        })
        .collect()
}

pub fn broadcast(app: &AppHandle) {
    let _ = app.emit("models", list(app));
}

pub fn net_config(app: &AppHandle) -> NetConfig {
    NetConfig { proxy: app.state::<AppState>().setting("net.proxy").filter(|p| !p.trim().is_empty()) }
}

enum Work {
    Download(Source),
    Import(PathBuf),
}

fn start(app: &AppHandle, id: &str, work: Work) -> Result<(), String> {
    let spec = models::by_id(id).ok_or_else(|| format!("modelo desconhecido: {id}"))?;
    let m = app.state::<Models>();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut jobs = m.jobs.lock().unwrap();
        if jobs.contains_key(id) {
            return Err("já está em andamento".into());
        }
        jobs.insert(id.into(), Job { cancel: cancel.clone(), step: None });
    }
    m.errors.lock().unwrap().remove(id);
    broadcast(app);

    let app = app.clone();
    let id = id.to_string();
    std::thread::spawn(move || {
        let cfg = net_config(&app);
        let mut last_state = "";
        let mut on = |s: Step| {
            let st = match s {
                Step::Downloading { .. } => "downloading",
                Step::Verifying { .. } => "verifying",
                Step::Extracting { .. } => "extracting",
            };
            if let Some(j) = app.state::<Models>().jobs.lock().unwrap().get_mut(&id) {
                j.step = Some(s);
            }
            if st != last_state {
                last_state = st;
                broadcast(&app);
            }
            let _ = app.emit("model-progress", (&id, s));
        };
        let r = match &work {
            Work::Download(source) => {
                log::info!("modelo {id}: download iniciado ({source:?})");
                models::download(&spec, &cfg, *source, &cancel, &mut on)
            }
            Work::Import(path) => {
                // Regra 8: só o tipo da origem, não o caminho.
                log::info!("modelo {id}: importação iniciada ({})", if path.is_dir() { "pasta" } else { "arquivo" });
                models::import(&spec, path, &cancel, &mut on)
            }
        };
        let m = app.state::<Models>();
        m.jobs.lock().unwrap().remove(&id);
        match r {
            Ok(()) => log::info!("modelo {id}: pronto"),
            Err(InstallError::Cancelled) => log::info!("modelo {id}: pausado"),
            Err(e) => {
                log::warn!("modelo {id}: {e}");
                m.errors.lock().unwrap().insert(id.clone(), e.to_string());
            }
        }
        broadcast(&app);
    });
    Ok(())
}

pub fn download(app: &AppHandle, id: &str, source: Source) -> Result<(), String> {
    start(app, id, Work::Download(source))
}

/// Importar com um download em andamento (o plano B fica visível durante as
/// tentativas): cancela o download e importa assim que ele terminar.
pub fn import(app: &AppHandle, id: &str, path: PathBuf) -> Result<(), String> {
    let running = app.state::<Models>().jobs.lock().unwrap().contains_key(id);
    if !running {
        return start(app, id, Work::Import(path));
    }
    pause(app, id);
    let (app, id) = (app.clone(), id.to_string());
    std::thread::spawn(move || {
        // O cancelamento pode levar até o tempo limite de conexão (20 s).
        while app.state::<Models>().jobs.lock().unwrap().contains_key(&id) {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if let Err(e) = start(&app, &id, Work::Import(path)) {
            app.state::<Models>().errors.lock().unwrap().insert(id, e);
            broadcast(&app);
        }
    });
    Ok(())
}

pub fn pause(app: &AppHandle, id: &str) {
    if let Some(j) = app.state::<Models>().jobs.lock().unwrap().get(id) {
        j.cancel.store(true, Ordering::Relaxed);
    }
}

pub fn remove(app: &AppHandle, id: &str) -> Result<(), String> {
    let spec = models::by_id(id).ok_or("modelo desconhecido")?;
    if spec.embedded {
        return Err("este modelo vem com o app".into());
    }
    if app.state::<Models>().jobs.lock().unwrap().contains_key(id) {
        return Err("pause antes de apagar".into());
    }
    models::remove(&spec).map_err(|e| e.to_string())?;
    app.state::<Models>().errors.lock().unwrap().remove(id);
    log::info!("modelo {id}: apagado");
    broadcast(app);
    Ok(())
}

/// O Silero vem dentro do exe; garante a cópia na pasta de modelos ao iniciar.
pub fn ensure_embedded() {
    for spec in models::catalog().into_iter().filter(|m| m.embedded) {
        if let Err(e) = models::ensure_embedded(&spec) {
            log::error!("modelo embutido {}: {e}", spec.id);
        }
    }
}
