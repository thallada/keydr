# keydr - Terminal Typing Tutor Architecture Plan

## Context

**Problem**: No terminal-based typing tutor exists that combines keybr.com's adaptive learning algorithm (gradual letter unlocking, per-key confidence tracking, phonetic pseudo-word generation) with code syntax training. Existing tools either lack adaptive learning entirely (ttyper, smassh, typr) or have incomplete implementations (gokeybr intentionally ignores error stats, ivan-volnov/keybr is focused on Anki integration).

**Goal**: Build a full-featured Rust TUI typing tutor that clones keybr.com's core algorithm, extends it to code syntax training, and provides a polished statistics dashboard - all in the terminal.

---

## Research Summary

### keybr.com Algorithm (from reading source: `packages/keybr-lesson/lib/guided.ts`, `keybr-phonetic-model/lib/phoneticmodel.ts`, `keybr-result/lib/keystats.ts`)

**Letter Unlocking**: Letters sorted by frequency. Starts with minimum 6. New letter unlocked only when ALL included keys have `confidence >= 1.0`. Weakest key (lowest confidence) gets "focused" - drills bias heavily toward it.

**Confidence Model**: `confidence = target_time_ms / filtered_time_to_type`, where `target_time_ms = 60000 / target_speed_cpm` (default target: 175 CPM ~ 35 WPM). `filtered_time_to_type` is an exponential moving average (alpha=0.1) of raw per-key typing times.

**Phonetic Word Generation**: Markov chain transition table maps character bigrams to next-character probability distributions. Chain is walked with a `Filter` that restricts to unlocked characters only. Focused letter gets prefix biasing - the generator searches for chain states containing the focused letter and starts from there. Words are 3-10 chars; space probability boosted by `1.3^word_length` to keep words short.

**Scoring**: `score = (speed_cpm * complexity) / (errors + 1) * (length / 50)`

**Learning Rate**: Polynomial regression (degree 1-3 based on sample count) on last 30 per-key time samples, with R^2 threshold of 0.5 for meaningful predictions.

### Key Insights from Prior Art

- **gokeybr**: Trigram-based scoring with `frequency * effort(speed)` is a good complementary approach. Its Bellman-Ford shortest-path for drill generation is clever but complex.
- **ttyper**: Clean Rust/Ratatui architecture to reference. Uses `crossterm` events, `State::Test | State::Results` enum, `Config` from TOML. Dependencies: `ratatui ^0.25`, `crossterm ^0.27`, `clap`, `serde`, `toml`, `rand`, `rust-embed`.
- **keybr-code**: Uses PEG grammars to generate code snippets for 12+ languages. Each grammar produces realistic syntax patterns.

---

## Architecture

### Technology Stack
- **TUI**: Ratatui + Crossterm (the standard Rust TUI stack, battle-tested by ttyper and many others)
- **CLI**: Clap (derive)
- **Serialization**: Serde + serde_json + toml
- **HTTP**: Reqwest (blocking, for GitHub API)
- **Persistence**: JSON files via `dirs` crate (XDG paths)
- **Embedded Assets**: rust-embed
- **Error Handling**: anyhow + thiserror
- **Time**: chrono

### Project Structure

```
src/
  main.rs                        # CLI parsing, terminal init, main event loop
  app.rs                         # App state machine (TEA pattern), message dispatch
  event.rs                       # Crossterm event polling thread -> AppMessage channel
  config.rs                      # Config loading (~/.config/keydr/config.toml)

  engine/
    mod.rs
    letter_unlock.rs             # Letter ordering, unlock logic, focus selection
    key_stats.rs                 # Per-key EMA, confidence, best-time tracking
    scoring.rs                   # Lesson score formula, gamification (levels, streaks)
    learning_rate.rs             # Polynomial regression for speed prediction
    filter.rs                    # Active character set filter

  generator/
    mod.rs                       # TextGenerator trait
    phonetic.rs                  # Markov chain pseudo-word generator
    transition_table.rs          # Binary transition table (de)serialization
    code_syntax.rs               # PEG grammar interpreter for code snippets
    passage.rs                   # Book passage loading
    github_code.rs               # GitHub API code fetching + caching

  session/
    mod.rs
    lesson.rs                    # LessonState: target text, cursor, timing
    input.rs                     # Keystroke processing, match/mismatch, backspace
    result.rs                    # LessonResult computation from raw events

  store/
    mod.rs                       # StorageBackend trait
    json_store.rs                # JSON file persistence with atomic writes
    schema.rs                    # Serializable data models

  ui/
    mod.rs                       # Root render dispatcher
    theme.rs                     # Theme TOML parsing, color resolution
    layout.rs                    # Responsive screen layout (ratatui Rect splitting)
    components/
      mod.rs
      typing_area.rs             # Main typing widget (correct/incorrect/pending coloring)
      stats_sidebar.rs           # Live WPM, accuracy, key confidence bars
      keyboard_diagram.rs        # Visual keyboard with finger colors + focus highlight
      progress_bar.rs            # Letter unlock progress
      chart.rs                   # WPM-over-time line charts (ratatui Chart widget)
      menu.rs                    # Mode selection menu
      dashboard.rs               # Post-lesson results view
      stats_dashboard.rs         # Historical statistics with graphs

  keyboard/
    mod.rs
    layout.rs                    # KeyboardLayout, key positions, finger assignments
    finger.rs                    # Finger enum, hand assignment

assets/
  models/en.bin                  # Pre-built English phonetic transition table
  themes/*.toml                  # Built-in themes (catppuccin, dracula, gruvbox, nord, etc.)
  grammars/*.toml                # Code syntax grammars (rust, python, js, go, etc.)
  layouts/*.toml                 # Keyboard layouts (qwerty, dvorak, colemak)
```

### Core Data Flow

```
                    ┌─────────────┐
                    │  Event Loop │
                    └──────┬──────┘
                           │ AppMessage
                           ▼
┌──────────┐     ┌─────────────────┐     ┌───────────┐
│Generator │────▶│    App State    │────▶│  UI Layer  │
│(phonetic,│     │  (TEA pattern)  │     │ (ratatui)  │
│ code,    │     │                 │     │            │
│ passage) │     │ ┌─────────────┐ │     └───────────┘
└──────────┘     │ │   Engine    │ │
                 │ │ (key_stats, │ │     ┌───────────┐
                 │ │  unlock,    │ │────▶│   Store   │
                 │ │  scoring)   │ │     │ (JSON)    │
                 │ └─────────────┘ │     └───────────┘
                 └─────────────────┘
```

### App State Machine

```
Start → Menu
Menu → Lesson (on mode select)
Menu → StatsDashboard (on 's')
Menu → Settings (on 'c')
Lesson → LessonResult (on completion or ESC)
LessonResult → Lesson (on 'r' retry)
LessonResult → Menu (on 'q'/ESC)
LessonResult → StatsDashboard (on 's')
StatsDashboard → Menu (on ESC)
Settings → Menu (on ESC, saves config)
Any → Quit (on Ctrl+C)
```

### The Adaptive Algorithm

**Step 1 - Letter Order**: English frequency order: `e t a o i n s h r d l c u m w f g y p b v k j x q z`

**Step 2 - Unlock Logic** (after each lesson):
```
min_letters = 6
for each letter in frequency_order:
    if included.len() < min_letters:
        include(letter)
    elif all included keys have confidence >= 1.0:
        include(letter)
    else:
        break
```

**Step 3 - Focus Selection**:
```
focused = included_keys
    .filter(|k| k.confidence < 1.0)
    .min_by(|a, b| a.confidence.cmp(&b.confidence))
```

**Step 4 - Stats Update** (per key, after each lesson):
```
alpha = 0.1
stat.filtered_time = alpha * new_time + (1 - alpha) * stat.filtered_time
stat.best_time = min(stat.best_time, stat.filtered_time)
stat.confidence = (60000.0 / target_speed_cpm) / stat.filtered_time
```

**Step 5 - Text Generation Biasing**:
- Only allow characters in the unlocked set (Filter)
- When a focused letter exists, find Markov chain prefixes containing it and start words from those prefixes
- This naturally creates words heavy in the weak letter

### Code Syntax Extension

After all 26 prose letters are unlocked, the system transitions to code syntax training:
- Introduces code-relevant characters: `{ } [ ] ( ) < > ; : . , = + - * / & | ! ? _ " ' # @ \ ~ ^ %`
- Uses PEG grammars per language to generate realistic code snippets
- Gradual character unlocking continues for syntax characters
- Users select their target programming languages in config

### Theme System

Themes are TOML files with semantic color names:
```toml
[colors]
bg = "#1e1e2e"
text_correct = "#a6e3a1"
text_incorrect = "#f38ba8"
text_pending = "#585b70"
text_cursor_bg = "#f5e0dc"
focused_key = "#f9e2af"
# ... etc
```

Resolution order: CLI flag → config → user themes dir → bundled → default fallback.

Built-in themes: Catppuccin Mocha, Catppuccin Latte, Dracula, Gruvbox Dark, Nord, Tokyo Night, Solarized Dark, One Dark, plus an ANSI-safe default.

### Persistence

JSON files in `~/.local/share/keydr/`:
- `key_stats.json` - Per-key EMA, confidence, sample history
- `lesson_history.json` - Last 500 lesson results
- `profile.json` - Unlock state, settings, gamification data

Atomic writes (temp file → fsync → rename) to prevent corruption. Schema version field for forward-compatible migrations.

---

## Implementation Phases

### Phase 1: Foundation (Core Loop + Basic Typing)
Create the terminal init/restore with crossterm, event polling thread, TEA-based App state machine, basic typing against a hardcoded word list with correct/incorrect coloring.

**Key files**: `main.rs`, `app.rs`, `event.rs`, `session/lesson.rs`, `session/input.rs`, `ui/components/typing_area.rs`, `ui/layout.rs`

### Phase 2: Adaptive Engine + Statistics
Implement per-key stats (EMA, confidence), letter unlocking, focus selection, scoring, live stats sidebar, and progress bar.

**Key files**: `engine/key_stats.rs`, `engine/letter_unlock.rs`, `engine/scoring.rs`, `engine/filter.rs`, `session/result.rs`, `ui/components/stats_sidebar.rs`, `ui/components/progress_bar.rs`

### Phase 3: Phonetic Text Generation
Build the English transition table (offline tool or build script), implement the Markov chain walker with filter and focus biasing, integrate with the lesson system.

**Key files**: `generator/transition_table.rs`, `generator/phonetic.rs`, `generator/mod.rs`, a `build.rs` or `tools/` script for table generation

### Phase 4: Persistence + Theming
JSON storage backend, atomic writes, config loading, theme parsing, built-in theme files, apply themes throughout all UI components.

**Key files**: `store/json_store.rs`, `store/schema.rs`, `config.rs`, `ui/theme.rs`, `assets/themes/*.toml`

### Phase 5: Results + Dashboard
Post-lesson results screen, historical stats dashboard with charts (ratatui Chart widget), learning rate prediction.

**Key files**: `ui/components/dashboard.rs`, `ui/components/stats_dashboard.rs`, `ui/components/chart.rs`, `engine/learning_rate.rs`

### Phase 6: Code Practice + Passages
PEG grammar interpreter for code syntax generation, book passage mode, GitHub code fetching + caching.

**Key files**: `generator/code_syntax.rs`, `generator/passage.rs`, `generator/github_code.rs`, `assets/grammars/*.toml`

### Phase 7: Keyboard Diagram + Layouts
Visual keyboard widget with finger color coding, multiple layout support (QWERTY, Dvorak, Colemak).

**Key files**: `keyboard/layout.rs`, `keyboard/finger.rs`, `ui/components/keyboard_diagram.rs`, `assets/layouts/*.toml`

### Phase 8: Polish + Gamification
Level system, streaks, badges, CLI completeness, error handling, performance, testing, documentation.

---

## Verification

After each phase, verify by:
1. `cargo build` compiles without errors
2. `cargo test` passes all unit tests
3. Manual testing: launch `cargo run`, exercise the new features, verify UI rendering
4. For Phase 2+: verify letter unlocking by typing accurately and watching new letters appear
5. For Phase 3+: verify generated words only contain unlocked letters and bias toward the focused key
6. For Phase 4+: verify stats persist across app restarts
7. For Phase 5+: verify charts render correctly with historical data

---

## Dependencies (Cargo.toml)

```toml
[dependencies]
ratatui = "0.30"
crossterm = "0.28"
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
rand = { version = "0.8", features = ["small_rng"] }
reqwest = { version = "0.12", features = ["json", "blocking"] }
dirs = "6.0"
rust-embed = "8.5"
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1.0"
thiserror = "2.0"
```
