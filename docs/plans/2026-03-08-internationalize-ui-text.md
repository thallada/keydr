# Plan: Internationalize UI Text

## Context

keydr supports 21 languages for dictionaries and keyboard layouts, but all UI text is hardcoded English (~200 strings across inline literals, `format!()` templates, `const` arrays, `Display` impls, and prebuilt state like milestone messages). This plan translates all app-owned UI copy via a separate "UI Language" config setting. Source texts from code/passage drills remain untranslated. Nested error details from system/library errors (e.g. IO errors, serde errors) embedded in status messages remain in their original form — only the app-owned wrapper text around them is translated.

This initial change ships **English + German only**. Remaining languages will follow in a separate commit.

## Design Decisions

**Library: `rust-i18n` v3**
- `t!("key")` / `t!("key", var = val)` macro API
- Translations in YAML, compiled into binary — no runtime file loading

**Separate UI language setting:** `ui_language` config field independent of `dictionary_language`. Defaults to `"en"`.

**Separate supported locale list:** UI locale validation uses `SUPPORTED_UI_LOCALES` (initially `["en", "de"]`), decoupled from the dictionary language pack system.

**Language names as autonyms everywhere:** All places that display a language name (selectors, settings summaries, status messages) use the language's autonym ("Deutsch", "Français") via a new `autonym` field on `LanguagePack`. No exonyms or locale-translated language names. Tradeoff: users may not recognize unfamiliar languages by autonym alone (e.g. "Suomi" for Finnish), but this is consistent and avoids translating language names per-locale. The existing English `display_name` field remains available as context.

**Stale text on locale switch:** Already-rendered `StatusMessage.text` and open `KeyMilestonePopup` messages stay in the old language until dismissed. Only newly produced text uses the new locale.

**Domain errors stay UI-agnostic:** `LanguageLayoutValidationError::Display` keeps its current English implementation. Translation happens at the UI boundary via a helper function in the i18n module.

**Canonical import:** All files use `use crate::i18n::t;` as the single import style for the translation macro.

## Text Source Categories

| Category | Example Location | Strategy |
|----------|-----------------|----------|
| **Inline literals** | Render functions in `main.rs`, UI components | Replace with `t!()` |
| **`const` arrays** | `UNLOCK_MESSAGES`, `MASTERY_MESSAGES`, `TAB_LABELS`, `FOOTER_HINTS_*` | Convert to functions returning `Vec<String>` or build inline |
| **`format!()` templates** | `StatusMessage` construction in `app.rs`/`main.rs` | Replace template with `t!("key", var = val)` |
| **`Display` impls** | `LanguageLayoutValidationError` | Keep `Display` stable; translate at UI boundary in i18n module |
| **Domain display names** | `LanguagePack.display_name` | Add `autonym` field; code language names stay English ("Rust", "Python") |
| **Cached `'static` fields** | `KeyMilestonePopup.message: &'static str` | Change to `String` |

## Implementation Steps

### Step 1: Centralized i18n module and dependency setup

Add `rust-i18n = "3"` to `[dependencies]` and `serde_yaml = "0.9"` to `[dev-dependencies]` in `Cargo.toml`.

Create `src/i18n.rs`:

```rust
pub use rust_i18n::t;

rust_i18n::i18n!("locales", fallback = "en");

/// Available UI locale codes. Separate from dictionary language support.
pub const SUPPORTED_UI_LOCALES: &[&str] = &["en", "de"];

pub fn set_ui_locale(locale: &str) {
    let effective = if SUPPORTED_UI_LOCALES.contains(&locale) { locale } else { "en" };
    rust_i18n::set_locale(effective);
}

/// Translate a LanguageLayoutValidationError for display in the UI.
pub fn localized_language_layout_error(err: &crate::l10n::language_pack::LanguageLayoutValidationError) -> String {
    use crate::l10n::language_pack::LanguageLayoutValidationError::*;
    match err {
        UnknownLanguage(key) => t!("errors.unknown_language", key = key),
        UnknownLayout(key) => t!("errors.unknown_layout", key = key),
        UnsupportedLanguageLayoutPair { language_key, layout_key } =>
            t!("errors.unsupported_pair", language = language_key, layout = layout_key),
        LanguageBlockedBySupportLevel(key) =>
            t!("errors.language_blocked", key = key),
    }
}
```

**Crate root ownership — which targets compile translated modules:**

| Target | Declares `mod i18n` | Compiles modules that call `t!()` | Must call `set_ui_locale()` |
|--------|---------------------|-----------------------------------|----------------------------|
| `src/main.rs` (binary) | Yes | Yes (`app.rs`, UI components, `main.rs` itself) | Yes, at startup |
| `src/lib.rs` (library) | Yes | Yes (`app.rs` is in the lib module tree) | No — lib is for benchmarks/test profiles; locale defaults to English via `fallback = "en"` |
| `src/bin/generate_test_profiles.rs` | No | No — imports from `keydr::` lib but only uses data types, not UI/translated code | No |

**Invariant:** Any module that calls `t!()` must be in a crate whose root declares `mod i18n;` with the `i18n!()` macro. If a future change adds `t!()` calls to a module reachable from `generate_test_profiles`, that binary must also add `mod i18n;`. The `fallback = "en"` default ensures English output when `set_ui_locale()` is never called.

Create `locales/en.yml` with initial structure, verify `cargo build`.

### Step 2: Add `ui_language` config field

**`src/config.rs`:**
- Add `ui_language: String` with `#[serde(default = "default_ui_language")]`, default `"en"`
- Add `normalize_ui_language()` — validates against `i18n::SUPPORTED_UI_LOCALES`, resets to `"en"` if unsupported
- Add to `Default` impl and `validate()`

**`src/app.rs`:**
- Add `UiLanguageSelect` variant to `AppScreen`
- Change `KeyMilestonePopup.message` from `&'static str` to `String`

**`src/main.rs`:**
- Call `i18n::set_ui_locale(&app.config.ui_language)` after `App::new()`
- Add "UI Language" setting item in settings menu (before "Dictionary Language")
- Add `UiLanguageSelect` screen reusing language selection list pattern (filtered to `SUPPORTED_UI_LOCALES`)
- On selection: update `config.ui_language`, call `i18n::set_ui_locale()`
- After data import: call `i18n::set_ui_locale()` again

### Step 3: Add autonym field to LanguagePack

**`src/l10n/language_pack.rs`:**
- Add `autonym: &'static str` field to `LanguagePack`
- Populate for all 21 languages: "English", "Deutsch", "Español", "Français", "Italiano", "Português", "Nederlands", "Svenska", "Dansk", "Norsk bokmål", "Suomi", "Polski", "Čeština", "Română", "Hrvatski", "Magyar", "Lietuvių", "Latviešu", "Slovenščina", "Eesti", "Türkçe"

**Update all language name display sites in `main.rs`:**
- Dictionary language selector: show `pack.autonym` instead of `pack.display_name`
- UI language selector: show `pack.autonym`
- Settings value display for dictionary language: show `pack.autonym`
- Status messages mentioning languages (e.g. "Switched to {}"): use `pack.autonym`

### Step 4: Create English base translation file (`locales/en.yml`)

Populate all ~200 keys organized by component:

```yaml
en:
  menu:        # menu items, descriptions, subtitle
  drill:       # mode headers, footers, focus labels
  dashboard:   # results screen labels, hints
  sidebar:     # stats sidebar labels
  settings:    # setting names, toggle values, buttons
  status:      # import/export/error messages (format templates)
  skill_tree:  # status labels, hints, notices
  milestones:  # unlock/mastery messages, congratulations
  stats:       # tab names, chart titles, hints, empty states
  heatmap:     # month/day abbreviations, title
  keyboard:    # explorer labels, detail fields
  intro:       # passage/code download setup dialogs
  dialogs:     # confirmation dialogs
  errors:      # validation error messages (for UI boundary translation)
  common:      # WPM, CPM, Back, etc.
```

### Step 5: Convert source files to use `t!()` — vertical slice first

**Phase A — Vertical slice (one file per text category to establish patterns):**

1. `src/ui/components/menu.rs` — inline literals (9 strings)
2. `src/ui/components/stats_dashboard.rs` — inline literals + `const` arrays → functions
3. `src/app.rs` — `StatusMessage` format templates (~20 strings), `UNLOCK_MESSAGES`/`MASTERY_MESSAGES` → functions
4. Update `StatusMessage` creation sites in `main.rs` that reference `LanguageLayoutValidationError` to use `i18n::localized_language_layout_error()` instead of `err.to_string()`

**Phase B — Remaining components:**

5. `src/ui/components/chart.rs` (3 strings)
6. `src/ui/components/activity_heatmap.rs` (14 strings)
7. `src/ui/components/stats_sidebar.rs` (10 strings)
8. `src/ui/components/dashboard.rs` (12 strings)
9. `src/ui/components/skill_tree.rs` (15 strings)

**Phase C — main.rs (largest):**

10. `src/main.rs` (~120+ strings) — settings menu, drill rendering, milestone overlay rendering, keyboard explorer, intro dialogs, footer hints, status messages

**Key patterns:**
- `use crate::i18n::t;` in every file that needs translation
- `t!()` returns `String`; for `&str` contexts: `let label = t!("key"); &label`
- Footer hints like `"[ESC] Back"` — full string in YAML, translators preserve bracket keys: `"[ESC] Zurück"`
- `const` arrays → functions: e.g. `fn unlock_messages() -> Vec<String>`
- `StatusMessage.text` built via `t!()` at creation time

### Step 6: Create German translation file (`locales/de.yml`)

AI-generated translation of all keys from `en.yml`:
- Keep `%{var}` placeholders unchanged
- Keep key names inside `[brackets]` unchanged (`[ESC]`, `[Enter]`, `[Tab]`, etc.)
- Keep technical terms WPM/CPM untranslated
- Be concise — German text tends to run ~20-30% longer; keep terminal width in mind

### Step 7: Tests and validation

- Add `rust_i18n::set_locale("en")` in test setup where tests assert against English output
- Add a test that sets locale to `"de"` and verifies a rendered component uses German text
- Add a test that switches locale mid-run and verifies new `StatusMessage` text uses the new locale
- **Add a catalog parity test** (using `serde_yaml` dev-dependency): parse both `locales/en.yml` and `locales/de.yml` as `serde_yaml::Value`, recursively walk the key trees, verify every key in `en.yml` exists in `de.yml` and vice versa, and that `%{var}` placeholders in each value string match between corresponding entries
- Run `cargo test` and `cargo build`

## Files Modified

| File | Scope |
|------|-------|
| `Cargo.toml` | Add `rust-i18n = "3"`, `serde_yaml = "0.9"` (dev) |
| `src/config.rs` | Add `ui_language` field, default, validation |
| `src/lib.rs` | Add `mod i18n;` |
| `src/main.rs` | Add `mod i18n;`, `set_ui_locale()` calls, UI Language setting/select screen, ~120 string replacements, use `localized_language_layout_error()` |
| `src/app.rs` | Add `UiLanguageSelect` to `AppScreen`, `KeyMilestonePopup.message` → `String`, ~20 StatusMessage string replacements, convert milestone constants to functions |
| `src/l10n/language_pack.rs` | Add `autonym` field to `LanguagePack` |
| `src/ui/components/menu.rs` | 9 string replacements |
| `src/ui/components/dashboard.rs` | 12 string replacements |
| `src/ui/components/stats_dashboard.rs` | 25 string replacements, refactor `const` arrays to functions |
| `src/ui/components/skill_tree.rs` | 15 string replacements |
| `src/ui/components/stats_sidebar.rs` | 10 string replacements |
| `src/ui/components/activity_heatmap.rs` | 14 string replacements |
| `src/ui/components/chart.rs` | 3 string replacements |

## Files Created

| File | Content |
|------|---------|
| `src/i18n.rs` | Centralized i18n bootstrap, `SUPPORTED_UI_LOCALES`, `set_ui_locale()`, `localized_language_layout_error()` |
| `locales/en.yml` | English base translations (~200 keys) |
| `locales/de.yml` | German translations |

## Verification

1. `cargo build` — rust-i18n checks referenced keys at compile time (not a complete catalog correctness guarantee; parity test and manual checks cover the rest)
2. `cargo test` — including catalog parity test + locale-specific tests
3. Manual testing with UI set to English: navigate all screens, verify identical behavior to pre-i18n
4. Manual testing with UI set to German: navigate all screens, verify German text
5. Verify drill source text (passage/code content) is NOT translated
6. Verify language selectors show autonyms ("Deutsch", not "German")
7. Test locale switch: change UI language in settings, verify new text appears in new language, existing status banner stays in old language
8. Check for layout/truncation issues with German text
