use crate::case::{caps_lock, double_caps};
use crate::yo::yo_fix;
use crate::config::{Config, Sensitivity};
use crate::dict::Dictionaries;
use crate::keymap::Lang;
use crate::layout::{acronym, layout_after, other_layout_word, short_pair, split_tail};
use crate::replace::apply_case;
use crate::token::is_code_token;
use crate::typo::fix_rule;
use crate::undo::ActionKind;
use crate::word::{render, Key};

pub struct WordInput<'a> {
    pub keys: &'a [Key],
    pub lang: Lang,
    pub code_mode: bool,
    pub prev: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rule {
    Replace,
    Layout,
    LayoutAfterShort,
    ShortPair,
    Acronym,
    ShortFix,
    SplitFix,
    RealWord,
    Lone,
    Best,
    Context,
    Joint,
    FarFix,
    Early,
    Misspelled,
    CapsLock,
    DoubleCaps,
    Yo,
    WeakWord,
}

impl Rule {
    pub const ALL: [Rule; 19] = [
        Rule::Replace,
        Rule::Layout,
        Rule::LayoutAfterShort,
        Rule::ShortPair,
        Rule::Acronym,
        Rule::ShortFix,
        Rule::SplitFix,
        Rule::RealWord,
        Rule::Lone,
        Rule::Best,
        Rule::Context,
        Rule::Joint,
        Rule::FarFix,
        Rule::Early,
        Rule::Misspelled,
        Rule::CapsLock,
        Rule::DoubleCaps,
        Rule::Yo,
        Rule::WeakWord,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Rule::Replace => "replace",
            Rule::Layout => "layout",
            Rule::LayoutAfterShort => "layout_after_short",
            Rule::ShortPair => "short_pair",
            Rule::Acronym => "acronym",
            Rule::ShortFix => "short_fix",
            Rule::SplitFix => "split_fix",
            Rule::RealWord => "real_word",
            Rule::Lone => "lone",
            Rule::Best => "best",
            Rule::Context => "context",
            Rule::Joint => "joint",
            Rule::FarFix => "far_fix",
            Rule::Early => "early",
            Rule::Misspelled => "misspelled",
            Rule::CapsLock => "caps_lock",
            Rule::DoubleCaps => "double_caps",
            Rule::Yo => "yo",
            Rule::WeakWord => "weak_word",
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Fix {
    pub kind: ActionKind,
    pub rule: Rule,
    pub typed: String,
    pub text: String,
    pub to: Option<Lang>,
}

const RISKY: [Rule; 6] = [Rule::Context, Rule::Joint, Rule::Lone, Rule::ShortFix, Rule::SplitFix, Rule::WeakWord];

pub fn allowed(rule: Rule, level: Sensitivity) -> bool {
    level != Sensitivity::Careful || !RISKY.contains(&rule)
}

pub fn decide(input: &WordInput, dicts: &Dictionaries, cfg: &Config) -> Option<Fix> {
    decide_any(input, dicts, cfg).filter(|f| allowed(f.rule, cfg.sensitivity))
}

fn decide_any(input: &WordInput, dicts: &Dictionaries, cfg: &Config) -> Option<Fix> {
    let typed = render(input.keys, input.lang);
    let (core, tail) = split_tail(&typed, input.lang);
    if core.is_empty() || cfg.is_never(core) {
        return None;
    }
    let fix = |kind, rule, text: String, to| Some(Fix { kind, rule, typed: typed.clone(), text, to });
    if cfg.features.replace {
        if let Some(r) = cfg.replacement(core) {
            return fix(ActionKind::Replace, Rule::Replace, apply_case(core, r) + tail, None);
        }
    }
    if cfg.features.yo {
        if let Some(text) = yo_fix(core, input.lang) {
            return fix(ActionKind::Replace, Rule::Yo, text + tail, None);
        }
    }
    if cfg.features.caps && !input.code_mode {
        if let Some(text) = caps_lock(input.keys, core, input.lang, dicts) {
            return fix(ActionKind::Typo, Rule::CapsLock, text + tail, None);
        }
        if let Some(text) = double_caps(core, input.lang, dicts) {
            return fix(ActionKind::Typo, Rule::DoubleCaps, text + tail, None);
        }
    }
    let layout_on = cfg.features.layout && !(input.code_mode && is_code_token(core, true));
    if layout_on {
        if let Some((text, rule)) = layout_after(input.keys, input.lang, input.prev, dicts)
            .or_else(|| short_pair(input.keys, input.lang, input.prev, dicts).map(|t| (t, Rule::ShortPair)))
            .or_else(|| acronym(input.keys, input.lang, dicts).map(|t| (t, Rule::Acronym))) {
            return fix(ActionKind::Layout, rule, text, Some(input.lang.other()));
        }
    }
    if cfg.features.typos && !is_code_token(core, input.code_mode) && !(input.code_mode && input.lang == Lang::En) {
        if let Some((text, rule)) = fix_rule(core, input.prev, input.lang, dicts).filter(|_| !(layout_on && other_layout_word(input.keys, input.lang, dicts))) {
            return fix(ActionKind::Typo, rule, text + tail, None);
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
