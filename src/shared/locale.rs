// SPDX-License-Identifier: GPL-3.0-only

//! The language the texts are in: Portuguese when the locale says so,
//! English otherwise. Both the taskbar's and the Start menu's texts ask here.

/// Whether the first non-empty of `LC_ALL`, `LC_MESSAGES` and `LANG` (in that
/// order, as `values` gives them) is Portuguese.
fn is_portuguese(values: &[Option<String>]) -> bool {
    values
        .iter()
        .flatten()
        .find(|v| !v.is_empty())
        .is_some_and(|v| v.starts_with("pt"))
}

/// The languages the texts come in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Portuguese,
    French,
    English,
}

/// The language of the first non-empty of `LC_ALL`, `LC_MESSAGES` and `LANG`
/// (in that order, as `values` gives them): Portuguese, French, or else English.
fn language_of(values: &[Option<String>]) -> Language {
    match values.iter().flatten().find(|v| !v.is_empty()) {
        Some(v) if v.starts_with("pt") => Language::Portuguese,
        Some(v) if v.starts_with("fr") => Language::French,
        _ => Language::English,
    }
}

/// The session's language.
pub fn language() -> Language {
    let values: Vec<Option<String>> = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .map(|v| std::env::var(v).ok())
        .collect();
    language_of(&values)
}

/// Whether the session's language is Portuguese.
pub fn portuguese() -> bool {
    let values: Vec<Option<String>> = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .map(|v| std::env::var(v).ok())
        .collect();
    is_portuguese(&values)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some(s: &str) -> Option<String> {
        Some(s.to_owned())
    }

    #[test]
    fn the_language_is_portuguese_french_or_else_english() {
        assert_eq!(language_of(&[some("pt_BR.UTF-8"), None, None]), Language::Portuguese);
        assert_eq!(language_of(&[None, some("fr_FR.UTF-8"), some("pt_BR")]), Language::French);
        assert_eq!(language_of(&[some("de_DE.UTF-8"), None, None]), Language::English);
        assert_eq!(language_of(&[some(""), None, some("fr_CA")]), Language::French);
        assert_eq!(language_of(&[None, None, None]), Language::English);
    }

    #[test]
    fn portuguese_agrees_with_the_language() {
        let values = [some("pt_PT"), None, None];
        assert_eq!(is_portuguese(&values), language_of(&values) == Language::Portuguese);
    }

    #[test]
    fn the_first_set_variable_decides() {
        assert!(is_portuguese(&[some("pt_BR.UTF-8"), some("en_US.UTF-8"), None]));
        assert!(!is_portuguese(&[some("en_US.UTF-8"), some("pt_BR.UTF-8"), None]));
        assert!(is_portuguese(&[None, None, some("pt_PT.UTF-8")]));
    }

    #[test]
    fn empty_and_missing_variables_are_skipped() {
        assert!(is_portuguese(&[some(""), None, some("pt_BR")]));
        assert!(!is_portuguese(&[some(""), some(""), some("")]));
        assert!(!is_portuguese(&[None, None, None]));
    }

    #[test]
    fn only_a_language_that_starts_with_pt_counts() {
        assert!(!is_portuguese(&[some("C.UTF-8"), None, None]));
        assert!(!is_portuguese(&[some("es_ES.UTF-8"), None, None]));
    }
}
