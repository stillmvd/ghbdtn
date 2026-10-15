use crate::keymap::Lang;

fn word_of(line: &str) -> Option<String> {
    let w = line.split('\t').next().unwrap_or("").trim().to_lowercase();
    (!w.is_empty() && !w.starts_with('#')).then_some(w)
}

pub fn list(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    text.lines().filter_map(word_of).filter(|w| seen.insert(w.clone())).collect()
}

pub fn normalize(word: &str) -> Result<String, &'static str> {
    let w = word.trim().to_lowercase();
    if w.chars().count() < 2 {
        return Err("слово короче двух букв");
    }
    let ru = w.chars().all(|c| Lang::Ru.owns_letter(c) || c == '-');
    let en = w.chars().all(|c| c.is_ascii_alphabetic() || c == '\'');
    if !(ru || en) || w.chars().filter(|c| c.is_alphabetic()).count() < 2 {
        return Err("только русские или только английские буквы");
    }
    Ok(w)
}

pub fn add(text: &str, word: &str) -> String {
    let mut out = text.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(word);
    out.push('\n');
    out
}

pub fn remove(text: &str, word: &str) -> String {
    text.lines().filter(|l| word_of(l).as_deref() != Some(word)).map(|l| format!("{l}\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_keep_comments() {
        let text = "# шапка\nвиджет\t12\nstillmvd\n";
        assert_eq!(list(text), ["виджет", "stillmvd"]);
        assert_eq!(list(&add(text, "дебаунс")), ["виджет", "stillmvd", "дебаунс"]);
        assert_eq!(remove(text, "виджет"), "# шапка\nstillmvd\n");
        assert_eq!(add("ревью", "проп"), "ревью\nпроп\n");
    }

    #[test]
    fn validates_like_dictionary() {
        assert_eq!(normalize(" Дебаунс "), Ok("дебаунс".into()));
        assert_eq!(normalize("юнит-тесты"), Ok("юнит-тесты".into()));
        assert_eq!(normalize("don't"), Ok("don't".into()));
        assert!(normalize("я").is_err());
        assert!(normalize("юнит-tests").is_err());
        assert!(normalize("два слова").is_err());
        assert!(normalize("--").is_err());
        assert!(normalize("''").is_err());
        assert_eq!(list("ревью\nРевью\n"), ["ревью"]);
    }
}
