use ghbdtn_core::dict::Dictionaries;
use ghbdtn_core::keymap::Lang;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const SKIP: &[&str] = &["node_modules", ".git", "target", "dist", "build", ".next", "_archived", "gen", ".venv", "vendor"];

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if !SKIP.contains(&name.as_str()) {
                walk(&p, out);
            }
        } else if name.ends_with(".md") {
            out.push(p);
        }
    }
}

fn main() {
    let root = std::env::args().nth(1).expect("root");
    let min_files: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(2);
    let dicts = Dictionaries::load();
    let mut files = Vec::new();
    walk(Path::new(&root), &mut files);
    let mut seen_in: HashMap<(Lang, String), HashSet<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        for raw in text.split(|c: char| !(c.is_alphabetic() || c == '-')) {
            let w = raw.trim_matches('-');
            let len = w.chars().count();
            if !(4..=24).contains(&len) || w.chars().skip(1).any(char::is_uppercase) {
                continue;
            }
            let lang = if w.chars().all(|c| Lang::Ru.owns_letter(c) || c == '-') {
                Lang::Ru
            } else if w.chars().all(|c| c.is_ascii_alphabetic()) {
                Lang::En
            } else {
                continue;
            };
            if dicts.known(lang, w) {
                continue;
            }
            seen_in.entry((lang, w.to_lowercase())).or_default().insert(i);
        }
    }
    let mut list: Vec<_> = seen_in.into_iter().filter(|(_, s)| s.len() >= min_files).map(|(k, s)| (s.len(), k)).collect();
    list.sort_by(|a, b| b.0.cmp(&a.0).then(a.1 .1.cmp(&b.1 .1)));
    println!("files {} candidates {}", files.len(), list.len());
    for (n, (lang, w)) in list {
        println!("{lang:?}\t{n}\t{w}");
    }
}
