# Plan: Key Milestone Overlays + Keyboard Diagram Improvements

## Context

The app progressively unlocks keys as users master them via the skill tree system. Currently, when a key is unlocked or mastered, there's no celebratory feedback. This plan adds encouraging milestone overlays with keyboard visualization and finger guidance. It also improves the keyboard diagram to render modifier keys (shift, tab, enter, space, backspace) as interactive keys rather than static labels, and adds a new Keyboard Explorer screen.

## Implementation Phases

This plan is structured into 5 independent phases that can be implemented and validated separately to reduce regression risk.

---

## Phase 0: Key Display Adapter (prerequisite for all phases)

**File: `src/keyboard/display.rs` (new)**

Add a thin adapter module that centralizes all sentinel-char ↔ display-name conversions. This isolates encoding concerns so that UI, stats, and rendering code never directly match on sentinel chars.

```rust
/// Human-readable display name for a key character (including sentinels).
pub fn key_display_name(ch: char) -> &'static str {
    match ch {
        '\x08' => "Backspace",
        '\t' => "Tab",
        '\n' => "Enter",
        ' ' => "Space",
        _ => "", // caller uses ch.to_string() for printable chars
    }
}

/// Short label for compact UI contexts (heatmaps, compact keyboard).
pub fn key_short_label(ch: char) -> &'static str {
    match ch {
        '\x08' => "Bksp",
        '\t' => "Tab",
        '\n' => "Ent",
        ' ' => "Spc",
        _ => "",
    }
}

/// All sentinel chars used for non-printable keys.
pub const MODIFIER_SENTINELS: &[char] = &['\x08', '\t', '\n'];
```

Register in `src/keyboard/mod.rs` with `pub mod display;`.

All subsequent phases use these functions instead of inline sentinel matching. This makes future migration to a typed `KeyId` a single-module change.

**Sentinel boundary policy:** Sentinel chars (`'\x08'`, `'\t'`, `'\n'`) are allowed only at two boundaries:
1. **Input boundary** — `handle_key` in `src/main.rs` converts `KeyCode::Backspace/Tab/Enter` to sentinels for `depressed_keys` and drill input.
2. **Storage boundary** — `KeyStatsStore` and `drill_history` store sentinels as `char` keys.

All UI rendering, stats display, and business logic must consume the adapter functions (`key_display_name`, `key_short_label`, `MODIFIER_SENTINELS`) rather than matching sentinels directly. Add a code comment at the top of `display.rs` documenting this policy.

**Enforcement:** Add a `#[test]` in `display.rs` that runs `grep -rn '\\\\x08\|\\\\t.*=>\|\\\\n.*=>' src/` (or equivalent) and asserts that direct sentinel matches only appear in allowed files (`display.rs`, `main.rs` input handling, `key_stats.rs`). This is a lightweight lint that catches accidental sentinel leakage in UI/business logic during `cargo test` without requiring CI changes.

---

## Phase 1: Keyboard Diagram — Add Missing Keys & Shift Support

### 1a. Track modifier keys as depressed keys

**File: `src/main.rs` — `handle_key` function (~line 155)**

Currently only `KeyCode::Char(ch)` inserts into `depressed_keys`. Add tracking for:
- `KeyCode::Backspace` → insert `'\x08'` into `depressed_keys`
- `KeyCode::Tab` → insert `'\t'`
- `KeyCode::Enter` → insert `'\n'`
- Shift state is already tracked via `app.shift_held`

On `Release` events, remove these sentinels similarly to how `Char` releases work. The tick-based fallback clear (line 134-143) already handles `depressed_keys.clear()` which covers these sentinels too.

### 1b. Render modifier keys in the keyboard diagram

**File: `src/ui/components/keyboard_diagram.rs`**

Currently, modifiers are rendered as plain text labels (lines 253-286). Change them to rendered key boxes that participate in the highlight/depress system:

- **Row 0 (number row):** Add `[Bksp]` key after `=`/`+`. Highlight when `'\x08'` is in `depressed_keys`. Finger color: Right Pinky.
- **Row 1 (top row):** Add `[Tab]` key before `q`. Highlight when `'\t'` is in `depressed_keys`. Finger color: Left Pinky.
- **Row 2 (home row):** Add `[Enter]` key after `'`/`"`. Highlight when `'\n'` is in `depressed_keys`. Finger color: Right Pinky.
- **Row 3 (bottom row):** Add `[Shft]` at start and end. Highlight when `shift_held` is true. Left Shift = Left Pinky finger color, Right Shift = Right Pinky finger color.
- **Row 4 (new row):** Add `[       Space       ]` centered. Highlight when `' '` is in `depressed_keys`. Finger color: Thumb.

Full layout visualization (full mode):
```
[ ` ][ 1 ][ 2 ][ 3 ][ 4 ][ 5 ][ 6 ][ 7 ][ 8 ][ 9 ][ 0 ][ - ][ = ][Bksp]
 [Tab][ q ][ w ][ e ][ r ][ t ][ y ][ u ][ i ][ o ][ p ][ [ ][ ] ][ \ ]
  [   ][ a ][ s ][ d ][ f ][ g ][ h ][ j ][ k ][ l ][ ; ][ ' ][Enter]
  [Shft][ z ][ x ][ c ][ v ][ b ][ n ][ m ][ , ][ . ][ / ][Shft]
                     [       Space       ]
```

Note: Row 2 position before `a` renders as `[   ]` (caps lock, unused).

When `shift_held` is true:
- Shift keys light up with their finger color (brightened)
- All character keys show shifted variants (already implemented via `shift_held` field)

Compact mode: add `[S]` on each side of bottom row, and abbreviated `[T]`, `[E]`, `[B]` for Tab/Enter/Backspace where space permits.

Adaptive breakpoints for overlay/small terminal: if inner height < 6, skip space row; if < 5, use compact mode.

### 1c. Height adjustments

**File: `src/main.rs` — `render_drill` (~line 1011-1019)**

The `kbd_height` calculation needs to increase by 1-2 rows for the space row and modifier keys in full mode. Update:
- Full mode: `kbd_height = 8` (5 rows + 2 border + 1 spacing)
- Compact mode: `kbd_height = 6` (4 rows + 2 border)

### Phase 1 Verification
- `cargo build && cargo test`
- Press backspace during drill → verify `[Bksp]` lights up
- Press tab → verify `[Tab]` lights up
- Press enter → verify `[Enter]` lights up
- Press shift → verify both `[Shft]` keys light up and all keys show shifted variants
- Type space → verify space bar lights up
- Verify compact mode works on narrow terminals

---

## Phase 2: Key Milestone Detection

### 2a. Return change events from `SkillTree::update`

**File: `src/engine/skill_tree.rs`**

Add a return type:

```rust
pub struct SkillTreeUpdate {
    pub newly_unlocked: Vec<char>,
    pub newly_mastered: Vec<char>,
}
```

Modify `update()` to:
1. Snapshot current unlocked keys (via `unlocked_keys(DrillScope::Global)`) as a `HashSet<char>` before changes
2. Snapshot per-key confidence before changes (for keys currently unlocked)
3. Perform existing update logic
4. Snapshot unlocked keys after
5. `newly_unlocked` = keys in after but not in before
6. `newly_mastered` = keys where confidence was < 1.0 before but >= 1.0 after (only check keys in the before set)

### 2b. Finger info text generation

**File: `src/keyboard/finger.rs`**

Add `description()` method to `FingerAssignment`:

```rust
pub fn description(&self) -> &'static str {
    match (self.hand, self.finger) {
        (Hand::Left, Finger::Pinky)  => "left pinky",
        (Hand::Left, Finger::Ring)   => "left ring finger",
        (Hand::Left, Finger::Middle) => "left middle finger",
        (Hand::Left, Finger::Index)  => "left index finger",
        (Hand::Left, Finger::Thumb)  => "left thumb",
        (Hand::Right, Finger::Pinky) => "right pinky",
        (Hand::Right, Finger::Ring)  => "right ring finger",
        (Hand::Right, Finger::Middle)=> "right middle finger",
        (Hand::Right, Finger::Index) => "right index finger",
        (Hand::Right, Finger::Thumb) => "right thumb",
    }
}
```

Finger info is looked up via `KeyboardModel::finger_for_char(ch)` which uses position-based mapping that works across all layouts (QWERTY, Dvorak, Colemak).

### 2c. Find key's skill tree location

**File: `src/engine/skill_tree.rs`**

Add helper:

```rust
pub fn find_key_branch(ch: char) -> Option<(&'static BranchDefinition, &'static str, usize)> {
    // Returns (branch_def, level_name, 1-based position_in_level)
    for branch in ALL_BRANCHES {
        for level in branch.levels {
            if let Some(pos) = level.keys.iter().position(|&k| k == ch) {
                return Some((branch, level.name, pos + 1));
            }
        }
    }
    None
}
```

### Phase 2 Verification
- `cargo test` — existing tests pass
- Add unit test: `update()` returns correct `newly_unlocked` when keys are unlocked
- Add unit test: `update()` returns correct `newly_mastered` when confidence crosses 1.0
- Add unit test: `find_key_branch('e')` returns `(Lowercase, "Frequency Order", 1)`

---

## Phase 3: Milestone Overlay UI

### 3a. Milestone data structures

**File: `src/app.rs`**

Add to `App`:
```rust
pub milestone_queue: VecDeque<KeyMilestonePopup>,
```

Types (can live in `app.rs` or a new `src/milestone.rs`):
```rust
pub struct KeyMilestonePopup {
    pub kind: MilestoneKind,
    pub keys: Vec<char>,
    pub finger_info: Vec<(char, String)>,  // (key, "left ring finger")
}

pub enum MilestoneKind {
    Unlock,
    Mastery,
}
```

### 3b. Capture milestone events in `finish_drill`

**File: `src/app.rs` — `finish_drill` (~line 485)**

After `self.skill_tree.update(&self.key_stats)` (line 502), capture the `SkillTreeUpdate`. If `newly_unlocked` is non-empty, push an Unlock milestone to the queue with finger info for each key. If `newly_mastered` is non-empty, push a Mastery milestone to the queue. Both can be queued — they'll show one at a time.

Build finger info using `self.keyboard_model.finger_for_char(ch).description()`.

**Multi-key milestones:** Each `KeyMilestonePopup` can contain multiple keys (e.g., if 3 keys unlock in one drill completion). The overlay shows all keys together: "You unlocked: 'e', 'r', 'i'" with finger info for each. This is preferred over one overlay per key to avoid a long queue of nearly identical overlays. If both unlocks and masteries occur, they are separate milestones in the queue (one Unlock overlay, one Mastery overlay).

For shifted characters, also include shift key guidance:
- Left-hand characters → "Hold Right Shift (right pinky)"
- Right-hand characters → "Hold Left Shift (left pinky)"

### 3c. Milestone overlay rendering

**File: `src/main.rs` — `render_drill`**

After rendering the drill screen, check `app.milestone_queue.front()`. If present, render a centered overlay using `Clear` + bordered block. Layout adapts to terminal size:

- Large terminal (height >= 25): Full keyboard diagram + text
- Medium (height >= 15): Compact keyboard + text
- Small (height < 15): Text only, no keyboard diagram

Overlay content:
- Title: "Key Unlocked!" or "Key Mastered!"
- Key display: "You unlocked: 's'" / "You mastered: 's'"
- Finger info (unlock only): "Use your left ring finger"
- Encouraging message (randomly selected from pool)
- Keyboard diagram with `focused_key` set to the **first key** in the milestone's key list. For multi-key milestones, only the first key is highlighted on the diagram; all keys are listed textually above.
- For shifted characters: `shift_held = true` on diagram
- Footer: "Press any key to continue (Backspace dismisses only)"

Encouraging message pools:

**Unlock:**
- "Nice work! Keep building your typing skills."
- "Another key added to your arsenal!"
- "Your keyboard is growing! Keep it up."
- "One step closer to full keyboard mastery!"

**Mastery:**
- "This key is now at full confidence!"
- "You've got this key down pat!"
- "Muscle memory locked in!"
- "One more key conquered!"

### 3d. Milestone dismissal — per-key-class behavior

**File: `src/main.rs` — `handle_drill_key`**

At the top of `handle_drill_key`, check `app.milestone_queue.front()`. If present, pop the front milestone from the queue, then handle the dismissing key based on its class:

| Key class | Dismiss? | Replay into drill? | Notes |
|---|---|---|---|
| `KeyCode::Char(ch)` | Yes | Yes — fall through to normal input | Most common case; no keystrokes lost |
| `KeyCode::Tab` | Yes | Yes — fall through to tab handling | Tab is valid drill input |
| `KeyCode::Enter` | Yes | Yes — fall through to enter handling | Enter is valid drill input |
| `KeyCode::Backspace` | Yes | No — dismiss only | Replaying backspace would delete progress the user didn't intend to undo |
| `KeyCode::Esc` | Yes | Yes — Esc falls through to drill exit | Clears entire milestone queue and exits drill immediately |
| Other (arrows, etc.) | Yes | No — dismiss only | Non-drill keys just dismiss |

Implementation: after popping the milestone, check the key code. For `Char`, `Tab`, `Enter`, and `Esc`, let the key continue through the existing `handle_drill_key` logic. For `Backspace` and all other keys, return early after dismissal.

### Phase 3 Verification
- Start fresh, type until 7th key unlocks → milestone overlay appears
- Press a letter key → overlay disappears AND that letter is typed into the drill
- Press Tab during overlay → overlay disappears AND tab is processed as drill input
- Press Enter during overlay → overlay disappears AND enter is processed as drill input
- Press Backspace during overlay → overlay disappears, no drill input change
- Press Esc during overlay → overlay disappears AND drill exits
- Master a key → mastery overlay appears
- Multiple milestones in one drill → overlays show sequentially
- Verify correct finger info text
- Shifted character unlock → shift keys highlighted on diagram
- Small terminal → verify overlay degrades gracefully
- Small terminal + multi-key milestone → verify text-only layout shows all keys and finger info without overflow
- Encouraging messages: assert from message pool membership (not exact string) in any UI tests to avoid flaky assertions from randomness
- Multi-key milestone → verify first key is highlighted on keyboard diagram, all keys listed textually

---

## Phase 4: Stats Dashboard — Add Modifier Key Stats

### 4a. Add modifier key stats to keyboard heatmaps

**File: `src/ui/components/stats_dashboard.rs`**

In `render_keyboard_heatmap` (line 654) and `render_keyboard_timing` (line 768), after rendering the 4 character rows, render modifier key stats:

- **Backspace** (`'\x08'`): After number row, render `Bksp` + stat value
- **Tab** (`'\t'`): Before top row, render `Tab` + stat value
- **Enter** (`'\n'`): After home row, render `Ent` + stat value
- **Space** (`' '`): Below bottom row, render `Spc` + stat value

Use the same `get_key_accuracy` / `get_key_time_ms` methods (they work with any `char`).

### 4b. Include modifier keys in key ranking lists

In `render_worst_accuracy_keys` (line 957) and `render_best_accuracy_keys` (line 1030), add `' '`, `'\t'`, `'\n'` to the `all_keys` set so these keys appear in accuracy rankings. The `render_slowest_keys`/`render_fastest_keys` already pull from `key_stats.stats` which includes these keys automatically.

### Phase 4 Verification
- Open Stats → Accuracy tab → keyboard heatmap shows Tab, Enter, Space with stats
- Open Stats → Timing tab → same
- Tab/Space appear in worst/best accuracy lists when they have data

---

## Phase 5: Keyboard Explorer Screen

### 5a. Add `AppScreen::Keyboard` and menu item

**File: `src/app.rs`**

Add `Keyboard` to `AppScreen` enum. Add field:
```rust
pub keyboard_explorer_selected: Option<char>,
```

**File: `src/ui/components/menu.rs`**

Add menu item with key `"b"` (not `"k"` which conflicts with j/k vim navigation):
```rust
MenuItem {
    key: "b".to_string(),
    label: "Keyboard".to_string(),
    description: "Explore keyboard layout and key statistics".to_string(),
}
```

Insert between "Skill Tree" and "Statistics". Final menu order:
- 0: `[1]` Adaptive Drill
- 1: `[2]` Code Drill
- 2: `[3]` Passage Drill
- 3: `[t]` Skill Tree
- 4: `[b]` Keyboard
- 5: `[s]` Statistics
- 6: `[c]` Settings

### 5b. Menu routing

**File: `src/main.rs` — `handle_menu_key`**

Add `KeyCode::Char('b')` → `app.screen = AppScreen::Keyboard; app.keyboard_explorer_selected = None`. Update Enter handler indices: 4 → Keyboard, 5 → Stats, 6 → Settings.

Update footer hint: `" [1-3] Start  [t] Skill Tree  [b] Keyboard  [s] Stats  [c] Settings  [q] Quit "`.

### 5c. Keyboard Explorer rendering

**File: `src/main.rs`**

Add `render_keyboard_explorer` function. Layout:

1. **Header** (3 lines): " Keyboard Explorer " + "Press any key to see details"
2. **Keyboard diagram** (8 lines): Full `KeyboardDiagram` with:
   - `focused_key`: `app.keyboard_explorer_selected`
   - `next_key`: None
   - `unlocked_keys`: `app.skill_tree.unlocked_keys(DrillScope::Global)`
   - `depressed_keys`: `&app.depressed_keys`
   - `shift_held`: `app.shift_held`
3. **Key detail panel** (remaining space): Bordered block showing stats for selected key
4. **Footer** (1 line): "[ESC] Back"

Key detail panel content (when a key is selected):
```
┌─ Key Details: 's' ──────────────────────────────┐
│  Finger:      Left ring finger                   │
│  Unlocked:    Yes                                │
│  Mastery:     87% confidence                     │
│  Branch:      Lowercase a-z                      │
│  Level:       Frequency Order (key #7)           │
│  Avg Time:    245ms (best: 198ms)                │
│  Accuracy:    96.2% (385/400 correct)            │
│  Samples:     400                                │
└──────────────────────────────────────────────────┘
```

Data sources:
- Finger: `keyboard_model.finger_for_char(ch).description()`
- Unlocked: check if `ch` is in `skill_tree.unlocked_keys(DrillScope::Global)`
- Mastery: `key_stats.get_confidence(ch)` formatted as percentage
- Branch/Level: `find_key_branch(ch)` from Phase 2
- Avg Time / Best: `key_stats.get_stat(ch)` → `filtered_time_ms`, `best_time_ms`
- Accuracy: precomputed (see 5e)
- Samples: `key_stats.get_stat(ch)` → `sample_count`

### 5d. Key handling

**File: `src/main.rs`**

Add `handle_keyboard_explorer_key`:
- `Esc` → go to menu
- `KeyCode::Char('q')` when no key selected → go to menu; when key selected → select 'q' (so user can explore 'q')
- `KeyCode::Char(ch)` → set `keyboard_explorer_selected = Some(ch)` (see normalization below)
- `KeyCode::Tab` → set selected to `'\t'`
- `KeyCode::Enter` → set selected to `'\n'`
- `KeyCode::Backspace` → set selected to `'\x08'`

**Shifted character normalization strategy:** Store the literal `ch` value from the `KeyCode::Char(ch)` event as-is. Do NOT transform using `shift_held` state. crossterm delivers the already-shifted character in the event (e.g., Shift+a → `KeyCode::Char('A')`, Shift+1 → `KeyCode::Char('!')`), so the event `ch` is the correct key identity. The `shift_held` flag is used only for keyboard diagram rendering (to show shifted labels on all keys), not for determining which key was selected. Show shift guidance in the detail panel for any shifted character (uppercase or symbol) using `keyboard_model.finger_for_char(ch)` to determine hand and thus which shift key to recommend.

For Keyboard Explorer, also show shift key guidance for shifted keys in the detail panel:
- Left-hand characters → "Hold Right Shift (right pinky)"
- Right-hand characters → "Hold Left Shift (left pinky)"

### 5e. Precomputed accuracy for explorer

**File: `src/app.rs`**

Add a cached accuracy field to `App`:
```rust
pub explorer_accuracy_cache: Option<(char, usize, usize)>,  // (cached_key, correct, total)
```

Add a method `App::key_accuracy(ch: char) -> (usize, usize)` that checks the cache first. If `cached_key == ch`, return cached values. Otherwise, perform a single linear scan of `drill_history`, cache the result, and return it. The cache is invalidated automatically when `keyboard_explorer_selected` changes (set cache to `None` in the key handler). This avoids redundant O(n) scans on every render frame during key hold or rapid redraw.

### Phase 5 Verification
- `cargo build && cargo test`
- Open Keyboard from menu via `b` key → verify diagram shown
- Press any letter → detail panel shows finger, branch, level, stats
- Press shift → shift keys light up, all keys show shifted variants
- Press shifted key (e.g. Shift+a → 'A') → detail panel shows shifted character info with shift key guidance
- Tab/Enter/Backspace/Space → light up and show details
- Key with no stats → "No data yet"
- Esc → return to menu
- Verify `j`/`k` still work for menu navigation (no hotkey conflict)

---

## Finger Assignment Reference Data (informational)

The existing `KeyboardModel::finger_for_position` method (in `src/keyboard/model.rs`) handles finger assignments by physical position for all layouts. The table below is for reference only — the implementation in `finger_for_position` is the source of truth. Add unit tests against that method to validate correctness rather than maintaining this table. **Shifted characters use the same finger as their base key.**

### QWERTY — All 96 Keys by Finger

**Left Pinky (11 keys):**
- Base: `` ` `` `1` `q` `a` `z`
- Shifted: `~` `!` `Q` `A` `Z`
- Modifier: Tab (`\t`)

**Left Ring (8 keys):**
- Base: `2` `w` `s` `x`
- Shifted: `@` `W` `S` `X`

**Left Middle (8 keys):**
- Base: `3` `e` `d` `c`
- Shifted: `#` `E` `D` `C`

**Left Index (16 keys):**
- Base: `4` `5` `r` `t` `f` `g` `v` `b`
- Shifted: `$` `%` `R` `T` `F` `G` `V` `B`

**Right Index (16 keys):**
- Base: `6` `7` `y` `u` `h` `j` `n` `m`
- Shifted: `^` `&` `Y` `U` `H` `J` `N` `M`

**Right Middle (8 keys):**
- Base: `8` `i` `k` `,`
- Shifted: `*` `I` `K` `<`

**Right Ring (8 keys):**
- Base: `9` `o` `l` `.`
- Shifted: `(` `O` `L` `>`

**Right Pinky (21 keys):**
- Base: `0` `-` `=` `p` `[` `]` `\` `;` `'` `/`
- Shifted: `)` `_` `+` `P` `{` `}` `|` `:` `"` `?`
- Modifiers: Backspace (`\x08`), Enter (`\n`)

**Thumb (1 key):**
- Space (` `)

### Dvorak & Colemak

Finger assignments are **position-based** — the same physical key positions use the same fingers. `KeyboardModel::finger_for_char(ch)` looks up a character's physical position via `find_key_position` then calls `finger_for_position`, so it returns the correct finger for any layout automatically.

### Shift Key Guidance for Shifted Characters

- **Left-hand characters**: Hold **Right Shift** (right pinky)
- **Right-hand characters**: Hold **Left Shift** (left pinky)

---

## Critical Files to Modify

1. **`src/keyboard/display.rs`** (new) — Centralized key display adapter for sentinel ↔ display name conversions (Phase 0)
2. **`src/keyboard/finger.rs`** — Add `description()` method (Phase 2)
3. **`src/engine/skill_tree.rs`** — Add `SkillTreeUpdate` return type, `find_key_branch()` helper (Phase 2)
4. **`src/app.rs`** — Add `milestone_queue`, `keyboard_explorer_selected`, `AppScreen::Keyboard`, milestone structs (Phases 3, 5)
5. **`src/ui/components/keyboard_diagram.rs`** — Render Tab, Enter, Shift, Space, Backspace as interactive keys (Phase 1)
6. **`src/main.rs`** — Modifier depressed state tracking, milestone overlay, keyboard explorer screen, menu routing (Phases 1, 3, 5)
7. **`src/ui/components/stats_dashboard.rs`** — Add modifier keys to keyboard heatmaps and ranking lists (Phase 4)
8. **`src/ui/components/menu.rs`** — Add "Keyboard" menu item with key `b` (Phase 5)

## Terminology

Throughout the implementation, use consistent terminology:
- "Milestone" for the unlock/mastery event system (not "popup" or "notification")
- "Milestone overlay" for the UI element shown during a milestone (not "pop-up", "modal", or "dialog")
- "Enter" (not "Return") for the Enter key
- "Keyboard Explorer" for the new menu screen

## Scope Boundaries

- Non-US layouts beyond QWERTY/Dvorak/Colemak are out of scope for this plan
- The `KeyDisplay` adapter (Phase 0) is intentionally thin — a full typed `KeyId` enum migration is deferred to a future plan
- Left/right shift distinction is not tracked separately (both display as "Shift")
