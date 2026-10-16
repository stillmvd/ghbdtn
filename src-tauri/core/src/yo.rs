use crate::keymap::Lang;
use crate::replace::apply_case;
use std::collections::HashMap;
use std::sync::OnceLock;

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| include_str!("../data/ru.yo").lines().filter_map(|l| l.split_once(' ')).collect())
}

pub fn yo_fix(core: &str, lang: Lang) -> Option<String> {
    if lang != Lang::Ru || core.contains(['ё', 'Ё']) || !core.contains(['е', 'Е']) {
        return None;
    }
    table().get(core.to_lowercase().as_str()).map(|right| apply_case(core, right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mandatory_yo_only() {
        assert_eq!(yo_fix("еще", Lang::Ru).as_deref(), Some("ещё"));
        assert_eq!(yo_fix("Актер", Lang::Ru).as_deref(), Some("Актёр"));
        assert_eq!(yo_fix("ЕЛКА", Lang::Ru).as_deref(), Some("ЁЛКА"));
        for w in ["все", "чем", "небо", "звезды", "берет", "ещё", "привет"] {
            assert_eq!(yo_fix(w, Lang::Ru), None, "{w}");
        }
        assert_eq!(yo_fix("tree", Lang::En), None);
    }
}
