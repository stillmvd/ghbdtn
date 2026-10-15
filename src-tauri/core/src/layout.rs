use crate::dict::Dictionaries;
use crate::keymap::Lang;
use crate::word::{render, Key};

use crate::typo::near_freq;

const ALT_FREQ: u32 = 200;
const OVERRIDE_FREQ: u32 = 20_000;
const NEAR_RATIO: u32 = 10;
const TAIL: &[char] = &[',', '.', ';', ':', '\'', '"', '?', '!', ')', '»'];

pub fn split_tail(text: &str, lang: Lang) -> (&str, &str) {
    let core = text.trim_end_matches(|c: char| TAIL.contains(&c) && !lang.owns_letter(c));
    (core, &text[core.len()..])
}

fn plain_word(word: &str, lang: Lang) -> bool {
    let chars: Vec<char> = word.chars().collect();
    let letter = |c: &char| lang.owns_letter(*c);
    chars.first().is_some_and(letter)
        && chars.last().is_some_and(letter)
        && chars.iter().all(|c| letter(c) || *c == '-')
        && !crate::token::is_code_token(word, false)
}

fn acceptable(core: &str, lang: Lang, dicts: &Dictionaries) -> bool {
    let len = core.chars().filter(|c| c.is_alphabetic()).count();
    if len < 3 || !plain_word(core, lang) {
        return false;
    }
    let freq = dicts.freq(lang, core);
    if len <= 4 {
        return freq > 0 && dicts.known(lang, core);
    }
    dicts.certain(lang, core) || freq >= ALT_FREQ
}

pub fn decide_layout(keys: &[Key], lang: Lang, dicts: &Dictionaries) -> Option<String> {
    let typed = render(keys, lang);
    let (typed_core, _) = split_tail(&typed, lang);
    if typed_core.is_empty() {
        return None;
    }
    let alt = lang.other();
    let full = render(keys, alt);
    let (core, _) = split_tail(&full, alt);
    let (text, core) = if acceptable(core, alt, dicts) {
        (full.clone(), core.to_string())
    } else {
        let trailing = keys.iter().rev().take_while(|k| k.char_in(lang).is_some_and(|c| TAIL.contains(&c))).count();
        if trailing == 0 || trailing == keys.len() {
            return None;
        }
        let (head, tail) = keys.split_at(keys.len() - trailing);
        let core = render(head, alt);
        if !acceptable(&core, alt, dicts) {
            return None;
        }
        (core.clone() + &render(tail, lang), core)
    };
    let alt_freq = dicts.freq(alt, &core);
    if dicts.seen(lang, typed_core) {
        let rare_form = dicts.freq(lang, typed_core) == 0 && alt_freq >= OVERRIDE_FREQ;
        if !rare_form {
            return None;
        }
    }
    if near_freq(typed_core, lang, dicts).is_some_and(|n| alt_freq < NEAR_RATIO.saturating_mul(n)) {
        return None;
    }
    Some(text)
}

const SHORT_RU: &[&str] = &[
    "а", "в", "и", "к", "о", "с", "у", "я", "не", "на", "но", "же", "ли", "бы", "по", "за", "из", "от", "до", "да", "ты",
    "мы", "вы", "он", "то", "ну", "уж", "во", "со", "ко", "об", "ей", "им", "их", "её", "ее",
];
const SHORT_EN: &[&str] = &[
    "a", "i", "to", "of", "in", "on", "at", "is", "it", "be", "we", "he", "me", "my", "an", "or", "as", "by", "so", "no",
    "do", "go", "up", "us", "if", "am", "ok", "hi", "oh",
];

fn short_word(word: &str, lang: Lang) -> bool {
    let list = match lang {
        Lang::Ru => SHORT_RU,
        Lang::En => SHORT_EN,
    };
    list.contains(&word.to_lowercase().as_str())
}

pub fn short_alongside(keys: &[Key], lang: Lang) -> Option<String> {
    if keys.is_empty() || keys.len() > 2 {
        return None;
    }
    let typed = render(keys, lang);
    let alt = render(keys, lang.other());
    (!short_word(&typed, lang) && short_word(&alt, lang.other())).then_some(alt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word::keys_for;
    use std::sync::OnceLock;

    fn dicts() -> &'static Dictionaries {
        static D: OnceLock<Dictionaries> = OnceLock::new();
        D.get_or_init(Dictionaries::load)
    }

    fn fix(text: &str, lang: Lang) -> Option<String> {
        decide_layout(&keys_for(text, lang), lang, dicts())
    }

    #[test]
    fn wrong_layout_is_fixed() {
        assert_eq!(fix("ghbdtn", Lang::En).as_deref(), Some("привет"));
        assert_eq!(fix("Ghbdtn", Lang::En).as_deref(), Some("Привет"));
        assert_eq!(fix("сщву", Lang::Ru).as_deref(), Some("code"));
        assert_eq!(fix(",tp", Lang::En).as_deref(), Some("без"));
        assert_eq!(fix("j,obq", Lang::En).as_deref(), Some("общий"));
        assert_eq!(fix("сщвуб", Lang::Ru).as_deref(), Some("code,"));
        assert_eq!(fix("ghbdtn,", Lang::En).as_deref(), Some("привет,"));
        assert_eq!(fix("k.,jq", Lang::En).as_deref(), Some("любой"));
        assert_eq!(fix("еру", Lang::Ru).as_deref(), Some("the"));
        assert_eq!(fix("ершы", Lang::Ru).as_deref(), Some("this"));
        assert_eq!(fix("рфму", Lang::Ru).as_deref(), Some("have"));
        assert_eq!(fix("огые", Lang::Ru).as_deref(), Some("just"));
    }

    #[test]
    fn short_words_follow_neighbour() {
        let s = |t: &str, l| short_alongside(&keys_for(t, l), l);
        assert_eq!(s("b", Lang::En).as_deref(), Some("и"));
        assert_eq!(s("ye", Lang::En).as_deref(), Some("ну"));
        assert_eq!(s("a", Lang::En), None);
        assert_eq!(s("I", Lang::En), None);
        assert_eq!(s("с", Lang::Ru), None);
        assert_eq!(s("ш", Lang::Ru).as_deref(), Some("i"));
        assert_eq!(s("ghb", Lang::En), None);
    }

    #[test]
    fn right_layout_is_kept() {
        for (w, l) in [("hello", Lang::En), ("привет", Lang::Ru), ("ye", Lang::En), ("yt", Lang::En), ("GHBDTN", Lang::En), ("ya.ru", Lang::En), ("don't", Lang::En), ("Hello,", Lang::En)] {
            assert_eq!(fix(w, l), None, "{w}");
        }
    }
}
