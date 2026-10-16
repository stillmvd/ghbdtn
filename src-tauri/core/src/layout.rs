use crate::decide::Rule;
use crate::dict::Dictionaries;
use crate::keymap::Lang;
use crate::typo::near_freq;
use crate::word::{keys_for, render, Key};

const ALT_FREQ: u32 = 200;
const SHORT_ALT_FREQ: u32 = 1000;
const OVERRIDE_FREQ: u32 = 20_000;
const NEAR_RATIO: u32 = 10;
const PAIR_MIN: u32 = 10;
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
    if len == 3 && freq < SHORT_ALT_FREQ {
        return false;
    }
    if len <= 4 {
        return freq > 0 && dicts.known(lang, core);
    }
    dicts.certain(lang, core) || freq >= ALT_FREQ
}

pub fn other_layout_word(keys: &[Key], lang: Lang, dicts: &Dictionaries) -> bool {
    let alt = lang.other();
    let full = render(keys, alt);
    let (core, _) = split_tail(&full, alt);
    acceptable(core, alt, dicts)
}

pub fn decide_layout(keys: &[Key], lang: Lang, dicts: &Dictionaries) -> Option<String> {
    layout_after(keys, lang, None, dicts).map(|f| f.0)
}

pub fn layout_after(keys: &[Key], lang: Lang, prev: Option<&str>, dicts: &Dictionaries) -> Option<(String, Rule)> {
    let prev_alt = prev.and_then(|p| short_alongside(&keys_for(p, lang), lang));
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
    let lower = typed_core.to_lowercase();
    if prev_alt.is_some_and(|p| !dicts.known(lang, &lower) && dicts.bigram(alt, &p, &core.to_lowercase()) > 0) {
        return Some((text, Rule::LayoutAfterShort));
    }
    if dicts.seen(lang, typed_core) {
        let rare_form = dicts.freq(lang, typed_core) == 0 && alt_freq >= OVERRIDE_FREQ;
        if !rare_form {
            return None;
        }
    }
    if near_freq(typed_core, lang, dicts).is_some_and(|n| alt_freq < NEAR_RATIO.saturating_mul(n)) {
        return None;
    }
    Some((text, Rule::Layout))
}

const SHORT_RU: &[&str] = &[
    "а", "в", "и", "к", "о", "с", "у", "я", "не", "на", "но", "же", "ли", "бы", "по", "за", "из", "от", "до", "да", "ты",
    "мы", "вы", "он", "то", "ну", "уж", "во", "со", "ко", "об", "ей", "им", "их", "её", "ее",
];
const SHORT_EN: &[&str] = &[
    "a", "i", "to", "of", "in", "on", "at", "is", "it", "be", "we", "he", "me", "my", "an", "or", "as", "by", "so", "no",
    "do", "go", "up", "us", "if", "am", "ok", "hi", "oh",
];

const ACRO_EN: &[&str] = &[
    "AI", "API", "UI", "UX", "ML", "LLM", "GPT", "SDK", "CLI", "IDE", "CSS", "HTML", "JSON", "XML", "YAML", "SQL", "VPN",
    "USB", "PDF", "URL", "HTTP", "HTTPS", "DNS", "SSH", "SSL", "TLS", "CPU", "GPU", "RAM", "SSD", "HDD", "OS", "PC", "IT",
    "HR", "QA", "CI", "CD", "PR", "MR", "JS", "TS", "JSX", "TSX", "DB", "ID", "IP", "TV", "PM", "SEO", "MVP", "KPI", "CEO",
    "CTO", "FAQ", "ASAP", "FYI", "IMO", "BTW", "AWS", "GCP", "CDN", "CRM", "ERP", "OCR", "NFT", "VR", "AR", "IOT", "RGB",
    "SVG", "PNG", "JPG", "GIF", "MP3", "MP4", "DM", "PS", "TBD", "WIP", "ETA", "UTC", "GMT",
];
const ACRO_RU: &[&str] = &[
    "ИИ", "США", "РФ", "МВД", "ЖКХ", "ВУЗ", "ТЦ", "ЖК", "МГУ", "ФСБ", "ГИБДД", "ДТП", "ИП", "ООО", "НДС", "ЗП", "ТВ", "ПК",
    "СМИ", "ОС", "ЕГЭ", "ОГЭ", "ВВП", "РЖД", "МЧС", "ЦБ", "СНГ", "ЕС", "ТЗ", "ЛС", "ЛК", "ДР", "НГ", "МСК", "СПБ", "РАН",
    "ФНС", "ПФР", "МФЦ", "ДС", "ЧП", "ВК", "ТГ",
];
const ACRO_MAX: usize = 5;

pub fn acronym(keys: &[Key], lang: Lang, dicts: &Dictionaries) -> Option<String> {
    let trailing = keys.iter().rev().take_while(|k| k.char_in(lang).is_some_and(|c| TAIL.contains(&c))).count();
    let (keys, tail) = keys.split_at(keys.len() - trailing);
    let typed = render(keys, lang);
    let n = typed.chars().count();
    if !(2..=ACRO_MAX).contains(&n) || !typed.chars().all(|c| c.is_uppercase() && lang.owns_letter(c)) {
        return None;
    }
    let alt = render(keys, lang.other());
    let list = match lang.other() {
        Lang::Ru => ACRO_RU,
        Lang::En => ACRO_EN,
    };
    let own = match lang {
        Lang::Ru => ACRO_RU,
        Lang::En => ACRO_EN,
    };
    let lower = typed.to_lowercase();
    (list.contains(&alt.as_str()) && !own.contains(&typed.as_str()) && !dicts.seen(lang, &lower) && !dicts.known(lang, &lower))
        .then(|| alt + &render(tail, lang))
}

pub(crate) fn short_word(word: &str, lang: Lang) -> bool {
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

pub fn short_pair(keys: &[Key], lang: Lang, prev: Option<&str>, dicts: &Dictionaries) -> Option<String> {
    let prev = prev?;
    let alt = short_alongside(keys, lang)?;
    let prev_alt = short_alongside(&keys_for(prev, lang), lang)?;
    let typed = render(keys, lang);
    let lower = |t: &str| !t.chars().any(char::is_uppercase);
    (lower(&typed) && lower(prev) && typed.chars().count() + prev.chars().count() >= 3
        && dicts.bigram(lang.other(), &prev_alt, &alt) >= PAIR_MIN)
        .then_some(alt)
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
    fn short_word_before_sets_layout() {
        let f = |t: &str, prev| layout_after(&keys_for(t, Lang::En), Lang::En, prev, dicts()).map(|f| f.0);
        assert_eq!(f("botn", Some("yt")).as_deref(), Some("ищет"));
        assert_eq!(f("both", Some("yt")), None);
        assert_eq!(f("botn", Some("it")), None);
    }

    #[test]
    fn acronyms_switch() {
        let a = |t: &str, l| acronym(&keys_for(t, l), l, dicts());
        assert_eq!(a("ФШ", Lang::Ru).as_deref(), Some("AI"));
        assert_eq!(a("ФЗШ", Lang::Ru).as_deref(), Some("API"));
        assert_eq!(a("CIF", Lang::En).as_deref(), Some("США"));
        assert_eq!(a("фш", Lang::Ru), None);
        assert_eq!(a("ФШ?", Lang::Ru).as_deref(), Some("AI?"));
        assert_eq!(a("ОК", Lang::Ru), None);
        assert_eq!(a("IT", Lang::En), None);
    }

    #[test]
    fn short_pair_switches_together() {
        let p = |t: &str, prev, l| short_pair(&keys_for(t, l), l, Some(prev), dicts());
        assert_eq!(p("tq", "vs", Lang::En).as_deref(), Some("ей"));
        assert_eq!(p("yt", "z", Lang::En).as_deref(), Some("не"));
        assert_eq!(p("c", "b", Lang::En), None);
        assert_eq!(p("Tq", "vs", Lang::En), None);
        assert_eq!(p("jn", "lj", Lang::En), None);
        assert_eq!(p("it", "vs", Lang::En), None);
        assert_eq!(short_pair(&keys_for("tq", Lang::En), Lang::En, None, dicts()), None);
    }

    #[test]
    fn right_layout_is_kept() {
        for (w, l) in [("hello", Lang::En), ("привет", Lang::Ru), ("ye", Lang::En), ("yt", Lang::En), ("GHBDTN", Lang::En), ("ya.ru", Lang::En), ("don't", Lang::En), ("Hello,", Lang::En)] {
            assert_eq!(fix(w, l), None, "{w}");
        }
    }
}
