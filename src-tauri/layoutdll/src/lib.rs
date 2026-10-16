#![cfg(windows)]

use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use windows::core::w;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{ActivateKeyboardLayout, GetKeyboardLayoutList, ACTIVATE_KEYBOARD_LAYOUT_FLAGS, HKL};
use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, RegisterWindowMessageW, HC_ACTION, MSG, PM_REMOVE, WM_NULL};

static MESSAGE: AtomicU32 = AtomicU32::new(0);

fn message() -> u32 {
    match MESSAGE.load(Relaxed) {
        0 => {
            let m = unsafe { RegisterWindowMessageW(w!("GhbdtnActivateLayout")) };
            MESSAGE.store(m, Relaxed);
            m
        }
        m => m,
    }
}

fn installed(hkl: HKL) -> bool {
    let mut list = [HKL::default(); 32];
    let n = unsafe { GetKeyboardLayoutList(Some(&mut list)) }.max(0) as usize;
    list[..n.min(list.len())].contains(&hkl)
}

/// # Safety
/// Вызывается системой как процедура `WH_GETMESSAGE`; `lparam` указывает на `MSG`.
#[no_mangle]
pub unsafe extern "system" fn layout_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && wparam.0 == PM_REMOVE.0 as usize && lparam.0 != 0 {
        let msg = &mut *(lparam.0 as *mut MSG);
        if msg.message != 0 && msg.message == message() {
            let hkl = HKL(msg.lParam.0 as _);
            if installed(hkl) {
                let _ = ActivateKeyboardLayout(hkl, ACTIVATE_KEYBOARD_LAYOUT_FLAGS(0));
            }
            msg.message = WM_NULL;
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}
