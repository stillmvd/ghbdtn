use crate::keymap::Lang;
use spellbook::Dictionary;
use std::collections::{HashMap, HashSet};

const KNOWN_FREQ: u32 = 30;
const TECH_FREQ: u32 = 1000;
const TERM_FREQ: u32 = 300;
pub const BIGRAM_SCALE: f64 = 16.0;
const SUFFIX_SAMPLES: u32 = 3;
static RU_BIGRAMS: &[u8] = include_bytes!("../data/ru.bigram");
static EN_BIGRAMS: &[u8] = include_bytes!("../data/en.bigram");

pub struct Dictionaries {
    ru: Dictionary,
    en: Dictionary,
    ru_freq: HashMap<Box<str>, u32>,
    en_freq: HashMap<Box<str>, u32>,
    extra: HashSet<String>,
    ru_by_first: ByFirst,
    ru_ids: HashMap<Box<str>, u32>,
    en_ids: HashMap<Box<str>, u32>,
    ru_forms: Option<fst::Set<&'static [u8]>>,
    ru_starts: fst::Set<Vec<u8>>,
    en_starts: fst::Set<Vec<u8>>,
    ru_sorted: Vec<(Box<str>, u32)>,
    ru_total: f64,
    en_sorted: Vec<(Box<str>, u32)>,
    en_total: f64,
    user_starts: HashSet<String>,
}

type ByFirst = HashMap<char, Vec<(Box<[char]>, u32)>>;

fn norm(word: &str) -> String {
    word.to_lowercase().replace('ё', "е")
}

fn starts_set<'a>(words: impl Iterator<Item = &'a str>) -> fst::Set<Vec<u8>> {
    let mut all: Vec<String> = words.map(norm).filter(|w| !w.is_empty()).collect();
    all.sort_unstable();
    all.dedup();
    fst::Set::from_iter(all).expect("sorted words")
}

fn has_start<D: AsRef<[u8]>>(set: &fst::Set<D>, prefix: &str) -> bool {
    use fst::{IntoStreamer, Streamer};
    let mut s = set.range().ge(prefix).into_stream();
    s.next().is_some_and(|k| k.starts_with(prefix.as_bytes()))
}

fn sorted_freq(freq: &HashMap<Box<str>, u32>) -> Vec<(Box<str>, u32)> {
    let mut merged: HashMap<String, u32> = HashMap::new();
    for (w, &f) in freq {
        *merged.entry(norm(w)).or_insert(0) += f;
    }
    let mut out: Vec<(Box<str>, u32)> = merged.into_iter().map(|(w, f)| (w.into(), f)).collect();
    out.sort_unstable();
    out
}

fn stems(dic: &str) -> impl Iterator<Item = (&str, &str)> {
    dic.lines().map(|l| l.split_once('/').unwrap_or((l, "")))
}

fn en_prefixed(dic: &str) -> Vec<String> {
    let pfx: Vec<(char, &str)> = include_str!("../data/en.aff")
        .lines()
        .filter_map(|l| {
            let p: Vec<&str> = l.split_whitespace().collect();
            (p.len() >= 4 && p[0] == "PFX" && p[2] == "0").then(|| (p[1].chars().next().unwrap_or(' '), p[3]))
        })
        .collect();
    stems(dic)
        .flat_map(|(stem, flags)| pfx.iter().filter(move |(f, _)| flags.contains(*f)).map(move |(_, p)| format!("{p}{stem}")))
        .collect()
}

fn by_first(freq: &HashMap<Box<str>, u32>) -> ByFirst {
    let mut out: ByFirst = HashMap::new();
    for (w, &f) in freq {
        if f >= KNOWN_FREQ && w.chars().count() >= 4 {
            if let Some(c) = w.chars().next() {
                out.entry(c).or_default().push((w.chars().collect(), f));
            }
        }
    }
    for list in out.values_mut() {
        list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    }
    out
}

fn ru_flags_by_suffix() -> HashMap<String, &'static str> {
    let mut counts: HashMap<(String, &str), u32> = HashMap::new();
    for (stem, flags) in include_str!("../data/ru.dic").lines().filter_map(|l| l.split_once('/')) {
        if stem.starts_with(char::is_uppercase) {
            continue;
        }
        let chars: Vec<char> = stem.chars().collect();
        for l in [3, 4].into_iter().filter(|&l| chars.len() >= l + 2) {
            *counts.entry((chars[chars.len() - l..].iter().collect(), flags)).or_insert(0) += 1;
        }
    }
    let mut best: HashMap<String, (&str, u32)> = HashMap::new();
    for ((suffix, flags), n) in counts {
        let e = best.entry(suffix).or_insert((flags, 0));
        if n > e.1 || (n == e.1 && flags < e.0) {
            *e = (flags, n);
        }
    }
    best.into_iter().filter(|(_, (_, n))| *n >= SUFFIX_SAMPLES).map(|(s, (f, _))| (s, f)).collect()
}

fn parse_freq(text: &str) -> HashMap<Box<str>, u32> {
    text.lines()
        .filter_map(|l| {
            let (w, n) = l.split_once(' ')?;
            Some((w.into(), n.trim().parse().ok()?))
        })
        .collect()
}

impl Dictionaries {
    pub fn load() -> Dictionaries {
        let mut ru = Dictionary::new(include_str!("../data/ru.aff"), include_str!("../data/ru.dic"))
            .expect("ru dictionary");
        for line in include_str!("../data/ru-it.dic").lines().map(str::trim).filter(|l| !l.is_empty()) {
            let _ = ru.add(line);
        }
        let en = Dictionary::new(include_str!("../data/en.aff"), include_str!("../data/en.dic"))
            .expect("en dictionary");
        let mut en_freq = parse_freq(include_str!("../data/en.freq"));
        for (list, freq) in [(include_str!("../data/tech.txt"), TECH_FREQ), (include_str!("../data/terms.txt"), TERM_FREQ)] {
            for w in list.lines().map(str::trim).filter(|w| !w.is_empty()) {
                let f = en_freq.entry(w.into()).or_insert(0);
                *f = (*f).max(freq);
            }
        }
        let ru_freq = parse_freq(include_str!("../data/ru.freq"));
        let ids = |text: &str| text.lines().enumerate().map(|(i, w)| (w.into(), i as u32)).collect();
        let (ru_ids, en_ids) = (ids(include_str!("../data/ru.bigram.words")), ids(include_str!("../data/en.bigram.words")));
        let ru_forms = Some(fst::Set::new(include_bytes!("../data/ru.forms.fst").as_slice()).expect("ru forms"));
        let (ru_dic, en_dic, it_dic) = (include_str!("../data/ru.dic"), include_str!("../data/en.dic"), include_str!("../data/ru-it.dic"));
        let ru_starts = starts_set(ru_freq.keys().map(|w| &**w).chain(stems(ru_dic).map(|s| s.0)).chain(stems(it_dic).map(|s| s.0)));
        let en_extra = en_prefixed(en_dic);
        let en_starts = starts_set(en_freq.keys().map(|w| &**w).chain(stems(en_dic).map(|s| s.0)).chain(en_extra.iter().map(String::as_str)));
        Dictionaries {
            ru,
            en,
            ru_by_first: by_first(&ru_freq),
            ru_sorted: sorted_freq(&ru_freq),
            ru_total: ru_freq.values().map(|&f| f as f64).sum(),
            en_sorted: sorted_freq(&en_freq),
            en_total: en_freq.values().map(|&f| f as f64).sum(),
            ru_freq,
            en_freq,
            extra: HashSet::new(),
            ru_ids,
            en_ids,
            ru_forms,
            ru_starts,
            en_starts,
            user_starts: HashSet::new(),
        }
    }

    pub fn without_forms(mut self) -> Dictionaries {
        self.ru_forms = None;
        self
    }

    pub fn add_user_words(&mut self, text: &str) -> usize {
        let mut n = 0;
        let mut flags: Option<HashMap<String, &str>> = None;
        for w in text.lines().map(|l| l.split('\t').next().unwrap_or("").trim().to_lowercase()) {
            if w.chars().count() < 2 || w.starts_with('#') {
                continue;
            }
            self.user_starts.insert(norm(&w));
            if w.chars().all(|c| Lang::Ru.owns_letter(c) || c == '-') {
                let map = flags.get_or_insert_with(ru_flags_by_suffix);
                let chars: Vec<char> = w.chars().collect();
                let found = [4, 3]
                    .into_iter()
                    .filter(|&l| chars.len() >= l + 2 && !w.contains('-'))
                    .find_map(|l| map.get(&chars[chars.len() - l..].iter().collect::<String>()));
                n += self.ru.add(&found.map_or(w.clone(), |f| format!("{w}/{f}"))).is_ok() as usize;
            } else if w.chars().all(|c| c.is_ascii_alphabetic() || c == '\'') {
                let f = self.en_freq.entry(w.into()).or_insert(0);
                *f = (*f).max(TERM_FREQ);
                n += 1;
            }
        }
        n
    }

    pub fn set_extra<I: IntoIterator<Item = String>>(&mut self, words: I) {
        self.extra = words.into_iter().map(|w| w.to_lowercase()).collect();
    }

    fn hunspell(&self, lang: Lang) -> &Dictionary {
        match lang {
            Lang::Ru => &self.ru,
            Lang::En => &self.en,
        }
    }

    pub fn freq(&self, lang: Lang, word: &str) -> u32 {
        let map = match lang {
            Lang::Ru => &self.ru_freq,
            Lang::En => &self.en_freq,
        };
        let lower = word.to_lowercase();
        map.get(lower.as_str())
            .copied()
            .or_else(|| (lang == Lang::Ru).then(|| map.get(lower.replace('ё', "е").as_str()).copied()).flatten())
            .unwrap_or(0)
    }

    pub fn frequent_from(&self, lang: Lang, first: char) -> &[(Box<[char]>, u32)] {
        match lang {
            Lang::Ru => self.ru_by_first.get(&first).map(Vec::as_slice).unwrap_or(&[]),
            Lang::En => &[],
        }
    }

    fn bigram_table(&self, lang: Lang) -> (&HashMap<Box<str>, u32>, &'static [u8]) {
        match lang {
            Lang::Ru => (&self.ru_ids, RU_BIGRAMS),
            Lang::En => (&self.en_ids, EN_BIGRAMS),
        }
    }

    fn bigram_id(&self, lang: Lang, word: &str) -> Option<u32> {
        let key = word.trim_end_matches(['-', '\'']).to_lowercase().replace('ё', "е");
        self.bigram_table(lang).0.get(key.as_str()).copied()
    }

    pub fn in_bigrams(&self, lang: Lang, word: &str) -> bool {
        self.bigram_id(lang, word).is_some()
    }

    pub fn bigram(&self, lang: Lang, prev: &str, word: &str) -> u32 {
        let (Some(a), Some(b)) = (self.bigram_id(lang, prev), self.bigram_id(lang, word)) else { return 0 };
        let key = |r: &[u8; 7]| (u32::from_le_bytes([r[0], r[1], r[2], 0]), u32::from_le_bytes([r[3], r[4], r[5], 0]));
        let (recs, _) = self.bigram_table(lang).1.as_chunks::<7>();
        recs.binary_search_by(|r| key(r).cmp(&(a, b)))
            .map_or(0, |i| (recs[i][6] as f64 / BIGRAM_SCALE).exp().round() as u32)
    }

    fn lookup(&self, lang: Lang, word: &str, min_freq: u32) -> bool {
        if word.is_empty() || !word.chars().any(|c| lang.owns_letter(c)) {
            return false;
        }
        if word.chars().any(|c| c.is_alphabetic() && !lang.owns_letter(c)) {
            return false;
        }
        let lower = word.to_lowercase();
        if self.extra.contains(&lower) || self.freq(lang, &lower) >= min_freq {
            return true;
        }
        let h = self.hunspell(lang);
        (min_freq <= KNOWN_FREQ && self.is_form(lang, &lower)) || h.check(word) || (lower != word && h.check(&lower))
    }

    pub fn is_form(&self, lang: Lang, word: &str) -> bool {
        lang == Lang::Ru && self.ru_forms.as_ref().is_some_and(|f| f.contains(word.to_lowercase().replace('ё', "е")))
    }

    pub fn start_possible(&self, lang: Lang, prefix: &str) -> bool {
        let p = norm(prefix);
        let set = match lang {
            Lang::Ru => &self.ru_starts,
            Lang::En => &self.en_starts,
        };
        has_start(set, &p)
            || (lang == Lang::Ru && self.ru_forms.as_ref().is_some_and(|f| has_start(f, &p)))
            || self.user_starts.iter().any(|w| w.starts_with(&p))
            || self.extra.iter().any(|w| norm(w).starts_with(&p))
    }

    pub fn start_share(&self, lang: Lang, prefix: &str) -> f64 {
        let p = norm(prefix);
        let (list, total) = match lang {
            Lang::Ru => (&self.ru_sorted, self.ru_total),
            Lang::En => (&self.en_sorted, self.en_total),
        };
        let from = list.partition_point(|(w, _)| **w < *p);
        list[from..].iter().take_while(|(w, _)| w.starts_with(&p)).map(|&(_, f)| f as f64).sum::<f64>() / total
    }

    pub fn known(&self, lang: Lang, word: &str) -> bool {
        self.lookup(lang, word, KNOWN_FREQ)
    }

    pub fn seen(&self, lang: Lang, word: &str) -> bool {
        self.lookup(lang, word, 1)
    }

    pub fn certain(&self, lang: Lang, word: &str) -> bool {
        self.lookup(lang, word, u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_words() {
        let d = Dictionaries::load();
        for w in ["привет", "ввода", "текста", "Привет", "пришёл", "ёлка"] {
            assert!(d.known(Lang::Ru, w), "{w}");
        }
        for w in ["hello", "code", "claude", "git", "Hello"] {
            assert!(d.known(Lang::En, w), "{w}");
        }
        for w in ["ввожа", "сщву", "руддщ", "привет1"] {
            assert!(!d.known(Lang::Ru, w), "{w}");
        }
        for w in ["ghbdtn", "vbh", "hello"] {
            assert_eq!(d.known(Lang::En, w), w == "hello", "{w}");
        }
        assert!(!d.known(Lang::En, "привет"));
        assert!(d.freq(Lang::Ru, "ввода") > 0);
    }

    #[test]
    fn it_vocabulary() {
        let d = Dictionaries::load();
        for w in ["задеплоил", "закоммитить", "коммиты", "фичу", "фронтенде", "пофиксил", "мержим"] {
            assert!(d.certain(Lang::Ru, w), "{w}");
        }
        for w in ["grafana", "kubernetes", "nodemon", "tauri"] {
            assert!(d.known(Lang::En, w), "{w}");
        }
    }

    #[test]
    fn user_words() {
        let mut d = Dictionaries::load();
        assert!(!d.known(Lang::Ru, "толщиномер"));
        assert_eq!(d.add_user_words("толщиномер\t86\nstillmvd\n# коммент\nx\n"), 2);
        assert!(d.certain(Lang::Ru, "толщиномер"));
        assert!(d.known(Lang::En, "stillmvd"));
    }

    #[test]
    fn extra_words() {
        let mut d = Dictionaries::load();
        assert!(!d.known(Lang::Ru, "чето"));
        d.set_extra(["чето".to_string()]);
        assert!(d.known(Lang::Ru, "Чето"));
    }
}
