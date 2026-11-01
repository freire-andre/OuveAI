//! FFmpeg: detectado no PATH ou baixado sob demanda (só para importar arquivos).
//! Usa a mesma fila de trabalhos dos modelos, com a chave "ffmpeg".

use crate::models::{self, Job, Models};
use ouveai_core::media::{self, Ffmpeg, PACKAGE};
use ouveai_core::models::{InstallError, Step};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

const ID: &str = "ffmpeg";

#[derive(Clone, serde::Serialize)]
pub struct PackageView {
    pub version: &'static str,
    pub size: u64,
    pub license: &'static str,
    /// Só o host + caminho do projeto, para a confirmação ("de onde vem").
    pub source: &'static str,
}

#[derive(Clone, serde::Serialize)]
pub struct FfmpegView {
    pub found: Option<Ffmpeg>,
    /// absent · downloading · paused · verifying · extracting · ready
    pub state: &'static str,
    pub step: Option<Step>,
    pub partial: u64,
    pub error: Option<String>,
    pub package: PackageView,
}

pub fn status(app: &AppHandle) -> FfmpegView {
    let m = app.state::<Models>();
    let step = m.jobs.lock().unwrap().get(ID).map(|j| j.step);
    let error = m.errors.lock().unwrap().get(ID).cloned();
    let partial = media::partial_bytes();
    // Durante o download não roda o probe (o exe ainda não existe ou está sendo trocado).
    let found = if step.is_some() { None } else { media::find() };
    let state = match (step, &found) {
        (Some(Some(Step::Verifying { .. })), _) => "verifying",
        (Some(Some(Step::Extracting { .. })), _) => "extracting",
        (Some(_), _) => "downloading",
        (None, Some(_)) => "ready",
        (None, None) if partial > 0 => "paused",
        (None, None) => "absent",
    };
    FfmpegView {
        found,
        state,
        step: step.flatten(),
        partial,
        error,
        package: PackageView {
            version: PACKAGE.version,
            size: PACKAGE.size,
            license: PACKAGE.license,
            source: "github.com/GyanD/codexffmpeg (gyan.dev)",
        },
    }
}

fn broadcast(app: &AppHandle) {
    let _ = app.emit("ffmpeg", status(app));
}

pub fn download(app: &AppHandle) -> Result<(), String> {
    let m = app.state::<Models>();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut jobs = m.jobs.lock().unwrap();
        if jobs.contains_key(ID) {
            return Err("já está em andamento".into());
        }
        jobs.insert(ID.into(), Job { cancel: cancel.clone(), step: None });
    }
    m.errors.lock().unwrap().remove(ID);
    broadcast(app);

    let app = app.clone();
    std::thread::spawn(move || {
        log::info!("ffmpeg {}: download iniciado", PACKAGE.version);
        let cfg = models::net_config(&app);
        let mut last = std::mem::discriminant(&Step::Verifying { done: 0, total: 0 });
        let r = media::download(&cfg, &cancel, &mut |s| {
            if let Some(j) = app.state::<Models>().jobs.lock().unwrap().get_mut(ID) {
                j.step = Some(s);
            }
            if std::mem::discriminant(&s) != last {
                last = std::mem::discriminant(&s);
                broadcast(&app);
            }
            let _ = app.emit("ffmpeg-progress", s);
        });
        let m = app.state::<Models>();
        m.jobs.lock().unwrap().remove(ID);
        match r {
            Ok(f) => log::info!("ffmpeg pronto: {}", f.version),
            Err(InstallError::Cancelled) => log::info!("ffmpeg: pausado"),
            Err(e) => {
                log::warn!("ffmpeg: {e}");
                m.errors.lock().unwrap().insert(ID.into(), e.to_string());
            }
        }
        broadcast(&app);
    });
    Ok(())
}

pub fn pause(app: &AppHandle) {
    if let Some(j) = app.state::<Models>().jobs.lock().unwrap().get(ID) {
        j.cancel.store(true, Ordering::Relaxed);
    }
}
