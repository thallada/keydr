#![allow(dead_code)] // TODO(phase 1+): remove when all language-pack fields are consumed by runtime/UI.

use std::fmt;
use std::sync::OnceLock;

use crate::keyboard::model::KeyboardModel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Latin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupportLevel {
    Full,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityState {
    Enabled,
    // Reserved for selector UIs that show but disable unsupported entries.
    // Validation APIs still return typed errors for disabled combinations.
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanguageLayoutValidationError {
    UnknownLanguage(String),
    UnknownLayout(String),
    UnsupportedLanguageLayoutPair {
        language_key: String,
        layout_key: String,
    },
    LanguageBlockedBySupportLevel(String),
}

impl fmt::Display for LanguageLayoutValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLanguage(key) => write!(f, "Unknown language: {key}"),
            Self::UnknownLayout(key) => write!(f, "Unknown keyboard layout: {key}"),
            Self::UnsupportedLanguageLayoutPair {
                language_key,
                layout_key,
            } => write!(
                f,
                "Unsupported language/layout pair: {language_key} + {layout_key}"
            ),
            Self::LanguageBlockedBySupportLevel(key) => {
                write!(f, "Language is blocked by support level: {key}")
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RankedReadinessError {
    InvalidLanguageLayout(LanguageLayoutValidationError),
    MissingPrimaryLetterSequence(String),
}

impl fmt::Display for RankedReadinessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLanguageLayout(err) => write!(f, "{err}"),
            Self::MissingPrimaryLetterSequence(language_key) => {
                write!(
                    f,
                    "Language '{language_key}' has no usable primary letter sequence"
                )
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LanguagePack {
    pub language_key: &'static str,
    pub display_name: &'static str,
    pub autonym: &'static str,
    pub script: Script,
    pub dictionary_asset_id: &'static str,
    pub supported_keyboard_layout_keys: &'static [&'static str],
    pub primary_letter_sequence: &'static str,
    pub support_level: SupportLevel,
}

pub const DEFAULT_LATIN_PRIMARY_SEQUENCE: &str = "etaoinshrdlcumwfgypbvkjxqz";
const DE_PRIMARY_SEQUENCE: &str = "entrishlagcubdmfokwzüpävößjqxy";
const ES_PRIMARY_SEQUENCE: &str = "aerosintcdlmupbgvófhíjáézqyxñú";
const FR_PRIMARY_SEQUENCE: &str = "erisantoucélpmdvgfbhqzxèyjç";
const IT_PRIMARY_SEQUENCE: &str = "aieortnsclmpdugvfbzhqkyxjw";
const PT_PRIMARY_SEQUENCE: &str = "aeorsitncdmulpvgbfhçãáqíxzjéóõêúâôà";
const NL_PRIMARY_SEQUENCE: &str = "enratiosldgkvuhpmbcjwfzyxq";
const SV_PRIMARY_SEQUENCE: &str = "aertnsldkigoämvbfuöphåyjcxw";
const DA_PRIMARY_SEQUENCE: &str = "ertnsildagokmfvubpæhøyjåcwzxq";
const NB_PRIMARY_SEQUENCE: &str = "ertnsilakogdmpvfubjøyhåæcw";
const FI_PRIMARY_SEQUENCE: &str = "aitneslkuäomvrphyjdögfbcwxzq";
const PL_PRIMARY_SEQUENCE: &str = "aiezornwsycpdkmtułjlbęgćąśhóżfńź";
const CS_PRIMARY_SEQUENCE: &str = "oelantipvdsurmkhíázcěbyřjčýšéžůúfťgňďxó";
const RO_PRIMARY_SEQUENCE: &str = "eiartnuclosăpmdgvbzfîâhjțșx";
const HR_PRIMARY_SEQUENCE: &str = "aitoernspjlkuvdmzbgcčšžćhfđ";
const HU_PRIMARY_SEQUENCE: &str = "etalnskriozáémgdvbyjhpuföóőícüúűwxq";
const LT_PRIMARY_SEQUENCE: &str = "iasteuknrolmpdvgėjyšbžąųįūčęzcfh";
const LV_PRIMARY_SEQUENCE: &str = "asiternlkopmuādīvzēgjbcšfūņļķģžhč";
const SL_PRIMARY_SEQUENCE: &str = "aeiotnrsvpkldjzmučbgcšžhf";
const ET_PRIMARY_SEQUENCE: &str = "aeistulmnkrovpdhgäjõüböfš";
const TR_PRIMARY_SEQUENCE: &str = "aeinrlımkdysutobşzügğcçöhpvfj";

const EN_LAYOUTS: &[&str] = &["qwerty", "dvorak", "colemak"];
const DE_LAYOUTS: &[&str] = &["de_qwertz", "qwerty"];
const FR_LAYOUTS: &[&str] = &["fr_azerty", "qwerty"];
const ES_LAYOUTS: &[&str] = &["es_intl", "qwerty"];
const IT_LAYOUTS: &[&str] = &["it_intl", "qwerty"];
const PT_LAYOUTS: &[&str] = &["pt_intl", "qwerty"];
const NL_LAYOUTS: &[&str] = &["nl_intl", "qwerty"];
const SV_LAYOUTS: &[&str] = &["sv_intl", "qwerty"];
const DA_LAYOUTS: &[&str] = &["da_intl", "qwerty"];
const NB_LAYOUTS: &[&str] = &["nb_intl", "qwerty"];
const FI_LAYOUTS: &[&str] = &["fi_intl", "qwerty"];
const PL_LAYOUTS: &[&str] = &["pl_intl", "qwerty"];
const CS_LAYOUTS: &[&str] = &["cs_intl", "qwerty"];
const RO_LAYOUTS: &[&str] = &["ro_intl", "qwerty"];
const HR_LAYOUTS: &[&str] = &["hr_intl", "qwerty"];
const HU_LAYOUTS: &[&str] = &["hu_intl", "qwerty"];
const LT_LAYOUTS: &[&str] = &["lt_intl", "qwerty"];
const LV_LAYOUTS: &[&str] = &["lv_intl", "qwerty"];
const SL_LAYOUTS: &[&str] = &["sl_intl", "qwerty"];
const ET_LAYOUTS: &[&str] = &["et_intl", "qwerty"];
const TR_LAYOUTS: &[&str] = &["tr_intl", "qwerty"];

// Seed registry for phase 0. Support levels will be tightened as keyboard
// profiles and Unicode handling phases are implemented.
static LANGUAGE_PACKS: &[LanguagePack] = &[
    LanguagePack {
        language_key: "en",
        display_name: "English",
        autonym: "English",
        script: Script::Latin,
        dictionary_asset_id: "words-en",
        supported_keyboard_layout_keys: EN_LAYOUTS,
        primary_letter_sequence: DEFAULT_LATIN_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "de",
        display_name: "German",
        autonym: "Deutsch",
        script: Script::Latin,
        dictionary_asset_id: "words-de",
        supported_keyboard_layout_keys: DE_LAYOUTS,
        primary_letter_sequence: DE_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "es",
        display_name: "Spanish",
        autonym: "Español",
        script: Script::Latin,
        dictionary_asset_id: "words-es",
        supported_keyboard_layout_keys: ES_LAYOUTS,
        primary_letter_sequence: ES_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "fr",
        display_name: "French",
        autonym: "Français",
        script: Script::Latin,
        dictionary_asset_id: "words-fr",
        supported_keyboard_layout_keys: FR_LAYOUTS,
        primary_letter_sequence: FR_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "it",
        display_name: "Italian",
        autonym: "Italiano",
        script: Script::Latin,
        dictionary_asset_id: "words-it",
        supported_keyboard_layout_keys: IT_LAYOUTS,
        primary_letter_sequence: IT_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "pt",
        display_name: "Portuguese",
        autonym: "Português",
        script: Script::Latin,
        dictionary_asset_id: "words-pt",
        supported_keyboard_layout_keys: PT_LAYOUTS,
        primary_letter_sequence: PT_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "nl",
        display_name: "Dutch",
        autonym: "Nederlands",
        script: Script::Latin,
        dictionary_asset_id: "words-nl",
        supported_keyboard_layout_keys: NL_LAYOUTS,
        primary_letter_sequence: NL_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "sv",
        display_name: "Swedish",
        autonym: "Svenska",
        script: Script::Latin,
        dictionary_asset_id: "words-sv",
        supported_keyboard_layout_keys: SV_LAYOUTS,
        primary_letter_sequence: SV_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "da",
        display_name: "Danish",
        autonym: "Dansk",
        script: Script::Latin,
        dictionary_asset_id: "words-da",
        supported_keyboard_layout_keys: DA_LAYOUTS,
        primary_letter_sequence: DA_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "nb",
        display_name: "Norwegian Bokmal",
        autonym: "Norsk bokmål",
        script: Script::Latin,
        dictionary_asset_id: "words-nb",
        supported_keyboard_layout_keys: NB_LAYOUTS,
        primary_letter_sequence: NB_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "fi",
        display_name: "Finnish",
        autonym: "Suomi",
        script: Script::Latin,
        dictionary_asset_id: "words-fi",
        supported_keyboard_layout_keys: FI_LAYOUTS,
        primary_letter_sequence: FI_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "pl",
        display_name: "Polish",
        autonym: "Polski",
        script: Script::Latin,
        dictionary_asset_id: "words-pl",
        supported_keyboard_layout_keys: PL_LAYOUTS,
        primary_letter_sequence: PL_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "cs",
        display_name: "Czech",
        autonym: "Čeština",
        script: Script::Latin,
        dictionary_asset_id: "words-cs",
        supported_keyboard_layout_keys: CS_LAYOUTS,
        primary_letter_sequence: CS_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "ro",
        display_name: "Romanian",
        autonym: "Română",
        script: Script::Latin,
        dictionary_asset_id: "words-ro",
        supported_keyboard_layout_keys: RO_LAYOUTS,
        primary_letter_sequence: RO_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "hr",
        display_name: "Croatian",
        autonym: "Hrvatski",
        script: Script::Latin,
        dictionary_asset_id: "words-hr",
        supported_keyboard_layout_keys: HR_LAYOUTS,
        primary_letter_sequence: HR_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "hu",
        display_name: "Hungarian",
        autonym: "Magyar",
        script: Script::Latin,
        dictionary_asset_id: "words-hu",
        supported_keyboard_layout_keys: HU_LAYOUTS,
        primary_letter_sequence: HU_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "lt",
        display_name: "Lithuanian",
        autonym: "Lietuvių",
        script: Script::Latin,
        dictionary_asset_id: "words-lt",
        supported_keyboard_layout_keys: LT_LAYOUTS,
        primary_letter_sequence: LT_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "lv",
        display_name: "Latvian",
        autonym: "Latviešu",
        script: Script::Latin,
        dictionary_asset_id: "words-lv",
        supported_keyboard_layout_keys: LV_LAYOUTS,
        primary_letter_sequence: LV_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "sl",
        display_name: "Slovene",
        autonym: "Slovenščina",
        script: Script::Latin,
        dictionary_asset_id: "words-sl",
        supported_keyboard_layout_keys: SL_LAYOUTS,
        primary_letter_sequence: SL_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "et",
        display_name: "Estonian",
        autonym: "Eesti",
        script: Script::Latin,
        dictionary_asset_id: "words-et",
        supported_keyboard_layout_keys: ET_LAYOUTS,
        primary_letter_sequence: ET_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
    LanguagePack {
        language_key: "tr",
        display_name: "Turkish",
        autonym: "Türkçe",
        script: Script::Latin,
        dictionary_asset_id: "words-tr",
        supported_keyboard_layout_keys: TR_LAYOUTS,
        primary_letter_sequence: TR_PRIMARY_SEQUENCE,
        support_level: SupportLevel::Full,
    },
];

pub fn language_packs() -> &'static [LanguagePack] {
    LANGUAGE_PACKS
}

pub fn find_language_pack(language_key: &str) -> Option<&'static LanguagePack> {
    LANGUAGE_PACKS
        .iter()
        .find(|pack| pack.language_key == language_key)
}

pub fn supported_dictionary_languages() -> &'static [&'static str] {
    static SUPPORTED: OnceLock<Vec<&'static str>> = OnceLock::new();
    SUPPORTED
        .get_or_init(|| {
            LANGUAGE_PACKS
                .iter()
                .filter(|pack| matches!(pack.support_level, SupportLevel::Full))
                .map(|pack| pack.language_key)
                .collect()
        })
        .as_slice()
}

pub fn dictionary_languages_for_layout(layout_key: &str) -> Vec<&'static str> {
    LANGUAGE_PACKS
        .iter()
        .filter_map(
            |pack| match validate_language_layout_pair(pack.language_key, layout_key) {
                Ok(CapabilityState::Enabled) => Some(pack.language_key),
                _ => None,
            },
        )
        .collect()
}

pub fn default_keyboard_layout_for_language(language_key: &str) -> Option<&'static str> {
    let pack = find_language_pack(language_key)?;
    pack.supported_keyboard_layout_keys.first().copied()
}

pub fn validate_language_layout_pair(
    language_key: &str,
    layout_key: &str,
) -> Result<CapabilityState, LanguageLayoutValidationError> {
    let Some(pack) = find_language_pack(language_key) else {
        return Err(LanguageLayoutValidationError::UnknownLanguage(
            language_key.to_string(),
        ));
    };

    if !KeyboardModel::supported_layout_keys().contains(&layout_key) {
        return Err(LanguageLayoutValidationError::UnknownLayout(
            layout_key.to_string(),
        ));
    }

    if matches!(pack.support_level, SupportLevel::Blocked) {
        return Err(
            LanguageLayoutValidationError::LanguageBlockedBySupportLevel(language_key.to_string()),
        );
    }

    Ok(CapabilityState::Enabled)
}

pub fn normalized_primary_letter_sequence(sequence: &str) -> Vec<char> {
    let mut out = Vec::new();
    for ch in sequence.chars().filter(|ch| ch.is_alphabetic()) {
        if !out.contains(&ch) {
            out.push(ch);
        }
    }
    out
}

pub fn has_usable_primary_letter_sequence(sequence: &str) -> bool {
    !normalized_primary_letter_sequence(sequence).is_empty()
}

pub fn ranked_adaptive_readiness(
    language_key: &str,
    layout_key: &str,
) -> Result<(), RankedReadinessError> {
    validate_language_layout_pair(language_key, layout_key)
        .map_err(RankedReadinessError::InvalidLanguageLayout)?;
    let Some(pack) = find_language_pack(language_key) else {
        return Err(RankedReadinessError::InvalidLanguageLayout(
            LanguageLayoutValidationError::UnknownLanguage(language_key.to_string()),
        ));
    };
    if !has_usable_primary_letter_sequence(pack.primary_letter_sequence) {
        return Err(RankedReadinessError::MissingPrimaryLetterSequence(
            language_key.to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn enabled_pairs() -> Vec<(&'static str, &'static str)> {
        let mut pairs = Vec::new();
        for pack in language_packs() {
            for &layout_key in KeyboardModel::supported_layout_keys() {
                if matches!(
                    validate_language_layout_pair(pack.language_key, layout_key),
                    Ok(CapabilityState::Enabled)
                ) {
                    pairs.push((pack.language_key, layout_key));
                }
            }
        }
        pairs
    }

    #[test]
    fn language_pack_keys_are_unique() {
        let mut seen = HashSet::new();
        for pack in language_packs() {
            assert!(seen.insert(pack.language_key));
            assert!(pack.primary_letter_sequence.len() >= 10);
            assert!(!pack.dictionary_asset_id.is_empty());
            assert!(!pack.supported_keyboard_layout_keys.is_empty());
            assert!(matches!(pack.script, Script::Latin));
        }
    }

    #[test]
    fn english_pack_exists_and_is_full() {
        let en = find_language_pack("en").expect("missing en language pack");
        assert_eq!(en.support_level, SupportLevel::Full);
        assert_eq!(en.primary_letter_sequence, DEFAULT_LATIN_PRIMARY_SEQUENCE);
        assert!(en.primary_letter_sequence.starts_with("etaoin"));
    }

    #[test]
    fn german_pack_primary_sequence_contains_locale_letters() {
        let de = find_language_pack("de").expect("missing de language pack");
        assert!(de.primary_letter_sequence.contains('ä'));
        assert!(de.primary_letter_sequence.contains('ö'));
        assert!(de.primary_letter_sequence.contains('ü'));
        assert!(de.primary_letter_sequence.contains('ß'));
    }

    #[test]
    fn non_english_packs_have_language_specific_primary_sequences() {
        for pack in language_packs() {
            if pack.language_key == "en" {
                continue;
            }
            assert_ne!(
                pack.primary_letter_sequence, DEFAULT_LATIN_PRIMARY_SEQUENCE,
                "language {} should not reuse default English sequence",
                pack.language_key
            );
        }
    }

    #[test]
    fn locale_letters_are_typeable_on_language_native_layouts() {
        for pack in language_packs() {
            if pack.language_key == "en" {
                continue;
            }
            let native_layout_key = match pack.language_key {
                "de" => "de_qwertz".to_string(),
                "fr" => "fr_azerty".to_string(),
                key => format!("{key}_intl"),
            };

            let model = KeyboardModel::from_key(&native_layout_key)
                .expect("native layout key should map to a keyboard model");
            for ch in normalized_primary_letter_sequence(pack.primary_letter_sequence) {
                if ch.is_ascii_lowercase() {
                    continue;
                }
                assert!(
                    model.physical_key_for(ch).is_some(),
                    "native layout {} should type locale letter '{}' for language {}",
                    native_layout_key,
                    ch,
                    pack.language_key
                );
            }
        }
    }

    #[test]
    fn supported_dictionary_languages_are_registry_backed() {
        for key in supported_dictionary_languages() {
            assert!(find_language_pack(key).is_some());
        }
    }

    #[test]
    fn supported_dictionary_languages_include_non_english_languages() {
        let supported = supported_dictionary_languages();
        assert!(supported.contains(&"en"));
        assert!(supported.contains(&"de"));
        assert!(supported.contains(&"es"));
    }

    #[test]
    fn validate_language_layout_pair_unknown_language() {
        let err = validate_language_layout_pair("zz", "qwerty").unwrap_err();
        assert!(matches!(
            err,
            LanguageLayoutValidationError::UnknownLanguage(_)
        ));
    }

    #[test]
    fn validate_language_layout_pair_unknown_layout() {
        let err = validate_language_layout_pair("en", "foo").unwrap_err();
        assert!(matches!(
            err,
            LanguageLayoutValidationError::UnknownLayout(_)
        ));
    }

    #[test]
    fn validate_language_layout_pair_allows_cross_language_layout_pair() {
        let state = validate_language_layout_pair("en", "de_qwertz")
            .expect("cross-language/layout pair should be allowed");
        assert_eq!(state, CapabilityState::Enabled);
    }

    #[test]
    fn dictionary_languages_for_layout_qwerty_contains_english() {
        let keys = dictionary_languages_for_layout("qwerty");
        assert!(keys.contains(&"en"));
    }

    #[test]
    fn dictionary_languages_for_layout_contains_full_language_set_for_supported_layouts() {
        let de = dictionary_languages_for_layout("de_qwertz");
        assert_eq!(de.len(), supported_dictionary_languages().len());
        assert!(de.contains(&"de"));

        let fr = dictionary_languages_for_layout("fr_azerty");
        assert_eq!(fr.len(), supported_dictionary_languages().len());
        assert!(fr.contains(&"fr"));
    }

    #[test]
    fn normalized_primary_sequence_filters_non_letters_and_dedupes() {
        assert_eq!(
            normalized_primary_letter_sequence("a1áa!bB"),
            vec!['a', 'á', 'b', 'B']
        );
    }

    #[test]
    fn usable_primary_sequence_requires_at_least_one_letter() {
        assert!(!has_usable_primary_letter_sequence("12345!?"));
        assert!(has_usable_primary_letter_sequence("é"));
    }

    #[test]
    fn ranked_adaptive_readiness_rejects_invalid_layout() {
        let err = ranked_adaptive_readiness("en", "not_a_layout").unwrap_err();
        assert!(matches!(
            err,
            RankedReadinessError::InvalidLanguageLayout(
                LanguageLayoutValidationError::UnknownLayout(_)
            )
        ));
    }

    #[test]
    fn ranked_adaptive_readiness_accepts_all_enabled_pairs() {
        for (language_key, layout_key) in enabled_pairs() {
            assert!(
                ranked_adaptive_readiness(language_key, layout_key).is_ok(),
                "expected readiness for pair: {language_key}+{layout_key}"
            );
        }
    }
}
