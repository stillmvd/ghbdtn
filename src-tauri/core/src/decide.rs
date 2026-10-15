use crate::config::Config;
use crate::dict::Dictionaries;
use crate::keymap::Lang;
use crate::layout::{acronym, layout_after, short_pair, split_tail};
use crate::replace::apply_case;
use crate::token::is_code_token;
use crate::typo::fix_after;
use crate::undo::ActionKind;
use crate::word::{render, Key};

pub struct WordInput<'a> {
    pub keys: &'a [Key],
    pub lang: Lang,
    pub code_mode: bool,
    pub prev: Option<&'a str>,
}

#[derive(Debug, PartialEq)]
pub struct Fix {
    pub kind: ActionKind,
    pub typed: String,
    pub text: String,
    pub to: Option<Lang>,
}

pub fn decide(input: &WordInput, dicts: &Dictionaries, cfg: &Config) -> Option<Fix> {
    let typed = render(input.keys, input.lang);
    let (core, tail) = split_tail(&typed, input.lang);
    if core.is_empty() || cfg.is_never(core) {
        return None;
    }
    let fix = |kind, text: String, to| Some(Fix { kind, typed: typed.clone(), text, to });
    if cfg.features.replace {
        if let Some(r) = cfg.replacement(core) {
            return fix(ActionKind::Replace, apply_case(core, r) + tail, None);
        }
    }
    if cfg.features.layout && !(input.code_mode && is_code_token(core, true)) {
        if let Some(text) = layout_after(input.keys, input.lang, input.prev, dicts)
            .or_else(|| short_pair(input.keys, input.lang, input.prev, dicts))
            .or_else(|| acronym(input.keys, input.lang, dicts)) {
            return fix(ActionKind::Layout, text, Some(input.lang.other()));
        }
    }
    if cfg.features.typos && !is_code_token(core, input.code_mode) && !(input.code_mode && input.lang == Lang::En) {
        if let Some(text) = fix_after(core, input.prev, input.lang, dicts) {
            return fix(ActionKind::Typo, text + tail, None);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NeverSource;
    use crate::word::keys_for;
    use std::sync::OnceLock;

    fn dicts() -> &'static Dictionaries {
        static D: OnceLock<Dictionaries> = OnceLock::new();
        D.get_or_init(|| {
            let mut d = Dictionaries::load();
            d.set_extra(Config::default().replacements.into_keys());
            d
        })
    }

    fn run(text: &str, lang: Lang, cfg: &Config) -> Option<(ActionKind, String)> {
        let keys = keys_for(text, lang);
        decide(&WordInput { keys: &keys, lang, code_mode: false, prev: None }, dicts(), cfg).map(|f| (f.kind, f.text))
    }

    #[test]
    fn pipeline_order() {
        let cfg = Config::default();
        assert_eq!(run("Чето", Lang::Ru, &cfg), Some((ActionKind::Replace, "Чё-то".into())));
        assert_eq!(run("ghbdtn", Lang::En, &cfg), Some((ActionKind::Layout, "привет".into())));
        assert_eq!(run("ввожа", Lang::Ru, &cfg), Some((ActionKind::Typo, "ввода".into())));
        assert_eq!(run("ввожа,", Lang::Ru, &cfg), Some((ActionKind::Typo, "ввода,".into())));
        assert_eq!(run("привет", Lang::Ru, &cfg), None);
    }

    #[test]
    fn switches_and_never() {
        let mut cfg = Config::default();
        cfg.features.layout = false;
        assert_eq!(run("ghbdtn", Lang::En, &cfg), None);
        assert_eq!(run("ввожа", Lang::Ru, &cfg).map(|f| f.0), Some(ActionKind::Typo));
        cfg.features.replace = false;
        assert_eq!(run("чето", Lang::Ru, &cfg), None);
        let mut cfg = Config::default();
        cfg.add_never("ghbdtn", NeverSource::Undo);
        assert_eq!(run("Ghbdtn", Lang::En, &cfg), None);
    }

    #[test]
    fn code_mode_keeps_commands() {
        let cfg = Config::default();
        for w in ["--watch", "~", "$HOME", "a=b"] {
            let keys = keys_for(w, Lang::En);
            assert_eq!(decide(&WordInput { keys: &keys, lang: Lang::En, code_mode: true, prev: None }, dicts(), &cfg), None, "{w}");
        }
    }
}
