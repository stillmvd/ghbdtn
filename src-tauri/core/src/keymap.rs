use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    pub fn other(self) -> Lang {
        match self {
            Lang::Ru => Lang::En,
            Lang::En => Lang::Ru,
        }
    }

    pub fn langid(self) -> u16 {
        match self {
            Lang::Ru => 0x0419,
            Lang::En => 0x0409,
        }
    }

    pub fn from_langid(id: u16) -> Option<Lang> {
        match id {
            0x0419 => Some(Lang::Ru),
            0x0409 => Some(Lang::En),
            _ => None,
        }
    }

    pub fn alphabet(self) -> &'static str {
        match self {
            Lang::Ru => "абвгдеёжзийклмнопрстуфхцчшщъыьэюя",
            Lang::En => "abcdefghijklmnopqrstuvwxyz",
        }
    }

    pub fn owns_letter(self, c: char) -> bool {
        self.alphabet().contains(c.to_lowercase().next().unwrap_or(c))
    }
}

const ROWS: [(u16, &str, &str, &str, &str); 5] = [
    (0x02, "1234567890-=", "!@#$%^&*()_+", "1234567890-=", "!\"№;%:?*()_+"),
    (0x10, "qwertyuiop[]", "QWERTYUIOP{}", "йцукенгшщзхъ", "ЙЦУКЕНГШЩЗХЪ"),
    (0x1E, "asdfghjkl;'`", "ASDFGHJKL:\"~", "фывапролджэё", "ФЫВАПРОЛДЖЭЁ"),
    (0x2B, "\\zxcvbnm,./", "|ZXCVBNM<>?", "\\ячсмитьбю.", "/ЯЧСМИТЬБЮ,"),
    (0x39, " ", " ", " ", " "),
];

pub const SCAN_LIMIT: usize = 0x3A;
pub type Table = [[Option<char>; 2]; SCAN_LIMIT];

static SYSTEM: std::sync::OnceLock<[Option<Table>; 2]> = std::sync::OnceLock::new();

fn slot(lang: Lang) -> usize {
    match lang {
        Lang::Ru => 0,
        Lang::En => 1,
    }
}

pub fn scans() -> impl Iterator<Item = u16> {
    ROWS.iter().flat_map(|&(start, en, _, _, _)| start..start + en.chars().count() as u16)
}

pub fn accept_table(lang: Lang, table: &Table) -> bool {
    let letters: std::collections::HashSet<char> = table.iter().flatten().flatten().flat_map(|c| c.to_lowercase()).collect();
    lang.alphabet().chars().all(|c| letters.contains(&c)) && table[0x39][0] == Some(' ')
}

pub fn set_system(ru: Option<Table>, en: Option<Table>) -> bool {
    let pick = |lang, t: Option<Table>| t.filter(|t| accept_table(lang, t) && *t != builtin_table(lang));
    SYSTEM.set([pick(Lang::Ru, ru), pick(Lang::En, en)]).is_ok()
}

pub fn system_active(lang: Lang) -> bool {
    SYSTEM.get().is_some_and(|s| s[slot(lang)].is_some())
}

pub fn builtin_table(lang: Lang) -> Table {
    let mut t: Table = [[None; 2]; SCAN_LIMIT];
    for scan in scans() {
        for shift in [false, true] {
            t[scan as usize][shift as usize] = builtin(scan, shift, lang);
        }
    }
    t
}

fn lookup(scan: u16, shift: bool, lang: Lang) -> Option<char> {
    system_cell(scan, shift, lang).or_else(|| builtin(scan, shift, lang))
}

fn system_cell(scan: u16, shift: bool, lang: Lang) -> Option<char> {
    SYSTEM.get().and_then(|s| s[slot(lang)].as_ref()).and_then(|t| t.get(scan as usize)).and_then(|r| r[shift as usize])
}

fn builtin(scan: u16, shift: bool, lang: Lang) -> Option<char> {
    ROWS.iter().find_map(|&(start, en, en_s, ru, ru_s)| {
        let row = match (lang, shift) {
            (Lang::En, false) => en,
            (Lang::En, true) => en_s,
            (Lang::Ru, false) => ru,
            (Lang::Ru, true) => ru_s,
        };
        let i = scan.checked_sub(start)? as usize;
        row.chars().nth(i)
    })
}

pub fn char_of(scan: u16, shift: bool, caps: bool, lang: Lang) -> Option<char> {
    let base = lookup(scan, false, lang)?;
    let flip = caps && lang.owns_letter(base);
    lookup(scan, shift != flip, lang)
}

pub fn key_of(c: char, lang: Lang) -> Option<(u16, bool)> {
    if system_active(lang) {
        let found = [false, true].into_iter().find_map(|shift| scans().find(|&s| system_cell(s, shift, lang) == Some(c)).map(|s| (s, shift)));
        return found.or_else(|| builtin_key(c, lang).filter(|&(s, shift)| system_cell(s, shift, lang).is_none()));
    }
    builtin_key(c, lang)
}

fn builtin_key(c: char, lang: Lang) -> Option<(u16, bool)> {
    ROWS.iter().find_map(|&(start, en, en_s, ru, ru_s)| {
        let (plain, shifted) = match lang {
            Lang::En => (en, en_s),
            Lang::Ru => (ru, ru_s),
        };
        if let Some(i) = plain.chars().position(|x| x == c) {
            return Some((start + i as u16, false));
        }
        shifted.chars().position(|x| x == c).map(|i| (start + i as u16, true))
    })
}

pub fn is_letter_key(scan: u16, lang: Lang) -> bool {
    lookup(scan, false, lang).is_some_and(|c| lang.owns_letter(c))
}

pub fn script_of(text: &str) -> Option<Lang> {
    let (mut ru, mut en) = (0, 0);
    for c in text.chars() {
        if Lang::Ru.owns_letter(c) {
            ru += 1;
        } else if Lang::En.owns_letter(c) {
            en += 1;
        }
    }
    match (ru, en) {
        (0, 0) => None,
        _ if ru >= en => Some(Lang::Ru),
        _ => Some(Lang::En),
    }
}

pub fn convert_text(text: &str, from: Lang) -> String {
    let to = from.other();
    text.chars()
        .map(|c| match key_of(c, from) {
            Some((scan, shift)) if c != ' ' => lookup(scan, shift, to).unwrap_or(c),
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_words() {
        assert_eq!(convert_text("ghbdtn", Lang::En), "привет");
        assert_eq!(convert_text("сщву", Lang::Ru), "code");
        assert_eq!(convert_text(",tp", Lang::En), "без");
        assert_eq!(convert_text("Ghbdtn vbh", Lang::En), "Привет мир");
        assert_eq!(convert_text("Привет мир", Lang::Ru), "Ghbdtn vbh");
        assert_eq!(convert_text("`krf", Lang::En), "ёлка");
    }

    #[test]
    fn caps_only_flips_letters() {
        assert_eq!(char_of(0x22, false, true, Lang::En), Some('G'));
        assert_eq!(char_of(0x22, true, true, Lang::En), Some('g'));
        assert_eq!(char_of(0x33, false, true, Lang::En), Some(','));
        assert_eq!(char_of(0x33, false, true, Lang::Ru), Some('Б'));
        assert_eq!(char_of(0x02, false, true, Lang::Ru), Some('1'));
    }

    #[test]
    fn letter_keys() {
        assert!(is_letter_key(0x33, Lang::Ru));
        assert!(!is_letter_key(0x33, Lang::En));
        assert!(!is_letter_key(0x35, Lang::Ru));
        assert_eq!(script_of("Ghbdtn, мир"), Some(Lang::En));
        assert_eq!(script_of("123"), None);
    }
}
