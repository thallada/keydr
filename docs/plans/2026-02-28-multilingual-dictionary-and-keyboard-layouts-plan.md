# keydr Multilingual Dictionary + Keyboard Layout Internationalization Plan

## Context

We currently use an English-only dictionary and an ASCII-centric adaptive model:

- Dictionary is hardcoded to `assets/words-en.json` in `src/generator/dictionary.rs`.
- Dictionary ingestion filters to ASCII lowercase only (`is_ascii_lowercase`).
- Transition table building (`src/generator/transition_table.rs`) skips non-ASCII words.
- Adaptive drill generation in `src/app.rs` builds lowercase filter from `is_ascii_lowercase`.
- Skill tree lowercase branch is fixed to English `a-z` frequency in `src/engine/skill_tree.rs`.
- Keyboard rendering/hit-testing logic has hardcoded row offsets and row count assumptions in `src/ui/components/keyboard_diagram.rs` and `src/ui/components/stats_dashboard.rs`.

## Explicit product decision: clean break

This app is currently work-in-progress and has no real user base. We explicitly do
not need to preserve old config/state/export compatibility for this change. If data
must be recreated from scratch, that is acceptable.

## Goals

1. Add user-selectable dictionary language (default `en`) using keybr-provided dictionary files.
2. Add user-selectable keyboard layout profiles for multiple languages.
3. Ensure keyboard visualizations, explorer, and stats heatmaps render correctly for variable row shapes and non-English keycaps.
4. Use a clean-break implementation with no backward-compatibility requirements.
5. Maintain license compliance for newly imported dictionaries.

## Non-goals (first delivery)

1. Full IME/dead-key composition support.
2. Full rewrite of adaptive model for every script from day one.
3. Perfect locale-specific pedagogy for all languages in phase 1.
4. Backward compatibility for old config/profile/export data.

## Execution constraints (must be explicit before implementation)

1. **Unicode normalization policy:** Use NFC as canonical storage/matching form for dictionary ingestion, generated text, keystroke comparison, and persisted stats keys. Do not use NFKC in phase 1 to avoid compatibility-fold surprises.
2. **Character equivalence policy:** Equality is by normalized scalar sequence (NFC), not by glyph appearance. Composed/decomposed equivalents must compare equal after normalization.
3. **Clean-break schema cutover policy:** This rollout uses hard reset semantics for old unscoped stats/profile files. On first run of the new schema version, old files are ignored (optionally archived with `.legacy` suffix); no partial migration path.
4. **Capability gating policy:** Only language/layout pairs marked supported in the registry capability matrix are selectable in UI during phased rollout.
5. **Performance envelope policy:** Keyboard geometry recomputation must be bounded and cached by profile key + render mode + viewport size.

## Upstream data availability

`keybr-content-words` includes dictionaries for:

`ar, be, cs, da, de, el, en, es, et, fa, fi, fr, he, hr, hu, it, ja, lt, lv, nb, nl, pl, pt, ro, ru, sl, sv, th, tr, uk`

Recommended rollout strategy:

- Initial support for Latin-script languages first (`en, de, es, fr, it, pt, nl, sv, da, nb, fi, pl, cs, ro, hr, hu, lt, lv, sl, et, tr`).
- Later support for non-Latin scripts (`el, ru, uk, be, ar, fa, he, ja, th`) after script-specific input/model behavior is in place.

---

## Key Architectural Decisions

### 1) Language Pack registry

Add a registry module (e.g. `src/l10n/language_pack.rs`) containing:

- `language_key`
- `display_name`
- `script`
- `dictionary_asset_id`
- `supported_keyboard_layout_keys`
- `primary_letter_sequence` (for ranked progression)
- `starter_weights` and optional `vowel_set` for generator fallback behavior
- `support_level` (`full`, `experimental`, `blocked`)
- `normalization_form` (phase 1 fixed to `NFC`)
- `input_capabilities` (for example `direct_letters_only`, `needs_ime`)

This becomes the single source of truth for language behavior.

### 2) Runtime dictionary/generator rebuild is required

Changing `dictionary_language` must immediately take effect without restart.

Implement `App::rebuild_language_assets(&mut self)` that rebuilds:

- `Dictionary`
- `TransitionTable`
- any cached generator state derived from language assets
- focused-character transforms derived from language rules
- drill-generation allowlists that depend on language pack data

Call it whenever language or language-dependent layout changes in settings.

`rebuild_language_assets` must also refresh capitalization/case behavior inputs used by adaptive generation.

`rebuild_language_assets` invalidation contract (required):

- always invalidate and rebuild `Dictionary` and `TransitionTable`
- clear adaptive cross-drill dictionary history cache
- clear/refresh any cached language-specific focus mapping
- do **not** mutate in-progress drill text
- all newly generated drills after rebuild must use new language assets

### 3) Asset loading strategy: compile-time embedded assets

For Phase 1 scope, dictionaries will be embedded at compile-time (generated asset map + `include_str!`/equivalent), not runtime file discovery.

Rationale:

- deterministic packaging
- no runtime path resolution complexity
- simpler cross-platform behavior

Tradeoff: larger binary size, acceptable for this phase.

### 4) Transition table fallback strategy

`TransitionTable::build_english()` will be gated to `language_key == "en"` only.

For non-English languages:

- use dictionary-derived transition table only
- if sparse, degrade gracefully to simple dictionary sampling behavior rather than English fallback model

### 5) Keyboard geometry refactor strategy

`src/ui/components/keyboard_diagram.rs` is a substantial refactor (all render and hit-test paths).

Implement shared `KeyboardGeometry` computed once per render context and consumed by:

- compact/full/fallback renderers
- all key hit-testing paths
- shift hit-testing paths

No duplicate hardcoded offsets should remain.

Performance constraints for geometry:

- geometry cache key: `(layout_key, render_mode, viewport_width, viewport_height)`
- recompute only when cache key changes
- hit-testing must be O(number_of_keys) or better per event with no per-key allocation
- include a benchmark/smoke check to detect regressions in repeated render/hit-test loops

### 6) Finger assignment source of truth

Finger assignment must be profile metadata, not inferred by QWERTY column heuristics.

Each keyboard profile defines finger mapping for each physical key position.

### 7) Stats isolation strategy

Stats are language-scoped and layout-scoped.

Adopt per-scope storage files (for example):

- `key_stats_<language>_<layout>.json`
- `key_stats_ranked_<language>_<layout>.json`
- optional scoped drill history files

No mixed-language key stats in a single store.

Profile/scoring scoping policy:

- `skill_tree` progress is language-scoped (at minimum by `language_key`).
- `total_score`, `total_drills`, `streak_days`, and `best_streak` remain global.
- `ProfileData` will separate global fields from language-scoped progression state.

Scoped-file discovery mechanism:

- registry-driven + current-config driven only
- app loads current scope directly and only enumerates scopes from supported language/layout registry pairs
- no unconstrained glob-based discovery of arbitrary stale files

Import/export strategy for scoped stats:

- export bundles all supported scoped stats files present in the data dir
- each bundle entry includes explicit `language_key` and `layout_key` metadata
- import applies two-phase commit per scoped target file
- export/import also includes language-scoped `skill_tree` progress entries with `language_key` metadata

Atomicity requirements for scoped import:

- stage writes to `<target>.tmp`
- flush file contents (`sync_all`) before rename
- rename temp file onto target atomically where supported
- on any failure, remove temp file and keep existing target untouched
- no commit of partially imported scope bundles

### 8) Settings architecture

Current index-based settings handling is fragile.

Phase 1 includes refactor from positional integer indices to enum/struct-based settings entries before adding multilingual controls.

Profile key validation must be registry-backed. Do not rely on `KeyboardModel::from_name()` fallback behavior.

Validation error taxonomy (typed, stable):

- `UnknownLanguage`
- `UnknownLayout`
- `UnsupportedLanguageLayoutPair`
- `LanguageBlockedBySupportLevel`

UI must show deterministic user-facing error text for each class (used by tests).

In-progress drill behavior on language/layout change:

- language/layout changes rebuild assets immediately for future generation
- current in-progress drill text is not mutated mid-drill
- new language/layout applies on the next drill generation

### 9) Unicode handling architecture

Define one shared Unicode utility module used by dictionary ingestion, generators, and input matching:

- normalize all dictionary entries to NFC at load time
- normalize typed characters before comparison against expected text
- normalize persisted per-key identifiers before write/read
- provide helper tests for composed/decomposed equivalence (for example `é` vs `e + ◌́`)

### 10) Rollout capability matrix architecture

Add a single registry-backed capability matrix keyed by `(language_key, layout_key)`:

- `enabled`: selectable and fully supported
- `preview`: selectable with warning banner
- `disabled`: visible but not selectable

Phase-gating must read this matrix in settings and selection screens; no ad-hoc checks.

---

## Phased Implementation

## Phase 0: Data + compliance groundwork

### Tasks

1. Import selected dictionaries to `assets/dictionaries/words-<lang>.json`.
2. Add sidecar license/provenance files for each imported dictionary.
3. Update `THIRD_PARTY_NOTICES.md` with imported assets.
4. Add validation script for dictionary manifest/checksums.
5. Define language pack registry seed data (including temporary `primary_letter_sequence` values).
6. Add `support_level` and capability-matrix seed entries for every language/layout pair.
7. Add a build-time utility that derives letter frequency sequence from each dictionary (seed data source of truth; manual overrides allowed but documented).
8. Write `docs/unicode-normalization-policy.md` (NFC/equivalence rules + examples).

### Verification

1. All imported dictionaries listed in third-party notices.
2. Sidecar license/provenance file exists for each imported dictionary.
3. Manifest validation script passes.
4. Build-time frequency derivation utility emits reproducible output for seeded languages.
5. Unicode policy doc exists and includes composed/decomposed test cases.

---

## Phase 1: Settings and configuration foundation

### Tasks

1. Add `dictionary_language` to `Config`.
2. Refactor settings implementation from raw indices to typed settings entries (enum/descriptor model).
3. Add settings controls for:
   - dictionary language
   - canonical keyboard layout profile key
4. Implement explicit invalid combination handling (reject with message), not silent fallback.
5. Wire language/layout change actions to `App::rebuild_language_assets(&mut self)`.
6. Introduce clean-break schema/version update for config/profile/store formats with hard-reset behavior for old files.
7. Replace `from_name` wildcard fallback paths with explicit lookup failure handling tied to registry validation.
8. Update import/export schema and transaction flow for scoped stats bundles.
9. Split profile persistence into global fields + language-scoped skill tree progress map.
10. Enforce capability-matrix gating in settings/selectors (`enabled/preview/disabled` states).
11. Add typed validation errors and stable user-facing status messages.

### Code areas

- `src/config.rs`
- `src/main.rs` (settings UI rendering and input handling)
- `src/app.rs` (settings action handlers, rebuild trigger)
- `src/store/schema.rs`
- `src/store/json_store.rs`

### Verification

1. Unit tests for config defaults/validation.
2. Unit tests for settings navigation/editing after index refactor.
3. Runtime test: changing dictionary language updates generated drills without restart.
4. Runtime test: invalid language/layout pair is rejected with visible error/status.
5. Export/import test: scoped stats for multiple language/layout pairs round-trip correctly.
6. Runtime test: changing language mid-drill preserves current drill text and applies new language on next drill.
7. Schema cutover test: old-format files are ignored/archived and never partially loaded.
8. UI test: disabled/preview capability-matrix entries render and behave correctly.

---

## Phase 2: Dictionary, transition table, and generator internationalization

### Tasks

1. Refactor `Dictionary::load(language_key)` with embedded asset map.
2. Remove ASCII-only filtering from dictionary ingestion and transition building.
3. Extend `phonetic.rs` to remove English hardcoding:
   - replace hardcoded starter biases with language-pack starter data or derived frequencies
   - replace fallback `"the"` with language-aware fallback (for example: top dictionary word)
   - make vowel recovery optional/parameterized by language pack
   - remove `is_ascii_lowercase` focus filtering and rely on allowed-character logic
4. Implement transition fallback policy:
   - `build_english()` only for English
   - non-English graceful degradation path without English fallback table
5. Address adaptive and non-adaptive mode filters:
   - remove hardcoded `('a'..='z')` filters in code/passage modes
   - use language-pack allowed sets where applicable
6. Refactor capitalization pipeline to Unicode-aware behavior:
   - replace ASCII-only case checks/conversions in `capitalize.rs`
   - use Unicode case mapping and language-pack constraints
   - ensure non-ASCII letters (for example `ä/Ä`, `é/É`) are handled correctly
7. Implement shared normalization utility and apply it consistently in:
   - dictionary load path
   - generated text comparison/matching paths
   - persisted key identity paths
8. Multilingual audit checklist (required pass/fail):
   - `rg -n "is_ascii" src/app.rs src/generator/*.rs` has no unreviewed hits affecting multilingual behavior
   - every remaining `is_ascii*` hit has a documented justification comment or issue reference

### Code areas

- `src/generator/dictionary.rs`
- `src/generator/transition_table.rs`
- `src/generator/phonetic.rs`
- `src/generator/capitalize.rs`
- `src/app.rs` (adaptive/code/passage filter construction)

### Verification

1. Unit tests for dictionary loading per supported language.
2. Unit tests for transition table generation with non-English characters.
3. Unit tests for phonetic fallback behavior per language pack.
4. Unit tests for capitalization correctness on non-ASCII letters.
5. Regression tests for English output quality.
6. Unit tests for NFC normalization and composed/decomposed equivalence.

---

## Phase 3: Keyboard layout profile system

### Tasks

1. Replace ad-hoc constructors with canonical keyboard profile registry.
2. Add language-relevant profiles (`de_qwertz`, `fr_azerty`, etc.).
3. Add profile metadata:
   - key rows and shifted/base pairs
   - geometry hints
   - modifier placement metadata
   - per-key finger assignments
4. Remove legacy alias layer and enforce canonical profile keys.
5. Evaluate `src/keyboard/layout.rs` usage:
   - if unused, delete it
   - otherwise fold it into the new profile registry without duplicate sources of truth

### Code areas

- `src/keyboard/model.rs`
- `src/keyboard/layout.rs`
- `src/keyboard/display.rs` (if locale labels/short labels need extension)
- `src/config.rs`

### Verification

1. Unit tests for all canonical profile keys.
2. Unit tests for profile completeness and unique key mapping.
3. Unit tests for finger assignment coverage/consistency.

---

## Phase 4: Keyboard visualization and hit-testing refactor

### Tasks

1. Implement shared `KeyboardGeometry` used by all keyboard rendering modes.
2. Rewrite keyboard diagram rendering paths to use shared geometry.
3. Rewrite all keyboard hit-testing paths to use shared geometry.
4. Refactor stats dashboard keyboard heatmap/timing rendering to use profile geometry metadata.
5. Ensure explorer and selection logic works for variable row counts and locale keycaps.
6. Update sentinel boundary tests if new files must reference sentinel constants.
7. Remove ASCII shift-display guards in keyboard rendering:
   - replace `is_ascii_alphabetic()`-based shifted display checks
   - use profile-defined shiftability (`base != shifted` or explicit shiftable set)
8. Audit and replace ASCII-specific input-handling logic in `main.rs`:
   - caps-lock inference
   - depressed-key normalization
   - shift guidance and shifted-key detection in keyboard UI paths
9. Add geometry cache and recompute guards keyed by `(layout_key, render_mode, viewport)` with benchmark coverage.

### Code areas

- `src/ui/components/keyboard_diagram.rs`
- `src/ui/components/stats_dashboard.rs`
- `src/main.rs` keyboard explorer handlers
- `src/main.rs` input handling (`handle_key`, caps/shift logic, keyboard guidance/render helpers)
- `src/app.rs` explorer state/focus use
- `src/keyboard/display.rs` tests

### Verification

1. Snapshot/golden tests for compact/full/fallback rendering per profile.
2. Hit-test roundtrip tests per profile.
3. Manual keyboard explorer smoke tests for US + non-US profiles.
4. Sentinel boundary tests pass with updated policy.
5. Manual test: shifted rendering works for non-ASCII letter keys where profile defines shifted forms.
6. Manual test: caps/shift guidance and depressed-key behavior are correct for non-ASCII key input.
7. Benchmark/smoke test: repeated render + hit-test loops meet baseline without per-frame geometry rebuild when cache key is unchanged.

---

## Phase 5: Skill tree and ranked progression internationalization

### Tasks

1. Replace fixed English lowercase progression with language-pack `primary_letter_sequence`.
2. Replace hardcoded "lowercase as background" branch logic with language-pack primary-letter background behavior.
3. Remove UI copy assumptions of "26 lowercase" and `a-z`.
4. Ensure ranked gating uses language-pack readiness (sequence + profile support).
5. Define letter-frequency derivation approach:
   - derive initial sequence from dictionary frequency data (build-time utility), not hand-curated long-term
6. Milestone-copy audit checklist (required pass/fail):
   - grep for hardcoded milestone language in `main.rs` (`26`, `a-z`, `A-Z`, `lowercase`)
   - replace with language-pack-aware dynamic copy
   - add tests asserting copy adjusts with different sequence lengths

### Code areas

- `src/engine/skill_tree.rs`
- `src/app.rs` (focus/background/filter logic)
- `src/main.rs` (milestone/help copy)

### Verification

1. Tests for progression with multiple language sequences.
2. Tests for background-branch selection correctness.
3. Snapshot tests for milestone text across languages.

---

## Phase 6: UX polish, test parameterization, and rollout

### Tasks

1. Add dedicated language/layout selector screens where needed.
   - Implemented in `src/main.rs` + `src/app.rs` with `DictionaryLanguageSelect` and `KeyboardLayoutSelect`.
2. Add explicit support-matrix messaging for partially supported scripts.
   - Implemented in selector + settings UI copy in `src/main.rs` (`preview`/`disabled` state messaging).
3. Add parameterized test helpers:
   - language-aware allowed key sets
   - expected progression counts
   - profile fixtures
   - Implemented via cross-language/layout fixtures and property tests in `src/l10n/language_pack.rs`, `src/engine/skill_tree.rs`, and `src/ui/components/keyboard_diagram.rs`.
4. Document that Phase 2 may temporarily allow language/dictionary mismatch with keyboard visuals until Phase 3/4 is complete.
5. Add explicit note in docs that Phase 2 mismatch window is expected and resolved by Phase 4.
   - Implemented in `docs/multilingual-rollout-notes.md`.
6. Add cross-language property tests:
   - key uniqueness per profile
   - hit-test round-trip invariants
   - progression monotonicity per language sequence
   - Implemented in `src/keyboard/model.rs`, `src/ui/components/keyboard_diagram.rs`, and `src/engine/skill_tree.rs`.

### Code areas

- `src/main.rs`
- `src/app.rs`
- test modules across `src/*`
- `docs/`

### Verification

1. End-to-end manual flows for language switch + layout switch + drill generation + keyboard explorer + stats.
2. Performance checks for embedded dictionary footprint and startup latency.
3. Test suite passes with parameterized language/profile cases.
4. Property/invariant tests pass for key uniqueness, hit-test round-trip, and progression monotonicity.

---

## File-by-file Impact Matrix

### Core config and app wiring

- `src/config.rs`
  - add `dictionary_language` and canonical `keyboard_layout` profile key validation
- `src/app.rs`
  - add `rebuild_language_assets`
  - remove ASCII-only filters and audit residual ASCII assumptions (`rg is_ascii` pass)
  - wire settings actions to runtime rebuild
- `src/main.rs`
  - refactor settings UI to typed entries
  - add/update selectors and error/status handling
  - audit/replace ASCII-specific input/caps/shift handling

### Generators and adaptive engine

- `src/generator/dictionary.rs`
  - dynamic, language-aware load via embedded registry
- `src/generator/transition_table.rs`
  - non-ASCII support and explicit English-only fallback gating
- `src/generator/phonetic.rs`
  - remove hardcoded English starter/vowel/fallback assumptions
- `src/generator/capitalize.rs`
  - replace ASCII-only casing logic with Unicode-aware capitalization rules

### Skill progression

- `src/engine/skill_tree.rs`
  - language-pack primary sequence
  - language-pack background branch behavior

### Keyboard modeling and visualization

- `src/keyboard/model.rs`
  - canonical profile registry with per-key finger mapping
- `src/keyboard/layout.rs`
  - delete or fold into model registry
- `src/ui/components/keyboard_diagram.rs`
  - shared geometry + full hit-test rewrite
- `src/ui/components/stats_dashboard.rs`
  - geometry-driven keyboard heatmap/timing rendering
- `src/keyboard/display.rs`
  - sentinel boundary test updates as needed

### Persistence/schema

- `src/store/schema.rs`
  - clean-break schema/version bump as needed
  - split profile data into global fields + language-scoped skill tree progress
- `src/store/json_store.rs`
  - scoped stats storage by language/layout
  - scoped file discovery based on supported registry pairs
  - export/import scoped bundle handling with language/layout metadata
  - export/import language-scoped skill tree progress entries

### Assets/compliance/docs

- `assets/dictionaries/*`
- `assets/dictionaries/*.license`
- `THIRD_PARTY_NOTICES.md`
- `docs/license-compliance.md`
- `docs/unicode-normalization-policy.md`

---

## Risks and mitigations

1. **Risk:** Non-Latin scripts break assumptions in multiple modules.
   - **Mitigation:** staged rollout by script; support matrix gating.
2. **Risk:** Keyboard visualization regressions during geometry rewrite.
   - **Mitigation:** shared geometry abstraction + dedicated hit-test/render tests.
3. **Risk:** Clean-break schema reset discards local data.
   - **Mitigation:** explicitly documented and accepted by product decision.
4. **Risk:** Settings refactor increases short-term scope.
   - **Mitigation:** do it early to avoid repeated index-cascade bugs.
5. **Risk:** Embedded dictionary set increases binary size/startup memory.
   - **Mitigation:** track size/startup metrics per release and switch to hybrid packaging if thresholds are exceeded.

---

## Definition of Done

1. Language switch updates dictionary-driven generation without restart.
2. Keyboard profiles are canonical and language-aware; no legacy alias dependency.
3. Keyboard diagram, explorer, and stats views are geometry-driven and correct for supported profiles.
4. Ranked progression uses language-pack primary sequences and background logic.
5. Code/passage/adaptive modes no longer depend on hardcoded `a-z` filters.
6. Stats are isolated by language/layout scope.
7. Skill tree progression is language-scoped while streak/score totals remain global.
8. Third-party attributions and license sidecars cover all imported dictionary assets.
9. Automated tests cover runtime rebuild, generator behavior, keyboard geometry/hit-testing, progression invariants, and parameterized language/profile cases.
10. Unicode normalization policy is implemented and tested across ingestion, generation, input matching, and persisted stats keys.
11. Clean-break schema cutover behavior is deterministic (hard-reset semantics) and covered by automated tests.
12. Capability matrix gating is enforced consistently across settings/selectors and covered by UI/runtime tests.
