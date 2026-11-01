//! Importar arquivo: fila de transcrição em segundo plano.
//!
//! - Um reconhecedor só: carregado no início do lote e liberado quando a fila
//!   esvazia (são ~700 MB de RAM).
//! - A reunião tem prioridade (L18): com uma gravação em andamento a fila espera, com aviso.
//! - As saídas (`.json`/`.txt`/`.srt`) vão ao lado do arquivo; a sessão vai para o banco.
//! - Regra 8: o log não leva nome de arquivo nem texto, só números.

use crate::state::AppState;
use ouveai_core::asr::{self, Asr};
use ouveai_core::media;
use ouveai_core::store::SessionRow;
use ouveai_core::suspects::{self, Suspect};
use ouveai_core::transcribe::{self as tr, Report, Stage, Transcript};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, serde::Serialize)]
pub struct Item {
    pub id: u64,
    pub name: String,
    pub path: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Phase {
    /// Esperando a gravação da reunião terminar (L18).
    Waiting,
    Loading,
    Working { stage: Stage },
}

#[derive(Clone, Default, serde::Serialize)]
pub struct QueueView {
    pub current: Option<(Item, Phase)>,
    pub pending: Vec<Item>,
    /// Último resultado de cada item (erro ou sessão criada), para a UI avisar.
    pub finished: Vec<Finished>,
}

#[derive(Clone, serde::Serialize)]
pub struct Finished {
    pub item: Item,
    pub session_id: Option<i64>,
    pub error: Option<String>,
    /// Aviso que não impede a sessão (ex.: pasta sem permissão de escrita).
    pub warning: Option<String>,
}

#[derive(Default)]
pub struct Files {
    queue: Mutex<VecDeque<Item>>,
    view: Mutex<QueueView>,
    running: AtomicBool,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    next_id: Mutex<u64>,
    /// Relatório da transcrição por sessão (só enquanto o app está aberto).
    reports: Mutex<HashMap<i64, ReportView>>,
}

#[derive(Clone, serde::Serialize)]
pub struct ReportView {
    pub load_s: f64,
    #[serde(flatten)]
    pub report: Report,
}

fn emit(app: &AppHandle) {
    let v = app.state::<Files>().view.lock().unwrap().clone();
    let _ = app.emit("files", v);
}

fn set_phase(app: &AppHandle, item: &Item, phase: Phase) {
    app.state::<Files>().view.lock().unwrap().current = Some((item.clone(), phase));
    emit(app);
}

/// Erros que a UI trata de um jeito especial.
pub const NEED_FFMPEG: &str = "ffmpeg";

pub fn enqueue(app: &AppHandle, paths: Vec<String>) -> Result<(), String> {
    if media::find().is_none() {
        return Err(NEED_FFMPEG.into());
    }
    let f = app.state::<Files>();
    {
        let mut q = f.queue.lock().unwrap();
        let mut next = f.next_id.lock().unwrap();
        for p in paths {
            let path = PathBuf::from(&p);
            if !path.is_file() {
                continue;
            }
            *next += 1;
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            q.push_back(Item { id: *next, name, path: p });
        }
        f.view.lock().unwrap().pending = q.iter().cloned().collect();
    }
    emit(app);
    if !f.running.swap(true, Ordering::SeqCst) {
        let app = app.clone();
        std::thread::spawn(move || worker(&app));
    }
    Ok(())
}

pub fn cancel(app: &AppHandle, id: u64) {
    let f = app.state::<Files>();
    let mut q = f.queue.lock().unwrap();
    if let Some(pos) = q.iter().position(|i| i.id == id) {
        q.remove(pos);
        f.view.lock().unwrap().pending = q.iter().cloned().collect();
        drop(q);
        emit(app);
        return;
    }
    drop(q);
    let current = f.view.lock().unwrap().current.as_ref().map(|(i, _)| i.id);
    if current == Some(id)
        && let Some(c) = f.cancel.lock().unwrap().as_ref()
    {
        c.store(true, Ordering::Relaxed);
    }
}

pub fn view(app: &AppHandle) -> QueueView {
    app.state::<Files>().view.lock().unwrap().clone()
}

pub fn dismiss(app: &AppHandle, id: u64) {
    app.state::<Files>().view.lock().unwrap().finished.retain(|f| f.item.id != id);
    emit(app);
}

fn worker(app: &AppHandle) {
    let f = app.state::<Files>();
    let mut asr: Option<(Asr, f64)> = None;
    let pop = || {
        let mut q = f.queue.lock().unwrap();
        let it = q.pop_front();
        f.view.lock().unwrap().pending = q.iter().cloned().collect();
        it
    };
    while let Some(item) = pop() {
        let cancel = Arc::new(AtomicBool::new(false));
        *f.cancel.lock().unwrap() = Some(cancel.clone());

        // L18: a reunião tem prioridade; a importação espera.
        let mut waited = false;
        while app.state::<AppState>().is_recording() && !cancel.load(Ordering::Relaxed) {
            if !waited {
                log::info!("importação aguardando a reunião terminar");
                set_phase(app, &item, Phase::Waiting);
                waited = true;
            }
            std::thread::sleep(Duration::from_millis(500));
        }

        let result = (|| -> Result<(i64, Option<String>), String> {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelado".into());
            }
            if asr.is_none() {
                set_phase(app, &item, Phase::Loading);
                let t = Instant::now();
                let a = Asr::load(asr::file_threads())?;
                let load_s = t.elapsed().as_secs_f64();
                log::info!("modelo carregado em {load_s:.2} s");
                asr = Some((a, load_s));
            }
            let (a, load_s) = asr.as_ref().unwrap();
            let ff = media::find().ok_or("FFmpeg não encontrado")?;
            let input = Path::new(&item.path);
            let (t, report) = tr::transcribe(input, &ff.path, a, &cancel, &mut |stage| set_phase(app, &item, Phase::Working { stage }))
                .map_err(|e| e.to_string())?;
            set_phase(app, &item, Phase::Working { stage: Stage::Saving });
            log::info!(
                "arquivo transcrito: {:.0} s de áudio em {:.1} s ({:.1}× o tempo real), {} pedaço(s), {} corte(s) forçado(s)",
                report.duration,
                report.transcribe_s,
                report.rtf,
                report.chunks,
                report.forced_cuts.len()
            );
            // Ao lado da entrada; se a pasta não deixar gravar, a sessão fica só no app.
            let warning = tr::write_all(&t, &tr::outputs_for(input))
                .err()
                .map(|e| format!("não foi possível gravar .json/.txt/.srt ao lado do arquivo ({e}); use Exportar"));
            let title = input.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| item.name.clone());
            let id = app
                .state::<AppState>()
                .store
                .lock()
                .unwrap()
                .create_file_session(&title, input, &t, env!("CARGO_PKG_VERSION"))
                .map_err(|e| format!("erro ao salvar no banco: {e}"))?;
            f.reports.lock().unwrap().insert(id, ReportView { load_s: *load_s, report });
            Ok((id, warning))
        })();

        let finished = match result {
            Ok((id, warning)) => Finished { item: item.clone(), session_id: Some(id), error: None, warning },
            Err(e) => {
                if e != "cancelado" {
                    log::warn!("importação falhou: {e}");
                }
                Finished { item: item.clone(), session_id: None, error: Some(e), warning: None }
            }
        };
        {
            let mut v = f.view.lock().unwrap();
            v.current = None;
            v.finished.retain(|x| x.item.id != item.id);
            v.finished.push(finished.clone());
        }
        emit(app);
        let _ = app.emit("file-done", finished);
    }
    *f.cancel.lock().unwrap() = None;
    f.running.store(false, Ordering::SeqCst);
    drop(asr); // libera o reconhecedor (~700 MB)
    // Algo pode ter entrado na fila entre o fim do laço e o `running = false`.
    if !f.queue.lock().unwrap().is_empty() && !f.running.swap(true, Ordering::SeqCst) {
        let app = app.clone();
        std::thread::spawn(move || worker(&app));
    }
}

// ---------------------------------------------------------------------------
// Sessões de arquivo (lista, editor, exportar)

#[derive(serde::Serialize)]
pub struct Detail {
    pub session: SessionRow,
    pub transcript: Transcript,
    pub suspects: Vec<Suspect>,
    pub suspects_per_min: f64,
    pub report: Option<ReportView>,
    /// Os arquivos ao lado da entrada existem (para "Abrir pasta").
    pub outputs_exist: bool,
}

pub fn list(app: &AppHandle) -> Result<Vec<SessionRow>, String> {
    app.state::<AppState>().store.lock().unwrap().sessions(Some("file")).map_err(|e| e.to_string())
}

pub fn detail(app: &AppHandle, id: i64) -> Result<Detail, String> {
    let store = app.state::<AppState>();
    let store = store.store.lock().unwrap();
    let session = store.session(id).map_err(|e| e.to_string())?.ok_or("sessão não encontrada")?;
    let transcript = store.transcript(id).map_err(|e| e.to_string())?.ok_or("sessão não encontrada")?;
    drop(store);
    let suspects = suspects::find(&transcript.words);
    let suspects_per_min = suspects.len() as f64 / (transcript.duration / 60.0).max(1.0 / 60.0);
    let outputs_exist = session.source_path.as_deref().is_some_and(|p| tr::outputs_for(Path::new(p)).srt.exists());
    let report = app.state::<Files>().reports.lock().unwrap().get(&id).cloned();
    Ok(Detail { session, transcript, suspects, suspects_per_min, report, outputs_exist })
}

/// Corrige o texto de uma palavra (nunca os tempos) e regenera os arquivos ao lado da entrada.
pub fn update_word(app: &AppHandle, id: i64, idx: i64, text: &str) -> Result<Option<String>, String> {
    let text = text.trim();
    if text.is_empty() || text.contains(char::is_whitespace) {
        return Err("a palavra não pode ficar vazia nem ter espaço (corrija uma palavra por vez)".into());
    }
    let st = app.state::<AppState>();
    let mut store = st.store.lock().unwrap();
    if !store.update_word(id, idx, text).map_err(|e| e.to_string())? {
        return Err("palavra não encontrada".into());
    }
    let t = store.transcript(id).map_err(|e| e.to_string())?.ok_or("sessão não encontrada")?;
    let src = store.session(id).map_err(|e| e.to_string())?.and_then(|s| s.source_path);
    drop(store);
    // Regenera só se os arquivos já estavam lá (equivale ao --from-json).
    let warning = match src.map(PathBuf::from) {
        Some(p) if tr::outputs_for(&p).json.exists() => tr::write_all(&t, &tr::outputs_for(&p)).err().map(|e| format!("não foi possível atualizar os arquivos: {e}")),
        _ => None,
    };
    Ok(warning)
}

pub fn export(app: &AppHandle, id: i64, dest: &str) -> Result<(), String> {
    let t = app.state::<AppState>().store.lock().unwrap().transcript(id).map_err(|e| e.to_string())?.ok_or("sessão não encontrada")?;
    let dest = Path::new(dest);
    let body = match dest.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("srt") => ouveai_core::subs::to_srt(&ouveai_core::subs::segment(&t.words, &t.pauses)),
        Some("txt") => ouveai_core::subs::to_txt(&t.words, &t.pauses),
        Some("json") => tr::to_json(&t),
        _ => return Err("use .srt, .txt ou .json".into()),
    };
    std::fs::write(dest, body).map_err(|e| e.to_string())
}

pub fn reveal(app: &AppHandle, id: i64) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let src = app.state::<AppState>().store.lock().unwrap().session(id).map_err(|e| e.to_string())?.and_then(|s| s.source_path).ok_or("sem arquivo de origem")?;
    let srt = tr::outputs_for(Path::new(&src)).srt;
    let target = if srt.exists() { srt } else { PathBuf::from(src) };
    app.opener().reveal_item_in_dir(target).map_err(|e| e.to_string())
}

/// Há importação na fila ou em andamento.
pub fn busy(app: &AppHandle) -> bool {
    let f = app.state::<Files>();
    f.running.load(Ordering::SeqCst) || !f.queue.lock().unwrap().is_empty()
}

/// Depois do "Excluir tudo": os relatórios em memória apontam para sessões que não existem mais.
pub fn clear_reports(app: &AppHandle) {
    app.state::<Files>().reports.lock().unwrap().clear();
}

pub fn delete(app: &AppHandle, id: i64) -> Result<(), String> {
    app.state::<AppState>().store.lock().unwrap().delete_session(id).map_err(|e| e.to_string())?;
    app.state::<Files>().reports.lock().unwrap().remove(&id);
    Ok(())
}
