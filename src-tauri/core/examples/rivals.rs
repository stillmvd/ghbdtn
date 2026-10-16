use ghbdtn_core::keymap::{key_of, Lang};
use ghbdtn_core::win::{hook, lang, Engine};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const CASES: &[(&str, &str, Lang, &str)] = &[
    ("layout_ru", "привет как дела ", Lang::En, "привет как дела "),
    ("layout_ru", "сегодня хорошая погода ", Lang::En, "сегодня хорошая погода "),
    ("layout_ru", "я думаю что это работает ", Lang::En, "я думаю что это работает "),
    ("layout_ru", "напиши мне завтра утром ", Lang::En, "напиши мне завтра утром "),
    ("layout_ru", "можно посмотреть документы ", Lang::En, "можно посмотреть документы "),
    ("layout_ru", "где находится ближайшая аптека ", Lang::En, "где находится ближайшая аптека "),
    ("layout_ru", "отправь отчёт коллегам ", Lang::En, "отправь отчёт коллегам "),
    ("layout_en", "hello world ", Lang::Ru, "hello world "),
    ("layout_en", "please check the build ", Lang::Ru, "please check the build "),
    ("layout_en", "open the settings window ", Lang::Ru, "open the settings window "),
    ("layout_en", "this is a simple test ", Lang::Ru, "this is a simple test "),
    ("layout_en", "remote server returned error ", Lang::Ru, "remote server returned error "),
    ("short", "мы ей позвоним ", Lang::En, "мы ей позвоним "),
    ("short", "он и я ", Lang::En, "он и я "),
    ("short", "да нет ", Lang::En, "да нет "),
    ("short", "ну ок ", Lang::En, "ну ок "),
    ("keep", "привет мир это обычный текст ", Lang::Ru, "привет мир это обычный текст "),
    ("keep", "the quick brown fox jumps ", Lang::En, "the quick brown fox jumps "),
    ("keep", "git npm api json url ", Lang::En, "git npm api json url "),
    ("keep", "ООО ИП НДС ", Lang::Ru, "ООО ИП НДС "),
    ("keep", "iPhone macOS JavaScript ", Lang::En, "iPhone macOS JavaScript "),
    ("keep", "ok yes no ", Lang::En, "ok yes no "),
    ("keep", "rust tauri react ", Lang::En, "rust tauri react "),
    ("typo", "приверяю почту ", Lang::Ru, "проверяю почту "),
    ("typo", "я увдел его ", Lang::Ru, "я увидел его "),
    ("typo", "ввожа клавиатра ", Lang::Ru, "ввода клавиатура "),
    ("typo", "сеогдня хорошо ", Lang::Ru, "сегодня хорошо "),
    ("typo", "teh project ", Lang::En, "the project "),
    ("mixed", "запусти build и проверь ", Lang::Ru, "запусти build и проверь "),
    ("mixed", "открой github потом почту ", Lang::Ru, "открой github потом почту "),
];

static EXTRA: AtomicUsize = AtomicUsize::new(0);

fn input(scan: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_SCANCODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: EXTRA.load(SeqCst) } },
    }
}

fn vk_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, time: 0, dwExtraInfo: EXTRA.load(SeqCst) },
        },
    }
}

fn send(i: &[INPUT]) {
    unsafe { SendInput(i, std::mem::size_of::<INPUT>() as i32) };
}

fn guard(target: HWND) {
    if unsafe { GetForegroundWindow() } != target {
        eprintln!("ABORT: фокус ушёл из Блокнота");
        std::process::exit(2);
    }
}

fn press(c: char, gap: u64) {
    let l = if c.is_ascii_alphabetic() { Lang::En } else { Lang::Ru };
    let (scan, shift) = key_of(c, l).or_else(|| key_of(c, Lang::En)).expect("char");
    if shift {
        send(&[input(0x2A, false)]);
    }
    send(&[input(scan, false), input(scan, true)]);
    if shift {
        send(&[input(0x2A, true)]);
    }
    std::thread::sleep(Duration::from_millis(gap));
}

fn text_of(top: HWND) -> String {
    unsafe extern "system" fn find(child: HWND, lp: LPARAM) -> windows::core::BOOL {
        let out = &mut *(lp.0 as *mut HWND);
        let mut buf = [0u16; 64];
        let n = GetClassNameW(child, &mut buf);
        let class = String::from_utf16_lossy(&buf[..n as usize]);
        if class.starts_with("RichEdit") || class == "Edit" {
            *out = child;
            return false.into();
        }
        true.into()
    }
    unsafe {
        let mut edit = HWND::default();
        let _ = EnumChildWindows(Some(top), Some(find), LPARAM(&mut edit as *mut _ as isize));
        let mut buf = vec![0u16; 4096];
        let n = SendMessageW(edit, WM_GETTEXT, Some(WPARAM(buf.len())), Some(LPARAM(buf.as_mut_ptr() as isize))).0;
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

fn clear(np: HWND) {
    guard(np);
    send(&[vk_input(VK_CONTROL, false), vk_input(VK_A, false), vk_input(VK_A, true), vk_input(VK_CONTROL, true)]);
    std::thread::sleep(Duration::from_millis(80));
    send(&[vk_input(VK_DELETE, false), vk_input(VK_DELETE, true)]);
    std::thread::sleep(Duration::from_millis(150));
}

fn settle(np: HWND, echo: &str) -> (String, Option<f64>) {
    let t0 = Instant::now();
    let mut last = text_of(np);
    let mut changed_at: Option<Duration> = None;
    let mut stable_since = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(2));
        let now = text_of(np);
        if now != last {
            last = now;
            stable_since = Instant::now();
            if last != echo {
                changed_at = Some(t0.elapsed());
            }
        }
        let idle = stable_since.elapsed();
        let waited = t0.elapsed();
        if waited > Duration::from_millis(1500) || (idle > Duration::from_millis(300) && waited > Duration::from_millis(400)) {
            break;
        }
    }
    let ms = changed_at.filter(|_| last != echo).map(|d| d.as_secs_f64() * 1000.0);
    (last, ms)
}

fn find_notepad(file: &str) -> HWND {
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(200));
        let hwnd = unsafe { GetForegroundWindow() };
        let mut buf = [0u16; 256];
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
        if String::from_utf16_lossy(&buf[..n as usize]).contains(file) {
            return hwnd;
        }
    }
    eprintln!("ABORT: Блокнот не стал активным");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let target = args.get(1).cloned().unwrap_or_else(|| "external".into());
    let out = args.get(2).cloned().unwrap_or_else(|| format!("rivals-{target}.jsonl"));
    let gap: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(35);
    let dir = std::env::temp_dir().join("ghbdtn-rivals");
    let _ = std::fs::create_dir_all(&dir);
    let file = format!("rivals-{target}.txt");
    std::fs::write(dir.join(&file), "").unwrap();
    if target == "installed" {
        EXTRA.store(hook::TEST_MARK, SeqCst);
    }
    let _engine = (target == "ghbdtn").then(|| {
        hook::ACCEPT_INJECTED.store(true, SeqCst);
        let cfg = dir.join("config.toml");
        let _ = std::fs::remove_file(&cfg);
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || while rx.recv().is_ok() {});
        Engine::start(cfg, tx)
    });
    let mut notepad = std::process::Command::new("notepad.exe").arg(dir.join(&file)).spawn().expect("notepad");
    let np = find_notepad(&file);
    std::thread::sleep(Duration::from_millis(800));

    let mut lines = Vec::new();
    let (mut ok_n, mut lat) = (0, Vec::<f64>::new());
    for (cat, text, start, want) in CASES {
        clear(np);
        lang::switch(*start);
        std::thread::sleep(Duration::from_millis(250));
        let mut words = Vec::new();
        for word in text.split_inclusive(' ') {
            for c in word.trim_end().chars() {
                guard(np);
                press(c, gap);
            }
            std::thread::sleep(Duration::from_millis(gap));
            let echo = format!("{} ", text_of(np));
            guard(np);
            press(' ', 0);
            let (_, ms) = settle(np, &echo);
            if let Some(ms) = ms {
                lat.push(ms);
            }
            words.push(ms);
        }
        let got = text_of(np);
        let ok = got == *want;
        ok_n += ok as usize;
        println!("{} [{cat}] {:?} → {:?} {:?}", if ok { "OK  " } else { "FAIL" }, want, got, words);
        lines.push(serde_json::json!({"target": target, "mode": "paced", "cat": cat, "want": want, "got": got, "ok": ok, "ms": words}));
    }
    let mut burst_ok = 0;
    let burst: Vec<_> = CASES.iter().filter(|c| c.0 == "layout_ru" || c.0 == "layout_en").collect();
    for (cat, text, start, want) in &burst {
        clear(np);
        lang::switch(*start);
        std::thread::sleep(Duration::from_millis(250));
        for c in text.chars() {
            guard(np);
            press(c, 5);
        }
        std::thread::sleep(Duration::from_millis(1200));
        let got = text_of(np);
        let ok = got == **want;
        burst_ok += ok as usize;
        println!("{} [burst {cat}] {:?} → {:?}", if ok { "OK  " } else { "FAIL" }, want, got);
        lines.push(serde_json::json!({"target": target, "mode": "burst", "cat": cat, "want": want, "got": got, "ok": ok}));
    }
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |p: f64| lat.get(((lat.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(0.0);
    let summary = serde_json::json!({
        "target": target, "mode": "summary", "paced_ok": ok_n, "paced_total": CASES.len(),
        "burst_ok": burst_ok, "burst_total": burst.len(), "actions": lat.len(),
        "ms_p50": pct(0.5), "ms_p95": pct(0.95), "ms_max": lat.last().copied().unwrap_or(0.0),
        "engine_fixes": hook::FIXES.load(SeqCst),
        "engine_ms_avg": hook::FIX_US_TOTAL.load(SeqCst).checked_div(hook::FIXES.load(SeqCst)).unwrap_or(0) as f64 / 1000.0,
        "engine_ms_max": hook::FIX_US_MAX.load(SeqCst) as f64 / 1000.0,
        "replayed": hook::REPLAYED.load(SeqCst), "dropped": hook::DROPPED.load(SeqCst),
        "engine_ms_without_send": hook::engine_ms_without_send(), "send_ms_per_event": hook::SEND.per_unit_ms(), "hold_left": hook::hold_state(),
        "stages": hook::STAGES.iter().map(|s| (s.name.to_string(), serde_json::json!({"n": s.count(), "avg_ms": s.avg_ms(), "max_ms": s.max_ms()}))).collect::<serde_json::Map<_, _>>(),
    });
    println!("{summary}");
    lines.push(summary);
    let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(&out, body).unwrap();
    clear(np);
    send(&[vk_input(VK_CONTROL, false), vk_input(VK_S, false), vk_input(VK_S, true), vk_input(VK_CONTROL, true)]);
    std::thread::sleep(Duration::from_millis(300));
    unsafe {
        let _ = PostMessageW(Some(np), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
    let _ = notepad.wait();
}
