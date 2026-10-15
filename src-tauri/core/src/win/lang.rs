use crate::keymap::Lang;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, GetKeyboardLayoutList, HKL};
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
    let (_, thread) = foreground_thread();
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

pub fn switch(lang: Lang) -> bool {
    let Some(hkl) = hkl_for(lang) else { return false };
    let target = focus_window();
    unsafe {
        let _ = PostMessageW(Some(target), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(hkl.0 as isize));
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(50) {
        if current() == Some(lang) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}
