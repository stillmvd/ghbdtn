use serde::Serialize;
use std::collections::HashSet;
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT};
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::System::Registry::{RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_DWORD};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::BOOL;

const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const SHELL: [&str; 6] = ["Shell_TrayWnd", "Shell_SecondaryTrayWnd", "NotifyIconOverflowWindow", "TopLevelWindowForOverflowXamlIsland", "Windows.UI.Core.CoreWindow", "Progman"];

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn process_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }
}

pub fn exe_name(path: &str) -> String {
    path.rsplit('\\').next().unwrap_or_default().to_lowercase()
}

pub fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    pid
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

pub fn foreground_app() -> Option<(String, String)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() || SHELL.contains(&class_name(hwnd).as_str()) {
            return None;
        }
        let pid = window_pid(hwnd);
        if pid == std::process::id() {
            return None;
        }
        let path = process_path(pid)?;
        let exe = exe_name(&path);
        (exe != "explorer.exe").then_some((exe, path))
    }
}

pub fn own_foreground() -> bool {
    unsafe { window_pid(GetForegroundWindow()) == std::process::id() }
}

pub fn friendly_name(path: &str) -> String {
    let fallback = || exe_name(path).trim_end_matches(".exe").to_string();
    unsafe {
        let file = wide(path);
        let size = GetFileVersionInfoSizeW(PCWSTR(file.as_ptr()), None);
        if size == 0 {
            return fallback();
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(PCWSTR(file.as_ptr()), None, size, data.as_mut_ptr().cast()).is_err() {
            return fallback();
        }
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(data.as_ptr().cast(), w!("\\VarFileInfo\\Translation"), &mut ptr, &mut len).as_bool() || len < 4 {
            return fallback();
        }
        let pair = *(ptr as *const [u16; 2]);
        let key = wide(&format!("\\StringFileInfo\\{:04x}{:04x}\\FileDescription", pair[0], pair[1]));
        if !VerQueryValueW(data.as_ptr().cast(), PCWSTR(key.as_ptr()), &mut ptr, &mut len).as_bool() || len < 2 {
            return fallback();
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr as *const u16, len as usize - 1));
        let text = text.trim().to_string();
        if text.is_empty() { fallback() } else { text }
    }
}

pub fn taskbar_light() -> bool {
    let mut value = 0u32;
    let mut size = 4u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    r.is_ok() && value == 1
}

pub fn set_autostart(on: bool) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Ok(());
    }
    unsafe {
        if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let value = wide(&format!("\"{}\"", exe.display()));
            RegSetKeyValueW(HKEY_CURRENT_USER, RUN, w!("Ghbdtn"), REG_SZ.0, Some(value.as_ptr().cast()), (value.len() * 2) as u32)
                .ok()
                .map_err(|e| e.message())
        } else {
            let r = RegDeleteKeyValueW(HKEY_CURRENT_USER, RUN, w!("Ghbdtn"));
            if r.is_ok() || r.0 == 2 { Ok(()) } else { Err(r.to_hresult().message()) }
        }
    }
}

#[derive(Serialize, Clone)]
pub struct Proc {
    pub exe: String,
    pub title: String,
}

unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
    let out = &mut *(data.0 as *mut Vec<(u32, String)>);
    if IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd, GW_OWNER).map_or(true, |o| o.is_invalid()) {
        let mut buf = [0u16; 256];
        let n = GetWindowTextW(hwnd, &mut buf);
        if n > 0 {
            out.push((window_pid(hwnd), String::from_utf16_lossy(&buf[..n as usize])));
        }
    }
    BOOL(1)
}

pub fn list_processes() -> Vec<Proc> {
    let mut found: Vec<(u32, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(&mut found as *mut _ as isize));
    }
    let mut seen = HashSet::new();
    let own = std::process::id();
    let mut out: Vec<Proc> = found
        .into_iter()
        .filter(|(pid, _)| *pid != own)
        .filter_map(|(pid, title)| process_path(pid).map(|p| Proc { exe: exe_name(&p), title }))
        .filter(|p| seen.insert(p.exe.clone()))
        .collect();
    out.sort_by(|a, b| a.exe.cmp(&b.exe));
    out
}

pub fn play_sound() {
    let file = wide("C:\\Windows\\Media\\Windows Navigation Start.wav");
    unsafe {
        let _ = PlaySoundW(PCWSTR(file.as_ptr()), None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT);
    }
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}
