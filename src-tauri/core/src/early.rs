use crate::config::Config;
use crate::decide::{Fix, Rule, WordInput};
use crate::dict::Dictionaries;
use crate::keymap::Lang;
use crate::token::is_code_token;
use crate::typo::adjacent;
use crate::undo::ActionKind;
use crate::word::render;

pub const EARLY_MIN: usize = 3;
pub const EARLY_MAX: usize = 6;
pub const EARLY_SHARE: f64 = 2e-5;
pub const EARLY_RATIO: f64 = 10.0;

pub fn early_fix(input: &WordInput, dicts: &Dictionaries, cfg: &Config) -> Option<Fix> {
    if input.code_mode || !cfg.features.layout || !cfg.features.early || !(EARLY_MIN..=EARLY_MAX).contains(&input.keys.len()) {
        return None;
    }
    let (from, to) = (input.lang, input.lang.other());
    let typed = render(input.keys, from);
    let text = render(input.keys, to);
    if typed.chars().count() != input.keys.len() || !text.chars().all(|c| to.owns_letter(c)) || is_code_token(&text, false) {
        return None;
    }
    if cfg.never_starts(&typed) || dicts.start_possible(from, &typed) {
        return None;
    }
    let share = dicts.start_share(to, &text);
    if share < EARLY_SHARE || share < EARLY_RATIO * typo_share(&typed, from, dicts) {
        return None;
    }
    Some(Fix { kind: ActionKind::Layout, rule: Rule::Early, typed, text, to: Some(to) })
}

fn typo_share(typed: &str, lang: Lang, dicts: &Dictionaries) -> f64 {
    let c: Vec<char> = typed.to_lowercase().chars().collect();
    let mut total = 0.0;
    for i in 0..c.len() {
        for r in lang.alphabet().chars().filter(|&r| adjacent(c[i], r, lang)) {
            let mut v = c.clone();
            v[i] = r;
            total += dicts.start_share(lang, &v.iter().collect::<String>());
        }
        if i + 1 < c.len() && c[i] != c[i + 1] {
            let mut v = c.clone();
            v.swap(i, i + 1);
            total += dicts.start_share(lang, &v.iter().collect::<String>());
        }
        if i > 0 {
            for r in lang.alphabet().chars() {
                let mut v = c.clone();
                v.insert(i, r);
                total += dicts.start_share(lang, &v.iter().collect::<String>());
            }
        }
    }
    total
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

    fn early(text: &str, lang: Lang) -> Option<String> {
        let keys = keys_for(text, lang);
        early_fix(&WordInput { keys: &keys, lang, code_mode: false, prev: None }, dicts(), &Config::default()).map(|f| f.text)
    }

    #[test]
    fn fixes_impossible_starts() {
        assert_eq!(early("ghb", Lang::En), None);
        assert_eq!(early("ghbd", Lang::En).as_deref(), Some("прив"));
        assert_eq!(early("ckjd", Lang::En).as_deref(), Some("слов"));
        assert_eq!(early("Ghbd", Lang::En).as_deref(), Some("Прив"));
        assert_eq!(early("рудд", Lang::Ru).as_deref(), Some("hell"));
    }

    #[test]
    fn keeps_possible_starts() {
        for w in ["hel", "the", "wor", "str", "xyl"] {
            assert_eq!(early(w, Lang::En), None, "{w}");
        }
        for w in ["при", "руд", "что", "здр"] {
            assert_eq!(early(w, Lang::Ru), None, "{w}");
        }
        assert_eq!(early("gh", Lang::En), None);
        assert_eq!(early("GHBD", Lang::En), None);
        assert_eq!(early("gHbd", Lang::En), None);
        let keys = keys_for("ghb", Lang::En);
        assert_eq!(early_fix(&WordInput { keys: &keys, lang: Lang::En, code_mode: true, prev: None }, dicts(), &Config::default()), None);
    }

    #[test]
    fn never_list_blocks() {
        let mut cfg = Config::default();
        cfg.add_never("ghbdtn", crate::config::NeverSource::Undo);
        let keys = keys_for("ghb", Lang::En);
        assert_eq!(early_fix(&WordInput { keys: &keys, lang: Lang::En, code_mode: false, prev: None }, dicts(), &cfg), None);
    }
}
