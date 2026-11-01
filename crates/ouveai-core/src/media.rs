//! FFmpeg: só para importar arquivos de áudio/vídeo (a reunião não precisa).
//! Usa o do PATH; se não houver, o baixado em `bin\ffmpeg.exe`.

use crate::models::{self, InstallError, Step};
use crate::net::{self, NetConfig, Progress};
use crate::paths;
use std::fs;
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

/// Sem isso o Windows abre (e pisca) uma janela de console a cada chamada.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Path,
    Local,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Ffmpeg {
    pub path: PathBuf,
    /// Primeira linha de `ffmpeg -version`, ex.: "ffmpeg version 8.1-full_build-www.gyan.dev".
    pub version: String,
    pub origin: Origin,
}

/// `Command` já configurado para rodar sem janela de console.
pub fn command(exe: &Path) -> Command {
    let mut c = Command::new(exe);
    c.creation_flags(CREATE_NO_WINDOW).stdin(Stdio::null());
    c
}

fn probe(exe: &Path) -> Option<String> {
    let out = command(exe).arg("-hide_banner").arg("-version").stderr(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let first = String::from_utf8_lossy(&out.stdout).lines().next()?.trim().to_string();
    first.starts_with("ffmpeg version").then_some(first)
}

fn in_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join("ffmpeg.exe")).find(|p| p.is_file())
}

pub fn local_path() -> PathBuf {
    paths::bin_dir().join("ffmpeg.exe")
}

/// Procura um FFmpeg que funcione: primeiro o do PATH, depois o local.
pub fn find() -> Option<Ffmpeg> {
    let candidates = [in_path().map(|p| (p, Origin::Path)), Some((local_path(), Origin::Local))];
    candidates.into_iter().flatten().filter(|(p, _)| p.is_file()).find_map(|(path, origin)| {
        let version = probe(&path)?;
        Some(Ffmpeg { path, version, origin })
    })
}

/// Build estática da gyan.dev, release fixa no GitHub (o mesmo fornecedor do `winget install Gyan.FFmpeg`).
/// Licença GPL; o texto vai junto para `bin\FFMPEG-LICENSE.txt`.
pub struct Package {
    pub version: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub license: &'static str,
}

pub const PACKAGE: Package = Package {
    version: "9.0.2",
    url: "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip",
    size: 114_768_076,
    sha256: "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba",
    license: "GPL-3.0",
};

fn zip_part() -> PathBuf {
    paths::bin_dir().join("_downloads").join("ffmpeg.zip.part")
}

/// Bytes já baixados (para "continuar de onde parou").
pub fn partial_bytes() -> u64 {
    fs::metadata(zip_part()).map(|m| m.len()).unwrap_or(0)
}

/// Baixa o pacote, confere o SHA-256 e extrai só o `ffmpeg.exe` (e a licença) para `bin\`.
pub fn download(cfg: &NetConfig, cancel: &AtomicBool, on: &mut dyn FnMut(Step)) -> Result<Ffmpeg, InstallError> {
    let p = &PACKAGE;
    let part = zip_part();
    // Pacote + o exe extraído (~ do mesmo tamanho).
    models::check_space((p.size - partial_bytes().min(p.size)) + p.size)?;
    net::download(cfg, p.url, &part, p.size, cancel, &mut |pr: Progress| {
        on(Step::Downloading { done: pr.done, total: pr.total, bytes_per_sec: pr.bytes_per_sec, retry: pr.retry })
    })?;
    let verified = models::verify_one(&part, p.size, p.sha256, &mut |d| on(Step::Verifying { done: d, total: p.size }));
    // Completo (bom ou adulterado): o ETag não serve mais para retomar.
    fs::remove_file(part.with_extension("part.etag")).ok();
    if verified.is_err() {
        fs::remove_file(&part).ok();
    }
    verified?;
    let r = install_zip(&part, cancel, on);
    if !matches!(r, Err(InstallError::Cancelled)) {
        fs::remove_file(&part).ok();
        if let Some(dir) = part.parent() {
            fs::remove_dir(dir).ok(); // só se estiver vazia
        }
    }
    r?;
    let path = local_path();
    let version = probe(&path).ok_or_else(|| InstallError::Invalid("o ffmpeg.exe baixado não executou".into()))?;
    Ok(Ffmpeg { path, version, origin: Origin::Local })
}

/// Extrai `*/bin/ffmpeg.exe` e `*/LICENSE` do zip; troca o exe de forma atômica.
fn install_zip(zip_path: &Path, cancel: &AtomicBool, on: &mut dyn FnMut(Step)) -> Result<(), InstallError> {
    let bad = |e: zip::result::ZipError| InstallError::Invalid(format!("pacote do FFmpeg inválido: {e}"));
    let mut zip = zip::ZipArchive::new(fs::File::open(zip_path)?).map_err(bad)?;
    let names: Vec<String> = zip.file_names().filter_map(|n| n.ok().map(|n| n.into_owned())).collect();
    let exe = names
        .iter()
        .find(|n| n.ends_with("/bin/ffmpeg.exe"))
        .ok_or_else(|| InstallError::Invalid("o pacote não tem bin/ffmpeg.exe".into()))?;
    let bin = paths::bin_dir();
    fs::create_dir_all(&bin)?;
    let tmp = bin.join("ffmpeg.exe.tmp");

    let mut entry = zip.by_name(exe).map_err(bad)?;
    let total = entry.size();
    let mut out = fs::File::create(&tmp)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    let copy = (|| -> Result<(), InstallError> {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(InstallError::Cancelled);
            }
            // O leitor do zip confere o CRC-32 ao chegar no fim da entrada.
            let n = entry.read(&mut buf)?;
            if n == 0 {
                return Ok(());
            }
            out.write_all(&buf[..n])?;
            done += n as u64;
            on(Step::Extracting { done, total });
        }
    })();
    drop(out);
    drop(entry);
    if let Err(e) = copy {
        fs::remove_file(&tmp).ok();
        return Err(e);
    }
    fs::rename(&tmp, local_path())?;

    if let Some(lic) = names.iter().find(|n| n.matches('/').count() == 1 && n.ends_with("/LICENSE")) {
        let mut text = String::new();
        if zip.by_name(lic).map_err(bad)?.read_to_string(&mut text).is_ok() {
            fs::write(bin.join("FFMPEG-LICENSE.txt"), text)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Conversão e análise do áudio (Fase 3)

/// Erros da conversão/análise com o FFmpeg.
#[derive(Debug)]
pub enum MediaError {
    NoFfmpeg,
    Cancelled,
    /// O FFmpeg falhou; leva as últimas linhas do stderr.
    Failed(String),
    Io(std::io::Error),
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaError::NoFfmpeg => write!(f, "FFmpeg não encontrado"),
            MediaError::Cancelled => write!(f, "cancelado"),
            MediaError::Failed(e) => write!(f, "o FFmpeg não conseguiu ler o arquivo: {e}"),
            MediaError::Io(e) => write!(f, "erro de disco: {e}"),
        }
    }
}

impl From<std::io::Error> for MediaError {
    fn from(e: std::io::Error) -> Self {
        MediaError::Io(e)
    }
}

/// Roda o FFmpeg sem console, lendo o stderr numa thread (para o pipe não
/// travar) e matando o processo se `cancel` for ligado.
fn run(exe: &Path, args: &[&std::ffi::OsStr], cancel: &AtomicBool) -> Result<String, MediaError> {
    let mut child = command(exe).args(args).stdout(Stdio::null()).stderr(Stdio::piped()).spawn()?;
    let mut err = child.stderr.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = Vec::new();
        err.read_to_end(&mut s).ok();
        String::from_utf8_lossy(&s).into_owned()
    });
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            child.kill().ok();
            child.wait().ok();
            return Err(MediaError::Cancelled);
        }
        if let Some(st) = child.try_wait()? {
            break st;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let stderr = reader.join().unwrap_or_default();
    if !status.success() {
        let tail: Vec<&str> = stderr.lines().rev().take(3).collect();
        return Err(MediaError::Failed(tail.into_iter().rev().collect::<Vec<_>>().join(" | ")));
    }
    Ok(stderr)
}

/// WAV temporário 16 kHz mono PCM16 em `%LOCALAPPDATA%\br.com.ouveai\tmp`,
/// apagado quando o valor sai de escopo (inclusive em erro ou cancelamento).
pub struct TempWav {
    pub path: PathBuf,
}

impl Drop for TempWav {
    fn drop(&mut self) {
        fs::remove_file(&self.path).ok();
    }
}

pub fn to_wav(ff: &Path, input: &Path, cancel: &AtomicBool) -> Result<TempWav, MediaError> {
    let dir = paths::data_dir().join("tmp");
    fs::create_dir_all(&dir)?;
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let wav = TempWav { path: dir.join(format!("import-{}-{nanos}.wav", std::process::id())) };
    let args: Vec<&std::ffi::OsStr> = vec![
        "-nostdin".as_ref(),
        "-hide_banner".as_ref(),
        "-loglevel".as_ref(),
        "error".as_ref(),
        "-y".as_ref(),
        "-i".as_ref(),
        input.as_os_str(),
        "-vn".as_ref(),
        "-ac".as_ref(),
        "1".as_ref(),
        "-ar".as_ref(),
        "16000".as_ref(),
        "-c:a".as_ref(),
        "pcm_s16le".as_ref(),
        wav.path.as_os_str(),
    ];
    run(ff, &args, cancel)?;
    Ok(wav)
}

/// Parâmetros do `silencedetect` (prompt.txt).
pub const SILENCE_DB: i32 = -35;
pub const SILENCE_MIN: f64 = 0.3;

/// Silêncios medidos pelo `silencedetect` (-35 dB, 0,3 s).
pub fn silences(ff: &Path, wav: &Path, duration: f64, cancel: &AtomicBool) -> Result<Vec<crate::words::Pause>, MediaError> {
    let filter = format!("silencedetect=noise={SILENCE_DB}dB:d={SILENCE_MIN}");
    let args: Vec<&std::ffi::OsStr> = vec![
        "-nostdin".as_ref(),
        "-hide_banner".as_ref(),
        "-nostats".as_ref(),
        "-i".as_ref(),
        wav.as_os_str(),
        "-af".as_ref(),
        filter.as_ref(),
        "-f".as_ref(),
        "null".as_ref(),
        "-".as_ref(),
    ];
    Ok(parse_silences(&run(ff, &args, cancel)?, duration))
}

pub fn parse_silences(stderr: &str, duration: f64) -> Vec<crate::words::Pause> {
    let num = |line: &str, key: &str| -> Option<f64> {
        let rest = &line[line.find(key)? + key.len()..];
        rest.trim_start().split(|c: char| c.is_whitespace() || c == '|').next()?.parse().ok()
    };
    let mut out = Vec::new();
    let mut open: Option<f64> = None;
    for line in stderr.lines() {
        if let Some(s) = num(line, "silence_start:") {
            open = Some(s.max(0.0));
        } else if let Some(e) = num(line, "silence_end:")
            && let Some(s) = open.take() {
                out.push(crate::words::Pause { start: s, end: e.min(duration) });
            }
    }
    // Silêncio que vai até o fim do arquivo não tem `silence_end`.
    if let Some(s) = open {
        out.push(crate::words::Pause { start: s, end: duration });
    }
    out
}

// Áudio longo: pedaços de até 60 s, cortados num silêncio entre 45 s e 60 s.
// O prompt pedia 10 min, mas o encoder do Parakeet em ONNX aborta acima de
// 400 s, e em trechos longos com ruído de fundo ele pula frases inteiras.
// Medido na Fase 3 (podcast de 65 min com legenda de referência + gravação
// ruidosa): 60 s empata no áudio limpo, recupera as frases no ruidoso e é
// 1,6× mais rápido que 5 min. Decidido com o usuário; ver docs/fase3/resultado.md.
pub const CHUNK_MAX: f64 = 60.0;
pub const CUT_FROM: f64 = 45.0;

/// Tamanho dos pedaços: até `max` segundos, corte na janela `[cut_from, max]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkPlan {
    pub max: f64,
    pub cut_from: f64,
}

impl Default for ChunkPlan {
    fn default() -> Self {
        ChunkPlan { max: CHUNK_MAX, cut_from: CUT_FROM }
    }
}

impl ChunkPlan {
    /// Para testes de qualidade (`ouveai-cli transcribe --chunk N`): janela de 3/4 a 4/4.
    pub fn with_max(max: f64) -> Self {
        ChunkPlan { max, cut_from: max * 0.75 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chunk {
    pub start: f64,
    pub end: f64,
    /// Sem silêncio na janela: cortado no ponto de menor volume (avisar).
    pub forced: bool,
}

/// Planeja os pedaços. O corte cai no meio do maior silêncio da janela; sem
/// silêncio, no quadro de 20 ms com menor energia (medida nas amostras).
pub fn plan_chunks(samples: &[f32], sr: u32, pauses: &[crate::words::Pause]) -> Vec<Chunk> {
    plan_chunks_with(samples, sr, pauses, ChunkPlan::default())
}

pub fn plan_chunks_with(samples: &[f32], sr: u32, pauses: &[crate::words::Pause], plan: ChunkPlan) -> Vec<Chunk> {
    let duration = samples.len() as f64 / sr as f64;
    let mut out = Vec::new();
    let mut pos = 0.0;
    while duration - pos > plan.max {
        let (a, b) = (pos + plan.cut_from, (pos + plan.max).min(duration));
        let best = pauses
            .iter()
            .map(|p| (p.start.max(a), p.end.min(b)))
            .filter(|(s, e)| e > s)
            .max_by(|x, y| (x.1 - x.0).total_cmp(&(y.1 - y.0)));
        let (cut, forced) = match best {
            Some((s, e)) => ((s + e) / 2.0, false),
            None => (quietest(samples, sr, a, b), true),
        };
        out.push(Chunk { start: pos, end: cut, forced });
        pos = cut;
    }
    out.push(Chunk { start: pos, end: duration, forced: false });
    out
}

fn quietest(samples: &[f32], sr: u32, a: f64, b: f64) -> f64 {
    let frame = (sr as usize / 50).max(1); // 20 ms
    let (i0, i1) = ((a * sr as f64) as usize, ((b * sr as f64) as usize).min(samples.len()));
    let mut best = (f64::MAX, a);
    let mut i = i0;
    while i + frame <= i1 {
        let e: f64 = samples[i..i + frame].iter().map(|&x| (x as f64) * (x as f64)).sum();
        if e < best.0 {
            best = (e, (i + frame / 2) as f64 / sr as f64);
        }
        i += frame;
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_a_saida_do_silencedetect() {
        let err = "[silencedetect @ 0x1] silence_start: 1.25\n[silencedetect @ 0x1] silence_end: 2.5 | silence_duration: 1.25\n[silencedetect @ 0x1] silence_start: -0.01\n";
        let p = parse_silences(err, 9.0);
        assert_eq!(p, vec![crate::words::Pause { start: 1.25, end: 2.5 }, crate::words::Pause { start: 0.0, end: 9.0 }]);
    }

    #[test]
    fn corta_audio_longo_no_silencio_ou_no_ponto_mais_baixo() {
        let sr = 100; // 100 Hz basta para o teste
        let n = 150 * sr as usize; // 2:30
        let mut samples = vec![0.5f32; n];
        // 1º pedaço: silêncio em 50–52 s → corte em 51 s.
        let pauses = [crate::words::Pause { start: 50.0, end: 52.0 }];
        // 2º pedaço (de 51 s): janela 96–111 s sem silêncio → ponto mais baixo em 100 s.
        for s in &mut samples[100 * 100..100 * 100 + 2] {
            *s = 0.0;
        }
        let c = plan_chunks(&samples, sr, &pauses);
        assert_eq!(c.len(), 3);
        assert_eq!((c[0].start, c[0].end, c[0].forced), (0.0, 51.0, false));
        assert!(c[1].forced && (c[1].end - 100.0).abs() < 0.05, "{:?}", c[1]);
        assert_eq!(c[2].end, 150.0);
        assert!(c.iter().all(|c| c.end - c.start <= CHUNK_MAX));
        let curto = plan_chunks(&vec![0.0; 100 * 60], 100, &[]);
        assert_eq!(curto, vec![Chunk { start: 0.0, end: 60.0, forced: false }]);
        // Plano de teste (--chunk 300): janela 225–300 s.
        let longo = plan_chunks_with(&vec![0.5; 100 * 400], 100, &[crate::words::Pause { start: 250.0, end: 254.0 }], ChunkPlan::with_max(300.0));
        assert_eq!(longo[0].end, 252.0);
    }

    #[test]
    fn probe_rejeita_o_que_nao_e_ffmpeg() {
        // cmd.exe existe em todo Windows e não responde "ffmpeg version".
        let cmd = PathBuf::from(std::env::var("ComSpec").unwrap());
        assert!(probe(&cmd).is_none());
    }

    fn fake_zip(path: &Path, exe: &[u8]) {
        let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        w.start_file("ffmpeg-x/LICENSE", o).unwrap();
        w.write_all(b"GPL").unwrap();
        w.start_file("ffmpeg-x/bin/ffprobe.exe", o).unwrap();
        w.write_all(b"nao").unwrap();
        w.start_file("ffmpeg-x/bin/ffmpeg.exe", o).unwrap();
        w.write_all(exe).unwrap();
        w.finish().unwrap();
    }

    #[test]
    fn extrai_so_o_ffmpeg_e_a_licenca() {
        let _g = crate::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ouveai-media-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        unsafe { std::env::set_var("OUVEAI_DATA_DIR", &dir) };
        let z = dir.join("p.zip");
        fake_zip(&z, &[7u8; 3_000_000]);
        let cancel = AtomicBool::new(false);
        let mut last = 0;
        install_zip(&z, &cancel, &mut |s| {
            if let Step::Extracting { done, .. } = s {
                last = done
            }
        })
        .unwrap();
        assert_eq!(last, 3_000_000);
        assert_eq!(fs::read(local_path()).unwrap().len(), 3_000_000);
        assert_eq!(fs::read_to_string(paths::bin_dir().join("FFMPEG-LICENSE.txt")).unwrap(), "GPL");
        assert!(!paths::bin_dir().join("ffprobe.exe").exists());
        unsafe { std::env::remove_var("OUVEAI_DATA_DIR") };
        fs::remove_dir_all(&dir).ok();
    }
}
