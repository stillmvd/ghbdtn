use crate::dict::Dictionaries;
use crate::keymap::Lang;
use crate::layout::is_acronym;
use crate::word::Key;

const MIN_CAPS: usize = 3;
const MIN_DOUBLE: usize = 4;

fn capitalize(lower: &str) -> String {
    let mut chars = lower.chars();
    chars.next().map(|f| f.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn plain(chars: &[char], lang: Lang) -> bool {
    chars.iter().all(|&c| lang.owns_letter(c) || c == '-') && chars.first().is_some_and(|&c| lang.owns_letter(c))
}

pub fn caps_lock(keys: &[Key], core: &str, lang: Lang, dicts: &Dictionaries) -> Option<String> {
    let chars: Vec<char> = core.chars().collect();
    if chars.len() < MIN_CAPS || !plain(&chars, lang) || !keys.iter().all(|k| k.caps) {
        return None;
    }
    let letters = || chars.iter().filter(|c| c.is_alphabetic());
    if !chars[0].is_lowercase() || !letters().skip(1).all(|c| c.is_uppercase()) {
        return None;
    }
    let lower = core.to_lowercase();
    (dicts.known(lang, &lower) && dicts.freq(lang, &lower) > 0).then(|| capitalize(&lower))
}

pub fn double_caps(core: &str, lang: Lang, dicts: &Dictionaries) -> Option<String> {
    let chars: Vec<char> = core.chars().collect();
    if chars.len() < MIN_DOUBLE || !plain(&chars, lang) {
        return None;
    }
    if !(chars[0].is_uppercase() && chars[1].is_uppercase() && chars[2..].iter().all(|c| c.is_lowercase() || *c == '-')) {
        return None;
    }
    let prefix: String = chars[..2].iter().collect();
    let lower = core.to_lowercase();
    (!is_acronym(&prefix, lang) && dicts.certain(lang, &lower)).then(|| capitalize(&lower))
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

    fn with_caps(intended: &str, lang: Lang) -> (Vec<Key>, String) {
        let keys: Vec<Key> = keys_for(intended, lang).into_iter().map(|k| Key { caps: true, ..k }).collect();
        let typed = crate::word::render(&keys, lang);
        (keys, typed)
    }

    #[test]
    fn caps_lock_words() {
        let (keys, typed) = with_caps("Hello", Lang::En);
        assert_eq!(typed, "hELLO");
        assert_eq!(caps_lock(&keys, &typed, Lang::En, dicts()).as_deref(), Some("Hello"));
        let (keys, typed) = with_caps("Привет", Lang::Ru);
        assert_eq!(caps_lock(&keys, &typed, Lang::Ru, dicts()).as_deref(), Some("Привет"));
        let plain = keys_for("hELLO", Lang::En);
        assert_eq!(caps_lock(&plain, "hELLO", Lang::En, dicts()), None);
        let (keys, typed) = with_caps("Qzxw", Lang::En);
        assert_eq!(caps_lock(&keys, &typed, Lang::En, dicts()), None);
    }

    #[test]
    fn double_capitals() {
        let d = |w: &str, l| double_caps(w, l, dicts());
        assert_eq!(d("ПРивет", Lang::Ru).as_deref(), Some("Привет"));
        assert_eq!(d("HEllo", Lang::En).as_deref(), Some("Hello"));
        for (w, l) in [("IDs", Lang::En), ("ITer", Lang::En), ("ИПшник", Lang::Ru), ("PRivet", Lang::En), ("Привет", Lang::Ru), ("HELLO", Lang::En)] {
            assert_eq!(d(w, l), None, "{w}");
        }
    }
}
