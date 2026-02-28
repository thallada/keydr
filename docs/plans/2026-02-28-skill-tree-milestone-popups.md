# Skill Tree Milestone Popups

## Context

When users reach major skill tree milestones, they should see celebratory popups explaining what they've achieved and what's next. Four milestone types:

1. **Lowercase complete** — all 26 lowercase keys mastered, other branches become available
2. **Branch complete** — a non-lowercase branch fully mastered
3. **All keys unlocked** — every key on the keyboard is available for practice
4. **All keys mastered** — every key at full confidence, ultimate achievement

These popups appear after key unlock/mastery popups and before the drill summary screen, using the existing `milestone_queue` system. The existing post-drill input lock (800ms) applies to these popups when they're the first popup shown after a drill.

## Implementation

### 1. Extend `SkillTreeUpdate` (`src/engine/skill_tree.rs`)

Add fields to `SkillTreeUpdate`:
```rust
pub branches_newly_available: Vec<BranchId>,  // Locked → Available transitions
pub branches_newly_completed: Vec<BranchId>,  // → Complete transitions
pub all_keys_unlocked: bool,                  // every key now in practice pool
pub all_keys_mastered: bool,                  // every key at confidence >= 1.0
```

**In `update()`:**
- Snapshot non-lowercase branch statuses before the auto-unlock loop. After it, collect `Locked` → `Available` transitions into `branches_newly_available`.
- Snapshot all branch statuses before updates. After `update_lowercase()` and all `update_branch_level()` calls, collect branches that became `Complete` into `branches_newly_completed`.
- `all_keys_unlocked`: compare `total_unlocked_count()` against `compute_total_unique_keys()`. Set to `true` only if they're equal now AND they weren't equal before (using a before-snapshot of unlocked count).
- `all_keys_mastered`: `true` if every branch in `ALL_BRANCHES` has `BranchStatus::Complete` after updates AND at least one wasn't `Complete` before.

`BranchId` is already used across all layers. Display names come from `get_branch_definition(id).name`.

### 2. Add milestone variants to `MilestoneKind` (`src/app.rs`)

```rust
pub enum MilestoneKind {
    Unlock,
    Mastery,
    BranchesAvailable,   // lowercase complete → other branches available
    BranchComplete,      // a non-lowercase branch fully completed
    AllKeysUnlocked,     // every key on the keyboard is unlocked
    AllKeysMastered,     // every key at full confidence
}
```

**In `finish_drill()`, after mastery popup queueing**, check each flag and push popups in order:

1. `branches_newly_available` non-empty → push `BranchesAvailable`
2. `branches_newly_completed` non-empty (excluding `BranchId::Lowercase` since `BranchesAvailable` covers it) → push `BranchComplete`
3. `all_keys_unlocked` → push `AllKeysUnlocked`
4. `all_keys_mastered` → push `AllKeysMastered`

For all four: `keys` and `finger_info` are empty, `message` is unused. The renderer owns all copy.

**Input lock**: These popups are pushed to `milestone_queue`, so the existing check `!self.milestone_queue.is_empty()` at `finish_drill()` already triggers `arm_post_drill_input_lock()`. No changes needed — the lock applies to whatever the first popup is.

### 3. Render popup variants in `render_milestone_overlay()` (`src/main.rs`)

Each variant gets its own rendering branch. No keyboard diagram for any of these. All use the standard footer (input lock remaining / "Press any key to continue").

**`BranchesAvailable`:**
- Title: `"New Skill Branches Available!"`
- Body:
  ```
  Congratulations! You've mastered all 26 lowercase
  keys!

  New skill branches are now available:
    • Capitals A-Z
    • Numbers 0-9
    • Prose Punctuation
    • Whitespace
    • Code Symbols

  Visit the Skill Tree to unlock a new branch and
  start training!

  Press [t] from the menu to open the Skill Tree
  ```
  (Branch names rendered dynamically from `get_branch_definition(id).name` for each ID in `branches_newly_available`.)

**`BranchComplete`:**
- Title: `"Branch Complete!"`
- Body:
  ```
  You've fully mastered the {branch_name} branch!

  Other branches are waiting to be unlocked in the
  Skill Tree. Keep going!

  Press [t] from the menu to open the Skill Tree
  ```
  (If multiple branches completed simultaneously, list them all: "You've fully mastered the {name1} and {name2} branches!")

**`AllKeysUnlocked`:**
- Title: `"Every Key Unlocked!"`
- Body:
  ```
  You've unlocked every key on the keyboard!

  All keys are now part of your practice drills.
  Keep training to build full confidence with each
  key!
  ```

**`AllKeysMastered`:**
- Title: `"Full Keyboard Mastery!"`
- Body:
  ```
  Incredible! You've reached full confidence with
  every single key on the keyboard!

  You've completed everything keydr has to teach.
  Keep practicing to maintain your skills!
  ```

### 4. Sequencing

Queue order in `finish_drill()`:
1. Key unlock popups (existing)
2. Key mastery popups (existing)
3. `BranchesAvailable` (if applicable)
4. `BranchComplete` (if applicable, excluding lowercase)
5. `AllKeysUnlocked` (if applicable)
6. `AllKeysMastered` (if applicable)

The input lock is armed once when `milestone_queue` is non-empty (existing logic). User dismisses each popup with any keypress.

### 5. Tests

**In `src/engine/skill_tree.rs` tests:**
- `branches_newly_available` non-empty on first `update()` after lowercase completion, empty on second call
- `branches_newly_completed` contains the branch ID when a non-lowercase branch completes
- `all_keys_unlocked` fires when the last key becomes available, not on subsequent calls
- `all_keys_mastered` fires when all branches reach Complete, not on subsequent calls
- `branches_newly_available` only contains the five non-lowercase branch IDs

**In `src/app.rs` tests:**
- Queue order test: last lowercase key mastered → queue contains unlock → mastery → BranchesAvailable (no BranchComplete for lowercase)
- Branch complete test: non-lowercase branch completes → BranchComplete queued
- Helper: `seed_near_complete_lowercase(app)` — 25 keys at confidence 1.0, last key at 0.95

## Files to Modify

1. `src/engine/skill_tree.rs` — Extend `SkillTreeUpdate`, detect transitions in `update()`
2. `src/app.rs` — Add variants to `MilestoneKind`, queue popups in `finish_drill()`
3. `src/main.rs` — Render the four new popup variants in `render_milestone_overlay()`

## Verification

1. `cargo build` — compiles cleanly
2. `cargo test` — all existing + new tests pass
3. Manual testing with test profiles for each milestone scenario
