use crate::keymap::{char_of, is_letter_key, Lang};
use zeroize::Zeroize;

pub const MAX_KEYS: usize = 64;
pub const SEPARATORS: &[char] = &[',', '.', '?', '!', ';', ':'];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Zeroize)]
pub struct Key {
    pub scan: u16,
    pub shift: bool,
    pub caps: bool,
}

impl Key {
    pub fn new(scan: u16, shift: bool, caps: bool) -> Key {
        Key { scan, shift, caps }
    }

    pub fn char_in(&self, lang: Lang) -> Option<char> {
        char_of(self.scan, self.shift, self.caps, lang)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Separator {
    Space,
    Enter,
    Punct(Key),
}

impl Separator {
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Separator::Space => " ".into(),
            Separator::Enter => String::new(),
            Separator::Punct(k) => k.char_in(lang).map(String::from).unwrap_or_default(),
        }
    }
}

pub fn separator_kind(key: Key, lang: Lang) -> Option<Separator> {
    if key.scan == 0x39 {
        return Some(Separator::Space);
    }
    if is_letter_key(key.scan, Lang::Ru) || is_letter_key(key.scan, Lang::En) {
        return None;
    }
    let mine = key.char_in(lang)?;
    let alt = key.char_in(lang.other());
    let sep = SEPARATORS.contains(&mine) || alt.is_some_and(|c| SEPARATORS.contains(&c));
    sep.then_some(Separator::Punct(key))
}

#[derive(Debug, Default)]
pub struct Word {
    keys: Vec<Key>,
}

impl Word {
    pub fn push(&mut self, key: Key) {
        if self.keys.len() >= MAX_KEYS {
            self.clear();
            return;
        }
        self.keys.push(key);
    }

    pub fn backspace(&mut self) {
        if let Some(mut k) = self.keys.pop() {
            k.zeroize();
        }
    }

    pub fn clear(&mut self) {
        self.keys.zeroize();
        self.keys.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn take(&mut self) -> Vec<Key> {
        std::mem::take(&mut self.keys)
    }
}

impl Drop for Word {
    fn drop(&mut self) {
        self.clear();
    }
}

pub fn render(keys: &[Key], lang: Lang) -> String {
    keys.iter().filter_map(|k| k.char_in(lang)).collect()
}

pub fn keys_for(text: &str, lang: Lang) -> Vec<Key> {
    text.chars()
        .filter_map(|c| crate::keymap::key_of(c, lang).map(|(scan, shift)| Key::new(scan, shift, false)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_both_layouts() {
        let keys = keys_for("Ghbdtn", Lang::En);
        assert_eq!(render(&keys, Lang::Ru), "Привет");
        assert_eq!(render(&keys, Lang::En), "Ghbdtn");
    }

    #[test]
    fn word_buffer() {
        let mut w = Word::default();
        for k in keys_for("abc", Lang::En) {
            w.push(k);
        }
        w.backspace();
        assert_eq!(render(w.keys(), Lang::En), "ab");
        for _ in 2..MAX_KEYS {
            w.push(Key::new(0x1E, false, false));
        }
        assert_eq!(w.keys().len(), MAX_KEYS);
        w.push(Key::new(0x1E, false, false));
        assert!(w.is_empty());
    }

    #[test]
    fn separators() {
        let k = |c, l| keys_for(c, l)[0];
        assert_eq!(separator_kind(k(" ", Lang::En), Lang::En), Some(Separator::Space));
        assert!(separator_kind(k(",", Lang::En), Lang::En).is_none());
        assert!(separator_kind(k(".", Lang::En), Lang::En).is_none());
        assert!(separator_kind(k("?", Lang::En), Lang::En).is_some());
        assert!(separator_kind(k("/", Lang::En), Lang::En).is_some());
        assert!(separator_kind(k("!", Lang::En), Lang::En).is_some());
        assert!(separator_kind(k(",", Lang::Ru), Lang::Ru).is_some());
        assert!(separator_kind(k(".", Lang::Ru), Lang::Ru).is_some());
        assert!(separator_kind(k("-", Lang::Ru), Lang::Ru).is_none());
        assert!(separator_kind(k("a", Lang::En), Lang::En).is_none());
        let q = Separator::Punct(k("?", Lang::En));
        assert_eq!(q.text(Lang::Ru), ",");
    }
}
