# Plan: Fix Remaining Untranslated UI Strings

## Context

The i18n system is implemented but several categories of strings were missed:
1. Menu item labels/descriptions are cached as `String` at construction and never refreshed when locale changes
2. Skill tree branch names and level names are hardcoded `&'static str` in `BranchDefinition`/`LevelDefinition`
3. Passage selector labels ("All (Built-in + all books)", "Built-in passages only", "Book: ...") are hardcoded
4. Branch progress list (`branch_progress_list.rs`) renders branch names and "Overall Key Progress" / "unlocked" / "mastered" in English

## Fix 1: Menu Items — Translate at Render Time

**Problem:** `Menu::new()` calls `t!()` once during `App::new()`. Even though `set_ui_locale()` runs after construction, the items are cached as `String` and never refreshed when the user changes UI language mid-session.

**Fix:** Define a shared static item list (keys + translation keys) and build rendered strings from it in both `Widget::render()` and navigation code.

**Files:** `src/ui/components/menu.rs`

- Define a `const MENU_ITEMS` array of `(&str, &str, &str)` tuples: `(shortcut_key, label_i18n_key, desc_i18n_key)`. This is the single authoritative definition.
- Remove `MenuItem` struct and the `items: Vec<MenuItem>` field.
- Keep `selected: usize` and `theme` fields. `next()`/`prev()` use `MENU_ITEMS.len()`.
- Add a `Menu::item_count() -> usize` helper returning `MENU_ITEMS.len()`.
- In `Widget::render()`, iterate `MENU_ITEMS` and call `t!()` for label/description each frame.
- Replace `app.menu.items.len()` in `src/main.rs` mouse handler (~line 660) with `Menu::item_count()`.

## Fix 2: Skill Tree Branch and Level Names — Replace `name` with `name_key`

**Problem:** `BranchDefinition.name` and `LevelDefinition.name` are `&'static str` with English text. They are used purely for UI display (confirmed: no serialization, logging, or export uses).

**Fix:** Replace `name` with `name_key` on both structs. The `name_key` holds a translation key (e.g. `"skill_tree.branch_primary_letters"`). All display sites use `t!(def.name_key)`.

Add `BranchDefinition::display_name()` and `LevelDefinition::display_name()` convenience methods that return `t!(self.name_key)` so call sites stay simple.

Change `find_key_branch()` to return `(&'static BranchDefinition, &'static LevelDefinition, usize)` instead of `(&'static BranchDefinition, &'static str, usize)`. This gives callers access to the `LevelDefinition` and its `name_key` so they can localize the level name themselves.

**Complete consumer inventory:**

| File | Lines | Usage |
|------|-------|-------|
| `src/ui/components/skill_tree.rs` | ~366 | Branch name in branch list header |
| `src/ui/components/skill_tree.rs` | ~445 | Branch name in detail header |
| `src/ui/components/skill_tree.rs` | ~483 | Level name in detail level list |
| `src/ui/components/branch_progress_list.rs` | ~95 | Branch name in single-branch drill sidebar |
| `src/ui/components/branch_progress_list.rs` | ~188 | Branch name in multi-branch progress cells |
| `src/main.rs` | ~3931 | Branch name in "branches available" milestone |
| `src/main.rs` | ~3961 | Branch names in "branch complete" milestone text |
| `src/main.rs` | ~6993 | Branch name in unlock confirmation dialog |
| `src/main.rs` | ~7327 | Branch name + level name in keyboard detail panel (via `find_key_branch()`) |

**Files:**
- `src/engine/skill_tree.rs` — Replace `name` with `name_key` on both structs; add `display_name()` methods; change `find_key_branch()` return type; populate `name_key` for all entries
- `src/ui/components/skill_tree.rs` — Use `def.display_name()` / `level.display_name()` at 3 sites
- `src/ui/components/branch_progress_list.rs` — Use `def.display_name()` at 2 sites; also translate "Overall Key Progress", "unlocked", "mastered"
- `src/main.rs` — Use `def.display_name()` at 4 sites; update `find_key_branch()` call site to use `level.display_name()`
- `locales/en.yml` — Add branch/level name keys under `skill_tree:`
- `locales/de.yml` — Add German translations

Note on truncation: `branch_progress_list.rs` uses fixed-width formatting (`{:<14}`, truncation widths 10/12/14). German branch names that exceed these widths will be truncated. This is acceptable for now — the widget already handles this via `truncate_and_pad()`. Proper dynamic-width layout is a separate concern.

Translation keys to add:
```yaml
skill_tree:
  branch_primary_letters: 'Primary Letters'
  branch_capital_letters: 'Capital Letters'
  branch_numbers: 'Numbers 0-9'
  branch_prose_punctuation: 'Prose Punctuation'
  branch_whitespace: 'Whitespace'
  branch_code_symbols: 'Code Symbols'
  level_frequency_order: 'Frequency Order'
  level_common_sentence_capitals: 'Common Sentence Capitals'
  level_name_capitals: 'Name Capitals'
  level_remaining_capitals: 'Remaining Capitals'
  level_common_digits: 'Common Digits'
  level_all_digits: 'All Digits'
  level_essential: 'Essential'
  level_common: 'Common'
  level_expressive: 'Expressive'
  level_enter_return: 'Enter/Return'
  level_tab_indent: 'Tab/Indent'
  level_arithmetic_assignment: 'Arithmetic & Assignment'
  level_grouping: 'Grouping'
  level_logic_reference: 'Logic & Reference'
  level_special: 'Special'
```

Also add to `progress` section (translation values contain only text, no alignment whitespace — padding is applied in rendering code):
```yaml
progress:
  overall_key_progress: 'Overall Key Progress'
  unlocked_mastered: '%{unlocked}/%{total} unlocked (%{mastered} mastered)'
```

## Fix 3: Passage Book Selector Labels

**Problem:** `passage_options()` returns hardcoded `"All (Built-in + all books)"`, `"Built-in passages only"`, and `"Book: {title}"`.

**Fix:** Add `t!()` calls in `passage_options()`. Book titles (proper nouns like "Pride and Prejudice") stay untranslated per plan.

**Files:**
- `src/generator/passage.rs` — Add `use crate::i18n::t;`, convert the two label strings and the "Book:" prefix
- `locales/en.yml` — Add keys under `select:`:
  ```yaml
  select:
    passage_all: 'All (Built-in + all books)'
    passage_builtin: 'Built-in passages only'
    passage_book_prefix: 'Book: %{title}'
  ```
- `locales/de.yml` — German translations

## Verification

1. `cargo check` — must compile
2. `cargo test --lib i18n::tests` — catalog parity and placeholder parity tests catch missing keys
3. `cargo test --lib` — no new test failures
4. Add tests for the new translated surfaces. To avoid parallel-test races on global locale state, new tests use `t!("key", locale = "de")` directly on the translation keys rather than calling ambient-locale helpers like `display_name()` or `passage_options()`. This keeps tests deterministic without needing serial execution or locale-parameterized API variants.
   - Test that `t!("skill_tree.branch_primary_letters", locale = "de")` returns the expected German text
   - Test that `t!("select.passage_all", locale = "de")` returns the expected German text
