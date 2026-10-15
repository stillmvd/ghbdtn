use crate::keymap::Lang;
use crate::word::{separator_kind, Key};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering::SeqCst};
use std::sync::OnceLock;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const MARK: usize = 0x4748_4244;
const HOLD_LIMIT_MS: u32 = 1000;
const WM_REINSTALL: u32 = WM_APP + 1;
const WM_QUIT_HOOK: u32 = WM_APP + 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawKey {
    pub vk: u16,
    pub scan: u16,
    pub up: bool,
    pub extended: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub win: bool,
    pub eaten: bool,
    pub held: bool,
    pub at: std::time::Instant,
}

#[derive(Clone, Copy, Debug)]
pub enum HookEvent {
    Key(RawKey),
    Foreign,
    Click,
    Focus,
    Words,
}

pub static HOLD: AtomicBool = AtomicBool::new(false);
static HOLD_SINCE: AtomicU32 = AtomicU32::new(0);
pub static ARMED_UNDO: AtomicBool = AtomicBool::new(false);
pub static ENABLED: AtomicBool = AtomicBool::new(true);
pub static ALLOWED: AtomicBool = AtomicBool::new(false);
pub static PENDING: AtomicBool = AtomicBool::new(false);
pub static HK_CONVERT: AtomicU32 = AtomicU32::new(0);
pub static HK_TOGGLE: AtomicU32 = AtomicU32::new(0);
pub static LAST_TICK: AtomicU32 = AtomicU32::new(0);
pub static HOOK_THREAD: AtomicU32 = AtomicU32::new(0);
pub static LANG: AtomicU8 = AtomicU8::new(0);
pub static ACCEPT_INJECTED: AtomicBool = AtomicBool::new(false);
pub static EVENTS: AtomicU64 = AtomicU64::new(0);
pub static DROPPED: AtomicU64 = AtomicU64::new(0);
pub static REPLAYED: AtomicU64 = AtomicU64::new(0);
pub static REINSTALLS: AtomicU64 = AtomicU64::new(0);
pub static FIXES: AtomicU64 = AtomicU64::new(0);
pub static FIX_US_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static FIX_US_MAX: AtomicU64 = AtomicU64::new(0);
static EATEN: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
static SENDER: OnceLock<Sender<HookEvent>> = OnceLock::new();

pub const MOD_SHIFT: u32 = 1 << 16;
pub const MOD_CTRL: u32 = 1 << 17;
pub const MOD_ALT: u32 = 1 << 18;

pub fn parse_hotkey(text: &str) -> Option<u32> {
    let mut mods = 0;
    let mut vk = None;
    for part in text.split('+').map(|p| p.trim().to_lowercase()) {
        match part.as_str() {
            "shift" => mods |= MOD_SHIFT,
            "ctrl" | "control" => mods |= MOD_CTRL,
            "alt" => mods |= MOD_ALT,
            "pause" | "break" => vk = Some(VK_PAUSE.0),
            "scrolllock" | "scroll" => vk = Some(VK_SCROLL.0),
            "insert" => vk = Some(VK_INSERT.0),
            p if p.len() > 1 && p.starts_with('f') => {
                let n: u16 = p[1..].parse().ok().filter(|n| (1..=24).contains(n))?;
                vk = Some(VK_F1.0 + n - 1);
            }
            p if p.len() == 1 && p.chars().all(|c| c.is_ascii_alphanumeric()) => {
                vk = Some(p.to_ascii_uppercase().as_bytes()[0] as u16);
            }
            _ => return None,
        }
    }
    vk.map(|v| mods | v as u32)
}

pub fn set_lang(lang: Option<Lang>) {
    LANG.store(
        match lang {
            Some(Lang::Ru) => 1,
            Some(Lang::En) => 2,
            None => 0,
        },
        SeqCst,
    );
}

fn lang() -> Option<Lang> {
    match LANG.load(SeqCst) {
        1 => Some(Lang::Ru),
        2 => Some(Lang::En),
        _ => None,
    }
}

fn eaten_slot(vk: u16) -> (&'static AtomicU64, u64) {
    (&EATEN[(vk as usize >> 6) & 3], 1 << (vk & 63))
}

fn set_eaten(vk: u16) {
    let (slot, bit) = eaten_slot(vk);
    slot.fetch_or(bit, SeqCst);
}

fn take_eaten(vk: u16) -> bool {
    let (slot, bit) = eaten_slot(vk);
    slot.fetch_and(!bit, SeqCst) & bit != 0
}

pub fn hold() {
    HOLD_SINCE.store(unsafe { GetTickCount() }, SeqCst);
    HOLD.store(true, SeqCst);
}

fn holding() -> bool {
    if !HOLD.load(SeqCst) {
        return false;
    }
    if unsafe { GetTickCount() }.wrapping_sub(HOLD_SINCE.load(SeqCst)) > HOLD_LIMIT_MS {
        HOLD.store(false, SeqCst);
        return false;
    }
    true
}

fn down(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) < 0 }
}

fn push(ev: HookEvent) {
    if let Some(tx) = SENDER.get() {
        EVENTS.fetch_add(1, SeqCst);
        if tx.try_send(ev).is_err() {
            DROPPED.fetch_add(1, SeqCst);
        }
    }
}

fn is_modifier(vk: u16) -> bool {
    matches!(
        VIRTUAL_KEY(vk),
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU
            | VK_LWIN | VK_RWIN | VK_CAPITAL
    )
}

fn hotkey_matches(code: u32, vk: u16, k: &RawKey) -> bool {
    code != 0
        && (code & 0xFFFF) as u16 == vk
        && (code & MOD_SHIFT != 0) == k.shift
        && (code & MOD_CTRL != 0) == k.ctrl
        && (code & MOD_ALT != 0) == k.alt
}

fn on_key(kb: &KBDLLHOOKSTRUCT) -> bool {
    let flags = kb.flags;
    let up = flags.contains(LLKHF_UP);
    if flags.contains(LLKHF_INJECTED) && !ACCEPT_INJECTED.load(SeqCst) {
        PENDING.store(false, SeqCst);
        push(HookEvent::Foreign);
        return false;
    }
    let vk = kb.vkCode as u16;
    let mut k = RawKey {
        vk,
        scan: kb.scanCode as u16,
        up,
        extended: flags.contains(LLKHF_EXTENDED),
        shift: down(VK_SHIFT),
        ctrl: down(VK_CONTROL),
        alt: down(VK_MENU),
        win: down(VK_LWIN) || down(VK_RWIN),
        eaten: false,
        held: false,
        at: std::time::Instant::now(),
    };
    if up && take_eaten(vk) {
        k.eaten = true;
        push(HookEvent::Key(k));
        return true;
    }
    if holding() {
        k.held = true;
        push(HookEvent::Key(k));
        return true;
    }
    if up {
        push(HookEvent::Key(k));
        return false;
    }
    let active = ENABLED.load(SeqCst) && ALLOWED.load(SeqCst);
    let toggle = hotkey_matches(HK_TOGGLE.load(SeqCst), vk, &k);
    let held = (active && hotkey_matches(HK_CONVERT.load(SeqCst), vk, &k))
        || (vk == VK_BACK.0 && k.ctrl && !k.alt && ARMED_UNDO.load(SeqCst))
        || (vk == VK_RETURN.0 && active && PENDING.load(SeqCst) && !k.ctrl && !k.alt);
    if held {
        hold();
    }
    let eat = toggle || held;
    if !(is_modifier(vk) || vk == VK_BACK.0 && eat) {
        ARMED_UNDO.store(false, SeqCst);
    }
    if eat {
        PENDING.store(false, SeqCst);
        set_eaten(vk);
        k.eaten = true;
        push(HookEvent::Key(k));
        return true;
    }
    if !is_modifier(vk) {
        let key = Key::new(k.scan, k.shift, false);
        match lang() {
            _ if k.ctrl || k.alt || k.win => PENDING.store(false, SeqCst),
            Some(l) if separator_kind(key, l).is_some() => {
                if active && PENDING.swap(false, SeqCst) {
                    hold();
                }
            }
            Some(l) if key.char_in(l).is_some() => PENDING.store(true, SeqCst),
            _ if vk == VK_BACK.0 => {}
            _ => PENDING.store(false, SeqCst),
        }
    }
    push(HookEvent::Key(k));
    false
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        LAST_TICK.store(GetTickCount(), SeqCst);
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if kb.dwExtraInfo != MARK {
            let eat = std::panic::catch_unwind(|| on_key(kb)).unwrap_or(false);
            if eat {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        LAST_TICK.store(GetTickCount(), SeqCst);
        if matches!(wparam.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN) {
            PENDING.store(false, SeqCst);
            ARMED_UNDO.store(false, SeqCst);
            push(HookEvent::Click);
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn install() -> (Option<HHOOK>, Option<HHOOK>) {
    unsafe {
        (
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0).ok(),
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0).ok(),
        )
    }
}

fn uninstall(hooks: (Option<HHOOK>, Option<HHOOK>)) {
    unsafe {
        if let Some(h) = hooks.0 {
            let _ = UnhookWindowsHookEx(h);
        }
        if let Some(h) = hooks.1 {
            let _ = UnhookWindowsHookEx(h);
        }
    }
}

pub fn run(tx: Sender<HookEvent>) -> std::thread::JoinHandle<()> {
    let _ = SENDER.set(tx);
    std::thread::Builder::new()
        .name("ghbdtn-hook".into())
        .spawn(|| unsafe {
            HOOK_THREAD.store(GetCurrentThreadId(), SeqCst);
            LAST_TICK.store(GetTickCount(), SeqCst);
            let mut hooks = install();
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                match msg.message {
                    WM_REINSTALL => {
                        uninstall(hooks);
                        hooks = install();
                        LAST_TICK.store(GetTickCount(), SeqCst);
                    }
                    WM_QUIT_HOOK => break,
                    _ => {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
            uninstall(hooks);
        })
        .expect("hook thread")
}

fn post(msg: u32) {
    let id = HOOK_THREAD.load(SeqCst);
    if id != 0 {
        unsafe {
            let _ = PostThreadMessageW(id, msg, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn reinstall() {
    if HOOK_THREAD.load(SeqCst) != 0 {
        REINSTALLS.fetch_add(1, SeqCst);
    }
    post(WM_REINSTALL);
}

pub fn stop() {
    post(WM_QUIT_HOOK);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys() {
        assert_eq!(parse_hotkey("Pause"), Some(VK_PAUSE.0 as u32));
        assert_eq!(parse_hotkey("Shift+Pause"), Some(MOD_SHIFT | VK_PAUSE.0 as u32));
        assert_eq!(parse_hotkey("ctrl+alt+F12"), Some(MOD_CTRL | MOD_ALT | VK_F12.0 as u32));
        assert_eq!(parse_hotkey("Ctrl+Q"), Some(MOD_CTRL | b'Q' as u32));
        assert_eq!(parse_hotkey("Hyper+X"), None);
    }
}
