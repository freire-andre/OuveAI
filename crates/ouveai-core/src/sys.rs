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

/// Placa de vídeo dedicada. Só informativa (log e diagnóstico): o S10 mostrou que a GPU não
/// acelera o Parakeet int8, então a transcrição fica sempre na CPU.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Gpu {
    pub name: String,
    pub vendor: &'static str,
    pub vram_mb: u64,
    /// NVIDIA com o driver instalado (`nvcuda.dll` no System32): pré-requisito do CUDA.
    pub cuda_driver: bool,
}

/// Abaixo disso é a fatia reservada de uma placa integrada, não memória dedicada.
const MIN_DEDICATED_MB: u64 = 512;

fn vendor(id: u32) -> &'static str {
    match id {
        0x10DE => "NVIDIA",
        0x1002 | 0x1022 => "AMD",
        0x8086 => "Intel",
        0x1414 => "Microsoft",
        _ => "outro",
    }
}

/// Uma placa conta como dedicada se não for o adaptador de software e tiver memória própria.
fn dedicated(vendor_id: u32, vram_mb: u64, software: bool) -> bool {
    !software && vendor_id != 0x1414 && vram_mb >= MIN_DEDICATED_MB
}

/// Placas de vídeo dedicadas, pelo DXGI. Lista vazia em máquina só com placa integrada ou se
/// o DXGI falhar (nunca impede o app de abrir).
pub fn gpus() -> Vec<Gpu> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1};
    let cuda_dll = std::env::var_os("SystemRoot")
        .map(|r| Path::new(&r).join("System32").join("nvcuda.dll").exists())
        .unwrap_or(false);
    let mut out = Vec::new();
    // SAFETY: chamadas COM do DXGI sem estado compartilhado; os objetos são liberados no drop.
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else {
        return out;
    };
    for i in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(i) }) else { break };
        let Ok(d) = (unsafe { adapter.GetDesc1() }) else { continue };
        let vram_mb = d.DedicatedVideoMemory as u64 / (1024 * 1024);
        let software = d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0;
        if !dedicated(d.VendorId, vram_mb, software) {
            continue;
        }
        let len = d.Description.iter().position(|&c| c == 0).unwrap_or(d.Description.len());
        let name = String::from_utf16_lossy(&d.Description[..len]).trim().to_string();
        // O mesmo adaptador pode aparecer duas vezes (uma por saída de vídeo).
        if out.iter().any(|g: &Gpu| g.name == name) {
            continue;
        }
        out.push(Gpu { name, vendor: vendor(d.VendorId), vram_mb, cuda_driver: d.VendorId == 0x10DE && cuda_dll });
    }
    out
}

#[cfg(test)]
mod gpu_tests {
    use super::*;

    #[test]
    fn so_placa_dedicada_conta() {
        assert!(dedicated(0x10DE, 8188, false));
        assert!(dedicated(0x1002, 4096, false));
        // Integrada com a fatia reservada de 128 MB.
        assert!(!dedicated(0x8086, 128, false));
        // Adaptador de software (Microsoft Basic Render Driver).
        assert!(!dedicated(0x1414, 0, true));
        assert!(!dedicated(0x10DE, 8188, true));
    }

    #[test]
    fn detecta_sem_derrubar() {
        // Só garante que a chamada real ao DXGI não entra em pânico nesta máquina.
        let g = gpus();
        assert!(g.iter().all(|g| g.vram_mb >= MIN_DEDICATED_MB));
    }
}

