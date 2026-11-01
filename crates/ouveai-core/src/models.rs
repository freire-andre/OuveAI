//! Catálogo de modelos com hash fixado (L14) e instalação: download (GitHub ou espelho HF),
//! importação de arquivo/pasta, conferência por SHA-256 e troca atômica da pasta.
//! Hashes e URLs: `docs/fase0/catalogo-modelos.md`.

use crate::hash::sha256_file;
use crate::net::{self, NetConfig, NetError, Progress};
use crate::paths;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, serde::Serialize)]
pub struct FileSpec {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ArchiveSpec {
    pub url: String,
    pub size: u64,
    pub sha256: String,
    /// Pasta raiz dentro do `.tar.bz2`.
    pub root: String,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ModelSpec {
    pub id: String,
    pub name: String,
    pub license: String,
    pub files: Vec<FileSpec>,
    /// Fonte principal: o pacote do release do sherpa-onnx.
    pub archive: Option<ArchiveSpec>,
    /// Espelho com os arquivos soltos (`<base>/<nome>`), fixado por commit.
    pub mirror_base: Option<String>,
    /// Vem dentro do exe (não precisa de rede).
    pub embedded: bool,
    /// Para que serve, quando é opcional (mostrado na tela Modelos).
    pub note: Option<String>,
}

impl ModelSpec {
    pub fn files_size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    pub fn dir(&self) -> PathBuf {
        paths::models_dir().join(&self.id)
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.dir().join(name)
    }
}

fn f(name: &str, size: u64, sha256: &str) -> FileSpec {
    FileSpec { name: name.into(), size, sha256: sha256.into() }
}

pub const PARAKEET_ID: &str = "parakeet-tdt-0.6b-v3-int8";
pub const SILERO_ID: &str = "silero-vad";

pub fn parakeet() -> ModelSpec {
    ModelSpec {
        id: PARAKEET_ID.into(),
        name: "Parakeet TDT 0.6B v3 (int8)".into(),
        license: "CC-BY-4.0 (NVIDIA)".into(),
        files: vec![
            f("encoder.int8.onnx", 652184281, "acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247"),
            f("decoder.int8.onnx", 11845275, "179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e"),
            f("joiner.int8.onnx", 6355277, "3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3"),
            f("tokens.txt", 93939, "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d"),
        ],
        archive: Some(ArchiveSpec {
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2".into(),
            size: 487170055,
            sha256: "5793d0fd397c5778d2cf2126994d58e9d56b1be7c04d13c7a15bb1b4eafb16bf".into(),
            root: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8".into(),
        }),
        mirror_base: Some(
            "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/2bda32ec70b097a55adaa07d9a7173915b43cc78".into(),
        ),
        embedded: false,
        note: None,
    }
}

const SILERO_BYTES: &[u8] = include_bytes!("../assets/silero_vad.onnx");

pub fn silero() -> ModelSpec {
    ModelSpec {
        id: SILERO_ID.into(),
        name: "Silero VAD".into(),
        license: "MIT".into(),
        files: vec![f("silero_vad.onnx", 643854, "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6")],
        archive: None,
        mirror_base: None,
        embedded: true,
        note: None,
    }
}

pub const WHISPER_ID: &str = "whisper-large-v3-turbo-int8";

/// Parte 4 (opcional): Whisper large-v3-turbo em int8. Pacote do release do sherpa-onnx
/// (que também traz `test_wavs/`, ignorados na extração) e espelho fixado por commit.
pub fn whisper() -> ModelSpec {
    ModelSpec {
        id: WHISPER_ID.into(),
        name: "Whisper large-v3 turbo (int8)".into(),
        license: "MIT (OpenAI)".into(),
        files: vec![
            f("turbo-encoder.int8.onnx", 674716297, "b02dcdf54f348741e93fe732b67d933c8dcb6735655f710640143081db38878b"),
            f("turbo-decoder.int8.onnx", 361080764, "20accd02388482eb3a46bd615631adfdc85e1eb2c7db9ea3f02a40ffe6b81547"),
            f("turbo-tokens.txt", 816730, "b34b360dbb493e781e479794586d661700670d65564001f23024971d1f2fa126"),
        ],
        archive: Some(ArchiveSpec {
            url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-turbo.tar.bz2".into(),
            size: 563790207,
            sha256: "b11acbbcd660b44a8e0df33724feb5aaa709cf65668f2823d59f656312544f22".into(),
            root: "sherpa-onnx-whisper-turbo".into(),
        }),
        mirror_base: Some(
            "https://huggingface.co/csukuangfj/sherpa-onnx-whisper-turbo/resolve/2ca6ff69fc878651b770880507669577ac41c2ff".into(),
        ),
        embedded: false,
        note: Some("Opcional. Usado só em \"Comparar com o Whisper\", para conferir trechos difíceis da transcrição.".into()),
    }
}

pub fn catalog() -> Vec<ModelSpec> {
    vec![parakeet(), silero(), whisper()]
}

pub fn by_id(id: &str) -> Option<ModelSpec> {
    catalog().into_iter().find(|m| m.id == id)
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "lowercase")]
pub enum Presence {
    Ready,
    Absent,
    /// Pasta existe, mas falta arquivo ou o tamanho não bate.
    Corrupt(String),
}

/// Checagem rápida (só tamanhos). O SHA-256 completo roda ao instalar/importar.
pub fn presence(spec: &ModelSpec) -> Presence {
    let dir = spec.dir();
    if !dir.exists() {
        return Presence::Absent;
    }
    for fs_ in &spec.files {
        match fs::metadata(dir.join(&fs_.name)) {
            Ok(m) if m.len() == fs_.size => {}
            Ok(m) => return Presence::Corrupt(format!("{}: {} bytes, esperado {}", fs_.name, m.len(), fs_.size)),
            Err(_) => return Presence::Corrupt(format!("{} ausente", fs_.name)),
        }
    }
    Presence::Ready
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(tag = "step", rename_all = "lowercase")]
pub enum Step {
    Downloading { done: u64, total: u64, bytes_per_sec: f64, retry: Option<net::Retry> },
    Verifying { done: u64, total: u64 },
    Extracting { done: u64, total: u64 },
}

#[derive(Debug)]
pub enum InstallError {
    Cancelled,
    Net(NetError),
    /// Hash ou tamanho diferente do catálogo: arquivo corrompido ou adulterado. Nunca é usado.
    Corrupt(String),
    NoSpace { need_mb: u64, free_mb: u64 },
    Invalid(String),
    Io(std::io::Error),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstallError::Cancelled => write!(f, "pausado"),
            InstallError::Net(e) => write!(f, "{e}"),
            InstallError::Corrupt(e) => write!(f, "arquivo corrompido ou diferente do esperado ({e})"),
            InstallError::NoSpace { need_mb, free_mb } => {
                write!(f, "espaço insuficiente: precisa de {need_mb} MB livres, há {free_mb} MB")
            }
            InstallError::Invalid(e) => write!(f, "{e}"),
            InstallError::Io(e) => write!(f, "erro de disco: {e}"),
        }
    }
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e)
    }
}

impl From<NetError> for InstallError {
    fn from(e: NetError) -> Self {
        match e {
            NetError::Cancelled => InstallError::Cancelled,
            e => InstallError::Net(e),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// GitHub releases (pacote `.tar.bz2`).
    Primary,
    /// Hugging Face (arquivos soltos).
    Mirror,
}

fn downloads_dir() -> PathBuf {
    paths::models_dir().join("_downloads")
}

pub(crate) fn check_space(need: u64) -> Result<(), InstallError> {
    let free = crate::sys::disk_free_bytes(&paths::models_dir()).unwrap_or(u64::MAX);
    // 200 MB de folga para o banco, logs e o próprio Windows.
    let need = need + 200 * 1024 * 1024;
    if free < need {
        return Err(InstallError::NoSpace { need_mb: need / (1024 * 1024), free_mb: free / (1024 * 1024) });
    }
    Ok(())
}

/// Espaço que o download ainda precisa (desconta o que já está nos `.part`).
pub fn space_needed(spec: &ModelSpec, source: Source) -> u64 {
    match (source, &spec.archive) {
        (Source::Primary, Some(a)) => {
            let have = fs::metadata(downloads_dir().join(archive_name(a)).with_extension("bz2.part")).map(|m| m.len()).unwrap_or(0);
            a.size.saturating_sub(have) + spec.files_size()
        }
        _ => spec.files_size(),
    }
}

fn archive_name(a: &ArchiveSpec) -> String {
    a.url.rsplit('/').next().unwrap_or("modelo.tar.bz2").to_string()
}

/// Baixa e instala. Interrompido (`cancel`), o `.part` fica e a próxima chamada retoma.
pub fn download(
    spec: &ModelSpec,
    cfg: &NetConfig,
    source: Source,
    cancel: &AtomicBool,
    on: &mut dyn FnMut(Step),
) -> Result<(), InstallError> {
    if spec.embedded {
        return ensure_embedded(spec);
    }
    check_space(space_needed(spec, source))?;
    match (source, &spec.archive, &spec.mirror_base) {
        (Source::Primary, Some(a), _) => {
            let archive = downloads_dir().join(archive_name(a));
            let part = PathBuf::from(format!("{}.part", archive.display()));
            if !archive.exists() {
                net::download(cfg, &a.url, &part, a.size, cancel, &mut |p: Progress| {
                    on(Step::Downloading { done: p.done, total: p.total, bytes_per_sec: p.bytes_per_sec, retry: p.retry })
                })?;
                verify_one(&part, a.size, &a.sha256, &mut |d| on(Step::Verifying { done: d, total: a.size }))
                    .inspect_err(|_| {
                        // Adulterado/corrompido: descarta para o próximo download começar limpo.
                        fs::remove_file(&part).ok();
                        fs::remove_file(part.with_extension("part.etag")).ok();
                    })?;
                fs::rename(&part, &archive)?;
                fs::remove_file(part.with_extension("part.etag")).ok();
            }
            install_archive(spec, &archive, cancel, on)?;
            fs::remove_file(&archive).ok();
            Ok(())
        }
        (_, _, Some(base)) => {
            let tmp = paths::models_dir().join(format!(".mirror-{}", spec.id));
            fs::create_dir_all(&tmp)?;
            let total = spec.files_size();
            let mut offset = 0u64;
            for fs_ in &spec.files {
                let dest = tmp.join(&fs_.name);
                if !(dest.exists() && fs::metadata(&dest)?.len() == fs_.size) {
                    let part = tmp.join(format!("{}.part", fs_.name));
                    let url = format!("{base}/{}", fs_.name);
                    net::download(cfg, &url, &part, fs_.size, cancel, &mut |p: Progress| {
                        on(Step::Downloading { done: offset + p.done, total, bytes_per_sec: p.bytes_per_sec, retry: p.retry })
                    })?;
                    verify_one(&part, fs_.size, &fs_.sha256, &mut |d| on(Step::Verifying { done: offset + d, total }))
                        .inspect_err(|_| {
                            fs::remove_file(&part).ok();
                        })?;
                    fs::rename(&part, &dest)?;
                    fs::remove_file(part.with_extension("part.etag")).ok();
                }
                offset += fs_.size;
            }
            finalize(spec, &tmp)
        }
        _ => Err(InstallError::Invalid("este modelo não tem fonte de download".into())),
    }
}

/// Importa um `.tar.bz2` ou uma pasta (de pendrive, pasta de rede…), sem internet.
pub fn import(spec: &ModelSpec, path: &Path, cancel: &AtomicBool, on: &mut dyn FnMut(Step)) -> Result<(), InstallError> {
    if path.is_dir() {
        // Aceita a pasta com os arquivos ou a pasta que contém a raiz do pacote.
        let src = [path.to_path_buf(), spec.archive.as_ref().map(|a| path.join(&a.root)).unwrap_or_default()]
            .into_iter()
            .find(|d| spec.files.iter().all(|f| d.join(&f.name).is_file()))
            .ok_or_else(|| {
                let names: Vec<_> = spec.files.iter().map(|f| f.name.as_str()).collect();
                InstallError::Invalid(format!("a pasta não tem os arquivos do modelo ({})", names.join(", ")))
            })?;
        check_space(spec.files_size())?;
        let tmp = fresh_tmp(spec)?;
        let total = spec.files_size();
        let mut offset = 0u64;
        for fs_ in &spec.files {
            if cancel.load(Ordering::Relaxed) {
                return Err(InstallError::Cancelled);
            }
            fs::copy(src.join(&fs_.name), tmp.join(&fs_.name))?;
            offset += fs_.size;
            on(Step::Extracting { done: offset, total });
        }
        verify_dir(spec, &tmp, on)?;
        finalize(spec, &tmp)
    } else if path.is_file() {
        check_space(spec.files_size())?;
        install_archive(spec, path, cancel, on)
    } else {
        Err(InstallError::Invalid(format!("não encontrado: {}", path.display())))
    }
}

/// Extrai só os arquivos do catálogo (ignora `test_wavs/` etc.), confere e troca a pasta.
fn install_archive(spec: &ModelSpec, archive: &Path, cancel: &AtomicBool, on: &mut dyn FnMut(Step)) -> Result<(), InstallError> {
    let tmp = fresh_tmp(spec)?;
    let total = fs::metadata(archive)?.len();
    let file = fs::File::open(archive)?;
    let counter = CountingReader { inner: file, read: 0, total, cancel, on, last: 0 };
    let mut tar = tar::Archive::new(bzip2::read::BzDecoder::new(counter));
    let mut found = 0;
    let entries = tar.entries().map_err(|e| InstallError::Invalid(format!("pacote inválido: {e}")))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| {
            if cancel.load(Ordering::Relaxed) { InstallError::Cancelled } else { InstallError::Invalid(format!("pacote inválido: {e}")) }
        })?;
        // Só o nome final, comparado com o catálogo: nada fora de `tmp` é escrito.
        let name = entry.path().ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        let Some(name) = name.filter(|n| spec.files.iter().any(|f| &f.name == n)) else { continue };
        let mut out = fs::File::create(tmp.join(&name))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| {
            if cancel.load(Ordering::Relaxed) { InstallError::Cancelled } else { InstallError::Io(e) }
        })?;
        found += 1;
    }
    if found < spec.files.len() {
        return Err(InstallError::Invalid("o pacote não tem todos os arquivos do modelo".into()));
    }
    verify_dir(spec, &tmp, on)?;
    finalize(spec, &tmp)
}

struct CountingReader<'a> {
    inner: fs::File,
    read: u64,
    total: u64,
    cancel: &'a AtomicBool,
    on: &'a mut dyn FnMut(Step),
    last: u64,
}

impl Read for CountingReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelado"));
        }
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        if self.read - self.last >= 4 << 20 || n == 0 {
            (self.on)(Step::Extracting { done: self.read, total: self.total });
            self.last = self.read;
        }
        Ok(n)
    }
}

fn fresh_tmp(spec: &ModelSpec) -> std::io::Result<PathBuf> {
    let tmp = paths::models_dir().join(format!(".tmp-{}", spec.id));
    if tmp.exists() {
        fs::remove_dir_all(&tmp)?;
    }
    fs::create_dir_all(&tmp)?;
    Ok(tmp)
}

pub(crate) fn verify_one(path: &Path, size: u64, sha: &str, progress: &mut dyn FnMut(u64)) -> Result<(), InstallError> {
    let len = fs::metadata(path)?.len();
    if len != size {
        return Err(InstallError::Corrupt(format!("tamanho {len}, esperado {size}")));
    }
    let got = sha256_file(path, progress)?;
    if got != sha {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return Err(InstallError::Corrupt(format!("SHA-256 de {name} não confere")));
    }
    Ok(())
}

/// Confere tamanho e SHA-256 de todos os arquivos de `dir`.
pub fn verify_dir(spec: &ModelSpec, dir: &Path, on: &mut dyn FnMut(Step)) -> Result<(), InstallError> {
    let total = spec.files_size();
    let mut offset = 0u64;
    for fs_ in &spec.files {
        verify_one(&dir.join(&fs_.name), fs_.size, &fs_.sha256, &mut |d| on(Step::Verifying { done: offset + d, total }))?;
        offset += fs_.size;
    }
    Ok(())
}

/// Troca a pasta do modelo de uma vez: a antiga só some depois que a nova está no lugar.
fn finalize(spec: &ModelSpec, tmp: &Path) -> Result<(), InstallError> {
    let dest = spec.dir();
    let old = paths::models_dir().join(format!(".old-{}", spec.id));
    if old.exists() {
        fs::remove_dir_all(&old)?;
    }
    if dest.exists() {
        fs::rename(&dest, &old)?;
    }
    fs::rename(tmp, &dest)?;
    if old.exists() {
        fs::remove_dir_all(&old).ok();
    }
    log::info!("modelo {} instalado", spec.id);
    Ok(())
}

/// Grava o Silero embutido na pasta de modelos se faltar ou estiver diferente.
pub fn ensure_embedded(spec: &ModelSpec) -> Result<(), InstallError> {
    if !spec.embedded || spec.id != SILERO_ID {
        return Err(InstallError::Invalid(format!("{} não vem embutido", spec.id)));
    }
    if presence(spec) == Presence::Ready {
        return Ok(());
    }
    let tmp = fresh_tmp(spec)?;
    fs::write(tmp.join(&spec.files[0].name), SILERO_BYTES)?;
    verify_dir(spec, &tmp, &mut |_| {})?;
    finalize(spec, &tmp)
}

/// Bytes já baixados e guardados para retomar (pacote `.part` + arquivos do espelho).
pub fn partial_bytes(spec: &ModelSpec) -> u64 {
    let mut n = 0;
    if let Some(a) = &spec.archive {
        let archive = downloads_dir().join(archive_name(a));
        n += fs::metadata(format!("{}.part", archive.display())).map(|m| m.len()).unwrap_or(0);
        n += fs::metadata(&archive).map(|m| m.len()).unwrap_or(0);
    }
    if let Ok(rd) = fs::read_dir(paths::models_dir().join(format!(".mirror-{}", spec.id))) {
        n += rd.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum::<u64>();
    }
    n
}

/// Apaga o modelo e os restos de download.
pub fn remove(spec: &ModelSpec) -> std::io::Result<()> {
    for d in [spec.dir(), paths::models_dir().join(format!(".mirror-{}", spec.id)), paths::models_dir().join(format!(".tmp-{}", spec.id))] {
        if d.exists() {
            fs::remove_dir_all(d)?;
        }
    }
    if let Some(a) = &spec.archive {
        let archive = downloads_dir().join(archive_name(a));
        for p in [archive.clone(), PathBuf::from(format!("{}.part", archive.display())), PathBuf::from(format!("{}.part.etag", archive.display()))] {
            fs::remove_file(p).ok();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::sha256_bytes;
    fn with_data_dir<T>(name: &str, f: impl FnOnce(&Path) -> T) -> T {
        let _g = crate::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ouveai-models-{name}-{}", std::process::id()));
        fs::remove_dir_all(&dir).ok();
        fs::create_dir_all(&dir).unwrap();
        // SAFETY: protegido pelo LOCK; nenhum outro teste lê o ambiente ao mesmo tempo.
        unsafe { std::env::set_var("OUVEAI_DATA_DIR", &dir) };
        let r = f(&dir);
        unsafe { std::env::remove_var("OUVEAI_DATA_DIR") };
        fs::remove_dir_all(&dir).ok();
        r
    }

    fn fake_spec() -> ModelSpec {
        let (a, b) = (b"conteudo do encoder".as_slice(), b"tokens".as_slice());
        ModelSpec {
            id: "fake".into(),
            name: "Fake".into(),
            license: "-".into(),
            files: vec![f("a.onnx", a.len() as u64, &sha256_bytes(a)), f("tokens.txt", b.len() as u64, &sha256_bytes(b))],
            archive: Some(ArchiveSpec { url: "https://x/fake.tar.bz2".into(), size: 0, sha256: String::new(), root: "fake-root".into() }),
            mirror_base: None,
            embedded: false,
            note: None,
        }
    }

    fn write_tar_bz2(path: &Path, files: &[(&str, &[u8])]) {
        let enc = bzip2::write::BzEncoder::new(fs::File::create(path).unwrap(), bzip2::Compression::fast());
        let mut b = tar::Builder::new(enc);
        for (name, data) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, name, *data).unwrap();
        }
        b.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn importa_pacote_e_ignora_o_resto() {
        with_data_dir("tar", |root| {
            let spec = fake_spec();
            let pkg = root.join("fake.tar.bz2");
            write_tar_bz2(
                &pkg,
                &[("fake-root/a.onnx", b"conteudo do encoder"), ("fake-root/tokens.txt", b"tokens"), ("fake-root/test_wavs/x.wav", b"x")],
            );
            import(&spec, &pkg, &AtomicBool::new(false), &mut |_| {}).unwrap();
            assert_eq!(presence(&spec), Presence::Ready);
            assert!(!spec.dir().join("x.wav").exists());
        });
    }

    #[test]
    fn recusa_arquivo_adulterado_e_mantem_o_anterior() {
        with_data_dir("adulterado", |root| {
            let spec = fake_spec();
            let good = root.join("bom");
            fs::create_dir_all(&good).unwrap();
            fs::write(good.join("a.onnx"), b"conteudo do encoder").unwrap();
            fs::write(good.join("tokens.txt"), b"tokens").unwrap();
            import(&spec, &good, &AtomicBool::new(false), &mut |_| {}).unwrap();

            // Mesmo tamanho, um byte diferente.
            let bad = root.join("ruim").join("fake-root");
            fs::create_dir_all(&bad).unwrap();
            fs::write(bad.join("a.onnx"), b"conteudo do encodeR").unwrap();
            fs::write(bad.join("tokens.txt"), b"tokens").unwrap();
            let r = import(&spec, bad.parent().unwrap(), &AtomicBool::new(false), &mut |_| {});
            assert!(matches!(r, Err(InstallError::Corrupt(_))), "{r:?}");
            // O modelo bom continua lá.
            assert_eq!(presence(&spec), Presence::Ready);
            assert_eq!(fs::read(spec.dir().join("a.onnx")).unwrap(), b"conteudo do encoder");
        });
    }

    #[test]
    fn pasta_incompleta_e_invalida() {
        with_data_dir("incompleta", |root| {
            let spec = fake_spec();
            fs::write(root.join("a.onnx"), b"x").unwrap();
            let r = import(&spec, root, &AtomicBool::new(false), &mut |_| {});
            assert!(matches!(r, Err(InstallError::Invalid(_))), "{r:?}");
        });
    }

    #[test]
    fn silero_embutido_bate_com_o_catalogo() {
        with_data_dir("silero", |_| {
            let spec = silero();
            assert_eq!(sha256_bytes(SILERO_BYTES), spec.files[0].sha256);
            ensure_embedded(&spec).unwrap();
            assert_eq!(presence(&spec), Presence::Ready);
        });
    }
}
