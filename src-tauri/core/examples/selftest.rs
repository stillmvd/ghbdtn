use ghbdtn_core::config::{Insert, Program, Switch};
use ghbdtn_core::keymap::{key_of, Lang};
use ghbdtn_core::win::{hook, lang, CoreEvent, Engine};
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

fn input(vk: u16, scan: u16, up: bool) -> INPUT {
    let mut flags = if scan != 0 { KEYEVENTF_SCANCODE } else { KEYBD_EVENT_FLAGS(0) };
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

static GAP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(25);

fn send(i: &[INPUT]) {
    unsafe { SendInput(i, std::mem::size_of::<INPUT>() as i32) };
    std::thread::sleep(Duration::from_millis(GAP.load(SeqCst)));
}

fn clear(np: HWND) {
    tap_vk(np, &[VK_CONTROL, VK_A]);
    tap_vk(np, &[VK_DELETE]);
}

static EVENTS: std::sync::OnceLock<crossbeam_channel::Receiver<CoreEvent>> = std::sync::OnceLock::new();

fn trace() -> Vec<String> {
    EVENTS
        .get()
        .map(|rx| {
            rx.try_iter()
                .filter_map(|ev| match ev {
                    CoreEvent::Acted { kind, rule, typed, result } => Some(format!("acted {kind:?} {rule:?} {typed:?} → {result:?}")),
                    CoreEvent::Undone => Some("undone".into()),
                    CoreEvent::NeverAdded(w) => Some(format!("never {w:?}")),
                    CoreEvent::HookReinstalled => Some("reinstall".into()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn report(failed: &mut i32, ok: bool, line: String) {
    *failed += !ok as i32;
    let events = trace();
    println!("{} {line}", if ok { "OK  " } else { "FAIL" });
    if !ok {
        for e in events {
            println!("       · {e}");
        }
    }
}

fn guard(target: HWND) {
    if unsafe { GetForegroundWindow() } != target {
        eprintln!("ABORT: фокус ушёл из Блокнота");
        std::process::exit(2);
    }
}

fn type_text(target: HWND, text: &str, lang: Lang) {
    for c in text.chars() {
        guard(target);
        let (scan, shift) = key_of(c, lang).expect("char");
        if shift {
            send(&[input(0, 0x2A, false)]);
        }
        send(&[input(0, scan, false), input(0, scan, true)]);
        if shift {
            send(&[input(0, 0x2A, true)]);
        }
    }
    std::thread::sleep(Duration::from_millis(700));
}

fn tap_vk(target: HWND, vks: &[VIRTUAL_KEY]) {
    guard(target);
    let mut v: Vec<INPUT> = vks.iter().map(|k| input(k.0, 0, false)).collect();
    v.extend(vks.iter().rev().map(|k| input(k.0, 0, true)));
    send(&v);
    std::thread::sleep(Duration::from_millis(700));
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
    let dir = std::env::temp_dir().join("ghbdtn-selftest");
    let _ = std::fs::create_dir_all(&dir);
    let cfg_path = dir.join("config.toml");
    let _ = std::fs::remove_file(&cfg_path);
    let file = "ghbdtn-selftest.txt";
    let doc = dir.join(file);
    std::fs::write(&doc, "").unwrap();
    hook::ACCEPT_INJECTED.store(true, SeqCst);
    let (tx, rx) = crossbeam_channel::unbounded();
    let engine = Engine::start(cfg_path, tx);
    let _ = EVENTS.set(rx);
    std::process::Command::new("notepad.exe").arg(&doc).spawn().unwrap();
    let np = find_notepad(file);
    std::thread::sleep(Duration::from_millis(500));

    let cases: &[(&str, Lang, &str)] = &[
        ("ghbdtn vbh ", Lang::En, "привет мир "),
        ("сщву ", Lang::Ru, "code "),
        ("ghbdtn? ", Lang::En, "привет, "),
        ("ввожа клавиатра ", Lang::Ru, "ввода клавиатура "),
        ("чето ", Lang::Ru, "чё-то "),
        ("hello ", Lang::En, "hello "),
        ("in english b lf;t ", Lang::En, "in english и даже "),
        ("пкфафтф ", Lang::Ru, "grafana "),
        ("приверяю ", Lang::Ru, "проверяю "),
        ("увдел ", Lang::Ru, "увидел "),
        ("я нк знаю ", Lang::Ru, "я не знаю "),
        ("я, нк ", Lang::Ru, "я, нк "),
        ("гу ", Lang::Ru, "ну "),
        ("по хожу ", Lang::Ru, "по ходу "),
    ];
    let mut failed = 0;
    for (input_text, l, want) in cases {
        tap_vk(np, &[VK_CONTROL, VK_A]);
        tap_vk(np, &[VK_DELETE]);
        lang::switch(*l);
        std::thread::sleep(Duration::from_millis(200));
        type_text(np, input_text, *l);
        let got = text_of(np);
        report(&mut failed, got == *want, format!("{input_text:?} → {got:?} (ждали {want:?})"));
    }

    tap_vk(np, &[VK_CONTROL, VK_A]);
    tap_vk(np, &[VK_DELETE]);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    type_text(np, "ghbdtn ", Lang::En);
    tap_vk(np, &[VK_CONTROL, VK_BACK]);
    let got = text_of(np);
    let ok = got == "ghbdtn " && lang::current() == Some(Lang::En);
    failed += !ok as i32;
    println!("{} undo → {:?}, раскладка {:?}", if ok { "OK  " } else { "FAIL" }, got, lang::current());

    tap_vk(np, &[VK_CONTROL, VK_A]);
    tap_vk(np, &[VK_DELETE]);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    type_text(np, "ghbd", Lang::En);
    let early = text_of(np);
    tap_vk(np, &[VK_CONTROL, VK_BACK]);
    let undone = (text_of(np), lang::current());
    type_text(np, "tn ", Lang::En);
    let got = text_of(np);
    let never = engine.config().is_never("ghbdtn");
    report(
        &mut failed,
        early == "прив" && undone == ("ghbd".to_string(), Some(Lang::En)) && got == "ghbdtn " && never,
        format!("ранняя правка {early:?}, отмена посреди слова {undone:?}, дальше {got:?}, вторая отмена → не трогать {never}"),
    );
    let mut c = engine.config();
    c.never.clear();
    engine.set_config(c);

    tap_vk(np, &[VK_CONTROL, VK_A]);
    tap_vk(np, &[VK_DELETE]);
    type_text(np, "ye ", Lang::En);
    tap_vk(np, &[VK_PAUSE]);
    let got = text_of(np);
    let ok = got == "ну ";
    failed += !ok as i32;
    println!("{} pause → {:?}", if ok { "OK  " } else { "FAIL" }, got);

    let fixes = hook::FIXES.load(SeqCst);
    let max_ms = hook::FIX_US_MAX.load(SeqCst) as f64 / 1000.0;
    let dropped = hook::DROPPED.load(SeqCst);
    report(
        &mut failed,
        fixes > 0 && max_ms < 150.0 && dropped == 0 && hook::REINSTALLS.load(SeqCst) == 0,
        format!(
            "скорость: исправлений {fixes}, ср {:.1} мс, макс {max_ms:.1} мс, потеряно {dropped}, переотправлено {}",
            hook::FIX_US_TOTAL.load(SeqCst).checked_div(fixes).unwrap_or(0) as f64 / 1000.0,
            hook::REPLAYED.load(SeqCst)
        ),
    );
    let core_ms = hook::engine_ms_without_send();
    report(&mut failed, core_ms <= 15.0, format!("движок без SendInput {core_ms:.2} мс ср (порог 15); шаги: {}", hook::profile()));

    clear(np);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    GAP.store(2, SeqCst);
    type_text(np, "ghbdtn vbh ghbdtn ", Lang::En);
    GAP.store(25, SeqCst);
    let got = text_of(np);
    report(&mut failed, got == "привет мир привет ", format!("быстро 2 мс → {got:?}"));

    clear(np);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    type_text(np, "ye ye", Lang::En);
    tap_vk(np, &[VK_CONTROL, VK_A]);
    tap_vk(np, &[VK_PAUSE]);
    let got = text_of(np);
    report(&mut failed, got == "ну ну", format!("выделение+Pause → {got:?}"));

    for round in 1..=2 {
        clear(np);
        lang::switch(Lang::En);
        std::thread::sleep(Duration::from_millis(200));
        type_text(np, "jxtym ", Lang::En);
        let fixed = text_of(np);
        tap_vk(np, &[VK_CONTROL, VK_BACK]);
        let got = text_of(np);
        report(&mut failed, fixed == "очень " && got == "jxtym ", format!("отмена {round}: {fixed:?} → {got:?}"));
    }
    clear(np);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    type_text(np, "jxtym ", Lang::En);
    let got = text_of(np);
    report(&mut failed, got == "jxtym ", format!("после двух отмен не трогает → {got:?}"));

    clear(np);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    type_text(np, "ghbdtn", Lang::En);
    tap_vk(np, &[VK_RETURN]);
    let got = text_of(np);
    report(&mut failed, got.trim_end() == "привет" && got.ends_with(['\r', '\n']), format!("Enter → {got:?}"));

    clear(np);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    let mut c = engine.config();
    c.features.early = false;
    engine.set_config(c);
    std::thread::sleep(Duration::from_millis(100));
    type_text(np, "ghbdtnn ", Lang::En);
    tap_vk(np, &[VK_BACK]);
    tap_vk(np, &[VK_BACK]);
    tap_vk(np, &[VK_RETURN]);
    let got = text_of(np);
    report(&mut failed, got.trim_end() == "ghbdtn" && got.len() - got.trim_end().len() <= 2, format!("⌫⌫ Enter → {got:?}"));
    let mut c = engine.config();
    c.features.early = true;
    engine.set_config(c);

    clear(np);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    type_text(np, "hello  ", Lang::En);
    tap_vk(np, &[VK_BACK]);
    type_text(np, "ghbdtn ", Lang::En);
    let got = text_of(np);
    report(&mut failed, got == "hello привет ", format!("двойной пробел ⌫ → {got:?}"));

    clear(np);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    let mut c = engine.config();
    c.features.early = false;
    engine.set_config(c);
    std::thread::sleep(Duration::from_millis(100));
    type_text(np, "ghb", Lang::En);
    lang::switch(Lang::Ru);
    std::thread::sleep(Duration::from_millis(200));
    type_text(np, "вет ", Lang::Ru);
    let got = text_of(np);
    report(&mut failed, got == "ghbвет ", format!("ручная смена раскладки посреди слова → {got:?}"));
    let mut c = engine.config();
    c.features.early = true;
    engine.set_config(c);

    let sentinel = "ghbdtn-selftest-буфер";
    let _ = clipboard_win::set_clipboard_string(sentinel);
    for (insert, switch) in [(Insert::Paste, Switch::Message), (Insert::Keys, Switch::Message), (Insert::Unicode, Switch::Hotkey)] {
        let mut c = engine.config();
        c.programs.push(Program { exe: "notepad.exe".into(), insert, switch, ..Program::default() });
        engine.set_config(c);
        clear(np);
        lang::switch(Lang::En);
        std::thread::sleep(Duration::from_millis(200));
        type_text(np, "Ghbdtn vbh ", Lang::En);
        let got = text_of(np);
        let after = lang::current();
        std::thread::sleep(Duration::from_millis(600));
        let clip = clipboard_win::get_clipboard_string().unwrap_or_default();
        report(
            &mut failed,
            got == "Привет мир " && after == Some(Lang::Ru) && clip == sentinel,
            format!("профиль {insert:?}/{switch:?} (сочетание {:?}, не работает {}) → {got:?}, раскладка {after:?}, буфер восстановлен {}", lang::toggle(), lang::hotkey_dead(), clip == sentinel),
        );
        let mut c = engine.config();
        c.programs.retain(|p| p.exe != "notepad.exe");
        engine.set_config(c);
    }

    let dropped = hook::DROPPED.load(SeqCst);
    std::thread::sleep(Duration::from_millis(300));
    report(&mut failed, dropped == 0 && hook::REINSTALLS.load(SeqCst) == 0 && hook::hold_state() == 0, format!("всего: потеряно {dropped}, переустановок {}, задержка {}", hook::REINSTALLS.load(SeqCst), hook::hold_state()));

    clear(np);
    lang::switch(Lang::En);
    std::thread::sleep(Duration::from_millis(200));
    let watch = unsafe { FindWindowW(hook::WATCH_CLASS, None) }.unwrap_or_default();
    unsafe {
        let _ = SendMessageW(watch, WM_POWERBROADCAST, Some(WPARAM(PBT_APMRESUMEAUTOMATIC as usize)), Some(LPARAM(0)));
    }
    std::thread::sleep(Duration::from_millis(300));
    type_text(np, "ghbdtn ", Lang::En);
    let got = text_of(np);
    report(&mut failed, !watch.is_invalid() && hook::REINSTALLS.load(SeqCst) == 1 && got == "привет ", format!("выход из сна: окно {}, переустановок {}, дальше {got:?}", !watch.is_invalid(), hook::REINSTALLS.load(SeqCst)));

    tap_vk(np, &[VK_CONTROL, VK_A]);
    tap_vk(np, &[VK_DELETE]);
    tap_vk(np, &[VK_CONTROL, VK_S]);
    std::thread::sleep(Duration::from_millis(300));
    unsafe {
        let _ = PostMessageW(Some(np), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
    std::process::exit(failed);
}
