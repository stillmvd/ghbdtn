use ghbdtn_core::config::Config;
use ghbdtn_core::win::hook::{DROPPED, EVENTS, FIXES, FIX_US_MAX, FIX_US_TOTAL, REINSTALLS, REPLAYED};
use ghbdtn_core::win::{CoreEvent, Engine};
use std::sync::atomic::Ordering::SeqCst;
use std::time::{Duration, Instant};

fn stats(since: Instant) {
    let fixes = FIXES.load(SeqCst);
    let avg = FIX_US_TOTAL.load(SeqCst).checked_div(fixes).unwrap_or(0);
    println!(
        "[{} мин] событий хука {}, потеряно {}, переотправлено {}, переустановок {}, исправлений {fixes}, разделитель→ввод ср {:.1} мс, макс {:.1} мс",
        since.elapsed().as_secs() / 60,
        EVENTS.load(SeqCst),
        DROPPED.load(SeqCst),
        REPLAYED.load(SeqCst),
        REINSTALLS.load(SeqCst),
        avg as f64 / 1000.0,
        FIX_US_MAX.load(SeqCst) as f64 / 1000.0
    );
    println!("  шаги (ср/макс): {}", ghbdtn_core::win::hook::profile());
}

fn main() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let path = Config::default_path();
    let engine = Engine::start(path.clone(), tx);
    println!("Ghbdtn core работает. Конфиг: {}. Выход — Ctrl+C в этом окне.", path.display());
    let cfg = engine.config();
    println!("Конвертация: {}, выключатель: {}", cfg.hotkeys.convert, cfg.hotkeys.toggle);
    let started = Instant::now();
    let mut last = Instant::now();
    loop {
        if last.elapsed() >= Duration::from_secs(60) {
            stats(started);
            last = Instant::now();
        }
        let Ok(ev) = rx.recv_timeout(Duration::from_secs(1)) else { continue };
        match ev {
            CoreEvent::Acted { kind, rule, .. } => println!("{kind:?} {}", rule.map_or("", |r| r.name())),
            CoreEvent::Undone => println!("undo"),
            CoreEvent::Retyped { fix, .. } => println!("retyped{}", if fix.is_some() { " after fix" } else { "" }),
            CoreEvent::Status(s) => println!("status: {s:?} ({})", ghbdtn_core::win::context::current().exe),
            CoreEvent::NeverAdded(_) => println!("never +1"),
            CoreEvent::WordLearned(_) => println!("learned +1"),
            CoreEvent::ConfigChanged => println!("config saved"),
            CoreEvent::HookReinstalled => println!("hook reinstalled"),
        }
    }
}
