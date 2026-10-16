use crate::sys;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use ghbdtn_core::config::Signals;
use std::result::Result::Ok;
use std::time::{Duration, Instant};
use windows::core::{w, Interface, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Graphics::GdiPlus::*;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::{SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetUBound, SafeArrayUnaccessData};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

const SHOW_FOR: Duration = Duration::from_millis(1700);
const WM_HINT: u32 = WM_APP + 1;
const FRAME_MS: u32 = 15;
const PARGB: i32 = 0xE200B;
const FONT: &[u8] = include_bytes!("../assets/KockersSans-Bold.ttf");
const STAR: [(f32, f32); 8] = [(8.0, 0.5), (9.6, 6.4), (15.5, 8.0), (9.6, 9.6), (8.0, 15.5), (6.4, 9.6), (0.5, 8.0), (6.4, 6.4)];

pub struct Hint {
    pub typed: String,
    pub result: String,
    pub signals: Signals,
}

struct Scene {
    typed: String,
    hint: bool,
    k: f32,
    caret: Option<RECT>,
    word: Option<RECT>,
    mouse: Option<(i32, i32)>,
}

pub fn start() -> Sender<Hint> {
    let (tx, rx) = unbounded();
    let (tid_tx, tid_rx) = bounded(1);
    std::thread::Builder::new().name("ghbdtn-hint".into()).spawn(move || paint(tid_tx)).expect("hint thread");
    let tid = tid_rx.recv().unwrap_or(0);
    std::thread::Builder::new().name("ghbdtn-overlay".into()).spawn(move || run(tid, rx)).expect("overlay thread");
    tx
}

fn run(tid: u32, rx: Receiver<Hint>) {
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
        let Some(anchor) = caret.map(|c| (c.left, c.top)).or(mouse) else { continue };
        let scene = Scene {
            typed: std::mem::take(&mut hint.typed),
            hint: hint.signals.hint,
            k: dpi(anchor) as f32 / 96.0,
            caret,
            word,
            mouse: if hint.signals.cursor { mouse } else { None },
        };
        let raw = Box::into_raw(Box::new(scene));
        if unsafe { PostThreadMessageW(tid, WM_HINT, WPARAM(0), LPARAM(raw as isize)) }.is_err() {
            drop(unsafe { Box::from_raw(raw) });
        }
    }
}

fn dpi((x, y): (i32, i32)) -> u32 {
    let (mut dx, mut dy) = (96, 96);
    unsafe {
        let m = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let _ = GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
    }
    dx
}

fn paint(ready: Sender<u32>) {
    unsafe {
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        let _ = ready.send(GetCurrentThreadId());
        let mut token = 0usize;
        let input = GdiplusStartupInput { GdiplusVersion: 1, ..Default::default() };
        if GdiplusStartup(&mut token, &input, std::ptr::null_mut()) != Status(0) {
            return;
        }
        let Some(family) = Family::load() else { return };
        let Some(windows) = Windows::create() else { return };
        let mut anim: Option<Anim> = None;
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HINT && msg.hwnd.is_invalid() {
                let scene = *Box::from_raw(msg.lParam.0 as *mut Scene);
                anim = Some(Anim::new(scene, &family));
                SetTimer(Some(windows.hint), 1, FRAME_MS, None);
                msg.message = WM_TIMER;
            }
            if msg.message == WM_TIMER {
                let alive = anim.as_ref().is_some_and(|a| a.frame(&windows));
                if !alive {
                    anim = None;
                    let _ = KillTimer(Some(windows.hint), 1);
                    let _ = ShowWindow(windows.hint, SW_HIDE);
                    let _ = ShowWindow(windows.star, SW_HIDE);
                }
                continue;
            }
            DispatchMessageW(&msg);
        }
    }
}

struct Windows {
    hint: HWND,
    star: HWND,
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wp, lp)
}

impl Windows {
    unsafe fn create() -> Option<Self> {
        let module = GetModuleHandleW(None).ok()?;
        let class = w!("GhbdtnHint");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(proc),
            hInstance: module.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let ex = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST;
        let make = || CreateWindowExW(ex, class, PCWSTR::null(), WS_POPUP, 0, 0, 1, 1, None, None, Some(module.into()), None).ok();
        Some(Windows { hint: make()?, star: make()? })
    }
}

struct Family(*mut GpFontFamily);

impl Family {
    unsafe fn load() -> Option<Self> {
        let mut coll = std::ptr::null_mut();
        GdipNewPrivateFontCollection(&mut coll);
        if coll.is_null() || GdipPrivateAddMemoryFont(coll, FONT.as_ptr().cast(), FONT.len() as i32) != Status(0) {
            return None;
        }
        let mut fams = [std::ptr::null_mut(); 1];
        let mut found = 0;
        GdipGetFontCollectionFamilyList(coll, &mut fams, &mut found);
        (found == 1).then(|| Family(fams[0]))
    }
}

#[derive(Clone, Copy)]
struct Theme {
    acc: u32,
    ink: u32,
    shadow: f32,
}

impl Theme {
    fn current() -> Self {
        if sys::apps_light() {
            Theme { acc: 0x4f7a00, ink: 0xffffff, shadow: 0.14 }
        } else {
            Theme { acc: 0xc6ec5a, ink: 0x1a2400, shadow: 0.34 }
        }
    }
}

struct Badge {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

struct Anim {
    start: Instant,
    k: f32,
    theme: Theme,
    still: bool,
    text: Vec<u16>,
    font: *mut GpFont,
    format: *mut GpStringFormat,
    badge: Option<Badge>,
    line: Option<(f32, f32, f32)>,
    star: Option<(f32, f32)>,
}

impl Drop for Anim {
    fn drop(&mut self) {
        unsafe {
            GdipDeleteFont(self.font);
            GdipDeleteStringFormat(self.format);
        }
    }
}

fn animations_off() -> bool {
    let mut on = windows::core::BOOL(1);
    unsafe {
        let _ = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some((&mut on as *mut windows::core::BOOL).cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
    !on.as_bool()
}

impl Anim {
    fn new(s: Scene, family: &Family) -> Self {
        Self::with(s, family, Theme::current(), animations_off())
    }

    fn with(s: Scene, family: &Family, theme: Theme, still: bool) -> Self {
        let k = s.k;
        let text = sys::wide(&s.typed);
        let mut font = std::ptr::null_mut();
        let mut format = std::ptr::null_mut();
        let text_w: f32;
        unsafe {
            GdipCreateFont(family.0, 12.0 * k, FontStyleBold.0 | FontStyleStrikeout.0, UnitPixel, &mut font);
            let mut generic = std::ptr::null_mut();
            GdipStringFormatGetGenericTypographic(&mut generic);
            GdipCloneStringFormat(generic, &mut format);
            GdipSetStringFormatAlign(format, StringAlignmentCenter);
            GdipSetStringFormatLineAlign(format, StringAlignmentCenter);
            let mut bmp = std::ptr::null_mut();
            let mut g = std::ptr::null_mut();
            GdipCreateBitmapFromScan0(1, 1, 4, PARGB, None, &mut bmp);
            GdipGetImageGraphicsContext(bmp.cast(), &mut g);
            GdipSetTextRenderingHint(g, TextRenderingHintAntiAlias);
            let mut bound = RectF::default();
            let layout = RectF { X: 0.0, Y: 0.0, Width: 10000.0, Height: 1000.0 };
            GdipMeasureString(g, PCWSTR(text.as_ptr()), -1, font, &layout, generic, &mut bound, std::ptr::null_mut(), std::ptr::null_mut());
            text_w = bound.Width;
            GdipDeleteGraphics(g);
            GdipDisposeImage(bmp.cast());
        }
        let anchor = s.word.map(|r| (r.left as f32 - 8.0 * k, r.top as f32)).or_else(|| s.caret.map(|r| (r.left as f32 - 17.0 * k, r.top as f32)));
        let anchor = anchor.or_else(|| s.mouse.map(|(x, y)| (x as f32 - 17.0 * k, y as f32 - 4.0 * k)));
        let badge = anchor.filter(|_| s.hint).map(|(x, y)| {
            let h = 22.0 * k;
            Badge { x, y: y - 8.0 * k - h, w: text_w.ceil() + 14.0 * k, h }
        });
        let line = s.word.filter(|_| s.hint).map(|r| (r.left as f32, r.bottom as f32 - 2.0 * k, (r.right - r.left) as f32));
        let star = s.mouse.map(|(x, y)| (x as f32 + 13.0 * k, y as f32 - 13.0 * k));
        Anim { start: Instant::now(), k, theme, still, text, font, format, badge, line, star }
    }

    fn frame(&self, win: &Windows) -> bool {
        let t = self.start.elapsed();
        if t >= SHOW_FOR {
            return false;
        }
        let ms = t.as_secs_f32() * 1000.0;
        let k = self.k;
        if self.badge.is_some() || self.line.is_some() {
            let pad = 48.0 * k;
            let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            let mut grow = |x: f32, y: f32, w: f32, h: f32| b = (b.0.min(x), b.1.min(y), b.2.max(x + w), b.3.max(y + h));
            if let Some(bd) = &self.badge {
                grow(bd.x, bd.y - 2.0 * k, bd.w, bd.h + 12.0 * k);
            }
            if let Some((x, y, w)) = self.line {
                grow(x, y, w, 2.0 * k);
            }
            let r = RECT { left: (b.0 - pad) as i32, top: (b.1 - pad) as i32, right: (b.2 + pad) as i32, bottom: (b.3 + pad) as i32 };
            present(win.hint, r, |g| self.draw_hint(g, ms));
        } else {
            let _ = unsafe { ShowWindow(win.hint, SW_HIDE) };
        }
        if let Some((cx, cy)) = self.star {
            let half = (20.0 * k) as i32;
            let r = RECT { left: cx as i32 - half, top: cy as i32 - half, right: cx as i32 + half, bottom: cy as i32 + half };
            present(win.star, r, |g| self.draw_star(g, ms));
        } else {
            let _ = unsafe { ShowWindow(win.star, SW_HIDE) };
        }
        true
    }

    fn draw_hint(&self, g: *mut GpGraphics, ms: f32) {
        let k = self.k;
        if let Some(bd) = &self.badge {
            let (op, ty) = if self.still {
                (fade(ms), 0.0)
            } else {
                let p = ms / 1600.0;
                (keys(p, &[(0.0, 0.0), (0.12, 1.0), (0.8, 1.0), (1.0, 0.0)]), keys(p, &[(0.0, 5.0), (0.12, 0.0), (0.8, 0.0), (1.0, -2.0)]))
            };
            let y = bd.y + ty * k;
            unsafe {
                for i in 0..12 {
                    let e = 34.0 * k * ((i as f32 + 0.5) / 12.0 - 0.5);
                    let (w, h) = (bd.w + 2.0 * e, bd.h + 2.0 * e);
                    if w > 0.0 && h > 0.0 {
                        fill_round(g, argb(0x000000, self.theme.shadow * op / 12.0), bd.x - e, y + 14.0 * k - e, w, h, (6.0 * k + e).max(0.0));
                    }
                }
                fill_round(g, argb(self.theme.acc, op), bd.x, y, bd.w, bd.h, 6.0 * k);
                let tip = [
                    PointF { X: bd.x + 12.0 * k, Y: y + bd.h - 0.5 },
                    PointF { X: bd.x + 22.0 * k, Y: y + bd.h - 0.5 },
                    PointF { X: bd.x + 17.0 * k, Y: y + bd.h + 5.0 * k },
                ];
                fill_poly(g, argb(self.theme.acc, op), &tip);
                let mut brush = std::ptr::null_mut();
                GdipCreateSolidFill(argb(self.theme.ink, op), &mut brush);
                let rect = RectF { X: bd.x - 20.0 * k, Y: y + 0.5 * k, Width: bd.w + 40.0 * k, Height: bd.h };
                GdipDrawString(g, PCWSTR(self.text.as_ptr()), -1, self.font, &rect, self.format, brush.cast());
                GdipDeleteBrush(brush.cast());
            }
        }
        if let Some((x, y, w)) = self.line {
            let (op, sx) = if self.still {
                (fade(ms), 1.0)
            } else {
                let p = ms / 1600.0;
                (keys(p, &[(0.0, 1.0), (0.7, 1.0), (0.92, 0.0), (1.0, 0.0)]), keys(p, &[(0.0, 0.0), (0.22, 1.0), (1.0, 1.0)]))
            };
            if sx > 0.0 {
                unsafe { fill_round(g, argb(self.theme.acc, op), x, y, w * sx, 2.0 * k, k) };
            }
        }
    }

    fn draw_star(&self, g: *mut GpGraphics, ms: f32) {
        let Some((cx, cy)) = self.star else { return };
        let (op, scale, rot) = if self.still {
            (fade(ms), 1.0, 0.0)
        } else {
            let p = ms / 1100.0;
            (
                keys(p, &[(0.0, 0.0), (0.25, 1.0), (0.6, 1.0), (1.0, 0.0)]),
                keys(p, &[(0.0, 0.0), (0.25, 1.2), (0.6, 1.0), (1.0, 0.4)]),
                keys(p, &[(0.0, -60.0), (0.25, 0.0), (0.6, 10.0), (1.0, 40.0)]),
            )
        };
        let (sin, cos) = rot.to_radians().sin_cos();
        let s = scale * self.k;
        let pts: Vec<PointF> = STAR
            .iter()
            .map(|&(x, y)| {
                let (dx, dy) = ((x - 8.0) * s, (y - 8.0) * s);
                PointF { X: cx + dx * cos - dy * sin, Y: cy + dx * sin + dy * cos }
            })
            .collect();
        unsafe { fill_poly(g, argb(self.theme.acc, op), &pts) };
    }
}

fn fade(ms: f32) -> f32 {
    let p = ms / 1600.0;
    if p < 0.8 { 1.0 } else { ((1.0 - p) / 0.2).max(0.0) }
}

fn keys(p: f32, frames: &[(f32, f32)]) -> f32 {
    let p = p.clamp(0.0, 1.0);
    for w in frames.windows(2) {
        let ((p0, v0), (p1, v1)) = (w[0], w[1]);
        if p <= p1 {
            let local = if p1 > p0 { (p - p0) / (p1 - p0) } else { 1.0 };
            return v0 + (v1 - v0) * ease(local);
        }
    }
    frames.last().map_or(0.0, |f| f.1)
}

fn ease(x: f32) -> f32 {
    let (x1, y1, x2, y2) = (0.2f32, 0.0f32, 0.0f32, 1.0f32);
    let bez = |t: f32, a: f32, b: f32| 3.0 * (1.0 - t).powi(2) * t * a + 3.0 * (1.0 - t) * t * t * b + t * t * t;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if bez(mid, x1, x2) < x { lo = mid } else { hi = mid }
    }
    bez((lo + hi) / 2.0, y1, y2)
}

fn argb(rgb: u32, alpha: f32) -> u32 {
    (((alpha.clamp(0.0, 1.0) * 255.0).round() as u32) << 24) | rgb
}

unsafe fn fill_round(g: *mut GpGraphics, color: u32, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let d = r * 2.0;
    let mut path = std::ptr::null_mut();
    GdipCreatePath(FillModeWinding, &mut path);
    if d > 0.0 {
        GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
        GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
        GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
        GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
    } else {
        let pts = [PointF { X: x, Y: y }, PointF { X: x + w, Y: y }, PointF { X: x + w, Y: y + h }, PointF { X: x, Y: y + h }];
        GdipAddPathPolygon(path, pts.as_ptr(), 4);
    }
    GdipClosePathFigure(path);
    let mut brush = std::ptr::null_mut();
    GdipCreateSolidFill(color, &mut brush);
    GdipFillPath(g, brush.cast(), path);
    GdipDeleteBrush(brush.cast());
    GdipDeletePath(path);
}

unsafe fn fill_poly(g: *mut GpGraphics, color: u32, pts: &[PointF]) {
    let mut brush = std::ptr::null_mut();
    GdipCreateSolidFill(color, &mut brush);
    GdipFillPolygon(g, brush.cast(), pts.as_ptr(), pts.len() as i32, FillModeWinding);
    GdipDeleteBrush(brush.cast());
}

unsafe fn canvas(bits: *mut u8, w: i32, h: i32, ox: f32, oy: f32, draw: impl FnOnce(*mut GpGraphics)) {
    let mut bmp = std::ptr::null_mut();
    GdipCreateBitmapFromScan0(w, h, w * 4, PARGB, Some(bits), &mut bmp);
    let mut g = std::ptr::null_mut();
    GdipGetImageGraphicsContext(bmp.cast(), &mut g);
    GdipGraphicsClear(g, 0);
    GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
    GdipSetTextRenderingHint(g, TextRenderingHintAntiAlias);
    GdipTranslateWorldTransform(g, -ox, -oy, MatrixOrderPrepend);
    draw(g);
    GdipDeleteGraphics(g);
    GdipDisposeImage(bmp.cast());
}

fn present(hwnd: HWND, r: RECT, draw: impl FnOnce(*mut GpGraphics)) {
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    if w <= 0 || h <= 0 {
        return;
    }
    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(dib) = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) else { return };
        let dc = CreateCompatibleDC(None);
        let old = SelectObject(dc, dib.into());
        canvas(bits.cast(), w, h, r.left as f32, r.top as f32, draw);
        let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
        let pos = POINT { x: r.left, y: r.top };
        let size = SIZE { cx: w, cy: h };
        let src = POINT::default();
        let _ = UpdateLayeredWindow(hwnd, None, Some(&pos), Some(&size), Some(dc), Some(&src), COLORREF(0), Some(&blend), ULW_ALPHA);
        if !IsWindowVisible(hwnd).as_bool() {
            let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
        }
        SelectObject(dc, old);
        let _ = DeleteDC(dc);
        let _ = DeleteObject(dib.into());
    }
}

fn gui_caret() -> Option<RECT> {
    unsafe {
        let (_, thread) = ghbdtn_core::win::lang::input_thread();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn preview() {
        let out = std::env::var("HINT_PREVIEW").expect("HINT_PREVIEW=<папка>");
        unsafe {
            let mut token = 0usize;
            let input = GdiplusStartupInput { GdiplusVersion: 1, ..Default::default() };
            assert_eq!(GdiplusStartup(&mut token, &input, std::ptr::null_mut()), Status(0));
            let family = Family::load().expect("font");
            let png = windows::core::GUID::from_u128(0x557cf406_1a04_11d3_9a73_0000f81ef32e);
            for (name, theme, bg) in [("dark", Theme { acc: 0xc6ec5a, ink: 0x1a2400, shadow: 0.34 }, 0xff202020u32), ("light", Theme { acc: 0x4f7a00, ink: 0xffffff, shadow: 0.14 }, 0xfff3f3f3)] {
                for ms in [150.0f32, 500.0, 1450.0] {
                    let k = 1.0;
                    let scene = Scene {
                        typed: "руддщ".into(),
                        hint: true,
                        k,
                        caret: Some(RECT { left: 210, top: 120, right: 211, bottom: 150 }),
                        word: Some(RECT { left: 120, top: 120, right: 205, bottom: 150 }),
                        mouse: Some((330, 140)),
                    };
                    let a = Anim::with(scene, &family, theme, false);
                    let (w, h) = (420, 220);
                    let mut bmp = std::ptr::null_mut();
                    GdipCreateBitmapFromScan0(w, h, 0, PARGB, None, &mut bmp);
                    let mut g = std::ptr::null_mut();
                    GdipGetImageGraphicsContext(bmp.cast(), &mut g);
                    GdipGraphicsClear(g, bg);
                    GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
                    GdipSetTextRenderingHint(g, TextRenderingHintAntiAlias);
                    a.draw_hint(g, ms);
                    a.draw_star(g, ms);
                    GdipDeleteGraphics(g);
                    let path = sys::wide(&format!("{out}\\hint-{name}-{ms}.png"));
                    GdipSaveImageToFile(bmp.cast(), PCWSTR(path.as_ptr()), &png, std::ptr::null());
                    GdipDisposeImage(bmp.cast());
                }
            }
        }
    }
}
