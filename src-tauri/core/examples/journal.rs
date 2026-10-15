use ghbdtn_core::config::Config;
use serde_json::Value;
use std::collections::BTreeMap;

fn top(title: &str, map: &BTreeMap<String, u32>, limit: usize) {
    if map.is_empty() {
        return;
    }
    let mut v: Vec<_> = map.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    println!("\n## {title} ({} разных, {} всего)", v.len(), map.values().sum::<u32>());
    for (k, n) in v.into_iter().take(limit) {
        println!("{n:>5}  {k}");
    }
}

fn main() {
    let days: usize = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(30);
    let dir = Config::default_path().with_file_name("log");
    let mut files: Vec<_> = std::fs::read_dir(&dir).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    files.sort();
    let files = &files[files.len().saturating_sub(days)..];
    let mut kinds: BTreeMap<String, u32> = BTreeMap::new();
    let mut undone = BTreeMap::new();
    let mut wrong = BTreeMap::new();
    let mut missed = BTreeMap::new();
    let mut manual = BTreeMap::new();
    let mut silent = BTreeMap::new();
    let mut apps = BTreeMap::new();
    let mut never = BTreeMap::new();
    let mut last_acted: Option<String> = None;
    for f in files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
            let app = s("app");
            match s("ev").as_str() {
                "acted" => {
                    let kind = s("kind");
                    *kinds.entry(kind.clone()).or_default() += 1;
                    *apps.entry(app).or_default() += 1;
                    let pair = format!("{} → {}  [{kind}]", s("typed"), s("result"));
                    if kind == "manual" {
                        *manual.entry(format!("{} → {}", s("typed"), s("result"))).or_default() += 1;
                    }
                    last_acted = Some(pair);
                }
                "undo" => {
                    *kinds.entry("undo".into()).or_default() += 1;
                    if let Some(p) = last_acted.take() {
                        *undone.entry(p).or_default() += 1;
                    }
                }
                "retyped" => match v["fix"].as_array() {
                    Some(fix) if fix.len() == 2 => {
                        let typed = fix[0].as_str().unwrap_or("");
                        let ours = fix[1].as_str().unwrap_or("");
                        *wrong.entry(format!("{typed} → {ours}, а нужно {}", s("after"))).or_default() += 1;
                    }
                    _ => *missed.entry(format!("{} → {}", s("before"), s("after"))).or_default() += 1,
                },
                "never" => *never.entry(s("word")).or_default() += 1,
                "silent" => *silent.entry(format!("{}  {}", s("reason"), app)).or_default() += 1,
                _ => {}
            }
        }
    }
    println!("# Журнал Ghbdtn: {} дн., {}", files.len(), dir.display());
    top("Действия", &kinds, 20);
    top("Отменены Ctrl+Backspace (ложные исправления)", &undone, 40);
    top("Стёрто сразу после исправления и перепечатано (ложные, с правильным словом)", &wrong, 40);
    top("Стёрто и перепечатано без исправления (пропущенные опечатки)", &missed, 60);
    top("Ручной Pause (пропущенная раскладка)", &manual, 40);
    top("Ушло в «не трогать»", &never, 40);
    top("Молчал: причина и приложение", &silent, 30);
    top("Исправления по приложениям", &apps, 20);
}
