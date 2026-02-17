# Skill Tree Integration Fixes & UI Improvements

## Context
After adding a skill tree progression system, several parts of the app weren't fully integrated. This plan addresses 7 issues: progress bar confusion, broken skill tree bars, missing selectability, duplicate displays, incomplete keyboard visualization, code drill formatting issues, and a missing menu shortcut.

## Architecture Foundations

### A. Layout-Driven Keyboard Model
**Files:** `src/keyboard/layout.rs`, new `src/keyboard/model.rs`

The existing `KeyboardLayout` in `layout.rs` only stores `Vec<Vec<char>>` (base layer). We need a shared model used by both drill and stats keyboards.

Create `src/keyboard/model.rs`:
- `PhysicalKey { base: char, shifted: char }` - represents one physical key with both layers
- `KeyboardModel { rows: Vec<Vec<PhysicalKey>> }` - full keyboard definition
- Factory methods: `KeyboardModel::qwerty()`, `::dvorak()`, `::colemak()` - each returns the full layout
- Helper: `base_to_shifted(ch) -> Option<char>` and `shifted_to_base(ch) -> Option<char>` derived from the model
- Helper: `physical_key_for(ch) -> Option<&PhysicalKey>` - lookup by either base or shifted char

The QWERTY model:
```
Row 0 (number):   (`~) (1!) (2@) (3#) (4$) (5%) (6^) (7&) (8*) (9() (0)) (-_) (=+)
Row 1 (top):      (qQ) (wW) (eE) (rR) (tT) (yY) (uU) (iI) (oO) (pP) ([{) (]}) (\|)
Row 2 (home):     (aA) (sS) (dD) (fF) (gG) (hH) (jJ) (kK) (lL) (;:) ('")
Row 3 (bottom):   (zZ) (xX) (cC) (vV) (bB) (nN) (mM) (,<) (.>) (/?)
```

Update `KeyboardLayout` to use `KeyboardModel` internally (or replace it).

Replace `qwerty_finger(ch)` with a layout-aware API:
- `KeyboardModel::finger_for(&self, key: &PhysicalKey) -> FingerAssignment` - each layout defines finger assignments per physical key position (row, col)
- For shifted chars, callers first resolve to physical key via `physical_key_for(ch)`, then look up finger
- This eliminates the QWERTY-only char match and works for Dvorak/Colemak

Load the active layout from `config.keyboard_layout` and pass it through to all keyboard rendering.

### B. Dual Progress Metrics
**File:** `src/engine/skill_tree.rs`

Add `branch_unlocked_count(id: BranchId) -> usize` method:
- Lowercase: delegates to `lowercase_unlocked_count()`
- Others: sums `keys.len()` for levels `0..=current_level` when InProgress; all keys when Complete; 0 otherwise

All UI uses two metrics per branch:
- **Unlocked**: `branch_unlocked_count(id)` / `branch_total_keys(id)` - how far through the branch
- **Mastered**: `branch_confident_keys(id, stats)` / `branch_total_keys(id)` - how many keys at confidence >= 1.0

### C. Code Language Config
**File:** `src/config.rs`

Replace the implicit `code_languages: Vec<String>` usage with a clearer model:
- Add `code_language: String` field (single language: "rust", "python", "javascript", "go", "all")
- Keep `code_languages` for backwards compat but derive from `code_language`
- Settings cycling and code generation both read `code_language`
- "all" picks a random language per drill in `generate_text()`

---

## Implementation Changes (in order)

### 1. Fix missing `[c] Settings` shortcut in menu footer
**File:** `src/main.rs` (`render_menu` function)
- Change footer string to: `" [1-3] Start  [t] Skill Tree  [s] Stats  [c] Settings  [q] Quit "`
- Verify no other footers are missing hints by checking all `render_*` functions

### 2. Fix duplicate fraction display on Lowercase branch
**File:** `src/ui/components/skill_tree.rs` (`render_branch_list`)
- Currently shows `"6/26  0/26 keys"` because status_text and confident/total are concatenated
- Change to single display: `"6/26 unlocked"` when no mastered keys, or `"6/26 unlocked (3 mastered)"` when some exist
- Apply same pattern to all branches: `"Lvl 1/3  5/10 unlocked (2 mastered)"`

### 3. Make Lowercase a-z selectable in skill tree
**Files:** `src/ui/components/skill_tree.rs`, `src/main.rs` (`handle_skill_tree_key`)

- Add `BranchId::Lowercase` to `selectable_branches()` at index 0
- Merge the separate root Lowercase rendering (currently in `render_branch_list` lines 113-170) into the main branch loop
- Apply selection highlighting to Lowercase using same `is_selected` logic as other branches
- Keep "Branches (unlocked after a-z)" separator after Lowercase (index 0) and before Capitals (index 1)
- Detail panel for Lowercase: show progressive unlock state `"Unlocked 6/26 letters"` instead of `"Level 1/1"`. Show each unlocked key with its confidence, locked keys dimmed
- Enter on InProgress Lowercase starts branch drill (existing `start_branch_drill` handles this)
- Update `branch_list_height` calculation to account for the merged layout

### 4. Fix skill tree progress bars - combined unlocked/mastered bar
**Files:** `src/engine/skill_tree.rs`, `src/ui/components/skill_tree.rs`

- Add `branch_unlocked_count()` method (see Architecture B above)
- Change progress bars to a **combined dual-metric bar**: the bar is divided into three segments:
  - Filled (accent color): mastered keys (confidence >= 1.0)
  - Filled (dimmer color): unlocked but not yet mastered
  - Empty (background): locked keys
- This works because mastered <= unlocked <= total always holds
- Update `progress_bar_str` to accept two ratios and render with two fill colors
- **Rounding rule**: compute cell counts from raw counts (not ratios) to avoid rounding violations:
  - `mastered_cells = (mastered * width / total)` (floor)
  - `unlocked_cells = (unlocked * width / total).max(mastered_cells)` (floor, clamped)
  - `empty_cells = width - unlocked_cells`
  - This guarantees `mastered_cells <= unlocked_cells <= width` with no overlap
- Text label shows: `"6/26 unlocked, 3 mastered"`

### 5. Add per-key mastery display in skill tree detail panel (phase 2 if time allows)
**File:** `src/ui/components/skill_tree.rs` (`render_detail_panel`)

- In the detail view for the selected branch, show a mini progress bar per key
- Each key shows: `char [====----] 75%` where the bar represents confidence (0-100%)
- Keys already at confidence >= 1.0 show as fully filled with success color
- Keys not yet unlocked show dimmed with "locked" label
- Focused key is highlighted (existing logic already identifies it)
- Layout: keys in their level groups, each on its own line with the mini bar
- Note: This adds UI complexity. Implement after core issues (1-4, 6-8) are stable.

### 6. Replace drill screen progress bar with per-branch progress
**Files:** `src/main.rs` (`render_drill`), new `src/ui/components/branch_progress_list.rs`

Create a new `BranchProgressList` widget (not stretching the existing `ProgressBar`):
- Shows one compact line per active branch (InProgress or Complete), plus an overall line
- Each line: `"  ▶ Lowercase    [████░░░░] 6/26"`
- Uses the combined dual-metric bar from Issue 4 (mastered vs unlocked segments)
- Active drill branch (from `app.drill_scope`) is highlighted with accent color and `▶` prefix
- Other branches use dimmer color and `·` prefix

Layout budgeting by `LayoutTier` (unbordered, plain lines to maximize density):
- **Wide** (height >= 25): show all active branches (InProgress/Complete). `Constraint::Length(active_count.min(6) as u16 + 1)` (+1 for "Overall" line)
- **Wide** (height 20-24): show active drill branch + overall only. `Constraint::Length(2)`
- **Medium**: show active drill branch only. `Constraint::Length(1)`
- **Narrow**: hide progress (current behavior)

### 7. Full keyboard visualization
**Files:** `src/keyboard/model.rs` (new), `src/keyboard/layout.rs` (update), `src/ui/components/keyboard_diagram.rs`, `src/ui/components/stats_dashboard.rs`, `src/main.rs`, `src/app.rs`

#### 7a. Build KeyboardModel (Architecture A above)

#### 7b. Drill keyboard
- `KeyboardDiagram` takes `&KeyboardModel` instead of hardcoded `ROWS`
- Add `shift_held: bool` field
- **Shift state handling**: Primary source is `key.modifiers.contains(KeyModifiers::SHIFT)` checked on every Press event. Set `app.shift_held = true` when modifier present, `false` when absent. Additionally, on tick (100ms), if `shift_held` is true and no key event has been received in 200ms, clear it as a fallback. This means: shifted display appears when a shifted key is pressed, and naturally clears on the next unshifted keypress or after timeout. Acceptance: brief flicker (1-2 frames) on quick shift+key combos is acceptable; sustained wrong state is not.
- When `shift_held`, display `physical_key.shifted` for each key; otherwise `physical_key.base`
- Full mode: 4 rows (number, top, home, bottom) + visual-only labels for Tab/Backspace/Shift/Enter at row edges
- Compact mode: 3 rows letters only (current behavior, but driven from `KeyboardModel`)
- Height: `Constraint::Length(7)` for full (4 rows + 2 border + label), `Constraint::Length(5)` for compact
- Replace `finger_color(ch)` with layout-aware `finger_for(model, physical_key) -> FingerAssignment` that works for any layout (see 7a)
- `is_unlocked` check: map the displayed char against `unlocked_keys` list

#### 7c. Stats keyboard heatmap
- Two sub-rows per physical row: top = shifted layer (dimmer styling), bottom = base layer
- Each cell shows char + accuracy % (existing format)
- Height: `Constraint::Length(12)` (4 physical rows x 2 sub-rows + 2 borders + header)
- Load from `KeyboardModel` based on `config.keyboard_layout`
- Accuracy lookup: use existing `get_key_accuracy(char)` for each layer independently
- **Width fallback**: if terminal width < 70, collapse to base layer only (hide shifted sub-rows). Existing min-width guard pattern from `render_keyboard_heatmap` (width < 50 => skip) is preserved.

### 8. Code drill improvements
**Files:** `src/generator/code_syntax.rs`, `src/app.rs`, `src/main.rs`, `src/config.rs`

#### 8a. Multi-line embedded snippets
- Reformat all snippets in `rust_snippets()`, `python_snippets()`, `javascript_snippets()`, `go_snippets()` to be multi-line with realistic formatting
- Go: use `\t` for indentation (gofmt convention)
- Rust/Python/JavaScript: use 4 spaces
- Keep Tab key input as literal `\t` (do NOT convert to spaces) - this is needed for whitespace branch progression and the typing area already renders tabs properly
- Add basic validation for fetched snippets: require at least one newline and reject snippets that are all on one line (filter in `extract_code_snippets`)

#### 8b. Language selection screen
- Add `AppScreen::CodeLanguageSelect` to `AppScreen` enum
- Add `code_language_selected: usize` to `App`
- Screen flow: Menu `'2'` or Enter on "Code Drill" -> `CodeLanguageSelect` -> select language -> start drill
- ESC from language select returns to Menu
- Direct hotkeys in language select: `1`=Rust, `2`=Python, `3`=JavaScript, `4`=Go, `5`=All
- Enter confirms selection
- Arrow keys / j/k navigate
- Default selection: whichever language matches current `config.code_language`
- On confirm: update `config.code_language`, save config, set `drill_mode = Code`, start drill
- Render: centered bordered box with language list, highlighting selected item, showing `(current)` next to the default

#### 8c. Config changes
- Add `code_language: String` field to Config with default "rust"
- Settings screen language cycling updates `code_language`
- `generate_text` for Code mode reads `code_language` (if "all", picks random)

---

## Verification
- `cargo build` -- no compilation errors
- `cargo test` -- existing tests pass; add tests for:
  - `branch_unlocked_count` returns correct values for each branch state
  - `KeyboardModel::qwerty()` covers all skill tree chars
  - Selection bounds don't panic with Lowercase in `selectable_branches`
- Manual testing checklist:
  - Menu footer shows `[c] Settings`
  - Skill tree: Lowercase is selectable with arrow keys, Enter starts drill
  - Skill tree: single fraction display, no duplicate numbers
  - Skill tree: progress bars show dual unlocked/mastered segments
  - Skill tree detail: per-key mastery bars shown
  - Drill: branch progress bars visible, active branch highlighted
  - Drill keyboard: full layout visible, keys shift on Shift press
  - Stats keyboard: both layers shown
  - Code drill: language selection appears, snippets have proper newlines/indentation
  - Non-adaptive drills: ESC still shows partial result correctly
  - Dvorak/Colemak: keyboard renders correctly when layout config changed
