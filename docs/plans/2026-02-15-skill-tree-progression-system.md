# Skill Tree Progression System & Whitespace Support

## Context

keydr currently tracks only a-z lowercase letters in its adaptive unlock system. Since keydr aims to be a coding-focused typing tutor, it must also train capitals, numbers, punctuation, whitespace (tabs/newlines), and code-specific symbols. The current flat a-z progression needs to be replaced with a branching skill tree that lets players choose their training path after mastering lowercase letters. Additionally, code drills currently strip newlines into spaces, making them unrealistic for real-world code practice.

## Skill Tree Structure

The tree is flat: a-z is the root, and all other branches are direct siblings at the same level. Once a-z is complete, all branches unlock simultaneously and the user can choose any order.

```
                    ┌─────────────────┐
                    │   a-z Lowercase  │  (ROOT - everyone starts here)
                    │  26 keys, freq   │
                    │  order unlock    │
                    └────────┬────────┘
                             │
       ┌─────────┬──────────┼──────────┬──────────┐
       ▼         ▼          ▼          ▼          ▼
  ┌─────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌──────────┐
  │Capitals │ │Numbers │ │ Prose  │ │White-  │ │  Code    │
  │  A-Z    │ │  0-9   │ │ Punct. │ │ space  │ │ Symbols  │
  │ 3 lvls  │ │ 2 lvls │ │ 3 lvls │ │ 2 lvls │ │  4 lvls  │
  └─────────┘ └────────┘ └────────┘ └────────┘ └──────────┘
```

### Prerequisites

- **a-z Lowercase** (root): Always available from start
- **All other branches**: Require a-z complete (all 26 lowercase letters confident). Once a-z is done, all 5 branches unlock simultaneously. User freely chooses which to pursue.

---

## Branch Status State Machine

Each branch has an explicit status:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchStatus {
    Locked,      // Prerequisites not met
    Available,   // Prerequisites met, user hasn't started
    InProgress,  // User has begun drilling this branch
    Complete,    // All levels in branch are done
}
```

**Transitions:**
- `Locked → Available`: When a-z branch reaches `Complete`
- `Available → InProgress`: **Only** when user explicitly launches a branch drill from the skill tree (start-on-select model). The global adaptive drill does NOT auto-start branches.
- `InProgress → Complete`: When all keys in all levels of the branch reach confidence >= 1.0

**Multiple branches active**: Yes. The user can have multiple branches `InProgress` simultaneously. Each tracks its own current level independently.

**Global adaptive scope**: Only includes keys from `InProgress` and `Complete` branches. `Available` branches are not included — the user must visit the skill tree to start them.

---

## Detailed Level Breakdown

### Branch: a-z Lowercase (Root)

Uses existing frequency-order system. Starts with 6 keys, unlocks one at a time when all current keys reach confidence >= 1.0. Branch is "complete" when all 26 letters are confident.

Order: `e t a o i n s h r d l c u m w f g y p b v k j x q z`

Total keys: **26**

### Branch: Capital Letters (3 levels)

- **Level 1 — Common Sentence Capitals** (8 keys): `T I A S W H B M`
- **Level 2 — Name Capitals** (10 keys): `J D R C E N P L F G`
- **Level 3 — Remaining Capitals** (8 keys): `O U K V Y X Q Z`

Total keys: **26**

Text generation rules:
- First word of each "sentence" (after `.` `?` `!` or at drill start) gets capitalized
- ~10-15% of words get capitalized as proper-noun-like words
- Focused capital letter is boosted (40% chance to appear in word starts)

### Branch: Numbers (2 levels)

- **Level 1 — Common Digits** (5 keys): `1 2 3 4 5`
- **Level 2 — All Digits** (5 keys): `0 6 7 8 9`

Total keys: **10**

Text generation rules:
- ~15% of words replaced with number expressions using only unlocked digits
- Patterns: counts ("3 items"), years ("2024"), IDs ("room 42"), measurements ("7 miles")

### Branch: Prose Punctuation (3 levels)

- **Level 1 — Essential** (3 keys): `. , '`
- **Level 2 — Common** (4 keys): `; : " -`
- **Level 3 — Expressive** (4 keys): `? ! ( )`

Total keys: **11**

Text generation rules follow natural prose patterns:
- `.` ends sentences (every 5-15 words), `,` separates clauses
- `'` in contractions (don't, it's, we'll)
- `"` wrapping quoted phrases, `;` between clauses, `:` before lists
- `-` in compound words (well-known), `?` for questions, `!` for exclamations
- `( )` for parenthetical asides

### Branch: Whitespace (2 levels)

- **Level 1 — Enter/Return** (1 key): `\n`
- **Level 2 — Tab/Indent** (1 key): `\t`

Total keys: **2**

Text generation rules:
- Line breaks at sentence boundaries (every ~60-80 chars)
- Tabs for indentation in code-like structures
- Once unlocked, **default adaptive drills automatically become multi-line**

### Branch: Code Symbols (4 levels)

- **Level 1 — Arithmetic & Assignment** (5 keys): `= + * /` and `-` (shared with Prose Punct L2)
- **Level 2 — Grouping** (6 keys): `{ } [ ] < >`
- **Level 3 — Logic & Reference** (5 keys): `& | ^ ~` and `!` (shared with Prose Punct L3)
- **Level 4 — Special** (7 keys): `` @ # $ % _ \ ` ``

Total keys: **23** (21 unique + 2 shared with Prose Punctuation)

Text generation rules:
- L1: Prose with simple expressions (`x = a + b`, `total = price * qty`)
- L2: Code-pattern templates (`if (x) { return y; }`, `arr[0]`)
- L3: Bitwise/logical patterns (`a & b`, `!flag`, `*ptr`)
- L4: Language-specific patterns (`@decorator`, `#include`, `snake_case`)

**Grand total**: 98 keys across branches, **96 unique keys** (after deducting 2 shared: `-` and `!`). `TOTAL_UNIQUE_KEYS` is derived at startup by collecting all keys from all branch definitions into a `HashSet` and taking `len()`. Stored as a field on `SkillTree` for use in scoring and UI.

---

## Shared Keys Between Branches

Two keys appear in multiple branches:
- `-` appears in Prose Punctuation L2 and Code Symbols L1
- `!` appears in Prose Punctuation L3 and Code Symbols L3

**Rule**: Confidence is tracked once per character in `KeyStatsStore` (keyed by `char`). If a user masters `-` in Prose Punctuation, it is automatically confident in Code Symbols too. When checking level completion, the branch reads the single confidence value for that char. This is idempotent — no special handling needed.

---

## Focused Key Policy

### Global Adaptive Drill (from menu)

1. Collect all keys from all `InProgress` branches (current level's keys only) plus all `Complete` branch keys
2. Find the key with the **lowest confidence < 1.0** across this entire set
3. If all keys are confident, no focused key (maintenance mode)
4. Boost the focused key in text generation (40% probability)

### Branch-Specific Drill (from skill tree)

1. Collect keys from the selected branch including **all prior completed levels** (as background reinforcement) plus the **current level's keys**, plus all a-z keys
2. Find the key with the **lowest confidence < 1.0** within the **current level keys only** (prior level keys are reinforcement, not focus targets)
3. If all current level keys are confident, advance the level and focus on the weakest new key
4. Boost the focused key in text generation (40% probability)
5. Prior-level keys always appear in generated text for reinforcement but are never the focused key

### Branches with Zero Progress

When a branch is `Available` but user hasn't started it yet:
- Launching a drill from that branch transitions it to `InProgress` at level 1
- The focused key is the weakest among level 1's keys (likely all at 0.0 confidence, so pick the first in definition order)

---

## Scoring

Current formula: `complexity = unlocked_count / 26`

**New formula**: `complexity = total_unlocked_keys / TOTAL_UNIQUE_KEYS`

Where `TOTAL_UNIQUE_KEYS = 96` is computed from branch definitions (deduplicated across shared keys). This scales naturally — the more branches the user has unlocked, the higher the complexity multiplier.

Level formula remains: `level = floor(sqrt(total_score / 100))`.

Menu header changes from `"X/26 letters"` to `"X/96 keys"`.

---

## Skill Tree UI

### New Screen: `AppScreen::SkillTree`

Accessible from menu via `[t] Skill Tree`. Renders **vertically** as a scrollable list.

```
╔══════════════════════════════════════════════════════════════════╗
║                         SKILL TREE                              ║
╠══════════════════════════════════════════════════════════════════╣
║                                                                  ║
║  ★ Lowercase a-z                    COMPLETE  26/26              ║
║    ████████████████████████████████████████  Level 26/26         ║
║                                                                  ║
║  ── Branches (unlocked after a-z) ──────────────────────────     ║
║                                                                  ║
║  ► Capitals A-Z                     Lvl 2/3   18/26 keys        ║
║    ████████████████████░░░░░░░░░░░░  69%                        ║
║                                                                  ║
║    Numbers 0-9                      Lvl 0/2   0/10 keys         ║
║    ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   0%                         ║
║                                                                  ║
║    Prose Punctuation                Lvl 1/3   3/11 keys         ║
║    ██████████░░░░░░░░░░░░░░░░░░░░░  27%                        ║
║                                                                  ║
║    Whitespace                       Lvl 0/2   0/2 keys          ║
║    ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   0%                         ║
║                                                                  ║
║    Code Symbols                     Lvl 0/4   0/23 keys         ║
║    ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   0%                         ║
║                                                                  ║
╠══════════════════════════════════════════════════════════════════╣
║  ► Capitals A-Z                              Level 2/3          ║
║  L1: T I A S W H B M  (complete)                                ║
║  L2: J [D] R C E N P L F G  (in progress, focused: D)           ║
║  L3: O U K V Y X Q Z  (locked)                                  ║
║  Avg Confidence: ████████░░ 82%                                  ║
║                                                                  ║
║  [Enter] Start Drill   [↑↓/jk] Navigate   [q] Back              ║
╚══════════════════════════════════════════════════════════════════╝
```

**Layout:**
- **Top section**: Vertical list of all branches with status prefix, level, key count, progress bar
- **Bottom section**: Detail panel showing per-level key breakdown, confidence bars, focused key
- **Footer**: Controls

**Node states (prefix):**
- Locked: grayed out, no prefix, not selectable
- Available: normal color, no prefix
- In Progress `►`: accent color
- Complete `★`: gold/green

**Navigation:** `↑↓` / `j/k` move selection. `Enter` launches branch drill. `q` returns to menu.

**Keyboard diagram**: For non-printable keys (`Enter`, `Tab`), show them as labeled keys on the keyboard diagram in their standard positions. No special handling needed — they're physical keys with fixed positions.

---

## Code & Passage Drill Changes (Unranked Modes)

Code and Passage drills remain as separate menu options.

1. **Unranked tagging**: Add `ranked: bool` to `DrillResult` with `#[serde(default = "default_true")]` for backward compat
2. **Derive ranked from DrillContext**: At drill start, set `ranked = (drill_mode == Adaptive)`. Code/Passage → `ranked = false`.
3. **No progression**: `finish_drill()` gates skill tree updates on `result.ranked`
4. **History replay**: `rebuild_from_history()` uses `result.ranked` as the gate. No legacy fallback — since we reset on schema change (WIP policy), old history without `ranked` field won't exist.
5. **Visual indicators**:
   - Drill header: "Code Drill (Unranked)" / "Passage Drill (Unranked)" in dimmed/muted color
   - Result screen: "Unranked — does not count toward skill tree"
   - Stats dashboard history: unranked rows shown with muted styling

---

## Whitespace Handling

### Tokenized Render Model (`typing_area.rs`)

Replace direct char→span rendering with a `RenderToken` approach to handle one-to-many char-to-cell mapping:

```rust
struct RenderToken {
    target_idx: usize,    // Index into DrillState.target
    display: String,      // What to show (e.g., "↵", "→···", "a")
    style: Style,         // Computed style (correct/incorrect/cursor/pending)
}
```

**Display mapper:**
- `\n` → visible `↵` marker token + hard line break (new `Line` in paragraph)
- `\t` → visible `→` marker + padding `·` tokens to next 4-char tab stop
- All other chars → single token with char as display

**Cursor/style mapping:** Maintain a `Vec<(usize, usize)>` mapping from `target_idx` to first display cell position. When highlighting cursor or errors, look up the target index to find which display tokens to style.

**Multi-line rendering:** Change from single `Line` to `Vec<Line>`. Split on newline tokens. Each line is a separate `Line` in the `Paragraph`.

### Input Pipeline (`main.rs` + `session/input.rs`)

Current flow: `main.rs` matches `KeyCode::Char(ch)` → `app.type_char(ch)`. Enter/Tab are currently consumed by other handlers (menu nav, etc.).

**Changes in `main.rs`:**
- When `screen == Drill` and drill is active:
  - `KeyCode::Enter` → `app.type_char('\n')` **unconditionally** (correctness decided by `process_char()`)
  - `KeyCode::Tab` → `app.type_char('\t')` **unconditionally** (correctness decided by `process_char()`)
  - `KeyCode::BackTab` (Shift+Tab) → ignore (no action)
  - These must be checked **before** the existing Esc/Enter handlers for drill screen
  - If Enter/Tab is typed when not expected, it registers as an error on the current char — same as typing any wrong key

**No changes to `session/input.rs`**: `process_char()` already compares `ch == expected` generically. It will work with `'\n'` and `'\t'` as-is.

### Code Drill Updates (`generator/code_syntax.rs`)

- Embedded snippets change from single-line `&str` to multi-line string literals with preserved indentation
- `extract_code_snippets()`: preserve original newlines and leading whitespace instead of `split_whitespace().join(" ")`
- `generate()`: join snippets with `\n\n` instead of `" "`

---

## Data Model Changes

### Persistence Policy (WIP stage)

**No backward compatibility migration.** On schema mismatch, reset persisted files to defaults. Bump schema version to 2. Add a note in changelog that local progress is intentionally reset for this version. This avoids over-engineering migration logic during early development.

### `ProfileData` (schema v2)

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileData {
    pub schema_version: u32,            // 2
    pub skill_tree: SkillTreeProgress,  // Replaces unlocked_letters
    pub total_score: f64,
    pub total_drills: u32,
    pub streak_days: u32,
    pub best_streak: u32,
    pub last_practice_date: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillTreeProgress {
    pub branches: HashMap<String, BranchProgress>,  // String keys for stable JSON
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchProgress {
    pub status: BranchStatus,
    pub current_level: usize,  // 0-indexed into branch's levels array
    // current_level = 0 means working on first level (plan's "Level 1")
    // current_level = levels.len() only when status == Complete
}
```

**Indexing invariant**: `current_level` is always 0-indexed into `BranchDefinition.levels`. When the plan says "Level 1", "Level 2", etc. in human-readable text, that maps to `current_level = 0`, `current_level = 1`, etc. in code. A branch with `current_level = 0` and `status = InProgress` is actively working on its first level.

**HashMap uses `String` keys** (e.g., `"lowercase"`, `"capitals"`, `"numbers"`, etc.) for stable JSON serialization. `BranchId` enum has `to_key() -> &'static str` and `from_key()` methods.

### `DrillResult` Addition

```rust
#[serde(default = "default_true")]
pub ranked: bool,
```

### `KeyStatsStore`

No structural change. Already `HashMap<char, KeyStat>` — works for any char.

---

## Skill Tree Definition (Source of Truth)

Hard-coded static definition in `src/engine/skill_tree.rs`:

```rust
pub struct BranchDefinition {
    pub id: BranchId,
    pub name: &'static str,
    pub levels: Vec<LevelDefinition>,
}

pub struct LevelDefinition {
    pub name: &'static str,
    pub keys: Vec<char>,
}
```

All branch/level/key definitions are `const`/`static` arrays. No data-driven manifest needed at this stage. The `SkillTree` struct holds:
- The static definition (reference)
- The persisted `SkillTreeProgress` (mutable state)
- Methods: `unlocked_keys(scope)`, `focused_key(scope, &KeyStatsStore)`, `update(&KeyStatsStore)`, `branch_status(id)`, `all_branches()`

---

## Implementation Phases

### Phase 1: Skill Tree Core & Data Model

**Goal**: Replace `LetterUnlock` with `SkillTree`, update persistence.

1. Create `src/engine/skill_tree.rs`:
   - `BranchId` enum (`Lowercase, Capitals, Numbers, ProsePunctuation, Whitespace, CodeSymbols`)
   - `BranchStatus` enum (`Locked, Available, InProgress, Complete`)
   - `BranchDefinition`, `LevelDefinition` structs
   - Static branch definitions with all keys per level
   - `SkillTree` struct with `update()`, `unlocked_keys()`, `focused_key()`, `branch_status()`
2. Update `src/store/schema.rs`: new `ProfileData` with `SkillTreeProgress`, schema v2, reset on mismatch
3. Add `ranked: bool` to `DrillResult` in `src/session/result.rs`
4. Update `src/app.rs`: replace `letter_unlock: LetterUnlock` with `skill_tree: SkillTree`, update `finish_drill()` to gate on `ranked`, update `rebuild_from_history()`, update scoring complexity formula
5. Delete/replace `src/engine/letter_unlock.rs`

**Key files**: `src/engine/skill_tree.rs` (new), `src/engine/letter_unlock.rs` (delete), `src/store/schema.rs`, `src/session/result.rs`, `src/app.rs`

**Tests**:
- Skill tree status transitions (Locked → Available → InProgress → Complete)
- Shared key confidence propagation
- Focused key selection (global vs branch scope)
- Level completion and advancement
- Schema reset on version mismatch

**Acceptance criteria**: `cargo build` passes, `cargo test` passes, existing adaptive drills work with skill tree (a-z only), scoring uses new formula.

### Phase 2: Whitespace Input & Rendering

**Goal**: Support Enter/Tab in typing drills with proper display.

1. Update `src/ui/components/typing_area.rs`: tokenized render model with `RenderToken`, multi-line support, visible `↵` and `→` markers
2. Update `src/main.rs`: route `KeyCode::Enter` → `'\n'` and `KeyCode::Tab` → `'\t'` when in drill mode, ignore `BackTab`
3. Update `src/generator/code_syntax.rs`: preserve newlines/indentation in snippets, change embedded snippets to multi-line, fix `extract_code_snippets()` to preserve whitespace
4. Optionally update `src/generator/passage.rs` with multi-line passage variants

**Key files**: `src/ui/components/typing_area.rs`, `src/main.rs`, `src/generator/code_syntax.rs`

**Tests**:
- RenderToken generation for strings with `\n` and `\t`
- Cursor position mapping with expanded tokens
- Enter/Tab input processing (reuse existing `process_char()` — just verify `'\n'` and `'\t'` work)

**Acceptance criteria**: Code drills display multi-line with visible whitespace markers, Enter/Tab advance the cursor correctly, backspace works across line boundaries.

### Phase 3: Text Generation for Capitals & Punctuation

**Goal**: Generate drill text that naturally incorporates capitals and punctuation.

1. Create `src/generator/capitalize.rs`: post-processing pass that capitalizes sentence starts and occasional words, using only unlocked capital letters
2. Create `src/generator/punctuate.rs`: post-processing pass that inserts periods, commas, apostrophes, etc. at natural positions, using only unlocked punctuation
3. Update `src/generator/phonetic.rs` or `src/app.rs` `generate_text()`: apply capitalize/punctuate passes when those branches are active
4. Update `src/engine/filter.rs` `CharFilter`: add awareness of which char types are allowed (lowercase, uppercase, punctuation, etc.)

**Key files**: `src/generator/capitalize.rs` (new), `src/generator/punctuate.rs` (new), `src/generator/phonetic.rs`, `src/app.rs`, `src/engine/filter.rs`

**Acceptance criteria**: Adaptive drills with Capitals branch active produce properly capitalized text. Drills with Prose Punctuation active have natural punctuation placement.

### Phase 4: Text Generation for Numbers & Code Symbols

**Goal**: Generate drill text with numbers and code symbol patterns.

1. Create `src/generator/numbers.rs`: injects number expressions into prose using only unlocked digits
2. Create `src/generator/code_patterns.rs`: code-pattern templates for Code Symbols branch drills (expressions, brackets, operators)
3. Update `src/app.rs` `generate_text()`: apply number/code passes based on active branches
4. For whitespace branch: when active, insert `\n` at sentence boundaries in generated text

**Key files**: `src/generator/numbers.rs` (new), `src/generator/code_patterns.rs` (new), `src/app.rs`

**Acceptance criteria**: Number expressions use only unlocked digits. Code symbol drills produce recognizable code-like patterns. Whitespace branch generates multi-line output.

### Phase 5: Skill Tree UI

**Goal**: Navigable skill tree screen with branch detail and drill launch.

1. Add `AppScreen::SkillTree` to `src/app.rs`
2. Create `src/ui/components/skill_tree.rs`: vertical branch list + detail panel widget
3. Update `src/main.rs`: handle key events for skill tree screen (navigation, drill launch)
4. Update `src/ui/components/menu.rs`: add `[t] Skill Tree` option
5. Update menu header: show `"X/96 keys"` instead of `"X/26 letters"`
6. Add `DrillMode::BranchDrill(BranchId)` or similar to track drill origin for branch-specific focus

**Key files**: `src/ui/components/skill_tree.rs` (new), `src/app.rs`, `src/main.rs`, `src/ui/components/menu.rs`

**Acceptance criteria**: Can navigate to skill tree from menu, see all branches with correct status, launch a branch-specific drill, return to menu.

### Phase 6: Unranked Mode Polish

**Goal**: Clearly distinguish ranked vs unranked drills in UI.

1. Update drill header in `src/main.rs`: show "(Unranked)" for Code/Passage modes
2. Update `src/ui/components/dashboard.rs` result screen: note "does not count toward skill tree"
3. Update `src/ui/components/stats_dashboard.rs`: muted styling for unranked history rows
4. Verify `rebuild_from_history()` correctly uses `ranked` field to gate skill tree updates

**Key files**: `src/main.rs`, `src/ui/components/dashboard.rs`, `src/ui/components/stats_dashboard.rs`, `src/app.rs`

**Acceptance criteria**: Code/Passage drills clearly marked unranked. Stats history shows visual distinction. Ranked drills advance skill tree, unranked don't.

---

## Verification

### Automated Tests

- **Skill tree transitions**: `Locked → Available → InProgress → Complete` for each branch
- **Shared keys**: Mastering `!` in Prose Punct → confident in Code Symbols too
- **Focused key**: Global scope selects weakest across all active branches; branch scope selects within branch
- **Level advancement**: Completing all keys in a level auto-advances to next
- **Ranked/unranked**: Only ranked drills update skill tree in `rebuild_from_history()`
- **Whitespace tokens**: RenderToken expansion for `\n` and `\t` produces correct display strings and index mapping
- **Input routing**: `'\n'` and `'\t'` correctly processed as typed characters

### Manual Testing

1. Launch app → a-z trunk works as before
2. Complete a-z (or edit profile to simulate) → all 5 branches show as Available
3. Navigate skill tree → select Capitals → launch drill → see capitalized text
4. Complete Capitals L1 → L2 keys appear in drills
5. Launch default adaptive with multiple branches active → text mixes all unlocked keys
6. Launch Code/Passage drill → header shows "(Unranked)", no skill tree progress
7. Start Whitespace branch → default adaptive becomes multi-line
8. Type Enter/Tab in code drills → cursor advances correctly, errors tracked
9. Quit and relaunch → progress preserved
10. Delete `~/.local/share/keydr/` → app resets cleanly to fresh state
