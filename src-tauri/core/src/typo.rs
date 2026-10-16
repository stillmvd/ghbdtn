use crate::decide::Rule;
use crate::dict::Dictionaries;
use crate::keymap::{key_of, Lang};
use crate::layout::short_word;
use crate::replace::apply_case;
use std::collections::HashMap;

pub const MAX_WEIGHT: f32 = 0.6;
const LONG_WORD: usize = 7;
const LONG_MAX_WEIGHT: f32 = 1.0;
const GAP: f32 = 3.0;
const MIN_LEN: usize = 4;
const OUTRANK: u32 = 5;
const NEAR_WEIGHT: f32 = 0.5;
const FAR_MIN_LEN: usize = 6;
const SHORT_MIN_FREQ: u32 = 20_000;
const SHORT_LEN: usize = 3;
const PAIR_LEN: usize = 2;
const PAIR_MIN: u32 = 20;
const CTX_MIN: u32 = 3;
const CTX_RATIO: u32 = 10;
const EN_CTX_RATIO: u32 = 20;
const WEAK_FREQ: u32 = 1000;
const WEAK3_FREQ: u32 = 300;
const SHORT_GAP: f32 = 2.3;
const WEAK_RATIO: u32 = 300;
const SOLID_FREQ: u32 = 100;
const REAL_MIN: u32 = 100;
const PARTICLES: &[&str] = &["ну", "не", "но", "да", "мы", "вы", "ты", "он", "бы", "же", "ли", "по", "до", "из", "за", "то", "уж"];
const FAR_SHORT: f32 = 1.2;
const FAR_LONG: f32 = 1.6;
const MOVED: f32 = 0.7;
const MOVED_EDIT: f32 = 0.6;
const PREFIXES: &[&str] = &[
    "пере", "недо", "при", "раз", "рас", "под", "про", "пре", "без", "бес", "по", "на", "за", "вы", "от", "до", "из", "ис",
    "об", "во", "вз", "вс", "с", "у", "о", "в",
];
const VERB_ENDS: &[&str] = &[
    "ть", "ться", "ти", "л", "ла", "ли", "ло", "лся", "лась", "лись", "лось", "ешь", "ишь", "ет", "ют", "ут", "ит", "ят",
    "ат", "ем", "им", "ете", "ите", "ется", "ются", "ится", "ятся", "нный", "нная", "нное", "нные", "нным", "нных", "нн",
    "вший", "вшая", "вшие", "вши",
];
const PREFIXED_MIN: usize = 5;
const JOINT_LEAD: f32 = 1.5;
const SPLIT_MIN: usize = 5;
const FAR_BEATS_LONE: u32 = 1000;
const SPLIT_LONG_LEFT: usize = 6;
const SPLIT_OVER: u32 = 3;
const AFTER_NUMBER: &[&str] = &[
    "год", "года", "году", "лет", "раз", "раза", "рубль", "рубля", "рублей", "доллар", "доллара", "долларов", "евро",
    "минута", "минуты", "минут", "час", "часа", "часов", "секунд", "секунды", "день", "дня", "дней", "неделя", "недели",
    "недель", "месяц", "месяца", "месяцев", "тысяч", "тысячи", "человек", "процентов", "процента", "штук", "штуки",
    "января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря",
    "years", "year", "days", "hours", "minutes", "seconds", "times", "people", "percent", "dollars",
];

fn position(c: char, lang: Lang) -> Option<(f32, f32)> {
    let (scan, _) = key_of(c, lang)?;
    let (row, start, offset) = match scan {
        0x29 => return Some((0.0, 0.0)),
        0x02..=0x0D => (0.0, 0x02, 1.0),
        0x10..=0x1B => (1.0, 0x10, 1.5),
        0x1E..=0x28 => (2.0, 0x1E, 1.75),
        0x2C..=0x35 => (3.0, 0x2C, 2.25),
        _ => return None,
    };
    Some((offset + (scan - start) as f32, row))
}

pub fn adjacent(a: char, b: char, lang: Lang) -> bool {
    match (position(a, lang), position(b, lang)) {
        (Some((ax, ay)), Some((bx, by))) => a != b && (ay - by).abs() <= 1.0 && (ax - bx).abs() <= 1.0,
        _ => false,
    }
}

fn edits(word: &[char], lang: Lang) -> HashMap<String, f32> {
    let alphabet: Vec<char> = lang.alphabet().chars().collect();
    let mut out: HashMap<String, f32> = HashMap::new();
    let mut add = |chars: Vec<char>, w: f32| {
        let s: String = chars.into_iter().collect();
        let e = out.entry(s).or_insert(w);
        *e = e.min(w);
    };
    let n = word.len();
    for i in 0..n {
        let c = word[i];
        let prev = i.checked_sub(1).map(|j| word[j]);
        let next = word.get(i + 1).copied();
        let w = if prev == Some(c) || next == Some(c) {
            0.5
        } else if prev.is_some_and(|p| adjacent(c, p, lang)) || next.is_some_and(|x| adjacent(c, x, lang)) {
            0.6
        } else {
            0.8
        };
        let mut v = word.to_vec();
        v.remove(i);
        add(v, w);
        if i + 1 < n && word[i] != word[i + 1] {
            let mut v = word.to_vec();
            v.swap(i, i + 1);
            add(v, 0.5);
        }
        for &r in &alphabet {
            if r != c {
                let mut v = word.to_vec();
                v[i] = r;
                add(v, if adjacent(c, r, lang) { 0.5 } else { 1.0 });
            }
        }
    }
    for i in 0..=n {
        for &r in &alphabet {
            let mut v = word.to_vec();
            v.insert(i, r);
            let repeat = (i > 0 && word[i - 1] == r) || word.get(i) == Some(&r);
            add(v, if repeat { 0.5 } else { 0.6 });
        }
    }
    for i in 0..n.saturating_sub(2) {
        if word[i] != word[i + 1] && word[i + 1] != word[i + 2] {
            let mut v = word.to_vec();
            v[i..i + 3].rotate_left(1);
            add(v, MOVED_EDIT);
            let mut v = word.to_vec();
            v[i..i + 3].rotate_right(1);
            add(v, MOVED_EDIT);
        }
    }
    out
}

struct Far<'a> {
    typed: &'a [char],
    extra: Vec<f32>,
    pos: HashMap<char, (f32, f32)>,
    typed_pos: Vec<Option<(f32, f32)>>,
    cand_pos: Vec<Option<(f32, f32)>>,
    rows: [Vec<f32>; 4],
}

impl Far<'_> {
    fn near(&self, a: char, b: char) -> bool {
        match (self.pos.get(&a), self.pos.get(&b)) {
            (Some((ax, ay)), Some((bx, by))) => a != b && (ay - by).abs() <= 1.0 && (ax - bx).abs() <= 1.0,
            _ => false,
        }
    }

    fn distance(&mut self, cand: &[char], limit: f32) -> Option<f32> {
        let typed = self.typed;
        let (n, m) = (typed.len(), cand.len());
        self.cand_pos.clear();
        self.cand_pos.extend(cand.iter().map(|c| self.pos.get(c).copied()));
        const BAND: usize = 3;
        for r in &mut self.rows {
            r.clear();
            r.resize(m + 1, f32::INFINITY);
        }
        let mut prev_min = f32::INFINITY;
        for i in 0..=n {
            let [p3, p2, p1, cur] = &mut self.rows;
            cur.fill(f32::INFINITY);
            let lo = i.saturating_sub(BAND);
            let hi = (i + BAND).min(m);
            let mut row_min = f32::INFINITY;
            for j in lo..=hi {
                if i == 0 && j == 0 {
                    cur[0] = 0.0;
                    row_min = 0.0;
                    continue;
                }
                let mut v = f32::INFINITY;
                if i > 0 {
                    v = v.min(p1[j] + self.extra[i - 1]);
                }
                if j > 0 {
                    let c = cand[j - 1];
                    let repeat = (i > 0 && typed[i - 1] == c) || typed.get(i) == Some(&c);
                    v = v.min(cur[j - 1] + if repeat { 0.5 } else { 0.6 });
                }
                if i > 0 && j > 0 {
                    let (a, b) = (typed[i - 1], cand[j - 1]);
                    let sub = if a == b {
                        0.0
                    } else if self.typed_pos[i - 1].zip(self.cand_pos[j - 1]).is_some_and(|((ax, ay), (bx, by))| (ay - by).abs() <= 1.0 && (ax - bx).abs() <= 1.0) {
                        0.5
                    } else {
                        1.0
                    };
                    v = v.min(p1[j - 1] + sub);
                    if i > 1 && j > 1 && a == cand[j - 2] && typed[i - 2] == b && a != b {
                        v = v.min(p2[j - 2] + 0.5);
                    }
                    if i > 2 && j > 2 {
                        let (t, c) = (&typed[i - 3..i], &cand[j - 3..j]);
                        let early = t[0] == c[2] && t[1] == c[0] && t[2] == c[1];
                        let late = t[0] == c[1] && t[1] == c[2] && t[2] == c[0];
                        if (early || late) && t[0] != t[1] && t[1] != t[2] {
                            v = v.min(p3[j - 3] + MOVED);
                        }
                    }
                }
                cur[j] = v;
                row_min = row_min.min(v);
            }
            if row_min > limit && prev_min > limit {
                return None;
            }
            prev_min = row_min;
            self.rows.rotate_left(1);
        }
        Some(self.rows[2][m]).filter(|&v| v <= limit)
    }
}

fn far_fix(typed: &str, chars: &[char], prev: Option<&str>, lang: Lang, dicts: &Dictionaries) -> Option<String> {
    if chars.len() < FAR_MIN_LEN || typed.chars().next().is_some_and(char::is_uppercase) || chars.contains(&'-') {
        return None;
    }
    let limit = if chars.len() >= LONG_WORD { FAR_LONG } else { FAR_SHORT };
    let pos: HashMap<char, (f32, f32)> = lang.alphabet().chars().filter_map(|c| position(c, lang).map(|p| (c, p))).collect();
    let typed_pos = chars.iter().map(|c| pos.get(c).copied()).collect();
    let mut far = Far { typed: chars, extra: Vec::new(), pos, typed_pos, cand_pos: Vec::new(), rows: Default::default() };
    far.extra = (0..chars.len())
        .map(|i| {
            let c = chars[i];
            let prev = i.checked_sub(1).map(|j| chars[j]);
            let next = chars.get(i + 1).copied();
            if prev == Some(c) || next == Some(c) {
                0.5
            } else if prev.is_some_and(|p| far.near(c, p)) || next.is_some_and(|x| far.near(c, x)) {
                0.6
            } else {
                0.8
            }
        })
        .collect();
    let mut scored: Vec<(f32, &[char])> = Vec::new();
    let mut best = f32::NEG_INFINITY;
    for (w, f) in dicts.frequent_from(lang, chars[0]) {
        let top = (*f as f32).ln();
        if top < best - GAP {
            break;
        }
        if w.len().abs_diff(chars.len()) > 2 {
            continue;
        }
        let reach = limit.min((top - best + GAP) / 4.0);
        if let Some(dist) = far.distance(w, reach) {
            let s = top - 4.0 * dist;
            best = best.max(s);
            scored.push((s, w));
        }
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (best, _) = scored.first()?;
    let close: Vec<String> = scored.iter().take_while(|(s, _)| best - s < GAP).map(|(_, c)| c.iter().collect()).collect();
    let close: Vec<&str> = close.iter().map(String::as_str).collect();
    pick(prev, &close, lang, dicts).map(|(c, _)| c).filter(|c| solid(c, lang, dicts)).map(|c| apply_case(typed, c))
}

pub fn near_freq(typed: &str, lang: Lang, dicts: &Dictionaries) -> Option<u32> {
    let chars: Vec<char> = typed.to_lowercase().chars().collect();
    if chars.len() < MIN_LEN {
        return None;
    }
    edits(&chars, lang)
        .into_iter()
        .filter(|(cand, w)| *w <= NEAR_WEIGHT && dicts.known(lang, cand))
        .map(|(cand, _)| dicts.freq(lang, &cand).max(1))
        .max()
}

fn short_fix(typed: &str, chars: &[char], prev: Option<&str>, lang: Lang, dicts: &Dictionaries) -> Option<String> {
    if lang != Lang::Ru || !(PAIR_LEN..=SHORT_LEN).contains(&chars.len()) || typed.chars().any(char::is_uppercase) {
        return None;
    }
    let typed_freq = dicts.freq(lang, typed);
    let rare = !dicts.certain(lang, typed);
    let weak = rare && chars.len() == PAIR_LEN && typed_freq < WEAK_FREQ;
    let only_drop = rare && chars.len() == SHORT_LEN && typed_freq < WEAK3_FREQ && dicts.seen(lang, typed);
    if !chars.iter().all(|&c| lang.owns_letter(c)) || (dicts.seen(lang, typed) && !weak && !only_drop) {
        return None;
    }
    let (mut cands, mut drops): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for i in 0..chars.len() {
        for r in lang.alphabet().chars().filter(|&r| adjacent(chars[i], r, lang)) {
            let mut v = chars.to_vec();
            v[i] = r;
            cands.push(v.into_iter().collect());
        }
        if i + 1 < chars.len() && chars[i] != chars[i + 1] {
            let mut v = chars.to_vec();
            v.swap(i, i + 1);
            cands.push(v.into_iter().collect());
        }
        let fat = |j: Option<usize>| j.and_then(|j| chars.get(j)).is_some_and(|&n| n == chars[i] || adjacent(n, chars[i], lang));
        if chars.len() == SHORT_LEN && (fat(i.checked_sub(1)) || fat(Some(i + 1))) {
            let mut v = chars.to_vec();
            v.remove(i);
            drops.push(v.into_iter().collect());
        }
    }
    let floor = SHORT_MIN_FREQ.max(if only_drop { WEAK_RATIO * typed_freq } else { 0 });
    let rank = |list: Vec<String>| -> Vec<(u32, String)> {
        list.into_iter().filter(|c| !c.contains('ё') || dicts.certain(lang, c)).map(|c| (dicts.freq(lang, &c), c)).filter(|(f, _)| *f >= floor).collect()
    };
    let drops_only = only_drop && dicts.known(lang, typed);
    let mut scored = if drops_only { Vec::new() } else { rank(cands) };
    if scored.is_empty() {
        scored = rank(drops);
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.dedup_by(|a, b| a.1 == b.1);
    if chars.len() == PAIR_LEN {
        if chars[0] == chars[1] {
            return None;
        }
        let floor = WEAK_RATIO * dicts.freq(lang, typed).max(1);
        let all: Vec<&str> = scored.iter().filter(|(f, _)| *f >= floor).map(|(_, c)| c.as_str()).collect();
        let by_prev = by_context(prev, &all, lang, dicts).filter(|c| prev.is_some_and(|p| dicts.bigram(lang, p, c) >= PAIR_MIN));
        return by_prev.or(match all.as_slice() {
            [one] if PARTICLES.contains(one) => Some(*one),
            _ => None,
        })
        .map(String::from);
    }
    let (best, _) = scored.first()?;
    let close: Vec<&str> = scored.iter().take_while(|(f, _)| (*best as f32 / *f as f32).ln() < SHORT_GAP).map(|(_, c)| c.as_str()).collect();
    pick(prev, &close, lang, dicts)
        .map(|(c, _)| c)
        .filter(|c| !drops_only || PARTICLES.contains(c) || prev.is_some_and(|p| dicts.bigram(lang, p, c) >= PAIR_MIN))
        .map(String::from)
}

fn by_context<'a>(prev: Option<&str>, cands: &[&'a str], lang: Lang, dicts: &Dictionaries) -> Option<&'a str> {
    let prev = prev?;
    if prev.chars().all(|c| c.is_ascii_digit()) {
        let mut after = cands.iter().filter(|c| AFTER_NUMBER.contains(c));
        return after.next().filter(|_| after.next().is_none()).copied();
    }
    let mut s: Vec<(u32, &str)> = cands.iter().map(|c| (dicts.bigram(lang, prev, c), *c)).collect();
    s.sort_by_key(|x| std::cmp::Reverse(x.0));
    let (top, best) = *s.first()?;
    let second = s.get(1).map_or(0, |x| x.0);
    let ratio = if lang == Lang::En { EN_CTX_RATIO } else { CTX_RATIO };
    (top >= CTX_MIN && top >= ratio * second.max(1)).then_some(best)
}

fn pick<'a>(prev: Option<&str>, close: &[&'a str], lang: Lang, dicts: &Dictionaries) -> Option<(&'a str, Rule)> {
    match close {
        [] => None,
        [one] => Some((one, Rule::Best)),
        _ => by_context(prev, close, lang, dicts).map(|c| (c, Rule::Context)),
    }
}

fn split_fix(chars: &[char], lang: Lang, dicts: &Dictionaries) -> Option<String> {
    if chars.len() < SPLIT_MIN {
        return None;
    }
    let whole: String = chars.iter().collect();
    let whole_freq = dicts.freq(lang, &whole);
    let long = |w: &str| w.chars().count() >= 3 && dicts.certain(lang, w) && solid(w, lang, dicts);
    let mut found: Vec<(u32, String)> = Vec::new();
    for i in 1..chars.len() {
        let (l, r): (String, String) = (chars[..i].iter().collect(), chars[i..].iter().collect());
        let (ls, rs) = (short_word(&l, lang), short_word(&r, lang));
        if !(ls || long(&l)) || !(rs || long(&r)) || (ls && rs) {
            continue;
        }
        let ctx = dicts.bigram(lang, &l, &r);
        let bare_ok = rs && i >= SPLIT_LONG_LEFT;
        if (ctx >= CTX_MIN || bare_ok) && ctx >= SPLIT_OVER.saturating_mul(whole_freq) {
            found.push((ctx, format!("{l} {r}")));
        }
    }
    found.sort_by_key(|x| std::cmp::Reverse(x.0));
    match found.as_slice() {
        [one] => Some(one.1.clone()),
        [a, b, ..] if a.0 >= CTX_MIN && a.0 >= CTX_RATIO * b.0.max(1) => Some(a.1.clone()),
        _ => None,
    }
}

fn prefixed_verb(word: &str, dicts: &Dictionaries) -> bool {
    PREFIXES.iter().any(|p| {
        word.strip_prefix(p).is_some_and(|rest| {
            rest.chars().next() != p.chars().last()
                && rest.chars().count() >= PREFIXED_MIN && VERB_ENDS.iter().any(|e| rest.ends_with(e)) && dicts.known(Lang::Ru, rest)
        })
    })
}

fn solid(word: &str, lang: Lang, dicts: &Dictionaries) -> bool {
    dicts.freq(lang, word) >= SOLID_FREQ || dicts.certain(lang, word)
}

fn real_word_fix(typed: &str, chars: &[char], prev: Option<&str>, lang: Lang, dicts: &Dictionaries) -> Option<String> {
    let prev = prev?;
    if lang != Lang::Ru || typed.chars().any(char::is_uppercase) || !dicts.in_bigrams(lang, typed) || dicts.bigram(lang, prev, typed) > 0 {
        return None;
    }
    let mut s: Vec<(u32, String)> = edits(chars, lang)
        .into_iter()
        .filter(|(c, w)| *w <= NEAR_WEIGHT && c != typed && solid(c, lang, dicts))
        .map(|(c, _)| (dicts.bigram(lang, prev, &c), c))
        .collect();
    s.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    s.dedup_by(|a, b| a.1 == b.1);
    let (top, best) = s.first()?;
    let second = s.get(1).map_or(0, |x| x.0);
    (*top >= REAL_MIN && *top >= CTX_RATIO * second.max(1)).then(|| best.clone())
}

pub fn best_fix(typed: &str, lang: Lang, dicts: &Dictionaries) -> Option<String> {
    fix_after(typed, None, lang, dicts)
}

pub fn fix_after(typed: &str, prev: Option<&str>, lang: Lang, dicts: &Dictionaries) -> Option<String> {
    fix_rule(typed, prev, lang, dicts).map(|f| f.0)
}

pub fn fix_rule(typed: &str, prev: Option<&str>, lang: Lang, dicts: &Dictionaries) -> Option<(String, Rule)> {
    let lower = typed.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < MIN_LEN {
        return short_fix(typed, &chars, prev, lang, dicts).map(|t| (t, Rule::ShortFix));
    }
    if !chars.iter().all(|&c| lang.owns_letter(c) || c == '-') {
        return None;
    }
    if dicts.known(lang, &lower) && !dicts.certain(lang, &lower) && !dicts.is_form(lang, &lower) {
        if let Some(split) = split_fix(&chars, lang, dicts) {
            return Some((apply_case(typed, &split), Rule::SplitFix));
        }
    }
    if dicts.known(lang, typed) {
        return real_word_fix(typed, &chars, prev, lang, dicts).map(|t| (t, Rule::RealWord));
    }
    if lang == Lang::Ru && prefixed_verb(&lower, dicts) {
        return None;
    }
    let capital = typed.chars().next().is_some_and(char::is_uppercase);
    let limit = if capital {
        MAX_WEIGHT
    } else if chars.len() >= LONG_WORD {
        LONG_MAX_WEIGHT
    } else {
        MAX_WEIGHT
    };
    let typed_freq = dicts.freq(lang, typed);
    if typed_freq > 0 && capital {
        return None;
    }
    let mut scored: Vec<(f32, String)> = edits(&chars, lang)
        .into_iter()
        .filter(|(cand, w)| *w <= limit && dicts.known(lang, cand))
        .map(|(cand, w)| ((dicts.freq(lang, &cand).max(1) as f32).ln() - 4.0 * w, cand))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    if !scored.iter().any(|(_, c)| solid(c, lang, dicts)) {
        let weight = |s: f32, c: &str| ((dicts.freq(lang, c).max(1) as f32).ln() - s) / 4.0;
        let typical: Vec<&str> = scored.iter().filter(|(s, c)| weight(*s, c) <= MAX_WEIGHT + 1e-3).map(|(_, c)| c.as_str()).collect();
        let far = far_fix(typed, &chars, prev, lang, dicts).filter(|c| typed_freq == 0 || dicts.freq(lang, &c.to_lowercase()) >= OUTRANK * typed_freq);
        let far_strong = far.as_ref().is_some_and(|c| dicts.freq(lang, &c.to_lowercase()) >= FAR_BEATS_LONE);
        if let [one] = typical.as_slice() {
            if typed_freq == 0 && !capital && !far_strong {
                return Some((apply_case(typed, one), Rule::Lone));
            }
        }
        if let Some(split) = split_fix(&chars, lang, dicts) {
            return Some((apply_case(typed, &split), Rule::SplitFix));
        }
        return far.map(|t| (t, Rule::FarFix));
    }
    let (best, _) = scored.first()?;
    let close: Vec<&str> = scored.iter().take_while(|(s, _)| best - s < GAP).map(|(_, c)| c.as_str()).collect();
    let agree = || {
        let p = prev.filter(|_| lang == Lang::Ru)?;
        let mut joint: Vec<(f32, u32, &str)> = scored
            .iter()
            .take_while(|(s, _)| best - s < GAP)
            .map(|(s, c)| {
                let ctx = dicts.bigram(lang, p, c);
                (s + (ctx as f32 + 1.0).ln(), ctx, c.as_str())
            })
            .collect();
        joint.sort_by(|a, b| b.0.total_cmp(&a.0));
        let (top, ctx, word) = joint[0];
        let second = joint.get(1).map_or(f32::NEG_INFINITY, |x| x.0);
        (ctx >= CTX_MIN && top - second >= JOINT_LEAD).then_some(word)
    };
    let (cand, rule) = pick(prev, &close, lang, dicts).or_else(|| agree().map(|c| (c, Rule::Joint))).filter(|(c, _)| solid(c, lang, dicts))?;
    if typed_freq > 0 && dicts.freq(lang, cand) < OUTRANK * typed_freq {
        return None;
    }
    Some((apply_case(typed, cand), rule))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    fn dicts() -> &'static Dictionaries {
        static D: OnceLock<Dictionaries> = OnceLock::new();
        D.get_or_init(Dictionaries::load)
    }

    #[test]
    fn moved_letter_and_context() {
        let f = |w: &str, p| fix_after(w, p, Lang::Ru, dicts());
        assert_eq!(f("смники", None).as_deref(), Some("снимки"));
        assert_eq!(f("сметир", None).as_deref(), Some("смерти"));
        assert_eq!(f("грода", Some("прошлого")).as_deref(), Some("года"));
        assert_eq!(f("грода", Some("центр")).as_deref(), Some("города"));
        assert_eq!(f("грода", None), None);
        assert_eq!(f("грода", Some("2026")).as_deref(), Some("года"));
        assert_eq!(f("сгенерировали", None), None);
        assert_eq!(f("ссказать", None).as_deref(), Some("сказать"));
        assert_eq!(f("сгенерирован", Some("код")), None);
        assert_eq!(f("поменяд", Some("он")).as_deref(), Some("поменял"));
        assert_eq!(f("знаеи", Some("не")).as_deref(), Some("знает"));
        assert_eq!(f("знаеи", None), None);
        assert_eq!(f("исправляетсяна", None).as_deref(), Some("исправляется на"));
        assert_eq!(f("незнаю", None).as_deref(), Some("не знаю"));
        assert_eq!(f("вобщем", None).as_deref(), Some("в общем"));
        assert_eq!(f("Незнаю", None).as_deref(), Some("Не знаю"));
        assert_eq!(fix_after("ofthe", None, Lang::En, dicts()).as_deref(), Some("of the"));
        assert_eq!(f("сколько", None), None);
        assert_eq!(f("иам", None).as_deref(), Some("там"));
        assert_eq!(f("косячки", None), None);
        assert_eq!(f("косячкеи", None).as_deref(), Some("косячки"));
        for w in ["iphone", "ebay", "imac", "ipad", "awesome", "outfit"] {
            assert_eq!(fix_after(w, None, Lang::En, dicts()), None, "{w}");
        }
        assert_eq!(f("нго", None).as_deref(), Some("но"));
    }

    #[test]
    fn keyboard_neighbours() {
        assert!(adjacent('ж', 'д', Lang::Ru));
        assert!(adjacent('f', 'g', Lang::En));
        assert!(adjacent('f', 'r', Lang::En));
        assert!(!adjacent('f', 'p', Lang::En));
    }

    #[test]
    fn fixes_confident_typos() {
        assert_eq!(best_fix("ввожа", Lang::Ru, dicts()).as_deref(), Some("ввода"));
        assert_eq!(best_fix("клавиатра", Lang::Ru, dicts()).as_deref(), Some("клавиатура"));
        assert_eq!(best_fix("сделвй", Lang::Ru, dicts()).as_deref(), Some("сделай"));
        assert_eq!(best_fix("текса", Lang::Ru, dicts()), None);
        assert_eq!(best_fix("приверяю", Lang::Ru, dicts()).as_deref(), Some("проверяю"));
        assert_eq!(best_fix("Првиет", Lang::Ru, dicts()).as_deref(), Some("Привет"));
        assert_eq!(best_fix("hwllo", Lang::En, dicts()).as_deref(), Some("hello"));
    }

    #[test]
    fn fixes_three_letter_words_only_when_clear() {
        assert_eq!(best_fix("чтл", Lang::Ru, dicts()).as_deref(), Some("что"));
        assert_eq!(best_fix("кка", Lang::Ru, dicts()).as_deref(), Some("как"));
        assert_eq!(best_fix("нкт", Lang::Ru, dicts()).as_deref(), Some("нет"));
        assert_eq!(best_fix("ято", Lang::Ru, dicts()).as_deref(), Some("что"));
        for w in ["нк", "жк", "ыы", "спс", "мск", "спб", "лол", "кек", "ааа", "Чтл", "шдё", "щвё", "ьёз", "гёе"] {
            assert_eq!(best_fix(w, Lang::Ru, dicts()), None, "{w}");
        }
    }

    #[test]
    fn context_picks_between_close_candidates() {
        assert_eq!(fix_after("нк", Some("я"), Lang::Ru, dicts()).as_deref(), Some("не"));
        assert_eq!(fix_after("нк", Some("это"), Lang::Ru, dicts()).as_deref(), Some("не"));
        assert_eq!(fix_after("нк", None, Lang::Ru, dicts()), None);
        for prev in ["в", "и", "на", "это", "мой", "новый", "я", "а"] {
            for w in ["жк", "пк", "вк", "тг", "мб", "хз", "пж", "спс", "мск", "спб", "ыы", "ок"] {
                assert_eq!(fix_after(w, Some(prev), Lang::Ru, dicts()), None, "{prev} {w}");
            }
        }
    }

    #[test]
    fn journal_cases() {
        assert_eq!(fix_after("гу", None, Lang::Ru, dicts()).as_deref(), Some("ну"));
        assert_eq!(fix_after("хожу", Some("по"), Lang::Ru, dicts()).as_deref(), Some("ходу"));
        assert_eq!(fix_after("хожу", Some("я"), Lang::Ru, dicts()), None);
        assert_eq!(fix_after("увидер", None, Lang::Ru, dicts()), None);
        assert_eq!(fix_after("было", Some("они"), Lang::Ru, dicts()), None);
        assert_eq!(fix_after("черными", Some("с"), Lang::Ru, dicts()), None);
        for w in ["Настином", "ща", "тя", "ип", "нг", "пт", "сы", "домклик", "Гу", "кб", "гб", "ис", "ск", "кс"] {
            assert_eq!(fix_after(w, None, Lang::Ru, dicts()), None, "{w}");
        }
    }

    #[test]
    fn user_words_inflect_like_dictionary() {
        let mut d = Dictionaries::load();
        assert!(!d.known(Lang::Ru, "адверсариальным"));
        d.add_user_words("адверсариальный\n");
        assert!(d.known(Lang::Ru, "адверсариальным"));
        assert!(d.known(Lang::Ru, "адверсариальная"));
    }

    #[test]
    fn fixes_double_typos_in_long_words() {
        assert_eq!(best_fix("вилдидмо", Lang::Ru, dicts()).as_deref(), Some("видимо"));
        assert_eq!(best_fix("рпрзвивать", Lang::Ru, dicts()).as_deref(), Some("развивать"));
        assert_eq!(best_fix("здравстсвуйте", Lang::Ru, dicts()).as_deref(), Some("здравствуйте"));
        assert_eq!(best_fix("кринжовый", Lang::Ru, dicts()), None);
        assert_eq!(best_fix("Вилдидмо", Lang::Ru, dicts()), None);
        assert_eq!(best_fix("сщдщкщы", Lang::Ru, dicts()), None);
    }

    #[test]
    fn keeps_known_and_short() {
        assert_eq!(best_fix("привет", Lang::Ru, dicts()), None);
        assert_eq!(best_fix("кот", Lang::Ru, dicts()), None);
        assert_eq!(best_fix("hello", Lang::En, dicts()), None);
    }
}
