//! Dispositivos, processos e sessões de áudio (achados do S3).

use serde::Serialize;
use wasapi::{Device, DeviceEnumerator, Direction, Role, SessionState};

/// Roda `f` numa thread MTA (a thread principal do Tauri é STA).
pub(crate) fn on_mta<T: Send + 'static>(f: impl FnOnce(&DeviceEnumerator) -> T + Send + 'static) -> Option<T> {
    std::thread::spawn(move || {
        let _ = wasapi::initialize_mta();
        DeviceEnumerator::new().ok().map(|e| f(&e))
    })
    .join()
    .ok()
    .flatten()
}

fn prop_u32(dev: &Device, fmtid: u128, pid: u32) -> Option<u32> {
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::STGM_READ;
    let key = PROPERTYKEY { fmtid: windows::core::GUID::from_u128(fmtid), pid };
    unsafe {
        let store = dev.as_immdevice().OpenPropertyStore(STGM_READ).ok()?;
        let pv = store.GetValue(&key).ok()?;
        u32::try_from(&pv).ok()
    }
}

fn prop_string(dev: &Device, fmtid: u128, pid: u32) -> Option<String> {
    use windows::Win32::Foundation::PROPERTYKEY;
    use windows::Win32::System::Com::STGM_READ;
    let key = PROPERTYKEY { fmtid: windows::core::GUID::from_u128(fmtid), pid };
    unsafe {
        let store = dev.as_immdevice().OpenPropertyStore(STGM_READ).ok()?;
        let pv = store.GetValue(&key).ok()?;
        let s = pv.to_string();
        (!s.is_empty()).then_some(s)
    }
}

/// `PKEY_AudioEndpoint_FormFactor`: Speakers, Headphones, Headset, Microphone…
pub fn form_factor(dev: &Device) -> String {
    const NAMES: [&str; 11] = ["RemoteNetwork", "Speakers", "LineLevel", "Headphones", "Microphone", "Headset", "Handset", "DigitalPassthrough", "SPDIF", "HDMI", "Unknown"];
    match prop_u32(dev, 0x1da5d803_d492_4edd_8c23_e0c0ffee7f0e, 0) {
        Some(v) => NAMES.get(v as usize).map_or(format!("{v}"), |s| s.to_string()),
        None => "Unknown".into(),
    }
}

/// Bluetooth pelo enumerador do dispositivo (`BTHENUM`, `BTHHFENUM`, `BTHLEDEVICE`…).
pub fn is_bluetooth(dev: &Device) -> bool {
    // PKEY_Device_EnumeratorName {A45C254E-DF1C-4EFD-8020-67D146A850E0}, 24
    prop_string(dev, 0xa45c254e_df1c_4efd_8020_67d146a850e0, 24).is_some_and(|e| e.to_ascii_uppercase().starts_with("BTH"))
}

/// Saída que é fone (sem eco possível): Headphones ou Headset.
pub fn is_headphone_form(form: &str) -> bool {
    matches!(form, "Headphones" | "Headset" | "Handset")
}

#[derive(Clone, Debug, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub form: String,
    pub input: bool,
    pub bluetooth: bool,
    pub default: bool,
}

pub fn list_devices() -> Vec<DeviceInfo> {
    on_mta(|e| {
        let mut out = Vec::new();
        for (dir, input) in [(Direction::Capture, true), (Direction::Render, false)] {
            let def = e.get_default_device_for_role(&dir, &Role::Console).ok().and_then(|d| d.get_id().ok());
            if let Ok(coll) = e.get_device_collection(&dir) {
                for i in 0..coll.get_nbr_devices().unwrap_or(0) {
                    let Ok(d) = coll.get_device_at_index(i) else { continue };
                    let id = d.get_id().unwrap_or_default();
                    out.push(DeviceInfo {
                        default: def.as_deref() == Some(id.as_str()),
                        name: d.get_friendlyname().unwrap_or_else(|_| "?".into()),
                        form: form_factor(&d),
                        bluetooth: is_bluetooth(&d),
                        input,
                        id,
                    });
                }
            }
        }
        out
    })
    .unwrap_or_default()
}

/// O dispositivo de saída padrão é um fone? (padrão de "Estou usando fone")
pub fn default_output_is_headphone() -> Option<bool> {
    on_mta(|e| e.get_default_device_for_role(&Direction::Render, &Role::Console).ok().map(|d| is_headphone_form(&form_factor(&d)))).flatten()
}

// ---------- processos ----------

/// (pid, pid do pai, nome do exe) de todos os processos.
pub fn list_processes() -> Vec<(u32, u32, String)> {
    use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
    let mut procs = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return procs };
        let mut e = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
            procs.push((e.th32ProcessID, e.th32ParentProcessID, String::from_utf16_lossy(&e.szExeFile[..len])));
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = windows::Win32::Foundation::CloseHandle(snap);
    }
    procs
}

/// A raiz da árvore do exe (o processo com esse nome cujo pai não tem o mesmo nome).
pub fn find_process_root(exe: &str) -> Option<u32> {
    let procs = list_processes();
    let same: Vec<&(u32, u32, String)> = procs.iter().filter(|p| p.2.eq_ignore_ascii_case(exe)).collect();
    same.iter().find(|p| !same.iter().any(|q| q.0 == p.1)).or(same.first()).map(|p| p.0)
}

pub fn process_alive(pid: u32) -> bool {
    list_processes().iter().any(|p| p.0 == pid)
}

/// Apps de reunião conhecidos, em ordem de preferência para o EU automático.
const MEETING_APPS: &[&str] = &["ms-teams.exe", "teams.exe", "zoom.exe", "webex.exe", "slack.exe", "discord.exe", "chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe"];

fn meeting_rank(exe: &str) -> usize {
    MEETING_APPS.iter().position(|a| a.eq_ignore_ascii_case(exe)).unwrap_or(MEETING_APPS.len())
}

/// Quem (outro processo) está usando cada dispositivo agora.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SessionUser {
    pub device_id: String,
    pub device: String,
    pub exe: String,
}

pub(crate) fn active_users(e: &DeviceEnumerator, dir: Direction) -> Vec<SessionUser> {
    let procs = list_processes();
    let me = std::process::id();
    let mut out = Vec::new();
    let Ok(coll) = e.get_device_collection(&dir) else { return out };
    for i in 0..coll.get_nbr_devices().unwrap_or(0) {
        let Ok(d) = coll.get_device_at_index(i) else { continue };
        let Ok(sessions) = d.get_iaudiosessionmanager().and_then(|m| m.get_audiosessionenumerator()) else { continue };
        for s in 0..sessions.get_count().unwrap_or(0) {
            let Ok(c) = sessions.get_session(s) else { continue };
            if c.get_state().ok() != Some(SessionState::Active) {
                continue;
            }
            let pid = c.get_process_id().unwrap_or(0);
            if pid == me || pid == 0 {
                continue;
            }
            let exe = procs.iter().find(|p| p.0 == pid).map_or(format!("pid {pid}"), |p| p.2.clone());
            let u = SessionUser { device_id: d.get_id().unwrap_or_default(), device: d.get_friendlyname().unwrap_or_default(), exe };
            if !out.contains(&u) {
                out.push(u);
            }
        }
    }
    out.sort_by_key(|u| meeting_rank(&u.exe));
    out
}

/// O mic que o app de reunião está usando (o mais provável primeiro).
pub(crate) fn meeting_mic(e: &DeviceEnumerator) -> Option<SessionUser> {
    active_users(e, Direction::Capture).into_iter().next()
}

/// Apps tocando som agora (para o modo "Por aplicativo").
pub fn playing_apps() -> Vec<String> {
    on_mta(|e| {
        let mut v: Vec<String> = Vec::new();
        for u in active_users(e, Direction::Render) {
            if !u.exe.starts_with("pid ") && !v.iter().any(|x| x.eq_ignore_ascii_case(&u.exe)) {
                v.push(u.exe);
            }
        }
        v
    })
    .unwrap_or_default()
}
