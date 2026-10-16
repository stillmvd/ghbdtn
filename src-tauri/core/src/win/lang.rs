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
