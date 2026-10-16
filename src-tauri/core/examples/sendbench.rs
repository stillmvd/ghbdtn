use std::time::{Duration, Instant};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

unsafe extern "system" fn proc_(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    CallNextHookEx(None, code, w, l)
}

fn events(n: usize) -> Vec<INPUT> {
    (0..n)
        .map(|i| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0x88),
                    wScan: 0,
                    dwFlags: if i % 2 == 1 { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
        .collect()
}

fn measure(label: &str, n: usize) {
    let ev = events(n);
    let mut total = Duration::ZERO;
    for _ in 0..10 {
        let t = Instant::now();
        unsafe { SendInput(&ev, std::mem::size_of::<INPUT>() as i32) };
        total += t.elapsed();
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("{label}: {:.2} мс на {n} событий", total.as_secs_f64() * 100.0);
}

fn hook_thread(critical: bool) {
    std::thread::spawn(move || unsafe {
        if critical {
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        }
        let _h = SetWindowsHookExW(WH_KEYBOARD_LL, Some(proc_), None, 0);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
    });
    std::thread::sleep(Duration::from_millis(300));
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "hook" => hook_thread(false),
        "critical" => hook_thread(true),
        _ => {}
    }
    for n in [2, 10, 40, 80] { measure(&mode, n); }
}
