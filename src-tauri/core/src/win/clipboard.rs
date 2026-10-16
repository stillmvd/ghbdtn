use super::output;
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use std::sync::Mutex;
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
    let saved = take_saved().unwrap_or_else(|| clipboard_win::get_clipboard_string().ok());
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

struct Staged {
    saved: Option<String>,
    at: Instant,
    seq: Option<u32>,
    generation: u64,
}

static STAGED: Mutex<Option<Staged>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);
const PASTE_GAP: Duration = Duration::from_millis(80);
const RESTORE_AFTER: Duration = Duration::from_millis(400);
const PLAIN_FORMATS: [u32; 4] = [1, 7, 13, 16];

fn staged() -> std::sync::MutexGuard<'static, Option<Staged>> {
    STAGED.lock().unwrap_or_else(|e| e.into_inner())
}

fn plain_saved() -> Option<Option<String>> {
    let plain = {
        let _open = clipboard_win::Clipboard::new_attempts(10).ok()?;
        clipboard_win::raw::EnumFormats::new().all(|f| PLAIN_FORMATS.contains(&f))
    };
    plain.then(|| clipboard_win::get_clipboard_string().ok())
}

pub fn take_saved() -> Option<Option<String>> {
    staged().take().map(|s| s.saved)
}

pub fn stage(text: &str) -> bool {
    let mut staged = staged();
    let saved = match staged.take() {
        Some(prev) => {
            if let Some(wait) = PASTE_GAP.checked_sub(prev.at.elapsed()) {
                std::thread::sleep(wait);
            }
            prev.saved
        }
        None => match plain_saved() {
            Some(saved) => saved,
            None => return false,
        },
    };
    let ok = put(text);
    let generation = GENERATION.fetch_add(1, SeqCst) + 1;
    *staged = Some(Staged { saved, at: Instant::now(), seq: clipboard_win::raw::seq_num().map(|n| n.get()), generation });
    std::thread::spawn(move || {
        std::thread::sleep(RESTORE_AFTER);
        let mut staged = self::staged();
        if let Some(s) = staged.take_if(|s| s.generation == generation) {
            if clipboard_win::raw::seq_num().map(|n| n.get()) == s.seq {
                restore(s.saved);
            }
        }
    });
    ok
}

pub fn pasted() {
    if let Some(s) = staged().as_mut() {
        s.at = Instant::now();
    }
}

fn restore(saved: Option<String>) {
    match saved {
        Some(s) => {
            put(&s);
        }
        None => {
            if let Ok(_open) = clipboard_win::Clipboard::new_attempts(10) {
                let _ = clipboard_win::raw::empty();
            }
        }
    }
}

pub fn paste(text: &str, saved: Option<String>) {
    if !put(text) {
        return;
    }
    output::chord(VK_CONTROL, VK_V);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        restore(saved);
    });
}
