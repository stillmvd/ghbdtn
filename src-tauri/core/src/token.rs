const CODE_CHARS: &[char] = &['_', '.', '/', '\\', '@', '#'];
const CODE_MODE_START: &[char] = &['-', '~', '$', '%'];
const CODE_MODE_CHARS: &[char] = &['=', ':', '*', '|', '<', '>'];

pub fn is_code_token(word: &str, code_mode: bool) -> bool {
    if word.chars().any(|c| c.is_ascii_digit() || CODE_CHARS.contains(&c)) || word.contains("::") {
        return true;
    }
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase()) {
        return true;
    }
    if word.chars().zip(word.chars().skip(1)).any(|(a, b)| a.is_lowercase() && b.is_uppercase()) {
        return true;
    }
    code_mode
        && (word.starts_with(CODE_MODE_START) || word.chars().any(|c| CODE_MODE_CHARS.contains(&c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_tokens() {
        for w in ["getUserName", "http://ya.ru", "v2rayN", "NASA", "snake_case", "a@b.ru", "#tag", "std::io", "ya.ru", "GetX"] {
            assert!(is_code_token(w, false), "{w}");
        }
        for w in ["привет", "Привет", "hello", "чё-то", "I", "Ghbdtn", "--watch"] {
            assert!(!is_code_token(w, false), "{w}");
        }
        for w in ["--watch", "~", "$HOME", "%PATH%", "a=b", "http:", "*.rs", "|"] {
            assert!(is_code_token(w, true), "{w}");
        }
        assert!(!is_code_token("привет", true));
    }
}
