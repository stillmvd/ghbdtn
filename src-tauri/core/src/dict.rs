use crate::keymap::Lang;
use spellbook::Dictionary;
use std::collections::{HashMap, HashSet};

const KNOWN_FREQ: u32 = 30;
const TECH_FREQ: u32 = 1000;
const TERM_FREQ: u32 = 300;

pub struct Dictionaries {
    ru: Dictionary,
    en: Dictionary,
    ru_freq: HashMap<Box<str>, u32>,
    en_freq: HashMap<Box<str>, u32>,
    extra: HashSet<String>,
    ru_by_first: ByFirst,
}

type ByFirst = HashMap<char, Vec<(Box<[char]>, u32)>>;

fn by_first(freq: &HashMap<Box<str>, u32>) -> ByFirst {
    let mut out: ByFirst = HashMap::new();
    for (w, &f) in freq {
        if f >= KNOWN_FREQ && w.chars().count() >= 4 {
            if let Some(c) = w.chars().next() {
                out.entry(c).or_default().push((w.chars().collect(), f));
            }
        }
    }
    out
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
        Dictionaries { ru, en, ru_by_first: by_first(&ru_freq), ru_freq, en_freq, extra: HashSet::new() }
    }

    pub fn add_user_words(&mut self, text: &str) -> usize {
        let mut n = 0;
        for w in text.lines().map(|l| l.split('\t').next().unwrap_or("").trim().to_lowercase()) {
            if w.chars().count() < 2 || w.starts_with('#') {
                continue;
            }
            if w.chars().all(|c| Lang::Ru.owns_letter(c) || c == '-') {
                n += self.ru.add(&w).is_ok() as usize;
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
        h.check(word) || h.check(&lower)
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
