# keydr Improvement Plan

## Context

The app was built in a single first-pass implementation. Six issues need addressing: a broken settings menu, low-contrast pending text, poor phonetic word quality, a bare-bones stats dashboard, an undersized keyboard visualization, and hardcoded passage/code content.

---

## Issue 1: Settings Menu Not Working

**Root cause**: In `main.rs:handle_menu_key`, the `Enter` match handles `0..=3` but Settings is menu item index `4` — it falls through to `_ => {}`. Also no `KeyCode::Char('c')` shortcut handler exists.

**Fix** (`src/main.rs:124-158`):
- Add `4 => app.screen = AppScreen::Settings` in the Enter match arm
- Add `KeyCode::Char('c') => app.screen = AppScreen::Settings` handler

**Additionally**, make the Settings screen functional instead of a stub:
- Make it an interactive form with arrow keys to select fields, Enter to cycle values
- Fields: Target WPM (adjustable ±5), Theme (cycle through available), Word Count, Code Languages
- Save config on ESC via existing `Config::save()`
- New file: no new files needed; extend `render_settings` in `main.rs` and add `handle_settings_key` logic
- Add `settings_selected: usize` and `settings_editing: bool` fields to `App`

---

## Issue 2: Low Contrast Pending Text

**Root cause**: `text_pending` = `#585b70` (Catppuccin overlay0) on bg `#1e1e2e` is too dim for readable upcoming text.

**Fix**: Change `text_pending` in the default theme and all bundled theme files:
- `src/ui/theme.rs:92` default: `#585b70` → `#a6adc8` (Catppuccin subtext0, much brighter)
- Update all 8 theme TOML files in `assets/themes/` with appropriate brighter pending text colors for each theme

---

## Issue 3: Better Phonetic Word Generation

**Root cause**: Our current approach uses a hand-built order-2 trigram table from ~50 English patterns. keybr.com uses:
1. **Order-4 Markov chain** trained on top 10,000 words from a real frequency dictionary
2. **Pre-built binary model** (~47KB for English)
3. **Real word dictionary** — the `naturalWords` mode (keybr's default) primarily uses real English words filtered by unlocked letters, falling back to phonetic pseudo-words only when <15 words match

**Implementation plan**:

### Step A: Build a proper transition table from a word frequency list
- Create `tools/build_model.rs` (a build-time binary) that:
  1. Reads an English word frequency list (we'll embed a curated 10K-word list as `assets/wordfreq-en.csv`)
  2. Uses order-4 chain (matching keybr)
  3. Appends each word weighted by frequency (like keybr's `builder.append()` loop)
  4. Outputs binary `.data` file matching keybr's format
- **OR simpler approach**: Embed the word list directly and build the table at startup (it's fast enough)

### Step B: Upgrade TransitionTable to order-4
- Modify `TransitionTable` to support variable-order chains
- Change the key from `(char, char)` → a `Vec<char>` prefix of length `order - 1`
- Implement `segment(prefix: &[char])` matching keybr's approach

### Step C: Add a word dictionary for "natural words" mode
- Create `src/generator/dictionary.rs` with a `Dictionary` struct
- Embed a 10K English word list (JSON or plain text) via rust-embed
- `Dictionary::find(filter: &CharFilter, focused: Option<char>) -> Vec<&str>` returns real words where all characters are in the allowed set
- If focused letter exists, prefer words containing it

### Step D: Update PhoneticGenerator to use combined approach (like keybr's GuidedLesson)
- When `naturalWords` is enabled (default):
  1. Get real words matching the filter from Dictionary
  2. If >= 15 real words available, randomly pick from them
  3. Otherwise, supplement with phonetic pseudo-words from the Markov chain
- This is what makes keybr's output "feel like real words" — because they mostly ARE real words

**Key files to modify**:
- `src/generator/transition_table.rs` — upgrade to order-4
- `src/generator/phonetic.rs` — update word generation loop
- New: `src/generator/dictionary.rs` — real word dictionary
- New: `assets/words-en.json` — embedded 10K word list (we can extract from keybr's `clones/keybr.com/packages/keybr-content-words/lib/data/words-en.json`)
- `src/app.rs` — wire up dictionary

---

## Issue 4: Comprehensive Statistics Dashboard

**Current state**: Single screen with 4 summary numbers and 1 unlabeled WPM line chart.

**Target** (inspired by typr's three-tab layout):

### Tab navigation
- Add tab state to the stats dashboard: `Dashboard | History | Keystrokes`
- Keyboard: `D`, `H`, `K` to switch tabs, or `Tab` to cycle
- Render tabs as a header row with active tab highlighted

### Dashboard Tab
1. **Summary stats row**: Total lessons, Avg WPM, Best WPM, Avg Accuracy, Total time, Streak
2. **Progress bars** (3 columns): WPM vs goal, Accuracy vs 100%, Level progress
3. **WPM over time chart** (line chart, last 50 lessons) — already exists, add axis labels
4. **Accuracy over time chart** (line chart, last 50 lessons) — new chart

### History Tab
1. **Recent tests table**: Last 20 lessons with columns: #, WPM, Raw WPM, Accuracy, Time, Date
2. **Per-key average speed chart**: Bar chart of all 26 letters by avg typing time

### Keystrokes Tab
1. **Keyboard accuracy heatmap**: Render keyboard layout with per-key accuracy coloring (green=100%, yellow=90-100%, red=<90%)
2. **Slowest/Fastest keys tables**: Top 5 each with average time in ms
3. **Word/Character stats**: Total correct/wrong counts

**Key files to modify/create**:
- `src/ui/components/stats_dashboard.rs` — complete rewrite with tabs
- `src/ui/components/chart.rs` — add AccuracyChart, BarChart widgets
- New: `src/ui/components/keyboard_heatmap.rs` — per-key accuracy visualization
- `src/engine/key_stats.rs` — ensure per-key accuracy tracking exists (not just timing)
- `src/session/result.rs` — ensure per-key accuracy data is persisted
- `src/store/schema.rs` — may need to add per-key accuracy to KeyStatsData

---

## Issue 5: Keyboard Visualization Too Small

**Current state**: Keyboard diagram IS rendered in `render_lesson` (`main.rs:330-335`) but given only `Constraint::Length(4)` — with borders that's 2 inner rows, but QWERTY needs 3 rows.

**Fix**:
- Change keyboard constraint from `Length(4)` to `Length(5)` in `main.rs:316`
- Improve the keyboard rendering in `keyboard_diagram.rs`:
  - Use wider keys (5 chars instead of 4) for readability
  - Add finger-color coding (reuse existing `keyboard/finger.rs`)
  - Show the next key to type highlighted (pass current target char)
  - Improve spacing/centering

**Files**: `src/main.rs:311-318`, `src/ui/components/keyboard_diagram.rs`

---

## Issue 6: Embedded + Internet Content (Both Approaches)

### Embedded Baseline (always available, no network)
- Bundle ~50 passages from public domain literature directly in binary (via rust-embed)
- Bundle ~100 code snippets per language (Rust, Python, JS, Go) in embedded assets
- These replace the current ~15 hardcoded passages and ~12 code snippets per language

### Internet Fetching (on top of embedded, with caching)

**Passages: Project Gutenberg**
- Fetch from `https://www.gutenberg.org/cache/epub/{id}/pg{id}.txt`
- Curate ~20 popular book IDs (Pride and Prejudice, Alice in Wonderland, etc.)
- Extract random paragraphs (skip Gutenberg header/footer boilerplate)
- Cache fetched books to `~/.local/share/keydr/passages/`
- Gracefully fall back to embedded passages on network failure

**Code: GitHub Raw Files**
- Fetch raw files from curated popular repos (e.g., `tokio-rs/tokio`, `python/cpython`)
- Use direct raw.githubusercontent.com URLs for specific files (no API auth needed)
- Extract function-length snippets (20-50 lines)
- Cache to `~/.local/share/keydr/code_cache/`
- Gracefully fall back to embedded snippets on failure

**New dependency**: `reqwest = { version = "0.12", features = ["json", "blocking"] }`

**Files to modify**:
- `src/generator/passage.rs` — expand embedded + add Gutenberg fetching
- `src/generator/code_syntax.rs` — expand embedded + add GitHub fetching
- New: `src/generator/cache.rs` — shared disk caching logic
- New: `assets/passages/*.txt` — embedded passage files
- New: `assets/code/*.rs`, `*.py`, etc. — embedded code snippet files
- `Cargo.toml` — add reqwest dependency

---

## Implementation Order

1. **Issue 1** (Settings menu fix) — quick fix, unblocks testing
2. **Issue 2** (Text contrast) — quick theme change
3. **Issue 5** (Keyboard size) — quick layout fix
4. **Issue 3** (Word generation) — medium complexity, core improvement
5. **Issue 4** (Stats dashboard) — large UI rewrite
6. **Issue 6** (Internet content) — medium complexity, requires new dependency

---

## Verification

1. `cargo build` — compiles without errors
2. `cargo test` — all tests pass
3. Manual testing for each issue:
   - Settings: navigate to Settings in menu, change target WPM, verify it saves/loads
   - Contrast: verify pending text is readable in the typing area
   - Keyboard: verify all 3 QWERTY rows visible during lesson
   - Words: start adaptive mode, verify words look like real English
   - Stats: complete 2-3 lessons, check all three stats tabs render correctly
   - Passages: start passage mode, verify it fetches new content (with network), and falls back gracefully (without)
