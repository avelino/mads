use unicode_normalization::UnicodeNormalization;

/// Comparison form: NFC, lowercase, trimmed, internal whitespace collapsed to one space.
pub fn normalize(s: &str) -> String {
    let nfc: String = s.nfc().collect();
    nfc.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Length as Google counts it for Latin text: Unicode scalar values after NFC.
pub fn char_len(s: &str) -> usize {
    s.nfc().count()
}

/// True when the words of `needle` appear contiguously, in order, in `haystack`.
/// Both arguments are expected to be `normalize`d.
pub fn contains_word_sequence(haystack: &str, needle: &str) -> bool {
    let h: Vec<&str> = haystack.split_whitespace().collect();
    let n: Vec<&str> = needle.split_whitespace().collect();
    !n.is_empty() && h.windows(n.len()).any(|w| w == n.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lowercases_trims_and_collapses_whitespace() {
        assert_eq!(normalize("  Alamos   MALBEC \t"), "alamos malbec");
    }

    #[test]
    fn normalize_unifies_nfc_and_nfd() {
        assert_eq!(normalize("Seleção"), normalize("Selec\u{327}a\u{303}o"));
    }

    #[test]
    fn char_len_counts_scalars_after_nfc() {
        assert_eq!(char_len("Angélica"), 8);
        assert_eq!(char_len("Ange\u{301}lica"), 8);
    }

    #[test]
    fn contains_word_sequence_is_contiguous_and_word_bounded() {
        assert!(contains_word_sequence(
            "alamos malbec review",
            "malbec review"
        ));
        assert!(!contains_word_sequence(
            "alamos malbec review",
            "malbec alamos"
        ));
        assert!(!contains_word_sequence("alamosa malbec", "alamos"));
        assert!(contains_word_sequence("a b c", "a b c"));
    }
}
