use ghbdtn_core::config::Config;
use ghbdtn_core::decide::{decide, Rule, WordInput};
use ghbdtn_core::dict::Dictionaries;
use ghbdtn_core::early::early_fix;
use ghbdtn_core::keymap::Lang;
use ghbdtn_core::typo::{adjacent, fix_after};
use ghbdtn_core::undo::ActionKind;
use ghbdtn_core::word::{keys_for, render};
use std::collections::BTreeMap;
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
fn moved_letter() {
    let dicts = Dictionaries::load();
    let cfg = config();
    let (mut total, mut right, mut wrong) = (0, 0, 0);
    let mut wrong_samples = Vec::new();
    for w in top_words(Lang::Ru) {
        let c: Vec<char> = w.chars().collect();
        if c.len() < 6 || w.contains('-') {
            continue;
        }
        let a = 1 + total % (c.len() - 3);
        let mut v = c.clone();
        let x = v.remove(a);
        v.insert(a + 2, x);
        let v: String = v.into_iter().collect();
        if v == w || dicts.known(Lang::Ru, &v) {
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
    println!("Ru moved right {:.1}% wrong {:.2}% of {total} {wrong_samples:?}", r * 100.0, x * 100.0);
    assert!(x <= 0.02, "moved letter wrong");
}

fn early_at(word: &str, keys_lang: Lang, lang: Lang, dicts: &Dictionaries, cfg: &Config) -> Option<(usize, String)> {
    let keys = keys_for(word, keys_lang);
    (1..=keys.len()).find_map(|n| {
        early_fix(&WordInput { keys: &keys[..n], lang, code_mode: false, prev: None }, dicts, cfg).map(|f| (n, f.text))
    })
}

#[test]
fn early_starts() {
    let dicts = Dictionaries::load();
    let cfg = config();
    let leipzig = |lang: Lang| -> Vec<String> {
        let text = match lang {
            Lang::Ru => include_str!("../data/ru.bigram.words"),
            Lang::En => include_str!("../data/en.bigram.words"),
        };
        text.lines().filter(|w| w.chars().count() >= 3 && w.chars().all(|c| lang.owns_letter(c))).map(str::to_string).collect()
    };
    for lang in [Lang::Ru, Lang::En] {
        let top = top_words(lang);
        let false_top: Vec<String> = top.iter().filter_map(|w| early_at(w, lang, lang, &dicts, &cfg).map(|(n, t)| format!("{w}→{t}@{n}"))).collect();
        println!("{lang:?} early false top {} {:?}", false_top.len(), false_top.iter().take(15).collect::<Vec<_>>());
        assert!(false_top.is_empty(), "early false top {lang:?}");

        let held = leipzig(lang);
        let false_held: Vec<String> = held.iter().filter_map(|w| early_at(w, lang, lang, &dicts, &cfg).map(|(n, t)| format!("{w}→{t}@{n}"))).collect();
        let rate = false_held.len() as f64 / held.len() as f64;
        println!("{lang:?} early false leipzig {} of {} ({:.3}%) {:?}", false_held.len(), held.len(), rate * 100.0, false_held.iter().take(25).collect::<Vec<_>>());
        assert!(rate <= 0.001, "early false leipzig {lang:?}");

        let long: Vec<&String> = top.iter().filter(|w| w.chars().count() >= 4 && w.chars().all(|c| lang.owns_letter(c))).collect();
        let mut by_n: BTreeMap<usize, usize> = BTreeMap::new();
        let mut wrong = Vec::new();
        for w in &long {
            if let Some((n, t)) = early_at(w, lang, lang.other(), &dicts, &cfg) {
                if n < w.chars().count() {
                    *by_n.entry(n).or_default() += 1;
                }
                if !w.starts_with(&t) {
                    wrong.push(format!("{w}→{t}"));
                }
                let keys = keys_for(w, lang);
                let after = decide(&WordInput { keys: &keys, lang, code_mode: false, prev: None }, &dicts, &cfg);
                if after.as_ref().is_some_and(|f| f.to.is_some()) {
                    wrong.push(format!("{w} then {:?}", after.map(|f| f.text)));
                }
            }
        }
        let (mut typos, mut typo_early) = (0, Vec::new());
        for w in top.iter().filter(|w| w.chars().count() >= 5 && w.chars().all(|c| lang.owns_letter(c))) {
            let c: Vec<char> = w.chars().collect();
            for i in 0..3 {
                let mut vs = Vec::new();
                if let Some(r) = lang.alphabet().chars().find(|&r| adjacent(c[i], r, lang)) {
                    let mut v = c.clone();
                    v[i] = r;
                    vs.push(v);
                }
                if c[i] != c[i + 1] {
                    let mut v = c.clone();
                    v.swap(i, i + 1);
                    vs.push(v);
                }
                for v in vs {
                    let v: String = v.into_iter().collect();
                    if dicts.seen(lang, &v) {
                        continue;
                    }
                    typos += 1;
                    if let Some((n, t)) = early_at(&v, lang, lang, &dicts, &cfg) {
                        typo_early.push(format!("{v}→{t}@{n} ({w})"));
                    }
                }
            }
        }
        let typo_rate = typo_early.len() as f64 / typos as f64;
        println!("{lang:?} early on start typos {} of {typos} ({:.2}%) {:?}", typo_early.len(), typo_rate * 100.0, typo_early.iter().take(20).collect::<Vec<_>>());
        assert!(typo_rate <= 0.01, "early on start typos {lang:?}");
        let early: usize = by_n.values().sum();
        println!("{lang:?} early right {:.1}% of {} by length {by_n:?} wrong {:?}", early as f64 * 100.0 / long.len() as f64, long.len(), wrong.iter().take(15).collect::<Vec<_>>());
        assert!(wrong.is_empty(), "early wrong {lang:?}");
    }
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
    let avg = (0..3)
        .map(|_| {
            let t = Instant::now();
            for w in &sample {
                let _ = run(w, Lang::Ru, Lang::Ru, false, &dicts, &cfg);
            }
            t.elapsed() / sample.len() as u32
        })
        .min()
        .unwrap();
    println!("decide avg {avg:?}");
    assert!(avg.as_millis() < 2 || cfg!(debug_assertions));
}

fn ctx_variants(word: &str, lang: Lang) -> Vec<String> {
    let c: Vec<char> = word.chars().collect();
    if c.len() > 3 {
        return typo_variants(word, lang);
    }
    let mut out = Vec::new();
    for i in 0..c.len() {
        if let Some(r) = lang.alphabet().chars().find(|&r| adjacent(c[i], r, lang)) {
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

fn context_check(lang: Lang, text: &str) {
    let dicts = Dictionaries::load();
    let pairs: Vec<(&str, &str)> = text.lines().filter_map(|l| l.split_once(' ')).collect();
    let (mut total, mut short, mut changed) = (0, 0, 0);
    let mut stats = [[0u32; 2]; 2];
    let mut changed_samples = Vec::new();
    let mut wrong_samples = Vec::new();
    for (prev, w) in &pairs {
        if w.chars().count() < 2 || w.contains(['-', '\'']) {
            continue;
        }
        if let Some(out) = fix_after(w, Some(prev), lang, &dicts) {
            changed += 1;
            if changed_samples.len() < 15 {
                changed_samples.push(format!("{prev} {w}→{out}"));
            }
        }
        for v in ctx_variants(w, lang) {
            if dicts.seen(lang, &v) || (lang == Lang::En && v.chars().count() <= 3) {
                continue;
            }
            total += 1;
            short += (v.chars().count() <= 3) as u32;
            for (i, p) in [None, Some(*prev)].into_iter().enumerate() {
                match fix_after(&v, p, lang, &dicts) {
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
        "{lang:?} ctx of {total} (short {short}): alone right {:.1}% wrong {:.2}%, with prev right {:.1}% wrong {:.2}%, correct changed {changed} {changed_samples:?} {wrong_samples:?}",
        pct(stats[0][0]),
        pct(stats[0][1]),
        pct(stats[1][0]),
        pct(stats[1][1])
    );
    assert!(changed as f64 <= pairs.len() as f64 * 0.0005, "context changed correct words");
    let extra_wrong = stats[1][1].saturating_sub(stats[0][1]) as f64;
    let extra_right = stats[1][0].saturating_sub(stats[0][0]) as f64;
    assert!(extra_wrong <= 5.0 || extra_right >= extra_wrong * 5.0, "context adds wrong fixes");
}

#[test]
fn context_typos() {
    context_check(Lang::Ru, include_str!("ctx_pairs.txt"));
}

#[test]
fn context_typos_en() {
    context_check(Lang::En, include_str!("ctx_pairs_en.txt"));
}

struct Case {
    set: &'static str,
    lang: Lang,
    keys_lang: Lang,
    prev: Option<String>,
    typed: String,
    expect: Option<String>,
}

fn moved_variants(words: &[String], dicts: &Dictionaries) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for w in words {
        let c: Vec<char> = w.chars().collect();
        if c.len() < 6 || w.contains('-') {
            continue;
        }
        let a = 1 + out.len() % (c.len() - 3);
        let mut v = c.clone();
        let x = v.remove(a);
        v.insert(a + 2, x);
        let v: String = v.into_iter().collect();
        if v != *w && !dicts.known(Lang::Ru, &v) {
            out.push((v, w.clone()));
        }
    }
    out
}

fn cases(dicts: &Dictionaries) -> Vec<Case> {
    let mut out = Vec::new();
    let case = |set, lang, keys_lang, prev: Option<&str>, typed: &str, expect: Option<&str>| Case {
        set,
        lang,
        keys_lang,
        prev: prev.map(String::from),
        typed: typed.to_string(),
        expect: expect.map(String::from),
    };
    for lang in [Lang::Ru, Lang::En] {
        let words = top_words(lang);
        for w in &words {
            out.push(case("clean", lang, lang, None, w, None));
            if w.chars().count() >= 3 {
                out.push(case("layout", lang.other(), lang, None, w, Some(w)));
            }
            if w.chars().count() >= 5 && w.chars().all(|c| lang.owns_letter(c)) {
                for v in typo_variants(w, lang).into_iter().filter(|v| !dicts.seen(lang, v)) {
                    out.push(case("typo", lang, lang, None, &v, Some(w)));
                }
            }
        }
        let pairs = match lang {
            Lang::Ru => include_str!("ctx_pairs.txt"),
            Lang::En => include_str!("ctx_pairs_en.txt"),
        };
        for (prev, w) in pairs.lines().filter_map(|l| l.split_once(' ')) {
            if w.chars().count() < 2 || w.contains(['-', '\'']) {
                continue;
            }
            out.push(case("ctx_clean", lang, lang, Some(prev), w, None));
            let prev_alt = render(&keys_for(prev, lang), lang.other());
            out.push(case("layout_ctx", lang.other(), lang, Some(&prev_alt), w, Some(w)));
            for v in ctx_variants(w, lang) {
                if lang == Lang::Ru && v != w && dicts.known(lang, &v) && dicts.bigram(lang, prev, &v) == 0 {
                    out.push(case("real_word", lang, lang, Some(prev), &v, Some(w)));
                }
                if dicts.seen(lang, &v) || (lang == Lang::En && v.chars().count() <= 3) {
                    continue;
                }
                out.push(case("ctx_typo", lang, lang, Some(prev), &v, Some(w)));
            }
        }
    }
    let ru = top_words(Lang::Ru);
    for w in &ru {
        if let Some(v) = double_variant(w, Lang::Ru).filter(|v| !dicts.known(Lang::Ru, v)) {
            out.push(case("double", Lang::Ru, Lang::Ru, None, &v, Some(w)));
        }
    }
    for (v, w) in moved_variants(&ru, dicts) {
        out.push(case("moved", Lang::Ru, Lang::Ru, None, &v, Some(&w)));
    }
    let forms = fst::Set::new(include_bytes!("../data/ru.forms.fst").as_slice()).unwrap();
    let mut stream = forms.stream();
    let mut i = 0usize;
    while let Some(bytes) = fst::Streamer::next(&mut stream) {
        i += 1;
        let Ok(w) = std::str::from_utf8(bytes) else { continue };
        if i.is_multiple_of(300) && w.chars().count() >= 4 && w.chars().all(|c| Lang::Ru.owns_letter(c)) && dicts.freq(Lang::Ru, w) == 0 {
            out.push(case("rare_forms", Lang::Ru, Lang::Ru, None, w, None));
        }
    }
    out
}

#[derive(Default, serde::Serialize)]
struct Tally {
    right: u32,
    wrong: u32,
    us: f64,
    max_us: f64,
    slowest: String,
    samples: Vec<String>,
}

#[derive(Default, serde::Serialize)]
struct SetTally {
    total: u32,
    right: u32,
    wrong: u32,
    missed: u32,
    right_no_forms: u32,
    wrong_no_forms: u32,
}

fn outcome(c: &Case, dicts: &Dictionaries, cfg: &Config) -> (Option<(Rule, String)>, f64) {
    let keys = keys_for(&c.typed, c.keys_lang);
    let input = WordInput { keys: &keys, lang: c.lang, code_mode: false, prev: c.prev.as_deref() };
    let t = Instant::now();
    let early = (1..=keys.len()).find_map(|n| early_fix(&WordInput { keys: &keys[..n], lang: c.lang, code_mode: false, prev: None }, dicts, cfg));
    let fix = match early.and_then(|e| e.to) {
        Some(to) => Some(
            decide(&WordInput { keys: &keys, lang: to, code_mode: false, prev: c.prev.as_deref() }, dicts, cfg)
                .map_or((Rule::Early, render(&keys, to)), |f| (f.rule, f.text)),
        ),
        None => decide(&input, dicts, cfg).map(|f| (f.rule, f.text)),
    };
    let us = t.elapsed().as_secs_f64() * 1e6;
    (fix, us)
}

#[test]
#[ignore]
fn rule_stats() {
    let dicts = Dictionaries::load();
    let bare = Dictionaries::load().without_forms();
    let mut cfg = config();
    cfg.features.early = std::env::var_os("GHBDTN_NO_EARLY").is_none();
    let cases = cases(&dicts);
    let mut rules: BTreeMap<(&str, &str), Tally> = BTreeMap::new();
    let mut sets: BTreeMap<String, SetTally> = BTreeMap::new();
    let mut idle = (0u32, 0f64);
    for c in &cases {
        let set = sets.entry(format!("{}/{:?}", c.set, c.keys_lang)).or_default();
        set.total += 1;
        let (fix, us) = outcome(c, &dicts, &cfg);
        let right = |text: &str| c.expect.as_deref() == Some(text);
        match &fix {
            Some((rule, text)) => {
                let t = rules.entry((rule.name(), c.set)).or_default();
                t.us += us;
                if us > t.max_us {
                    t.max_us = us;
                    t.slowest = c.typed.clone();
                }
                if right(text) {
                    t.right += 1;
                    set.right += 1;
                } else {
                    t.wrong += 1;
                    set.wrong += 1;
                    if t.samples.len() < 8 {
                        let prev = c.prev.as_deref().map(|p| format!("{p} ")).unwrap_or_default();
                        let want = c.expect.as_deref().map(|e| format!(" ({e})")).unwrap_or_default();
                        t.samples.push(format!("{prev}{}→{text}{want}", c.typed));
                    }
                }
            }
            None => {
                idle = (idle.0 + 1, idle.1 + us);
                set.missed += c.expect.is_some() as u32;
            }
        }
        if c.lang == Lang::Ru {
            match outcome(c, &bare, &cfg).0 {
                Some((_, text)) if right(&text) => set.right_no_forms += 1,
                Some(_) => set.wrong_no_forms += 1,
                None => {}
            }
        }
    }
    println!("{:<20} {:>8} {:>7} {:>7} {:>8} {:>8}", "rule", "fires", "right", "wrong", "avg µs", "max µs");
    for rule in Rule::ALL {
        let mine: Vec<(&&str, &Tally)> = rules.iter().filter(|((r, _), _)| *r == rule.name()).map(|((_, s), t)| (s, t)).collect();
        let (right, wrong, us, max) = mine.iter().fold((0, 0, 0.0, 0f64), |a, (_, t)| (a.0 + t.right, a.1 + t.wrong, a.2 + t.us, a.3.max(t.max_us)));
        let fires = right + wrong;
        let avg = if fires > 0 { us / fires as f64 } else { 0.0 };
        println!("{:<20} {fires:>8} {right:>7} {wrong:>7} {avg:>8.1} {max:>8.0}", rule.name());
        for (set, t) in &mine {
            println!("  {set:<18} {:>8} {:>7} {:>7} slowest {} {:.0}µs {:?}", t.right + t.wrong, t.right, t.wrong, t.slowest, t.max_us, t.samples);
        }
    }
    println!("none {:>8} avg µs {:.1}", idle.0, idle.1 / idle.0.max(1) as f64);
    for (name, s) in &sets {
        println!(
            "{name:<16} total {:>6} right {:>6} wrong {:>5} missed {:>6} | no forms right {:>6} wrong {:>5}",
            s.total, s.right, s.wrong, s.missed, s.right_no_forms, s.wrong_no_forms
        );
    }
    let json = serde_json::json!({
        "rules": rules.iter().map(|((r, s), t)| serde_json::json!({"rule": r, "set": s, "tally": t})).collect::<Vec<_>>(),
        "sets": sets,
        "idle": {"count": idle.0, "us": idle.1},
    });
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("rules.json");
    std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    println!("json {}", path.display());
}
