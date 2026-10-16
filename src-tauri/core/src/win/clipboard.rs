use super::output;
use std::time::{Duration, Instant};
use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_C, VK_CONTROL, VK_V};

fn wait_change(seq: u32, limit: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if unsafe { GetClipboardSequenceNumber() } != seq {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

pub fn copy_selection() -> Option<(String, Option<String>)> {
    let saved = clipboard_win::get_clipboard_string().ok();
    let seq = unsafe { GetClipboardSequenceNumber() };
    output::chord(VK_CONTROL, VK_C);
    if !wait_change(seq, Duration::from_millis(500)) {
        return None;
    }
    std::thread::sleep(Duration::from_millis(10));
    let text = clipboard_win::get_clipboard_string().ok().filter(|t| !t.is_empty())?;
    Some((text, saved))
}

fn put(text: &str) -> bool {
    let Ok(_open) = clipboard_win::Clipboard::new_attempts(10) else { return false };
    if clipboard_win::raw::set_string(text).is_err() {
        return false;
    }
    for name in ["ExcludeClipboardContentFromMonitorProcessing", "CanIncludeInClipboardHistory", "CanUploadToCloudClipboard", "Clipboard Viewer Ignore"] {
        if let Some(f) = clipboard_win::raw::register_format(name) {
            let _ = clipboard_win::raw::set_without_clear(f.get(), &0u32.to_le_bytes());
        }
    }
    true
}

pub fn paste(text: &str, saved: Option<String>) {
    if !put(text) {
        return;
    }
    output::chord(VK_CONTROL, VK_V);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        if let Some(s) = saved {
            put(&s);
        }
    });
}
