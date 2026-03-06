use icu_normalizer::ComposingNormalizerBorrowed;

pub fn normalize_nfc(input: &str) -> String {
    ComposingNormalizerBorrowed::new_nfc()
        .normalize(input)
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_nfc_composes_equivalent_unicode_sequences() {
        let composed = "é";
        let decomposed = "e\u{0301}";
        assert_eq!(normalize_nfc(composed), normalize_nfc(decomposed));
    }

    #[test]
    fn normalize_nfc_is_stable_for_precomposed_and_ascii() {
        assert_eq!(normalize_nfc("Árvíztűrő"), "Árvíztűrő");
        assert_eq!(normalize_nfc("abc"), "abc");
    }
}
