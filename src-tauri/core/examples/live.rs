use ghbdtn_core::config::Config;
use ghbdtn_core::win::{CoreEvent, Engine};

fn main() {
    let (tx, rx) = crossbeam_channel::unbounded();
    let path = Config::default_path();
    let engine = Engine::start(path.clone(), tx);
    println!("Ghbdtn core работает. Конфиг: {}. Выход — Ctrl+C в этом окне.", path.display());
    let cfg = engine.config();
    println!("Конвертация: {}, выключатель: {}", cfg.hotkeys.convert, cfg.hotkeys.toggle);
    for ev in rx {
        match ev {
            CoreEvent::Acted { kind, .. } => println!("{kind:?}"),
            CoreEvent::Undone => println!("undo"),
            CoreEvent::Retyped { fix, .. } => println!("retyped{}", if fix.is_some() { " after fix" } else { "" }),
            CoreEvent::Status(s) => println!("status: {s:?} ({})", ghbdtn_core::win::context::current().exe),
            CoreEvent::NeverAdded(_) => println!("never +1"),
            CoreEvent::ConfigChanged => println!("config saved"),
            CoreEvent::HookReinstalled => println!("hook reinstalled"),
        }
    }
}
