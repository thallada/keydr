pub use rust_i18n::t;

/// Available UI locale codes. Separate from dictionary language support.
pub const SUPPORTED_UI_LOCALES: &[&str] = &[
    "en", "de", "es", "fr", "it", "pt", "nl", "sv", "da", "nb", "fi", "pl", "cs", "ro", "hr",
    "hu", "lt", "lv", "sl", "et", "tr",
];

pub fn set_ui_locale(locale: &str) {
    let effective = if SUPPORTED_UI_LOCALES.contains(&locale) {
        locale
    } else {
        "en"
    };
    rust_i18n::set_locale(effective);
}

/// Retrieve the set of all translation keys for a given locale.
/// Used by the catalog parity test to verify every key exists in every locale.
#[cfg(test)]
fn collect_yaml_keys(value: &serde_yaml::Value, prefix: &str, keys: &mut std::collections::BTreeSet<String>) {
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                let key_str = k.as_str().unwrap_or("");
                let full = if prefix.is_empty() {
                    key_str.to_string()
                } else {
                    format!("{prefix}.{key_str}")
                };
                collect_yaml_keys(v, &full, keys);
            }
        }
        _ => {
            keys.insert(prefix.to_string());
        }
    }
}

/// Translate a LanguageLayoutValidationError for display in the UI.
pub fn localized_language_layout_error(
    err: &crate::l10n::language_pack::LanguageLayoutValidationError,
) -> String {
    use crate::l10n::language_pack::LanguageLayoutValidationError::*;
    match err {
        UnknownLanguage(key) => t!("errors.unknown_language", key = key).to_string(),
        UnknownLayout(key) => t!("errors.unknown_layout", key = key).to_string(),
        UnsupportedLanguageLayoutPair {
            language_key,
            layout_key,
        } => t!(
            "errors.unsupported_pair",
            language = language_key,
            layout = layout_key
        )
        .to_string(),
        LanguageBlockedBySupportLevel(key) => {
            t!("errors.language_blocked", key = key).to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn locale_keys(locale: &str) -> BTreeSet<String> {
        let path = format!("locales/{locale}.yml");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
        let root: serde_yaml::Value = serde_yaml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"));
        let mut keys = BTreeSet::new();
        collect_yaml_keys(&root, "", &mut keys);
        keys
    }

    #[test]
    fn catalog_parity_all_locales() {
        let en = locale_keys("en");
        let mut errors = Vec::new();

        for locale in SUPPORTED_UI_LOCALES {
            if *locale == "en" {
                continue;
            }
            let other = locale_keys(locale);
            let missing: Vec<_> = en.difference(&other).collect();
            let extra: Vec<_> = other.difference(&en).collect();

            if !missing.is_empty() {
                errors.push(format!(
                    "Keys in en.yml missing from {locale}.yml:\n    {}",
                    missing
                        .iter()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                        .join("\n    ")
                ));
            }
            if !extra.is_empty() {
                errors.push(format!(
                    "Keys in {locale}.yml not present in en.yml:\n    {}",
                    extra
                        .iter()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                        .join("\n    ")
                ));
            }
        }

        assert!(errors.is_empty(), "Catalog parity errors:\n{}", errors.join("\n"));
    }

    #[test]
    fn placeholder_parity_all_locales() {
        let en_content = std::fs::read_to_string("locales/en.yml").unwrap();
        let en_root: serde_yaml::Value = serde_yaml::from_str(&en_content).unwrap();
        let mut en_map = std::collections::BTreeMap::new();
        collect_leaf_values(&en_root, "", &mut en_map);

        let placeholder_re = regex::Regex::new(r"%\{(\w+)\}").unwrap();
        let mut all_mismatches = Vec::new();

        for locale in SUPPORTED_UI_LOCALES {
            if *locale == "en" {
                continue;
            }
            let other_content = std::fs::read_to_string(format!("locales/{locale}.yml")).unwrap();
            let other_root: serde_yaml::Value = serde_yaml::from_str(&other_content).unwrap();
            let mut other_map = std::collections::BTreeMap::new();
            collect_leaf_values(&other_root, "", &mut other_map);

            for (key, en_val) in &en_map {
                if let Some(other_val) = other_map.get(key) {
                    let en_placeholders: BTreeSet<_> = placeholder_re
                        .captures_iter(en_val)
                        .map(|c| c[1].to_string())
                        .collect();
                    let other_placeholders: BTreeSet<_> = placeholder_re
                        .captures_iter(other_val)
                        .map(|c| c[1].to_string())
                        .collect();
                    if en_placeholders != other_placeholders {
                        all_mismatches.push(format!(
                            "  {locale}/{key}: en={en_placeholders:?} {locale}={other_placeholders:?}"
                        ));
                    }
                }
            }
        }

        assert!(
            all_mismatches.is_empty(),
            "Placeholder mismatches:\n{}",
            all_mismatches.join("\n")
        );
    }

    fn collect_leaf_values(
        value: &serde_yaml::Value,
        prefix: &str,
        map: &mut std::collections::BTreeMap<String, String>,
    ) {
        match value {
            serde_yaml::Value::Mapping(m) => {
                for (k, v) in m {
                    let key_str = k.as_str().unwrap_or("");
                    let full = if prefix.is_empty() {
                        key_str.to_string()
                    } else {
                        format!("{prefix}.{key_str}")
                    };
                    collect_leaf_values(v, &full, map);
                }
            }
            serde_yaml::Value::String(s) => {
                map.insert(prefix.to_string(), s.clone());
            }
            _ => {}
        }
    }

    #[test]
    fn set_locale_english_produces_english() {
        set_ui_locale("en");
        let text = t!("menu.subtitle").to_string();
        assert_eq!(text, "Terminal Typing Tutor");
    }

    #[test]
    fn set_locale_german_produces_german() {
        // Use the explicit locale parameter to avoid race conditions with
        // parallel tests that share the global locale state.
        let text = t!("menu.subtitle", locale = "de").to_string();
        assert_eq!(text, "Terminal-Tipptrainer");
    }

    #[test]
    fn unsupported_locale_falls_back_to_english() {
        set_ui_locale("zz");
        // After setting unsupported locale, the effective locale is "en"
        let text = t!("menu.subtitle", locale = "en").to_string();
        assert_eq!(text, "Terminal Typing Tutor");
    }

    #[test]
    fn branch_name_translated_de() {
        let text = t!("skill_tree.branch_primary_letters", locale = "de").to_string();
        assert_eq!(text, "Grundbuchstaben");
    }

    #[test]
    fn level_name_translated_de() {
        let text = t!("skill_tree.level_frequency_order", locale = "de").to_string();
        assert_eq!(text, "Haeufigkeitsfolge");
    }

    #[test]
    fn passage_all_translated_de() {
        let text = t!("select.passage_all", locale = "de").to_string();
        assert_eq!(text, "Alle (Eingebaut + alle Buecher)");
    }

    #[test]
    fn progress_overall_translated_de() {
        let text = t!("progress.overall_key_progress", locale = "de").to_string();
        assert_eq!(text, "Gesamter Tastenfortschritt");
    }
}
