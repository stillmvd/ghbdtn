pub fn apply_case(typed: &str, result: &str) -> String {
    let letters: Vec<char> = typed.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase()) {
        return result.to_uppercase();
    }
    let mut out = result.to_string();
    if typed.chars().next().is_some_and(char::is_uppercase) {
        let mut chars = out.chars();
        if let Some(first) = chars.next() {
            out = first.to_uppercase().chain(chars).collect();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_transfer() {
        assert_eq!(apply_case("чето", "чё-то"), "чё-то");
        assert_eq!(apply_case("Чето", "чё-то"), "Чё-то");
        assert_eq!(apply_case("ЧЕТО", "чё-то"), "ЧЁ-ТО");
        assert_eq!(apply_case("спс", "спасибо большое"), "спасибо большое");
    }
}
