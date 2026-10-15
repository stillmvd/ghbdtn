use super::hook::HookEvent;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::OnceLock;
use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, SetWinEventHook, HWINEVENTHOOK,
};
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    pub exe: String,
    pub elevated: bool,
    pub fullscreen: bool,
    pub password: bool,
}

static SENDER: OnceLock<Sender<HookEvent>> = OnceLock::new();
static CURRENT: OnceLock<arc_swap::ArcSwap<Context>> = OnceLock::new();
static BUSY: AtomicBool = AtomicBool::new(false);

thread_local! {
    static UIA: Option<IUIAutomation> = unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
    };
}

pub fn current() -> Context {
    CURRENT.get().map(|c| (**c.load()).clone()).unwrap_or_default()
}

fn process_name(pid: u32) -> String {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new() };
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().unwrap_or_default().to_lowercase()
    }
}

fn token_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut e = TOKEN_ELEVATION::default();
        let mut ret = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut e as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret,
        )
        .is_ok();
        let _ = CloseHandle(token);
        ok.then_some(e.TokenIsElevated != 0)
    }
}

fn elevated(pid: u32) -> bool {
    if token_elevated(unsafe { GetCurrentProcess() }) == Some(true) {
        return false;
    }
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return true };
        let r = token_elevated(h).unwrap_or(true);
        let _ = CloseHandle(h);
        r
    }
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn fullscreen(hwnd: HWND) -> bool {
    if let Ok(state) = unsafe { SHQueryUserNotificationState() } {
        if state == QUNS_RUNNING_D3D_FULL_SCREEN || state == QUNS_PRESENTATION_MODE {
            return true;
        }
    }
    if matches!(class_name(hwnd).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd") {
        return false;
    }
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    if style & (WS_CAPTION.0 | WS_THICKFRAME.0) != 0 {
        return false;
    }
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        GetMonitorInfoW(mon, &mut info).as_bool() && rect == info.rcMonitor
    }
}

fn password(focus: HWND) -> bool {
    if !focus.is_invalid() && class_name(focus).eq_ignore_ascii_case("edit") {
        let style = unsafe { GetWindowLongW(focus, GWL_STYLE) } as u32;
        if style & ES_PASSWORD as u32 != 0 {
            return true;
        }
    }
    UIA.with(|uia| {
        uia.as_ref()
            .and_then(|a| unsafe { a.GetFocusedElement() }.ok())
            .and_then(|e| unsafe { e.CurrentIsPassword() }.ok())
            .is_some_and(|b| b.as_bool())
    })
}

unsafe extern "system" fn find_child(child: HWND, lparam: LPARAM) -> BOOL {
    let out = &mut *(lparam.0 as *mut (u32, u32));
    let mut pid = 0;
    GetWindowThreadProcessId(child, Some(&mut pid));
    if pid != out.0 {
        out.1 = pid;
        return false.into();
    }
    true.into()
}

fn compute() -> Context {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut pid = 0;
        let thread = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut exe = process_name(pid);
        if exe == "applicationframehost.exe" {
            let mut pair = (pid, 0u32);
            let _ = EnumChildWindows(Some(hwnd), Some(find_child), LPARAM(&mut pair as *mut _ as isize));
            if pair.1 != 0 {
                pid = pair.1;
                exe = process_name(pid);
            }
        }
        let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        let focus = if GetGUIThreadInfo(thread, &mut info).is_ok() { info.hwndFocus } else { HWND::default() };
        Context { exe, elevated: elevated(pid), fullscreen: fullscreen(hwnd), password: password(focus) }
    }
}

pub fn refresh() {
    if BUSY.swap(true, SeqCst) {
        return;
    }
    let ctx = compute();
    BUSY.store(false, SeqCst);
    if let Some(c) = CURRENT.get() {
        c.store(ctx.into());
    }
    if let Some(tx) = SENDER.get() {
        let _ = tx.try_send(HookEvent::Focus);
    }
}

unsafe extern "system" fn on_event(_: HWINEVENTHOOK, _: u32, _: HWND, _: i32, _: i32, _: u32, _: u32) {
    let _ = std::panic::catch_unwind(refresh);
}

pub fn run(tx: Sender<HookEvent>) -> std::thread::JoinHandle<()> {
    let _ = SENDER.set(tx);
    let _ = CURRENT.set(arc_swap::ArcSwap::from_pointee(Context::default()));
    std::thread::Builder::new()
        .name("ghbdtn-context".into())
        .spawn(|| unsafe {
            refresh();
            let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
            let _a = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(on_event), 0, 0, flags);
            let _b = SetWinEventHook(EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS, None, Some(on_event), 0, 0, flags);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .expect("context thread")
}
