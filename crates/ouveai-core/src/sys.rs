//! Checagem da máquina (assistente de primeiro uso e antes de baixar o modelo).

use std::path::Path;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Machine {
    pub windows_build: u32,
    pub ram_total_mb: u64,
    pub ram_free_mb: u64,
    pub cores: usize,
    pub avx2: bool,
    pub disk_free_mb: u64,
}

/// Requisitos mínimos propostos (D6).
pub const MIN_BUILD: u32 = 19041;
pub const MIN_RAM_MB: u64 = 8 * 1024 - 512; // "8 GB" anunciados costumam aparecer como ~7,8 GB
pub const MIN_CORES: usize = 4;

pub fn machine(data_dir: &Path) -> Machine {
    let (ram_total_mb, ram_free_mb) = ram_mb();
    Machine {
        windows_build: windows_build(),
        ram_total_mb,
        ram_free_mb,
        cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        avx2: std::arch::is_x86_feature_detected!("avx2"),
        disk_free_mb: disk_free_bytes(data_dir).unwrap_or(0) / (1024 * 1024),
    }
}

/// Número do build do Windows (ex.: 19041, 26100), lido do registro. 0 se falhar.
pub fn windows_build() -> u32 {
    build_from_registry().unwrap_or(0)
}

fn build_from_registry() -> Option<u32> {
    reg_str(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "CurrentBuildNumber")?.parse().ok()
}

/// Valor texto em `HKLM\<sub>`.
fn reg_str(sub: &str, name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (sub, name) = (wide(sub), wide(name));
    let mut buf = [0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    let r = unsafe {
        RegGetValueW(HKEY_LOCAL_MACHINE, sub.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut size)
    };
    if r != 0 {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]).trim().to_string())
}

/// Nome do processador (ex.: "AMD Ryzen 5 5600X 6-Core Processor").
pub fn cpu_name() -> String {
    reg_str(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0", "ProcessorNameString").unwrap_or_else(|| "?".into())
}

/// Ex.: "Windows 11 Pro 24H2 (build 26100)". O registro ainda diz "Windows 10" no 11.
pub fn windows_version() -> String {
    let key = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build = windows_build();
    let mut name = reg_str(key, "ProductName").unwrap_or_else(|| "Windows".into());
    if build >= 22000 {
        name = name.replace("Windows 10", "Windows 11");
    }
    match reg_str(key, "DisplayVersion") {
        Some(v) => format!("{name} {v} (build {build})"),
        None => format!("{name} (build {build})"),
    }
}

fn ram_mb() -> (u64, u64) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut m: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
        return (0, 0);
    }
    (m.ullTotalPhys / (1024 * 1024), m.ullAvailPhys / (1024 * 1024))
}

/// Espaço livre no volume de `path` (sobe até achar um diretório que exista).
pub fn disk_free_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0u64;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
    (ok != 0).then_some(free)
}
