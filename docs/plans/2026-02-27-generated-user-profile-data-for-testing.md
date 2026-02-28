# Plan: Create Test User Profiles at Various Skill Tree Progression Levels

## Context

We need importable JSON test profiles representing users at every meaningful stage of skill tree progression. Each profile must have internally consistent key stats, drill history, and skill tree state so the app behaves as if a real user reached that level. The profiles will be used for manual regression testing of UI and logic at each progression stage.

## Design Decisions

- **Ranked mode**: Only profiles 5+ (multi-branch and beyond) include ranked key stats and ranked drills; earlier profiles have empty ranked stats since new users wouldn't have encountered ranked mode yet.
- **`total_score`**: Synthetic plausible value per profile (not replayed from drill history). The goal is UI/progression testing, not scoring fidelity. Score is set to produce a reasonable `level_from_score()` result for each progression stage.
- **Key stats coverage**: `KeyStatsStore` contains only keys that have been practiced (have at least one correct keystroke in drill history). Unlocked-but-unpracticed keys are absent — this is realistic since a freshly unlocked key has no stats. Locked keys are always absent.
- **Fixtures are committed assets**: Generated once, checked into `test-profiles/`. The generator binary is kept for regeneration if schema evolves. Output is deterministic (no RNG — all values computed from formulas).
- **Timestamps**: Monotonically increasing, spaced ~2 minutes apart within a day, spread across days matching `streak_days`. `last_practice_date` derived from the last drill timestamp.

## Consistency Invariants

Every generated profile must satisfy:

1. `KeyStatsStore` contains only practiced keys (subset of unlocked keys). No locked-branch keys ever appear. Every key in stats must have `sample_count > 0`.
2. `KeyStat.confidence >= 1.0` for all keys in completed levels; `< 1.0` for keys in the current in-progress level that are still being learned.
3. `ProfileData.total_drills == drill_history.drills.len()`
4. `ProfileData.total_score` is a plausible synthetic value producing a reasonable level via `level_from_score()`.
5. `ProfileData.streak_days` and `last_practice_date` are consistent with drill timestamps.
6. `DrillResult.per_key_times` only reference keys from the profile's final unlocked set. (Temporal progression fidelity within drill history is a non-goal — all drills use the final-state key pool for simplicity. The goal is testing UI/import behavior at each progression snapshot, not simulating the exact journey.)
7. `ranked_key_stats` is empty (default) for profiles 1-4; populated for profiles 5-7 with stats for keys appearing in ranked drills.
8. Branch marked `Complete` only if all keys in all levels have `confidence >= 1.0`.
9. Drill timestamps are monotonically increasing across the full history.

## Profiles

All files in `test-profiles/` at project root. Each is a valid `ExportData` JSON.

### 1. `01-brand-new.json` — Fresh Start
- **Skill tree**: Lowercase `InProgress` level 0, all others `Locked`
- **Key stats**: Empty
- **Ranked key stats**: Empty
- **Drill history**: Empty (0 drills)
- **Profile**: 0 drills, 0 score, 0 streak
- **Tests**: Initial onboarding, first-run UI, empty dashboard

### 2. `02-early-lowercase.json` — Early Lowercase (10 keys)
- **Skill tree**: Lowercase `InProgress` level 4 (6 base + 4 unlocked = 10 keys: e,t,a,o,i,n,s,h,r,d)
- **Key stats**: e,t,a,o,i,n at confidence >= 1.0 (mastered); s,h,r,d at confidence 0.3-0.7
- **Ranked key stats**: Empty
- **Drill history**: 15 adaptive drills
- **Profile**: 15 drills, synthetic score, 3-day streak
- **Tests**: Progressive lowercase unlock, focused key targeting weak keys, early dashboard

### 3. `03-mid-lowercase.json` — Mid Lowercase (18 keys)
- **Skill tree**: Lowercase `InProgress` level 12 (6 + 12 = 18 keys, through 'y')
- **Key stats**: First 14 keys mastered, next 4 at confidence 0.4-0.8
- **Ranked key stats**: Empty
- **Drill history**: 50 adaptive drills
- **Profile**: 50 drills, synthetic score, 7-day streak
- **Tests**: Many keys unlocked, skill tree partial progress display

Note: Lowercase level semantics — `current_level` = number of keys unlocked beyond the initial 6 (`LOWERCASE_MIN_KEYS`). So level 12 means 18 total keys.

### 4. `04-lowercase-complete.json` — Lowercase Complete
- **Skill tree**: Lowercase `Complete` (level 20, all 26 keys), all others `Available`
- **Key stats**: All 26 lowercase at confidence >= 1.0
- **Ranked key stats**: Empty
- **Drill history**: 100 adaptive drills
- **Profile**: 100 drills, synthetic score, 14-day streak
- **Tests**: Branch completion, all branches showing Available, branch start UI

### 5. `05-multi-branch.json` — Multiple Branches In Progress
- **Skill tree**:
  - Lowercase: `Complete`
  - Capitals: `InProgress` level 1 (L1 mastered, working on L2 "Name Capitals")
  - Numbers: `InProgress` level 0 (working on L1 "Common Digits")
  - Prose Punctuation: `InProgress` level 0 (working on L1 "Essential")
  - Whitespace: `Available`
  - Code Symbols: `Available`
- **Key stats**: All lowercase mastered; T,I,A,S,W,H,B,M mastered; J,D,R,C,E partial; 1,2,3 partial; period/comma/apostrophe partial
- **Ranked key stats**: Some ranked stats for lowercase keys (from ~20 ranked drills)
- **Drill history**: 200 drills (170 adaptive, 10 passage, 20 ranked adaptive)
- **Profile**: 200 drills, synthetic score, 21-day streak
- **Tests**: Multi-branch progress, branch-specific drills, global vs branch focus selection

### 6. `06-advanced.json` — Most Branches Complete
- **Skill tree**:
  - Lowercase: `Complete`
  - Capitals: `Complete`
  - Numbers: `Complete`
  - Prose Punctuation: `Complete`
  - Whitespace: `Complete`
  - Code Symbols: `InProgress` level 2 (L1+L2 done, working on L3 "Logic & Reference")
- **Key stats**: All mastered except Code Symbols L3 (&,|,^,~,!) at partial confidence and L4 absent
- **Ranked key stats**: Substantial ranked stats across all mastered keys
- **Drill history**: 500 drills (350 adaptive, 50 passage, 50 code, 50 ranked)
- **Profile**: 500 drills, synthetic score, 45-day streak, best_streak: 60
- **Tests**: Near-endgame, almost all keys, code symbols progression

### 7. `07-fully-complete.json` — Everything Mastered
- **Skill tree**: ALL branches `Complete`
- **Key stats**: All keys confidence >= 1.0, high sample counts, low error rates
- **Ranked key stats**: Full ranked stats for all keys
- **Drill history**: 800 drills (400 adaptive, 150 passage, 150 code, 100 ranked)
- **Profile**: 800 drills, synthetic score, 90-day streak
- **Tests**: Endgame, all complete, full dashboard, comprehensive ranked data

## Implementation

### File: `src/bin/generate_test_profiles.rs`

A standalone binary that imports keydr crate types and generates all profiles.

#### Helpers

```rust
/// Generate KeyStat with deterministic values derived from target confidence.
/// filtered_time_ms = target_time_ms / confidence
/// best_time_ms = filtered_time_ms * 0.85
/// sample_count and recent_times scaled to confidence level
fn make_key_stat(confidence: f64, sample_count: usize, target_cpm: f64) -> KeyStat

/// Generate a DrillResult with deterministic per_key_times.
/// Keys are chosen from the provided unlocked set.
fn make_drill_result(
    wpm: f64, accuracy: f64, char_count: usize,
    keys: &[char], timestamp: DateTime<Utc>,
    mode: &str, ranked: bool,
) -> DrillResult

/// Wrap all components into ExportData.
fn make_export(
    config: Config,
    profile: ProfileData,
    key_stats: KeyStatsData,
    ranked_key_stats: KeyStatsData,
    drill_history: DrillHistoryData,
) -> ExportData

/// Generate monotonic timestamps: base_date + day_offset + drill_offset * 2min
fn drill_timestamp(base: DateTime<Utc>, day: u32, drill_in_day: u32) -> DateTime<Utc>
```

#### Profile builders

One function per profile (`build_profile_01()` through `build_profile_07()`) that:
1. Constructs `SkillTreeProgress` with exact branch statuses and levels
2. Builds `KeyStatsStore` with stats only for unlocked/practiced keys
3. Generates drill history with proper timestamps and key references
4. Sets `total_score` to a synthetic plausible value for the progression stage
5. Derives `last_practice_date` and streak from drill timestamps
6. Returns `ExportData`

#### Main

```rust
fn main() {
    fs::create_dir_all("test-profiles").unwrap();
    for (name, data) in [
        ("01-brand-new", build_profile_01()),
        ("02-early-lowercase", build_profile_02()),
        // ...
    ] {
        let json = serde_json::to_string_pretty(&data).unwrap();
        fs::write(format!("test-profiles/{name}.json"), json).unwrap();
    }
}
```

### Key source files referenced
- `src/store/schema.rs` — ExportData, ProfileData, KeyStatsData, DrillHistoryData
- `src/engine/skill_tree.rs` — SkillTreeProgress, BranchProgress, BranchStatus, level definitions, LOWERCASE_MIN_KEYS=6
- `src/engine/key_stats.rs` — KeyStatsStore, KeyStat, DEFAULT_TARGET_CPM=175.0
- `src/session/result.rs` — DrillResult, KeyTime
- `src/config.rs` — Config defaults
- `src/engine/scoring.rs` — compute_score(), level_from_score()

## Verification

### Automated: `tests/test_profile_fixtures.rs`

Integration tests (separate from the generator binary) that for each generated JSON file:
- Deserializes into `ExportData` successfully
- Asserts `total_drills == drills.len()`
- Asserts no locked-branch keys appear in `KeyStatsStore`
- Asserts all keys in completed levels have `confidence >= 1.0`
- Asserts all keys in stats have `sample_count > 0`
- Asserts timestamps are monotonically increasing
- Asserts `ranked_key_stats` is empty for profiles 1-4
- Imports into a temp `JsonStore` via `import_all()` without error

### Manual smoke test per profile

| Profile | Check |
|---------|-------|
| 01 | Dashboard shows level 1, 0 drills, empty skill tree except lowercase InProgress |
| 02 | Skill tree shows 10/26 lowercase keys, focused key is from the weak-key pool (s,h,r,d) |
| 03 | Skill tree shows 18/26 lowercase keys, dashboard stats populated |
| 04 | All 6 branches visible, 5 show "Available", lowercase shows "Complete" |
| 05 | 3 branches InProgress with level indicators, branch drill selector works |
| 06 | 5 branches Complete, Code Symbols shows L3 in progress |
| 07 | All branches Complete, all stats filled, ranked data visible |

### Generation

`cargo run --bin generate_test_profiles` produces 7 files in `test-profiles/`

Generated JSON files are committed to the repo. CI runs fixture validation tests against the committed files (no regeneration step). If the schema changes, the developer reruns the generator manually and commits the updated fixtures.
