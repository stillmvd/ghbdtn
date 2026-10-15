use crate::keymap::Lang;
use serde::Serialize;
use std::collections::HashMap;
use zeroize::Zeroize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionKind {
    Layout,
    Typo,
    Replace,
    Manual,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub kind: ActionKind,
    pub word: String,
    pub typed: String,
    pub result: String,
    pub sep_before: String,
    pub sep_after: String,
    pub lang_before: Lang,
    pub lang_after: Lang,
}

impl Drop for Action {
    fn drop(&mut self) {
        self.word.zeroize();
        self.typed.zeroize();
        self.result.zeroize();
    }
}

#[derive(Default)]
pub struct UndoBook {
    last: Option<Action>,
    counts: HashMap<String, u8>,
}

impl UndoBook {
    pub fn record(&mut self, action: Action) {
        self.last = Some(action);
    }

    pub fn armed(&self) -> bool {
        self.last.is_some()
    }

    pub fn take(&mut self) -> Option<Action> {
        self.last.take()
    }

    pub fn close(&mut self) {
        self.last = None;
    }

    pub fn note_undo(&mut self, typed: &str) -> bool {
        let n = self.counts.entry(typed.to_lowercase()).or_insert(0);
        *n += 1;
        *n >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action() -> Action {
        Action {
            kind: ActionKind::Layout,
            word: "ghbdtn".into(),
            typed: "ghbdtn".into(),
            result: "привет".into(),
            sep_before: " ".into(),
            sep_after: " ".into(),
            lang_before: Lang::En,
            lang_after: Lang::Ru,
        }
    }

    #[test]
    fn undo_window() {
        let mut book = UndoBook::default();
        assert!(book.take().is_none());
        book.record(action());
        assert!(book.armed());
        book.close();
        assert!(book.take().is_none());
        book.record(action());
        assert_eq!(book.take().unwrap().typed, "ghbdtn");
        assert!(!book.armed());
    }

    #[test]
    fn second_undo_means_never() {
        let mut book = UndoBook::default();
        assert!(!book.note_undo("ghbdtn"));
        assert!(book.note_undo("Ghbdtn"));
    }
}
