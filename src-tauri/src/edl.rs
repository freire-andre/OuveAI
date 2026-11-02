//! Parte 5: tela "Legendas" (legenda de vídeo editado a partir de um `edl.json`).
//!
//! - Abrir: lê e valida o `edl.json` e os `.json` dos brutos, sem gravar nada.
//! - Gerar: grava o `.srt` do vídeo final.
//! - Verificar: transcreve o vídeo final exportado (`.check.json`, reaproveitado se for mais
//!   novo que o vídeo), alinha com a legenda e propõe correções no `edl.json`.
//! - Aplicar: só com o clique da pessoa; guarda `edl.json.bak` e refaz legenda e verificação.
//! - A verificação usa o Parakeet: não começa com gravação, comparação ou importação em
//!   andamento, e a fila de arquivos espera por ela.
//! - Regra 8: o log leva só números, nunca texto nem nome de arquivo.

use crate::state::AppState;
use ouveai_core::edl::{self, Check, Cut, Fix, Gap};
use ouveai_core::transcribe::{self as tr, Stage};
use ouveai_core::words::Word;
use ouveai_core::{asr, media, subs};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, serde::Serialize)]
pub struct CutView {
    pub file: String,
    #[serde(rename = "in")]
    pub in_: f64,
    pub out: f64,
    pub position: f64,
    /// Palavras do bruto que entram no corte (`None` sem o `.json` do bruto).
    pub words: Option<usize>,
}

/// O `edl.json` aberto: cortes, erros (impedem gerar), avisos e lacunas.
#[derive(Clone, serde::Serialize)]
pub struct EdlView {
    pub path: String,
    pub cuts: Vec<CutView>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub gaps: Vec<Gap>,
}

#[derive(Clone, serde::Serialize)]
pub struct Generated {
    pub srt: String,
    pub cues: usize,
    pub words: usize,
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Phase {
    Loading,
    Working { stage: Stage },
    Done { check: Check, generated: Generated },
    Failed { error: String },
}

#[derive(Clone, serde::Serialize)]
pub struct CheckView {
    pub edl: String,
    pub video: String,
    #[serde(flatten)]
    pub phase: Phase,
}

#[derive(Default)]
pub struct Edl {
    view: Mutex<Option<CheckView>>,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
}

fn set(app: &AppHandle, edl: &str, video: &str, phase: Phase) {
    let v = CheckView { edl: edl.into(), video: video.into(), phase };
    *app.state::<Edl>().view.lock().unwrap() = Some(v.clone());
    let _ = app.emit("edl", Some(v));
}

pub fn view(app: &AppHandle) -> Option<CheckView> {
    app.state::<Edl>().view.lock().unwrap().clone()
}

pub fn running(app: &AppHandle) -> bool {
    app.state::<Edl>().cancel.lock().unwrap().is_some()
}

/// Lê tudo o que é preciso para gerar. Erros de formato, de validação ou `.json` faltando
/// voltam como lista.
type Loaded = (Vec<Cut>, Vec<tr::Transcript>, Vec<Gap>);

fn load(path: &Path) -> Result<Loaded, Vec<String>> {
    let cuts = edl::read(path).map_err(|e| vec![e])?;
    let gaps = edl::validate(&cuts)?;
    let sources = edl::load_sources(path, &cuts)?;
    Ok((cuts, sources, gaps))
}

pub fn open(path: &str) -> EdlView {
    let p = Path::new(path);
    let mut v = EdlView { path: path.into(), cuts: Vec::new(), errors: Vec::new(), warnings: Vec::new(), gaps: Vec::new() };
    let cuts = match edl::read(p) {
        Ok(c) => c,
        Err(e) => {
            v.errors.push(e);
            return v;
        }
    };
    match edl::validate(&cuts) {
        Ok(g) => v.gaps = g,
        Err(e) => v.errors.extend(e),
    }
    let sources: Vec<Option<tr::Transcript>> = cuts.iter().map(|c| tr::read_json(&edl::source_json(p, &c.file)).ok()).collect();
    if let Err(e) = edl::load_sources(p, &cuts) {
        v.errors.extend(e);
    }
    v.cuts = cuts
        .iter()
        .zip(&sources)
        .map(|(c, t)| CutView { file: c.file.clone(), in_: c.in_, out: c.out, position: c.position, words: t.as_ref().map(|t| edl::map_cut(c, t).0.len()) })
        .collect();
    if v.errors.is_empty() {
        let sources: Vec<tr::Transcript> = sources.into_iter().flatten().collect();
        v.warnings = edl::warnings(&cuts, &sources);
    }
    v
}

fn write_srt(path: &Path, video: Option<&Path>, cuts: &[Cut], sources: &[tr::Transcript]) -> Result<(edl::Built, Generated), String> {
    let built = edl::build(cuts, sources);
    let srt = edl::srt_path(path, video);
    std::fs::write(&srt, subs::to_srt(&built.cues)).map_err(|e| format!("não foi possível gravar a legenda: {e}"))?;
    let g = Generated { srt: srt.display().to_string(), cues: built.cues.len(), words: built.words.len() };
    Ok((built, g))
}

pub fn generate(path: &str) -> Result<Generated, String> {
    let p = Path::new(path);
    let (cuts, sources, _) = load(p).map_err(|e| e.join("\n"))?;
    let (_, g) = write_srt(p, None, &cuts, &sources)?;
    log::info!("legenda de EDL gerada: {} cortes, {} blocos", cuts.len(), g.cues);
    Ok(g)
}

fn is_json(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

/// `.check.json` do vídeo final, se existir e for mais novo que o vídeo.
fn cached_check(video: &Path) -> Option<Vec<Word>> {
    let c = tr::check_path(video);
    let newer = c.metadata().and_then(|m| m.modified()).ok()? >= video.metadata().and_then(|m| m.modified()).ok()?;
    if newer { tr::read_json(&c).ok().map(|t| t.words) } else { None }
}

pub fn check_start(app: &AppHandle, path: String, video: String) -> Result<(), String> {
    let st = app.state::<AppState>();
    if st.is_recording() {
        return Err("termine a gravação antes de verificar".into());
    }
    if crate::files::busy(app) {
        return Err("espere a importação de arquivos terminar antes de verificar".into());
    }
    if crate::compare::running(app) {
        return Err("espere a comparação com o Whisper terminar antes de verificar".into());
    }
    let p = PathBuf::from(&path);
    let v = PathBuf::from(&video);
    if !v.is_file() {
        return Err("o vídeo final não foi encontrado".into());
    }
    load(&p).map_err(|e| e.join("\n"))?;
    let e = app.state::<Edl>();
    let cancel = {
        let mut slot = e.cancel.lock().unwrap();
        if slot.is_some() {
            return Err("já há uma verificação em andamento".into());
        }
        let flag = Arc::new(AtomicBool::new(false));
        *slot = Some(flag.clone());
        flag
    };
    set(app, &path, &video, Phase::Loading);
    let app = app.clone();
    std::thread::spawn(move || {
        let t = Instant::now();
        let r = run_check(&app, &p, &v, &cancel);
        *app.state::<Edl>().cancel.lock().unwrap() = None;
        match r {
            Ok((check, generated)) => {
                log::info!(
                    "verificação de EDL em {:.1} s: {} pares, mediano {:.3} s, máximo {:.3} s, {} trechos, {} correções",
                    t.elapsed().as_secs_f64(),
                    check.matched,
                    check.median,
                    check.max,
                    check.issues.len(),
                    check.fixes.len()
                );
                set(&app, &path, &video, Phase::Done { check, generated });
            }
            Err(_) if cancel.load(Ordering::SeqCst) => {
                *app.state::<Edl>().view.lock().unwrap() = None;
                let _ = app.emit("edl", None::<CheckView>);
            }
            Err(error) => {
                log::warn!("verificação de EDL não concluída após {:.1} s", t.elapsed().as_secs_f64());
                set(&app, &path, &video, Phase::Failed { error });
            }
        }
    });
    Ok(())
}

fn run_check(app: &AppHandle, path: &Path, video: &Path, cancel: &AtomicBool) -> Result<(Check, Generated), String> {
    let (cuts, sources, _) = load(path).map_err(|e| e.join("\n"))?;
    let words = if is_json(video) {
        tr::read_json(video).map_err(|e| e.to_string())?.words
    } else if let Some(w) = cached_check(video) {
        log::info!("verificação de EDL: reaproveitando a transcrição do vídeo final");
        w
    } else {
        let (ps, vs) = (path.display().to_string(), video.display().to_string());
        let a = asr::Asr::load(asr::file_threads())?;
        let ff = media::find().ok_or("o FFmpeg não foi encontrado (veja Modelos › Ferramentas)")?;
        let (t, _) = tr::transcribe(video, &ff.path, &a, cancel, &mut |stage| set(app, &ps, &vs, Phase::Working { stage })).map_err(|e| e.to_string())?;
        std::fs::write(tr::check_path(video), tr::to_json(&t)).map_err(|e| format!("não foi possível gravar a verificação: {e}"))?;
        t.words
    };
    let (built, generated) = write_srt(path, (!is_json(video)).then_some(video), &cuts, &sources)?;
    Ok((edl::check(&cuts, &built, &words), generated))
}

pub fn cancel(app: &AppHandle) {
    if let Some(c) = app.state::<Edl>().cancel.lock().unwrap().as_ref() {
        c.store(true, Ordering::SeqCst);
    }
}

pub fn dismiss(app: &AppHandle) {
    if !running(app) {
        *app.state::<Edl>().view.lock().unwrap() = None;
        let _ = app.emit("edl", None::<CheckView>);
    }
}

/// Aplica as correções aprovadas no `edl.json` (cópia em `.bak`) e refaz a legenda e a
/// verificação com a transcrição do vídeo final já feita. Devolve o nome da cópia.
pub fn apply(app: &AppHandle, fixes: Vec<Fix>) -> Result<String, String> {
    if running(app) {
        return Err("espere a verificação terminar".into());
    }
    let v = view(app).ok_or("verifique antes de aplicar")?;
    let (path, video) = (PathBuf::from(&v.edl), PathBuf::from(&v.video));
    let bak = edl::apply_fixes(&path, &fixes)?;
    log::info!("edl.json corrigido: {} campo(s)", fixes.len());
    let words = if is_json(&video) { tr::read_json(&video).map_err(|e| e.to_string())?.words } else { tr::read_json(&tr::check_path(&video)).map_err(|e| e.to_string())?.words };
    let (cuts, sources, _) = load(&path).map_err(|e| e.join("\n"))?;
    let (built, generated) = write_srt(&path, (!is_json(&video)).then_some(video.as_path()), &cuts, &sources)?;
    let check = edl::check(&cuts, &built, &words);
    set(app, &v.edl, &v.video, Phase::Done { check, generated });
    Ok(bak.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default())
}
