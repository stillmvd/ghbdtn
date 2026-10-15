use ghbdtn_core::config::Config;
use ghbdtn_core::decide::{decide, WordInput};
use ghbdtn_core::dict::Dictionaries;
use ghbdtn_core::keymap::Lang;
use ghbdtn_core::typo::{adjacent, fix_after};
use ghbdtn_core::undo::ActionKind;
use ghbdtn_core::word::keys_for;
use std::time::Instant;

const TOP: usize = 10_000;

fn top_words(lang: Lang) -> Vec<String> {
    let text = match lang {
        Lang::Ru => include_str!("../data/ru.freq"),
        Lang::En => include_str!("../data/en.freq"),
    };
    text.lines().filter_map(|l| l.split_once(' ')).map(|(w, _)| w.to_string()).take(TOP).collect()
}

fn config() -> Config {
    let mut cfg = Config::default();
    cfg.replacements.clear();
    cfg
}

fn run(text: &str, keys_lang: Lang, lang: Lang, code_mode: bool, dicts: &Dictionaries, cfg: &Config) -> Option<(ActionKind, String)> {
    let keys = keys_for(text, keys_lang);
    decide(&WordInput { keys: &keys, lang, code_mode, prev: None }, dicts, cfg).map(|f| (f.kind, f.text))
}

fn typo_variants(word: &str, lang: Lang) -> Vec<String> {
    let c: Vec<char> = word.chars().collect();
    let mid = c.len() / 2;
    let mut out = Vec::new();
    if let Some(r) = lang.alphabet().chars().find(|&r| adjacent(c[mid], r, lang)) {
        let mut v = c.clone();
        v[mid] = r;
        out.push(v);
    }
    let mut v = c.clone();
    v.remove(mid);
    out.push(v);
    if c[mid] != c[mid + 1] {
        let mut v = c.clone();
        v.swap(mid, mid + 1);
        out.push(v);
    }
    out.into_iter().map(|v| v.into_iter().collect()).collect()
}

#[test]
fn corpus_quality() {
    let dicts = Dictionaries::load();
    let cfg = config();
    for lang in [Lang::Ru, Lang::En] {
        let words = top_words(lang);

        let changed: Vec<&String> = words.iter().filter(|w| run(w, lang, lang, false, &dicts, &cfg).is_some()).collect();
        let fp = changed.len() as f64 / words.len() as f64;
        println!("{lang:?} SC-001 changed {:.3}% {:?}", fp * 100.0, changed.iter().take(15).collect::<Vec<_>>());
        assert!(fp <= 0.001, "SC-001 {lang:?}");

        let long: Vec<&String> = words.iter().filter(|w| w.chars().count() >= 3).collect();
        let missed: Vec<&&String> = long
            .iter()
            .filter(|w| run(w, lang, lang.other(), false, &dicts, &cfg) != Some((ActionKind::Layout, w.to_string())))
            .collect();
        let hit = 1.0 - missed.len() as f64 / long.len() as f64;
        println!("{lang:?} SC-002 fixed {:.2}% missed {:?}", hit * 100.0, missed.iter().take(15).collect::<Vec<_>>());
        assert!(hit >= 0.95, "SC-002 {lang:?}");

        let (mut total, mut right, mut wrong) = (0, 0, 0);
        let mut wrong_samples = Vec::new();
        for w in words.iter().filter(|w| w.chars().count() >= 5 && w.chars().all(|c| lang.owns_letter(c))) {
            for v in typo_variants(w, lang) {
                if dicts.seen(lang, &v) {
                    continue;
                }
                total += 1;
                match run(&v, lang, lang, false, &dicts, &cfg) {
                    Some((ActionKind::Typo, t)) if &t == w => right += 1,
                    Some((_, t)) => {
                        wrong += 1;
                        if wrong_samples.len() < 15 {
                            wrong_samples.push(format!("{v}→{t} ({w})"));
                        }
                    }
                    None => {}
                }
            }
        }
        let (r, x) = (right as f64 / total as f64, wrong as f64 / total as f64);
        println!("{lang:?} SC-003 right {:.1}% wrong {:.2}% of {total} {wrong_samples:?}", r * 100.0, x * 100.0);
        assert!(r >= 0.70 && x <= 0.01, "SC-003 {lang:?}");
    }
}

fn double_variant(word: &str, lang: Lang) -> Option<String> {
    let mut c: Vec<char> = word.chars().collect();
    if c.len() < 7 {
        return None;
    }
    let a = c.len() / 3;
    let r = lang.alphabet().chars().find(|&r| adjacent(c[a], r, lang))?;
    c[a] = r;
    let b = c.len() * 2 / 3;
    let extra = lang.alphabet().chars().find(|&r| r != c[b] && adjacent(c[b], r, lang))?;
    c.insert(b, extra);
    Some(c.into_iter().collect())
}

#[test]
fn double_typos() {
    let dicts = Dictionaries::load();
    let cfg = config();
    let (mut total, mut right, mut wrong) = (0, 0, 0);
    let mut wrong_samples = Vec::new();
    for w in top_words(Lang::Ru) {
        let Some(v) = double_variant(&w, Lang::Ru) else { continue };
        if dicts.known(Lang::Ru, &v) {
            continue;
        }
        total += 1;
        match run(&v, Lang::Ru, Lang::Ru, false, &dicts, &cfg) {
            Some((_, out)) if out == w => right += 1,
            Some((_, out)) => {
                wrong += 1;
                if wrong_samples.len() < 15 {
                    wrong_samples.push(format!("{v}→{out} ({w})"));
                }
            }
            None => {}
        }
    }
    let (r, x) = (right as f64 / total as f64, wrong as f64 / total as f64);
    println!("Ru double right {:.1}% wrong {:.2}% of {total} {wrong_samples:?}", r * 100.0, x * 100.0);
    assert!(x <= 0.02, "double typos wrong");
}

#[test]
fn hard_set_untouched() {
    let dicts = Dictionaries::load();
    let cfg = config();
    let en = [
        "git", "npm", "--watch", "-b", "~", "$HOME", "a=b", "getUserName", "NASA", "v2rayN", "README", "localhost",
        "ya.ru", "std::io", "snake_case", "cd", "ls", "rm", "pnpm", "cargo", "clippy", "tauri", "OK", "ok", "I", "a",
    ];
    for w in en {
        assert_eq!(run(w, Lang::En, Lang::En, true, &dicts, &cfg), None, "code {w}");
    }
    for w in ["я", "в", "и", "ок", "ну", "щас", "чё", "блин", "жк", "пк", "вк", "тг", "мб", "хз", "пж", "спс", "мск", "спб", "лол", "кек", "ыы", "ммм"] {
        assert_eq!(run(w, Lang::Ru, Lang::Ru, false, &dicts, &cfg), None, "{w}");
    }
}

#[test]
fn decide_is_fast() {
    let dicts = Dictionaries::load();
    let cfg = config();
    let words = top_words(Lang::Ru);
    let sample: Vec<String> = words.iter().take(500).flat_map(|w| [w.clone(), format!("{w}ъ")]).collect();
    let t = Instant::now();
    for w in &sample {
        let _ = run(w, Lang::Ru, Lang::Ru, false, &dicts, &cfg);
    }
    let avg = t.elapsed() / sample.len() as u32;
    println!("decide avg {avg:?}");
    assert!(avg.as_millis() < 2 || cfg!(debug_assertions));
}

fn ctx_variants(word: &str) -> Vec<String> {
    let c: Vec<char> = word.chars().collect();
    if c.len() > 3 {
        return typo_variants(word, Lang::Ru);
    }
    let mut out = Vec::new();
    for i in 0..c.len() {
        if let Some(r) = Lang::Ru.alphabet().chars().find(|&r| adjacent(c[i], r, Lang::Ru)) {
            let mut v = c.clone();
            v[i] = r;
            out.push(v);
        }
        if i + 1 < c.len() && c[i] != c[i + 1] {
            let mut v = c.clone();
            v.swap(i, i + 1);
            out.push(v);
        }
    }
    out.into_iter().map(|v| v.into_iter().collect()).collect()
}

#[test]
fn context_typos() {
    let dicts = Dictionaries::load();
    let pairs: Vec<(&str, &str)> = include_str!("ctx_pairs.txt").lines().filter_map(|l| l.split_once(' ')).collect();
    let (mut total, mut short, mut changed) = (0, 0, 0);
    let mut stats = [[0u32; 2]; 2];
    let mut changed_samples = Vec::new();
    let mut wrong_samples = Vec::new();
    for (prev, w) in &pairs {
        if w.chars().count() < 2 || w.contains('-') {
            continue;
        }
        if let Some(out) = fix_after(w, Some(prev), Lang::Ru, &dicts) {
            changed += 1;
            if changed_samples.len() < 15 {
                changed_samples.push(format!("{prev} {w}→{out}"));
            }
        }
        for v in ctx_variants(w) {
            if dicts.seen(Lang::Ru, &v) {
                continue;
            }
            total += 1;
            short += (v.chars().count() <= 3) as u32;
            for (i, p) in [None, Some(*prev)].into_iter().enumerate() {
                match fix_after(&v, p, Lang::Ru, &dicts) {
                    Some(out) if out == *w => stats[i][0] += 1,
                    Some(out) => {
                        stats[i][1] += 1;
                        if i == 1 && wrong_samples.len() < 15 {
                            wrong_samples.push(format!("{prev} {v}→{out} ({w})"));
                        }
                    }
                    None => {}
                }
            }
        }
    }
    let pct = |n: u32| n as f64 * 100.0 / total as f64;
    println!(
        "Ru ctx of {total} (short {short}): alone right {:.1}% wrong {:.2}%, with prev right {:.1}% wrong {:.2}%, correct changed {changed} {changed_samples:?} {wrong_samples:?}",
        pct(stats[0][0]),
        pct(stats[0][1]),
        pct(stats[1][0]),
        pct(stats[1][1])
    );
    assert!(changed as f64 <= pairs.len() as f64 * 0.0005, "context changed correct words");
    assert!(stats[1][1] as f64 <= stats[0][1] as f64 * 1.2 + 5.0, "context adds wrong fixes");
}
