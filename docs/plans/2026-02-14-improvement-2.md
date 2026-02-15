# Keydr Improvement Plan

## Context

The keydr typing tutor app needs six improvements to bring it closer to the quality of keybr.com and typr. Currently the app starts at a menu screen, doesn't properly count corrected errors, has a confusing keyboard visualization, lacks responsive layout, can't delete sessions, and has basic statistics views.

---

## 1. Start in Adaptive Drill by Default

**Files:** `src/app.rs`

**Implementation:** Change `App::new()` to use a `let mut app = Self { ... }; app.start_lesson(); app` pattern. The struct literal currently at `src/app.rs:99-120` sets `screen: AppScreen::Menu` — change this to construct `Self`, then call `start_lesson()` which sets `screen = AppScreen::Lesson` and generates text. The menu remains accessible via ESC from lesson/result screens (unchanged).

---

## 2. Fix Error Tracking for Backspaced Corrections

**Files:** `src/session/lesson.rs`, `src/session/input.rs`, `src/session/result.rs`, `src/ui/components/stats_sidebar.rs`

**Problem:** When a user types wrong, backspaces, then types correctly, keydr pops `CharStatus::Incorrect` from the input vector and replaces with `CharStatus::Correct`. Final accuracy shows 0 errors. keybr.com counts this as an error (see `packages/keybr-textinput/lib/textinput.ts` — `typo` flag persists through backspace corrections, and `stats.ts:42-49` counts all steps with `typo: true`).

**Implementation — keybr-style step-based tracking:**

### Two separate tracking systems:

**A. Live display counters (existing, unchanged):**
- `input: Vec<CharStatus>` continues to track current visible state (grows on type, shrinks on backspace)
- `incorrect_count()` and `correct_count()` show current snapshot for the sidebar display
- `accuracy()` on `LessonState` continues using `input.len()` as denominator — only reflects currently-visible chars

**B. Persistent typo tracking (new, for final results):**
- Add `typo_flags: HashSet<usize>` to `LessonState` — tracks positions where ANY incorrect key was ever pressed

**Process flow:**
1. `process_char()` — when `!correct`: insert `lesson.cursor` into `typo_flags`. Push to `input` as before.
2. `process_backspace()` — pop from `input`, decrement cursor. Do NOT remove from `typo_flags`.
3. When the lesson completes (all positions filled with correct/incorrect chars), `LessonResult::from_lesson()` builds the final result using `typo_flags` to determine error count:
   - `incorrect = typo_flags.len()` (positions where any error ever occurred)
   - `accuracy = (total_chars - typo_flags.len()) / total_chars * 100`
   - This avoids the denominator mismatch since we always use `target.len()` as the denominator

**Sidebar display during lesson:**
- Show "Errors: X" using `typo_flags.len()` (accumulated errors, never decreases)
- Live accuracy: count `typo_flags` entries that are `< cursor` (i.e., only count typos at positions already typed past), then: `((cursor - typos_before_cursor).max(0) as f64 / cursor as f64 * 100.0).clamp(0.0, 100.0)` where cursor > 0. This handles the backspace case correctly — if cursor retreats behind a typo'd position, that typo doesn't count in the live denominator.

### Unit tests:
- Type "abc" correctly → typo_flags empty, accuracy 100%
- Type wrong char at pos 0, backspace, type correct → typo_flags = {0}, accuracy < 100%
- Type wrong char, continue without backspace → typo_flags = {pos}, also in input as Incorrect
- Multiple errors at same position (wrong, backspace, wrong again, backspace, correct) → typo_flags = {pos}, counts as 1 error

---

## 3. Fix Keyboard Visualization

**Files:** `src/ui/components/keyboard_diagram.rs`, `src/main.rs`, `src/app.rs`, `src/event.rs`

**Problem:** All key colors shift constantly with no meaning. User expects pressed keys to light up.

**How keybr.com does it:**
- Uses physical key codes (W3C `event.code` like `KeyA`, `KeyQ`) for tracking depressed keys
- `Controller.tsx:99-107`: `onKeyDown` adds to `depressedKeys`, `onKeyUp` removes
- `KeyboardPresenter.tsx:36-39`: passes `depressedKeys` array and `suffixKeys` (next expected) to keyboard UI
- `KeyLayer.tsx`: pre-computes 8 states per key (depressed × toggled × showColors), selects based on current state

**Implementation — crossterm supports key Press/Release events:**

**Scope decision:** We track depressed state for **printable character keys only** (`KeyCode::Char(ch)`). This is intentional non-parity with keybr.com's physical-key-ID model — keybr runs in a browser with W3C key codes, but keydr's keyboard diagram only shows letter keys. Modifier keys (Shift, Ctrl, Alt) are not shown on the diagram and don't need depressed tracking. Characters are lowercased for matching against the diagram.

crossterm 0.28 provides `KeyEventKind::Press`, `KeyEventKind::Release`, and `KeyEventKind::Repeat` via `KeyEvent.kind`. However, terminal key-release support is inconsistent across terminals. We use a **hybrid approach**: track via Release events when available, with a 150ms timed fallback.

1. **Enable enhanced key events** (`src/main.rs`):
   - Call `crossterm::event::PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)` on startup (enables Release events on supported terminals)
   - Pop the flags on cleanup

2. **Track depressed keys** (`src/app.rs`):
   - Add `depressed_keys: HashSet<char>` field (stores lowercase chars)
   - Add `last_key_time: Option<Instant>` for fallback clearing
   - On `KeyEventKind::Press` with `KeyCode::Char(ch)`: insert `ch.to_ascii_lowercase()` into `depressed_keys`, set `last_key_time`
   - On `KeyEventKind::Release` with `KeyCode::Char(ch)`: remove `ch.to_ascii_lowercase()` from `depressed_keys`
   - On tick: if `last_key_time` > 150ms ago and no Release was received, clear `depressed_keys` (fallback for terminals without Release support)

3. **Update event handling** (`src/main.rs` `handle_key`):
   - Check `key.kind` — only process typing logic on `KeyEventKind::Press`
   - On `KeyEventKind::Release`: call `app.depressed_keys.remove(&ch.to_ascii_lowercase())`
   - Filter out `KeyEventKind::Repeat` to avoid double-counting (or treat same as Press for depressed tracking)

4. **Update KeyboardDiagram** (`src/ui/components/keyboard_diagram.rs`):
   - Accept `depressed_keys: &HashSet<char>` (all lowercase)
   - Rendering priority order: **depressed** (bright/inverted style) > **next_expected** (accent bg) > **focused** (yellow bg) > **unlocked** (finger zone color) > **locked** (dim)
   - Depressed style: bold white text on brighter version of the finger color

5. **Investigate "constantly shifting colors" bug:**
   - Current code at `main.rs:356-359` passes `lesson.target.get(lesson.cursor)` as `next_char` — this correctly changes on each keystroke
   - Verify the finger_color mapping is stable (it uses static match arms — should be fine)
   - Most likely the "shifting" perception is the `next_key` highlight moving to adjacent keys as user types — this is correct behavior. The depressed-key highlight will make the interaction much clearer.

### Unit tests:
- Verify `depressed_keys` set grows on Press and shrinks on Release
- Verify fallback clearing works after 150ms timeout

---

## 4. Responsive UI for Small Terminals

**Files:** `src/ui/layout.rs`, `src/main.rs`, `src/ui/components/keyboard_diagram.rs`, `src/ui/components/stats_sidebar.rs`, `src/ui/components/typing_area.rs`

**How typr handles it (from `clones/typr/lua/typr/stats/init.lua:15-17`):**
- Base width: `state.w = 80` columns
- Responsive threshold: `vim.o.columns > ((2 * state.w) + 10)` = `> 170` cols → horizontal stats layout
- Below 170 cols → vertical tabbed stats layout
- Drill view: fixed 80-col centered window, doesn't have a sidebar concept
- Window height adapts: `large_screen and state.h or vim.o.lines - 7`

**Implementation — tiered layout for keydr:**

### Drill View Layout Tiers (based on `area` from `AppLayout::new()`):

**Wide (≥100 cols):** Current layout — typing area (70%) + sidebar (30%) side-by-side, keyboard + progress bar below typing area

**Medium (60-99 cols):**
- Typing area takes full width (no sidebar)
- Compact stats in header bar: `WPM: XX | Acc: XX% | Errors: X`
- Keyboard diagram below typing area (compressed 3-char keys `[x]` instead of `[ x ]`)
- Progress bar below keyboard

**Narrow (<60 cols):**
- Typing area full width
- Stats in header bar only
- No keyboard diagram
- No progress bar

**Short (<20 rows):**
- No keyboard diagram (regardless of width)
- No progress bar
- Typing area + single-line header + single-line footer

### Stats View Layout Tiers:

**Wide (>170 cols):** Side-by-side panels (matching typr threshold: `(2 * 80) + 10`)
**Normal (≤170 cols):** Tabbed view (current behavior, improved styling per item 6)

### Implementation:
1. Modify `AppLayout::new()` to accept area and return different constraint sets based on dimensions
2. Add `LayoutTier` enum: `{ Wide, Medium, Narrow }` computed from `area.width` and `area.height`
3. `render_lesson()` checks tier to decide which components to render
4. `KeyboardDiagram` gets a `compact: bool` flag for 3-char key mode
5. Verify `TypingArea` wraps properly at narrow widths (current implementation should handle this via Ratatui's `Paragraph` wrapping)

---

## 5. Delete Sessions from History

**Files:** `src/app.rs`, `src/main.rs`, `src/ui/components/stats_dashboard.rs`

**Implementation — complete recalculation scope:**

### State machine for history tab interaction:

```
Normal browsing → [j/k/Up/Down] → Move selection cursor
Normal browsing → [x/Delete] → Show confirmation dialog
Confirmation dialog → [y] → Delete session, recalculate, return to Normal
Confirmation dialog → [n/ESC] → Cancel, return to Normal
Normal browsing → [Tab/d/h/k/1/2/3] → Switch tabs (existing behavior)
```

### App state additions (`src/app.rs`):
- `history_selected: usize` — selected row index in history view (0 = most recent)
- `history_confirm_delete: bool` — whether confirmation dialog is showing

### Key bindings — full precedence table for `handle_stats_key`:

**When `history_confirm_delete == true` (confirmation dialog active):**
- `y` → call `delete_session()`, set `history_confirm_delete = false`
- `n` / `ESC` → set `history_confirm_delete = false` (cancel)
- All other keys ignored

**When `stats_tab == 1` (history tab, no dialog):**
- `j` / `Down` → increment `history_selected` (clamp to history length)
- `k` / `Up` → decrement `history_selected` (clamp to 0)
- `x` / `Delete` → set `history_confirm_delete = true`
- `d` / `1` → switch to Dashboard tab (`stats_tab = 0`)
- `h` / `2` → switch to History tab (no-op, already there)
- `3` → switch to Keystrokes tab (`stats_tab = 2`). Note: `k` is NOT a Keystrokes tab shortcut when on history tab — it navigates rows instead.
- `Tab` / `BackTab` → cycle tabs
- `ESC` / `q` → back to menu

**When on other tabs (stats_tab == 0 or 2):**
- Existing behavior unchanged: `d`/`1`, `h`/`2`, `k`/`3` switch tabs, `Tab`/`BackTab` cycle, `ESC`/`q` back to menu

### Delete logic (`src/app.rs` `delete_session()`):

Full recalculation via **chronological replay** to make it "as if the session never happened":

1. **Remove the lesson** from `self.lesson_history` at the correct index (history tab shows reverse order, so actual index = `len - 1 - history_selected`)

2. **Chronological state replay** — reset and rebuild from scratch, oldest→newest:
   ```
   // Reset all derived state
   self.key_stats = KeyStatsStore::default();
   self.key_stats.target_cpm = self.config.target_cpm();
   self.letter_unlock = LetterUnlock::new();
   self.profile.total_score = 0.0;
   self.profile.total_lessons = 0;
   self.profile.streak_days = 0;
   self.profile.best_streak = 0;
   self.profile.last_practice_date = None;

   // Replay each remaining session oldest→newest
   for result in &self.lesson_history {
       // Update key stats (same as finish_lesson does)
       for kt in &result.per_key_times {
           if kt.correct {
               self.key_stats.update_key(kt.key, kt.time_ms);
           }
       }

       // Update letter unlock
       self.letter_unlock.update(&self.key_stats);

       // Compute score using current unlock state (matches runtime)
       let complexity = compute_complexity(self.letter_unlock.unlocked_count());
       let score = compute_score(result, complexity);
       self.profile.total_score += score;
       self.profile.total_lessons += 1;

       // Rebuild streak tracking (same logic as finish_lesson)
       let day = result.timestamp.format("%Y-%m-%d").to_string();
       // ... streak logic identical to App::finish_lesson
   }

   self.profile.unlocked_letters = self.letter_unlock.included.clone();
   ```
   This exactly reproduces the runtime scoring path (`src/app.rs:186-218`, `src/engine/scoring.rs:3-7`), including complexity that depends on unlock state at each point in progression.

3. **Persist:** Call `self.save_data()` to write all three files (profile, key_stats, lesson_history)

4. **Adjust selection:** Clamp `history_selected` to new valid range

**Implementation note:** Extract the replay logic into a reusable `rebuild_from_history(&mut self)` method on `App`, since it could also be useful for data recovery.

### Rendering (`stats_dashboard.rs`):
- Selected row gets `bg(colors.accent_dim())` highlight background (existing theme color `accent_dim` = `#45475a`, a subtle dark surface color)
- Confirmation dialog: centered overlay box with border: `"Delete session #X? (y/n)"`

### Unit tests:
- Delete last session → history shrinks by 1, total_lessons decremented
- Delete session → key_stats rebuilt without that session's key times
- Delete all sessions → profile reset to defaults, key_stats empty
- Delete session with only practice day → streak recalculated correctly

---

## 6. Improved Statistics Display (Full Typr-Style Overhaul)

**Files:** `src/ui/components/stats_dashboard.rs`, new `src/ui/components/activity_heatmap.rs`

**Data sources (all derivable from existing persisted data):**
- `lesson_history: Vec<LessonResult>` — has `wpm`, `cpm`, `accuracy`, `correct`, `incorrect`, `total_chars`, `elapsed_secs`, `timestamp`, `per_key_times`
- `key_stats: KeyStatsStore` — has per-key `filtered_time_ms`, `best_time_ms`, `confidence`, `sample_count`, `recent_times`
- No schema migration needed — all new visualizations derive from existing fields

### Dashboard Tab Improvements:

**Summary stats as bordered table:**
```
┌─────────────────────────────────────────────┐
│  Lessons: 42    Avg WPM: 65    Best WPM: 82 │
│  Accuracy: 94.2%    Total time: 2h 15m      │
└─────────────────────────────────────────────┘
```

**Progress bars** using `┃` filled / dim `┃` empty:
- WPM progress: `avg_wpm / target_wpm` (green ≥ goal, accent < goal)
- Accuracy progress: (green ≥ 95%, yellow ≥ 85%, red < 85%)
- Level progress to next level

**WPM bar graph** (last 20 sessions) using `▁▂▃▄▅▆▇█` block characters, replacing the Braille line chart. Color-coded: green above goal, red below.

**Keep accuracy trend chart** (Braille line chart works well for this).

### History Tab Improvements:

**Bordered table:**
```
┌────┬──────┬──────┬───────┬───────┬────────────┐
│  # │  WPM │  Raw │  Acc% │  Time │ Date       │
├────┼──────┼──────┼───────┼───────┼────────────┤
│ 42 │   68 │   72 │ 96.2% │ 45.2s │ 02/14 10:30│
│ 41 │   63 │   67 │ 93.1% │ 52.1s │ 02/14 09:15│
└────┴──────┴──────┴───────┴───────┴────────────┘
```

- Selected row highlighted with distinct background
- WPM goal indicator per row: small inline bar or color indicator

**Character speed distribution** (below table): dot/bar graph of all 26 letters (from typr's history view), using per-key `filtered_time_ms` data already available in `key_stats`.

### Keystrokes Tab Improvements:

**Activity heatmap** (new widget in `src/ui/components/activity_heatmap.rs`):
- 7-month calendar grid grouped by week
- Each day cell: `▪` or `█` colored by session count (0 = dim, 1-5 = light green, 6-15 = medium, 16+ = bright)
- Data source: group `lesson_history` by `timestamp.date()`, count per day
- Month labels along top, day-of-week labels on left (M/W/F or all 7)
- Toggle between first/last 6 months (optional, if space allows)

**Key accuracy heatmap:** show accuracy percentage text on each key, not just color. E.g., `[a 97%]` or use color intensity.

**Top 3 worst keys:** highlighted badges showing the keys with lowest accuracy, matching typr's approach.

**Char times analysis:** Slowest 5 / Fastest 5 keys with times (already exists, clean up formatting with box borders).

### Shared visual improvements:
- Unicode box-drawing borders (`┌─┬─┐`, `│`, `└─┴─┘`) via Ratatui's `Block::bordered()` with custom border set
- Bar graphs using `▁▂▃▄▅▆▇█` block characters
- Consistent 2-char padding inside bordered sections
- Color gradients for intensity (heatmap, speed distribution)

---

## File Summary

| File | Changes |
|------|---------|
| `src/app.rs` | Start in lesson mode, add `depressed_keys: HashSet<char>`, `last_key_time`, `history_selected`, `history_confirm_delete`, `delete_session()` with chronological replay via `rebuild_from_history()` |
| `src/main.rs` | Enable keyboard enhancement flags, handle Press/Release events, update `render_lesson` for responsive tiers, update `handle_stats_key` for history selection/deletion state machine |
| `src/event.rs` | Filter key events by kind (pass all events, let main.rs handle kind) |
| `src/session/input.rs` | Add `typo_flags` tracking — insert on incorrect, preserve through backspace |
| `src/session/lesson.rs` | Add `typo_flags: HashSet<usize>`, `typo_count()` method. Keep `accuracy()`/`incorrect_count()` for live display. |
| `src/session/result.rs` | Use `typo_flags.len()` for final `incorrect` count and accuracy |
| `src/ui/layout.rs` | Add `LayoutTier` enum, compute from area dimensions, return different constraint sets |
| `src/ui/components/keyboard_diagram.rs` | Accept `depressed_keys: &HashSet<char>`, render depressed state, add compact mode |
| `src/ui/components/stats_dashboard.rs` | Full overhaul: bordered tables, bar graphs, progress bars, row selection, delete confirmation overlay, character speed distribution |
| `src/ui/components/activity_heatmap.rs` | New: 7-month activity calendar heatmap widget |
| `src/ui/components/stats_sidebar.rs` | Compact single-line mode for medium terminals |
| `src/ui/components/typing_area.rs` | Verify wrapping at narrow widths |

---

## Verification

### Manual Testing:
1. **Start in drill:** Launch app → immediately in Adaptive typing lesson, no menu
2. **Error tracking:** Type wrong char, backspace, type correct char → accuracy < 100%, error count ≥ 1. Type wrong at same pos twice, backspace twice, type correct → still only 1 error for that position.
3. **Keyboard:** Type characters → pressed key visually highlights. Next expected key highlighted. Releasing key clears highlight (or after 150ms fallback).
4. **Responsive:** Resize terminal to 50×15, 80×25, 120×40, 200×50 → layout adapts, no panics, no overlapping text
5. **Delete sessions:** Stats → History → select row → press `x` → confirm dialog → press `y` → session gone, all stats recalculated. Verify key_stats and letter_unlock are consistent.
6. **Statistics:** Visual inspection of bordered tables, bar graphs, activity heatmap, progress bars

### Automated Tests:
- `session/lesson.rs`: typo_flags behavior (wrong→backspace→correct counts as error, multiple errors at same pos = 1 typo)
- `session/input.rs`: process_char sets typo_flags, process_backspace preserves them
- `app.rs`: delete_session recalculates total_lessons, total_score, key_stats, letter_unlock, streak fields
- `engine/key_stats.rs`: verify rebuild from scratch produces same results as incremental updates (within EMA tolerance)
