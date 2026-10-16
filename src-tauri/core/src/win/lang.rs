use super::output;
use crate::config::{Program, Switch};
use crate::keymap::Lang;
use std::sync::atomic::{AtomicU8, Ordering::SeqCst};
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardLayoutList, HKL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_OEM_3,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, PostMessageW, GUITHREADINFO,
    WM_INPUTLANGCHANGEREQUEST,
};

fn langid(hkl: HKL) -> u16 {
    (hkl.0 as usize & 0xFFFF) as u16
}

fn is_ime(hkl: HKL) -> bool {
    (hkl.0 as usize >> 28) & 0xF == 0xE
}

pub fn foreground_thread() -> (HWND, u32) {
    unsafe {
        let hwnd = GetForegroundWindow();
        (hwnd, GetWindowThreadProcessId(hwnd, None))
    }
}

pub fn focus_window() -> HWND {
    let (hwnd, thread) = foreground_thread();
    let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    match unsafe { GetGUIThreadInfo(thread, &mut info) } {
        Ok(()) if !info.hwndFocus.is_invalid() => info.hwndFocus,
        _ => hwnd,
    }
}

pub fn current() -> Option<Lang> {
    current_of(foreground_thread().1)
}

fn current_of(thread: u32) -> Option<Lang> {
    let hkl = unsafe { GetKeyboardLayout(thread) };
    if is_ime(hkl) {
        return None;
    }
    Lang::from_langid(langid(hkl))
}

fn layouts() -> Vec<HKL> {
    unsafe {
        let n = GetKeyboardLayoutList(None);
        let mut list = vec![HKL::default(); n.max(0) as usize];
        let got = GetKeyboardLayoutList(Some(&mut list));
        list.truncate(got.max(0) as usize);
        list
    }
}

pub fn hkl_for(lang: Lang) -> Option<HKL> {
    layouts().into_iter().find(|&h| !is_ime(h) && langid(h) == lang.langid())
}

pub fn has_ru_en() -> bool {
    hkl_for(Lang::Ru).is_some() && hkl_for(Lang::En).is_some()
}

pub fn request(lang: Lang) -> Option<u32> {
    let hkl = hkl_for(lang)?;
    let thread = foreground_thread().1;
    unsafe { PostMessageW(Some(focus_window()), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(hkl.0 as isize)) }.ok()?;
    Some(thread)
}

pub fn wait(lang: Lang, thread: u32) -> bool {
    let start = Instant::now();
    loop {
        if current_of(thread) == Some(lang) {
            return true;
        }
        if start.elapsed() >= Duration::from_millis(50) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub fn switch(lang: Lang) -> bool {
    request(lang).is_some_and(|thread| wait(lang, thread))
}

pub fn current_hkl() -> HKL {
    unsafe { GetKeyboardLayout(foreground_thread().1) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    AltShift,
    CtrlShift,
    Grave,
}

fn toggle_from(value: &str) -> Option<Toggle> {
    match value.trim() {
        "1" => Some(Toggle::AltShift),
        "2" => Some(Toggle::CtrlShift),
        "4" => Some(Toggle::Grave),
        _ => None,
    }
}

fn registry_string(name: PCWSTR) -> Option<String> {
    let mut buf = [0u16; 16];
    let mut size = std::mem::size_of_val(&buf) as u32;
    let status = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, w!("Keyboard Layout\\Toggle"), name, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size))
    };
    status.is_ok().then(|| String::from_utf16_lossy(&buf[..(size as usize / 2).saturating_sub(1)]))
}

pub fn toggle() -> Option<Toggle> {
    let value = registry_string(w!("Language Hotkey")).or_else(|| registry_string(w!("Hotkey"))).unwrap_or_else(|| "1".into());
    toggle_from(&value)
}

fn press(toggle: Toggle) {
    match toggle {
        Toggle::AltShift => output::press_keys(&[VK_LMENU, VK_LSHIFT]),
        Toggle::CtrlShift => output::press_keys(&[VK_LCONTROL, VK_LSHIFT]),
        Toggle::Grave => output::press_keys(&[VK_OEM_3]),
    };
}

fn poll(thread: u32, until: impl Fn(HKL) -> bool, limit: Duration) -> bool {
    let start = Instant::now();
    loop {
        if until(unsafe { GetKeyboardLayout(thread) }) {
            return true;
        }
        if start.elapsed() >= limit {
            return false;
        }
        std::thread::sleep(HOTKEY_POLL);
    }
}

const HOTKEY_POLL: Duration = Duration::from_millis(3);
const HOTKEY_LIMIT: Duration = Duration::from_millis(250);

fn switch_hotkey(lang: Lang, toggle: Toggle) -> bool {
    let thread = foreground_thread().1;
    for _ in 1..layouts().len().max(2) {
        if current_of(thread) == Some(lang) {
            return true;
        }
        let before = unsafe { GetKeyboardLayout(thread) };
        press(toggle);
        if !poll(thread, |h| h != before, HOTKEY_LIMIT) {
            HOTKEY_MISSES.fetch_add(1, SeqCst);
            return false;
        }
        HOTKEY_MISSES.store(0, SeqCst);
    }
    current_of(thread) == Some(lang)
}

static HOTKEY_MISSES: AtomicU8 = AtomicU8::new(0);
const HOTKEY_DEAD_AFTER: u8 = 2;

pub fn hotkey_dead() -> bool {
    HOTKEY_MISSES.load(SeqCst) >= HOTKEY_DEAD_AFTER
}

fn hotkey_for(program: &Program, toggle: impl FnOnce() -> Option<Toggle>, dead: bool) -> Option<Toggle> {
    (program.switch == Switch::Hotkey && !dead).then(toggle).flatten()
}

pub fn switch_with(lang: Lang, program: &Program) -> bool {
    if current() == Some(lang) {
        return true;
    }
    let ok = hotkey_for(program, toggle, hotkey_dead()).is_some_and(|t| switch_hotkey(lang, t))
        || current() == Some(lang)
        || (0..program.retries.max(1)).any(|_| switch(lang));
    if ok && program.delay_ms > 0 {
        std::thread::sleep(Duration::from_millis(program.delay_ms as u64));
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_values() {
        assert_eq!(toggle_from("1"), Some(Toggle::AltShift));
        assert_eq!(toggle_from("2"), Some(Toggle::CtrlShift));
        assert_eq!(toggle_from("4"), Some(Toggle::Grave));
        assert_eq!(toggle_from("3"), None);
        assert_eq!(toggle_from(""), None);
    }

    #[test]
    fn method_chain() {
        let hotkey = Program { switch: Switch::Hotkey, ..Program::default() };
        let alt = || Some(Toggle::AltShift);
        assert_eq!(hotkey_for(&hotkey, alt, false), Some(Toggle::AltShift));
        assert_eq!(hotkey_for(&hotkey, alt, true), None);
        assert_eq!(hotkey_for(&hotkey, || None, false), None);
        assert_eq!(hotkey_for(&Program::default(), alt, false), None);
    }
}
