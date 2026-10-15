use crate::dict::Dictionaries;
use crate::keymap::{key_of, Lang};
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
const FAR_SHORT: f32 = 1.2;
const FAR_LONG: f32 = 1.6;

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
    out
}

struct Far<'a> {
    typed: &'a [char],
    extra: Vec<f32>,
    pos: HashMap<char, (f32, f32)>,
    rows: [Vec<f32>; 3],
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
        const BAND: usize = 3;
        for r in &mut self.rows {
            r.clear();
            r.resize(m + 1, f32::INFINITY);
        }
        for i in 0..=n {
            let [p2, p1, cur] = &mut self.rows;
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
                    } else if self.pos.get(&a).zip(self.pos.get(&b)).is_some_and(|((ax, ay), (bx, by))| (ay - by).abs() <= 1.0 && (ax - bx).abs() <= 1.0) {
                        0.5
                    } else {
                        1.0
                    };
                    v = v.min(p1[j - 1] + sub);
                    if i > 1 && j > 1 && a == cand[j - 2] && typed[i - 2] == b && a != b {
                        v = v.min(p2[j - 2] + 0.5);
                    }
                }
                cur[j] = v;
                row_min = row_min.min(v);
            }
            if row_min > limit {
                return None;
            }
            self.rows.rotate_left(1);
        }
        Some(self.rows[1][m]).filter(|&v| v <= limit)
    }
}

fn far_fix(typed: &str, chars: &[char], lang: Lang, dicts: &Dictionaries) -> Option<String> {
    if chars.len() < FAR_MIN_LEN || typed.chars().next().is_some_and(char::is_uppercase) || chars.contains(&'-') {
        return None;
    }
    let limit = if chars.len() >= LONG_WORD { FAR_LONG } else { FAR_SHORT };
    let pos = lang.alphabet().chars().filter_map(|c| position(c, lang).map(|p| (c, p))).collect();
    let mut far = Far { typed: chars, extra: Vec::new(), pos, rows: [Vec::new(), Vec::new(), Vec::new()] };
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
    for (w, f) in dicts.frequent_from(lang, chars[0]) {
        if w.len().abs_diff(chars.len()) > 2 {
            continue;
        }
        if let Some(dist) = far.distance(w, limit) {
            scored.push(((*f as f32).ln() - 4.0 * dist, w));
        }
    }
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (best, cand) = scored.first()?;
    if scored.get(1).is_some_and(|(second, _)| best - second < GAP) {
        return None;
    }
    Some(apply_case(typed, &cand.iter().collect::<String>()))
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

pub fn best_fix(typed: &str, lang: Lang, dicts: &Dictionaries) -> Option<String> {
    let lower = typed.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < MIN_LEN || !chars.iter().all(|&c| lang.owns_letter(c) || c == '-') || dicts.known(lang, typed) {
        return None;
    }
    let limit = if chars.len() >= LONG_WORD { LONG_MAX_WEIGHT } else { MAX_WEIGHT };
    let typed_freq = dicts.freq(lang, typed);
    if typed_freq > 0 && typed.chars().next().is_some_and(char::is_uppercase) {
        return None;
    }
    let mut scored: Vec<(f32, String)> = edits(&chars, lang)
        .into_iter()
        .filter(|(cand, w)| *w <= limit && dicts.known(lang, cand))
        .map(|(cand, w)| ((dicts.freq(lang, &cand).max(1) as f32).ln() - 4.0 * w, cand))
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    if scored.is_empty() {
        return far_fix(typed, &chars, lang, dicts);
    }
    let (best, cand) = scored.first()?;
    if scored.get(1).is_some_and(|(second, _)| best - second < GAP) {
        return None;
    }
    if typed_freq > 0 && dicts.freq(lang, cand) < OUTRANK * typed_freq {
        return None;
    }
    Some(apply_case(typed, cand))
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
