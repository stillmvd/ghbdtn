use crate::sys;
use crossbeam_channel::{unbounded, Receiver, Sender};
use ghbdtn_core::config::Signals;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::core::Interface;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::Ole::{SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetUBound, SafeArrayUnaccessData};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const SHOW_FOR: Duration = Duration::from_millis(1700);

pub struct Hint {
    pub typed: String,
    pub result: String,
    pub signals: Signals,
}

#[derive(Serialize, Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[derive(Serialize, Clone)]
struct Payload {
    id: u64,
    typed: String,
    result: String,
    hint: bool,
    caret: Option<Rect>,
    word: Option<Rect>,
    mouse: Option<Rect>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html".into()))
        .title("Ghbdtn")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .focused(false)
        .focusable(false)
        .visible(false)
        .inner_size(400.0, 200.0)
        .build()?;
    w.set_ignore_cursor_events(true)?;
    let hwnd = HWND(w.hwnd()?.0 as _);
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TRANSPARENT.0) as isize);
    }
    Ok(())
}

pub fn start(app: AppHandle) -> Sender<Hint> {
    let (tx, rx) = unbounded();
    std::thread::Builder::new().name("ghbdtn-overlay".into()).spawn(move || run(app, rx)).expect("overlay thread");
    tx
}

fn run(app: AppHandle, rx: Receiver<Hint>) {
    let uia: Option<IUIAutomation> = unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
    };
    if let Some(a2) = uia.as_ref().and_then(|u| u.cast::<IUIAutomation2>().ok()) {
        unsafe {
            let _ = a2.SetConnectionTimeout(300);
            let _ = a2.SetTransactionTimeout(300);
        }
    }
    let seq = Arc::new(AtomicU64::new(0));
    while let Ok(mut hint) = rx.recv() {
        std::thread::sleep(Duration::from_millis(40));
        while let Ok(next) = rx.try_recv() {
            hint = next;
        }
        if hint.signals.sound {
            sys::play_sound();
        }
        if !hint.signals.hint && !hint.signals.cursor {
            continue;
        }
        let caret = gui_caret().or_else(|| uia.as_ref().and_then(uia_caret));
        let word = if hint.signals.hint { uia.as_ref().and_then(|u| uia_word(u, &hint.result)) } else { None };
        let mouse = sys::cursor_pos();
        let anchor = caret.map(|c| (c.left, c.top)).or(mouse);
        let Some(anchor) = anchor else { continue };
        let Some(mon) = monitor(anchor) else { continue };
        let local = |r: RECT| Rect { x: r.left - mon.left, y: r.top - mon.top, w: r.right - r.left, h: r.bottom - r.top };
        let id = seq.fetch_add(1, SeqCst) + 1;
        let payload = Payload {
            id,
            typed: std::mem::take(&mut hint.typed),
            result: std::mem::take(&mut hint.result),
            hint: hint.signals.hint,
            caret: caret.map(local),
            word: word.map(local),
            mouse: if hint.signals.cursor { mouse.map(|(x, y)| Rect { x: x - mon.left, y: y - mon.top, w: 0, h: 0 }) } else { None },
        };
        let Some(win) = app.get_webview_window("overlay") else { continue };
        let Ok(raw) = win.hwnd() else { continue };
        let hwnd = raw.0 as isize;
        let h = HWND(hwnd as _);
        unsafe {
            let _ = SetWindowPos(h, Some(HWND_TOPMOST), mon.left, mon.top, mon.right - mon.left, mon.bottom - mon.top, SWP_NOACTIVATE);
        }
        let _ = app.emit_to("overlay", "hint", payload);
        std::thread::sleep(Duration::from_millis(30));
        unsafe {
            let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
        }
        let seq = seq.clone();
        std::thread::spawn(move || {
            std::thread::sleep(SHOW_FOR);
            if seq.load(SeqCst) == id {
                unsafe {
                    let _ = ShowWindow(HWND(hwnd as _), SW_HIDE);
                }
            }
        });
    }
}

fn monitor((x, y): (i32, i32)) -> Option<RECT> {
    unsafe {
        let m = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(m, &mut info).as_bool().then_some(info.rcMonitor)
    }
}

fn gui_caret() -> Option<RECT> {
    unsafe {
        let (_, thread) = ghbdtn_core::win::lang::foreground_thread();
        let mut gi = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        GetGUIThreadInfo(thread, &mut gi).ok()?;
        if gi.hwndCaret.is_invalid() {
            return None;
        }
        let r = gi.rcCaret;
        if r.bottom - r.top <= 0 {
            return None;
        }
        let mut a = POINT { x: r.left, y: r.top };
        let mut b = POINT { x: r.right, y: r.bottom };
        if !ClientToScreen(gi.hwndCaret, &mut a).as_bool() || !ClientToScreen(gi.hwndCaret, &mut b).as_bool() {
            return None;
        }
        Some(RECT { left: a.x, top: a.y, right: b.x.max(a.x + 1), bottom: b.y })
    }
}

fn caret_range(uia: &IUIAutomation) -> Option<IUIAutomationTextRange> {
    unsafe {
        let el = uia.GetFocusedElement().ok()?;
        if let Ok(tp2) = el.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id) {
            let mut active = Default::default();
            if let Ok(r) = tp2.GetCaretRange(&mut active) {
                return Some(r);
            }
        }
        let tp = el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId).ok()?;
        let sel = tp.GetSelection().ok()?;
        if sel.Length().ok()? < 1 {
            return None;
        }
        let r = sel.GetElement(0).ok()?;
        r.MoveEndpointByRange(TextPatternRangeEndpoint_Start, &r, TextPatternRangeEndpoint_End).ok()?;
        Some(r)
    }
}

fn rects(range: &IUIAutomationTextRange) -> Vec<RECT> {
    unsafe {
        let Ok(arr) = range.GetBoundingRectangles() else { return Vec::new() };
        if arr.is_null() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if let Ok(ub) = SafeArrayGetUBound(arr, 1) {
            let n = (ub + 1) as usize;
            let mut data = std::ptr::null_mut();
            if SafeArrayAccessData(arr, &mut data).is_ok() {
                let v = std::slice::from_raw_parts(data as *const f64, n);
                for q in v.chunks_exact(4) {
                    let (l, t, w, h) = (q[0] as i32, q[1] as i32, q[2] as i32, q[3] as i32);
                    if w > 0 && h > 0 {
                        out.push(RECT { left: l, top: t, right: l + w, bottom: t + h });
                    }
                }
                let _ = SafeArrayUnaccessData(arr);
            }
        }
        let _ = SafeArrayDestroy(arr);
        out
    }
}

fn uia_caret(uia: &IUIAutomation) -> Option<RECT> {
    unsafe {
        let r = caret_range(uia)?;
        r.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -1).ok()?;
        let last = rects(&r).pop()?;
        Some(RECT { left: last.right, top: last.top, right: last.right + 1, bottom: last.bottom })
    }
}

fn uia_word(uia: &IUIAutomation, result: &str) -> Option<RECT> {
    let len = result.chars().count() as i32;
    unsafe {
        let r = caret_range(uia)?;
        r.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -(len + 1)).ok()?;
        r.MoveEndpointByUnit(TextPatternRangeEndpoint_End, TextUnit_Character, -1).ok()?;
        let text = r.GetText(len + 4).ok()?.to_string();
        if text != result {
            return None;
        }
        let rs = rects(&r);
        let first = *rs.first()?;
        if rs.iter().any(|x| x.top != first.top) {
            return None;
        }
        Some(rs.iter().fold(first, |a, b| RECT { left: a.left.min(b.left), top: a.top.min(b.top), right: a.right.max(b.right), bottom: a.bottom.max(b.bottom) }))
    }
}
