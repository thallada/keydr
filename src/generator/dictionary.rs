use crate::engine::filter::CharFilter;
use crate::l10n::unicode::normalize_nfc;

const WORDS_CS: &str = include_str!("../../assets/dictionaries/words-cs.json");
const WORDS_DA: &str = include_str!("../../assets/dictionaries/words-da.json");
const WORDS_DE: &str = include_str!("../../assets/dictionaries/words-de.json");
const WORDS_EN: &str = include_str!("../../assets/dictionaries/words-en.json");
const WORDS_ES: &str = include_str!("../../assets/dictionaries/words-es.json");
const WORDS_ET: &str = include_str!("../../assets/dictionaries/words-et.json");
const WORDS_FI: &str = include_str!("../../assets/dictionaries/words-fi.json");
const WORDS_FR: &str = include_str!("../../assets/dictionaries/words-fr.json");
const WORDS_HR: &str = include_str!("../../assets/dictionaries/words-hr.json");
const WORDS_HU: &str = include_str!("../../assets/dictionaries/words-hu.json");
const WORDS_IT: &str = include_str!("../../assets/dictionaries/words-it.json");
const WORDS_LT: &str = include_str!("../../assets/dictionaries/words-lt.json");
const WORDS_LV: &str = include_str!("../../assets/dictionaries/words-lv.json");
const WORDS_NB: &str = include_str!("../../assets/dictionaries/words-nb.json");
const WORDS_NL: &str = include_str!("../../assets/dictionaries/words-nl.json");
const WORDS_PL: &str = include_str!("../../assets/dictionaries/words-pl.json");
const WORDS_PT: &str = include_str!("../../assets/dictionaries/words-pt.json");
const WORDS_RO: &str = include_str!("../../assets/dictionaries/words-ro.json");
const WORDS_SL: &str = include_str!("../../assets/dictionaries/words-sl.json");
const WORDS_SV: &str = include_str!("../../assets/dictionaries/words-sv.json");
const WORDS_TR: &str = include_str!("../../assets/dictionaries/words-tr.json");
#[derive(Clone, Debug)]
pub struct Dictionary {
    words: Vec<String>,
}

impl Dictionary {
    fn raw_for_language(language_key: &str) -> Option<&'static str> {
        match language_key {
            "cs" => Some(WORDS_CS),
            "da" => Some(WORDS_DA),
            "de" => Some(WORDS_DE),
            "en" => Some(WORDS_EN),
            "es" => Some(WORDS_ES),
            "et" => Some(WORDS_ET),
            "fi" => Some(WORDS_FI),
            "fr" => Some(WORDS_FR),
            "hr" => Some(WORDS_HR),
            "hu" => Some(WORDS_HU),
            "it" => Some(WORDS_IT),
            "lt" => Some(WORDS_LT),
            "lv" => Some(WORDS_LV),
            "nb" => Some(WORDS_NB),
            "nl" => Some(WORDS_NL),
            "pl" => Some(WORDS_PL),
            "pt" => Some(WORDS_PT),
            "ro" => Some(WORDS_RO),
            "sl" => Some(WORDS_SL),
            "sv" => Some(WORDS_SV),
            "tr" => Some(WORDS_TR),
            _ => None,
        }
    }

    pub fn supports_language(language_key: &str) -> bool {
        Self::raw_for_language(language_key).is_some()
    }

    pub fn try_load_for_language(language_key: &str) -> Option<Self> {
        let raw = Self::raw_for_language(language_key)?;
        let words: Vec<String> = serde_json::from_str(raw).unwrap_or_default();

        // Filter to words of length >= 3 and normalize to NFC for consistent
        // matching across composed/decomposed forms.
        let words = words
            .into_iter()
            .map(|w| normalize_nfc(&w))
            .filter(|w| w.chars().count() >= 3)
            .filter(|w| !w.chars().any(|c| c.is_whitespace()))
            .collect::<Vec<String>>();

        Some(Self { words })
    }

    pub fn load_for_language(language_key: &str) -> Self {
        Self::try_load_for_language(language_key)
            .unwrap_or_else(|| panic!("unsupported dictionary language: {language_key}"))
    }

    pub fn words_list(&self) -> &[String] {
        &self.words
    }

    pub fn find_matching(&self, filter: &CharFilter, focused: Option<char>) -> Vec<&str> {
        let mut matching: Vec<&str> = self
            .words
            .iter()
            .filter(|w| w.chars().all(|c| filter.is_allowed(c)))
            .map(|s| s.as_str())
            .collect();

        // If there's a focused letter, prioritize words containing it
        if let Some(focus) = focused {
            matching.sort_by_key(|w| if w.contains(focus) { 0 } else { 1 });
        }

        matching
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::l10n::language_pack::{language_packs, supported_dictionary_languages};

    #[test]
    #[should_panic(expected = "unsupported dictionary language")]
    fn load_for_language_unknown_panics() {
        let _ = Dictionary::load_for_language("zz");
    }

    #[test]
    fn find_matching_focused_is_sort_only() {
        let dictionary = Dictionary::load_for_language("en");
        let filter = CharFilter::new(('a'..='z').collect());

        let without_focus = dictionary.find_matching(&filter, None);
        let with_focus = dictionary.find_matching(&filter, Some('k'));

        // Same membership — focused param only reorders, never filters
        let mut sorted_without: Vec<&str> = without_focus.clone();
        let mut sorted_with: Vec<&str> = with_focus.clone();
        sorted_without.sort();
        sorted_with.sort();

        assert_eq!(sorted_without, sorted_with);
        assert_eq!(without_focus.len(), with_focus.len());
    }

    #[test]
    fn non_english_dictionaries_load_substantial_word_lists() {
        for &lang in supported_dictionary_languages() {
            if lang == "en" {
                continue;
            }
            let dictionary = Dictionary::load_for_language(lang);
            assert!(
                dictionary.words_list().len() > 100,
                "expected substantial dictionary for language {lang}"
            );
        }
    }

    #[test]
    fn all_registered_language_packs_have_embedded_dictionary_assets() {
        for pack in language_packs() {
            assert!(
                Dictionary::supports_language(pack.language_key),
                "language pack {} is missing an embedded dictionary asset",
                pack.language_key
            );
            assert!(
                Dictionary::try_load_for_language(pack.language_key).is_some(),
                "dictionary load failed for language pack {}",
                pack.language_key
            );
        }
    }
}
