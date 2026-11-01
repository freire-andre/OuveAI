//! Consultas ao Windows.

/// Milissegundos desde a criação do processo (inclui o carregamento do exe, antes do `main`).
pub fn ms_since_process_start() -> f64 {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let ft = |f: FILETIME| ((f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64) as f64;
    let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exit, mut kernel, mut user, mut now) = (z, z, z, z, z);
    unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut created, &mut exit, &mut kernel, &mut user);
        GetSystemTimeAsFileTime(&mut now);
    }
    (ft(now) - ft(created)) / 10_000.0
}

/// Permissão do microfone em Configurações › Privacidade (registro). `Some(false)` =
/// negada (para todos os apps ou para apps da área de trabalho); `None` = sem registro.
pub fn mic_permission() -> Option<bool> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    let base = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";
    let read = |root, sub: &str| -> Option<String> {
        let sub: Vec<u16> = sub.encode_utf16().chain([0]).collect();
        let name: Vec<u16> = "Value".encode_utf16().chain([0]).collect();
        let mut buf = [0u16; 32];
        let mut len = (buf.len() * 2) as u32;
        let r = unsafe { RegGetValueW(root, sub.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len) };
        (r == 0).then(|| String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]))
    };
    let vals = [read(HKEY_LOCAL_MACHINE, base), read(HKEY_CURRENT_USER, base), read(HKEY_CURRENT_USER, &format!(r"{base}\NonPackaged"))];
    if vals.iter().flatten().any(|v| v.eq_ignore_ascii_case("Deny")) {
        Some(false)
    } else if vals.iter().flatten().any(|v| v.eq_ignore_ascii_case("Allow")) {
        Some(true)
    } else {
        None
    }
}
