# Plan: Bigram Metrics Overhaul — Error Anomaly & Speed Anomaly

## Context

The current bigram metrics use `difficulty = (1 - confidence) * redundancy` to gate eligibility and focus. This is fundamentally broken: when a user types faster than target WPM (`confidence > 1.0`), difficulty goes negative — even for bigrams with 100% error rate. The root cause is that "confidence" (a speed-vs-target ratio) and "redundancy" (an error-rate ratio) are conflated into a single metric that can cancel out genuine problems.

This overhaul replaces the conflated system with two orthogonal anomaly metrics:
- **`error_anomaly`** — how much worse a bigram's error rate is compared to what's expected from its constituent characters (same math as current `redundancy_score`, reframed as a percentage)
- **`speed_anomaly`** — how much slower a bigram transition is compared to the user's normal speed typing the second character (user-relative, no target WPM dependency)

Both are displayed as percentages where positive = worse than expected. The UI shows two side-by-side columns, one per anomaly type, with confirmed problems highlighted.

---

## Persistence / Migration

**NgramStat is NOT persisted to disk.** N-gram stores are rebuilt from drill history on every startup (see `json_store.rs:104` comment: "N-gram stats are not included — they are always rebuilt from drill history", and `app.rs:1152` `rebuild_ngram_stats()`). The stores are never saved via `save_data()` — only `profile`, `key_stats`, `ranked_key_stats`, and `drill_history` are persisted.

Therefore:
- No serde migration, `#[serde(alias)]`, or backward-compat handling is needed for NgramStat field renames/removals
- `#[serde(default)]` annotations on NgramStat fields are vestigial (the derive exists for in-memory cloning, not disk persistence) but harmless to leave
- The `Serialize`/`Deserialize` derives on NgramStat can stay (used by BigramStatsStore/TrigramStatsStore types which derive them transitively, though the stores themselves are also not persisted)

**KeyStat IS persisted** — `confidence` on KeyStat is NOT being changed (used by skill_tree progression). No migration needed there.

---

## Changes

### 1. `src/engine/ngram_stats.rs` — Metrics engine overhaul

**NgramStat struct** (line 34):
- Remove `confidence: f64` field
- Rename `redundancy_streak: u8` → `error_anomaly_streak: u8`
- Add `speed_anomaly_streak: u8` with `#[serde(default)]`
- **Preserved fields** (explicitly unchanged): `filtered_time_ms`, `best_time_ms`, `sample_count`, `error_count`, `hesitation_count`, `recent_times`, `recent_correct`, `last_seen_drill_index` — all remain and continue to be updated by `update_stat()`

**`update_stat()`** (line 65):
- Remove `confidence = target_time_ms / stat.filtered_time_ms` computation (line 82)
- Remove `target_time_ms` parameter (no longer needed)
- **Keep** `hesitation` parameter and `drill_index` parameter — these update `hesitation_count` (line 72) and `last_seen_drill_index` (line 66) which are used by trigram pruning and other downstream logic
- New signature (module-private, matching current visibility): `fn update_stat(stat: &mut NgramStat, time_ms: f64, correct: bool, hesitation: bool, drill_index: u32)`
- All other field updates remain identical (EMA on filtered_time_ms, best_time_ms, recent_times, recent_correct, error_count, sample_count)

**Constants** (lines 10-16):
- Rename `STABILITY_THRESHOLD` → `ERROR_ANOMALY_RATIO_THRESHOLD` (value stays 1.5)
- Rename `STABILITY_STREAK_REQUIRED` → `ANOMALY_STREAK_REQUIRED` (value stays 3)
- Rename `WATCHLIST_MIN_SAMPLES` → `ANOMALY_MIN_SAMPLES` (value stays 3)
- Add `SPEED_ANOMALY_PCT_THRESHOLD: f64 = 50.0` (50% slower than expected)
- Add `MIN_CHAR_SAMPLES_FOR_SPEED: usize = 10` (EMA alpha=0.1 needs ~10 samples for initial value to decay to ~35% influence; 5 samples still has ~59% initial-value bias, too noisy for baseline)
- Remove `DEFAULT_TARGET_CPM` (no longer used by update_stat or stores)

**`BigramStatsStore` struct** (line 102):
- Remove `target_cpm: f64` field and `default_target_cpm()` helper
- `BigramStatsStore::update()` (line 114): Remove `target_time_ms` calculation. Pass-through to `update_stat()` without it.

**`TrigramStatsStore` struct** (line 285):
- Remove `target_cpm: f64` field
- `TrigramStatsStore::update()` (line 293): Remove `target_time_ms` calculation. Pass-through to `update_stat()` without it.

**Remove `get_confidence()`** methods on both stores (lines 121, 300) — they read the deleted `confidence` field. Both are `#[allow(dead_code)]` already.

**Rename `redundancy_score()`** → **`error_anomaly_ratio()`** (line 132):
- Same math internally, just renamed. Returns `e_ab / expected_ab`.

**New methods on `BigramStatsStore`**:

```rust
/// Error anomaly as percentage: (ratio - 1.0) * 100
/// Returns None if bigram has no stats.
pub fn error_anomaly_pct(&self, key: &BigramKey, char_stats: &KeyStatsStore) -> Option<f64> {
    let _stat = self.stats.get(key)?;
    let ratio = self.error_anomaly_ratio(key, char_stats);
    Some((ratio - 1.0) * 100.0)
}

/// Speed anomaly: % slower than user types char_b in isolation.
/// Compares bigram filtered_time_ms to char_b's filtered_time_ms.
/// Returns None if bigram has no stats or char_b has < MIN_CHAR_SAMPLES_FOR_SPEED samples.
pub fn speed_anomaly_pct(&self, key: &BigramKey, char_stats: &KeyStatsStore) -> Option<f64> {
    let stat = self.stats.get(key)?;
    let char_b_stat = char_stats.stats.get(&key.0[1])?;
    if char_b_stat.sample_count < MIN_CHAR_SAMPLES_FOR_SPEED { return None; }
    let ratio = stat.filtered_time_ms / char_b_stat.filtered_time_ms;
    Some((ratio - 1.0) * 100.0)
}
```

**Rename `update_redundancy_streak()`** → **`update_error_anomaly_streak()`** (line 142):
- Same logic, uses renamed constant and renamed field

**New `update_speed_anomaly_streak()`**:
- Same pattern as error streak: call `speed_anomaly_pct()`, compare against `SPEED_ANOMALY_PCT_THRESHOLD`
- If `speed_anomaly_pct()` returns `None` (char baseline unavailable/under-sampled), **hold previous streak value** — don't reset or increment. The bigram simply can't be evaluated for speed yet.
- Requires both bigram samples >= `ANOMALY_MIN_SAMPLES` AND char_b samples >= `MIN_CHAR_SAMPLES_FOR_SPEED` before any streak update occurs.

**New `BigramAnomaly` struct**:
```rust
pub struct BigramAnomaly {
    pub key: BigramKey,
    pub anomaly_pct: f64,
    pub sample_count: usize,
    pub streak: u8,
    pub confirmed: bool,  // streak >= ANOMALY_STREAK_REQUIRED && samples >= MIN_SAMPLES_FOR_FOCUS
}
```

**Replace `focus_eligible_bigrams()` + `watchlist_bigrams()`** with:
- **`error_anomaly_bigrams(&self, char_stats: &KeyStatsStore, unlocked: &[char]) -> Vec<BigramAnomaly>`** — All bigrams with `error_anomaly_ratio > ERROR_ANOMALY_RATIO_THRESHOLD` and `samples >= ANOMALY_MIN_SAMPLES`, sorted by anomaly_pct desc. Each entry's `confirmed` flag = `error_anomaly_streak >= ANOMALY_STREAK_REQUIRED && samples >= MIN_SAMPLES_FOR_FOCUS`.
- **`speed_anomaly_bigrams(&self, char_stats: &KeyStatsStore, unlocked: &[char]) -> Vec<BigramAnomaly>`** — All bigrams where `speed_anomaly_pct() > Some(SPEED_ANOMALY_PCT_THRESHOLD)` and `samples >= ANOMALY_MIN_SAMPLES`, sorted by anomaly_pct desc. Same confirmed logic using `speed_anomaly_streak`.

**Replace `weakest_bigram()`** with **`worst_confirmed_anomaly()`**:
- Takes `char_stats: &KeyStatsStore` and `unlocked: &[char]`
- Collects all confirmed error anomalies and confirmed speed anomalies into a single candidate pool
- Each candidate is `(BigramKey, anomaly_pct, anomaly_type)` where type is `Error` or `Speed`
- **Dedup per bigram**: If a bigram appears in both error and speed lists, keep whichever has higher anomaly_pct (or prefer error on tie)
- Return the single bigram with highest anomaly_pct, or None if no confirmed anomalies
- This eliminates ambiguity about same-bigram-in-both-lists — each bigram gets at most one candidacy

**Update `FocusReasoning` enum** (line 471):
Current variants are: `BigramWins { bigram_difficulty, char_difficulty, char_key }`, `CharWins { char_key, char_difficulty, bigram_best }`, `NoBigrams { char_key }`, `Fallback`.

Replace with:
```rust
pub enum FocusReasoning {
    BigramWins {
        bigram_anomaly_pct: f64,
        anomaly_type: AnomalyType,  // Error or Speed
        char_key: Option<char>,
    },
    CharWins {
        char_key: char,
        bigram_best: Option<(BigramKey, f64)>,
    },
    NoBigrams {
        char_key: char,
    },
    Fallback,
}

pub enum AnomalyType { Error, Speed }
```

**Update `select_focus_target_with_reasoning()`** (line 489):
- Call `worst_confirmed_anomaly()` instead of `weakest_bigram()`
- **Focus priority rule**: Any confirmed bigram anomaly always wins over char focus. Rationale: char focus is the default skill-tree progression mechanism; confirmed bigram anomalies are exceptional problems that survived a conservative gate (3 consecutive drills above threshold + 20 samples). No cross-scale score comparison needed — confirmation itself is the signal.
- When no confirmed bigram anomalies exist, fall back to char focus as before.
- Anomaly_pct is unbounded (e.g. 200% = 3x worse than expected) — this is fine because confirmation gating prevents transient spikes from stealing focus, and the value is only used for ranking among confirmed anomalies, not for threshold comparison against char scores.

**Update `select_focus_target()`** (line 545):
- Same delegation change, pass `char_stats` through

### 2. `src/app.rs` — Streak update call sites & store cleanup

**`target_cpm` removal checklist** (complete audit of all references):

| Location | What | Action |
|---|---|---|
| `ngram_stats.rs:105-106` | `BigramStatsStore.target_cpm` field + serde attr | Remove field |
| `ngram_stats.rs:288-289` | `TrigramStatsStore.target_cpm` field + serde attr | Remove field |
| `ngram_stats.rs:109-111` | `fn default_target_cpm()` helper | Remove function |
| `ngram_stats.rs:11` | `const DEFAULT_TARGET_CPM` | Remove constant |
| `ngram_stats.rs:115` | `BigramStatsStore::update()` target_time_ms calc | Remove line |
| `ngram_stats.rs:294` | `TrigramStatsStore::update()` target_time_ms calc | Remove line |
| `ngram_stats.rs:1386` | Test helper `bigram_stats.target_cpm = DEFAULT_TARGET_CPM` | Remove line |
| `app.rs:1155` | `self.bigram_stats.target_cpm = ...` in rebuild_ngram_stats | Remove line |
| `app.rs:1157` | `self.ranked_bigram_stats.target_cpm = ...` | Remove line |
| `app.rs:1159` | `self.trigram_stats.target_cpm = ...` | Remove line |
| `app.rs:1161` | `self.ranked_trigram_stats.target_cpm = ...` | Remove line |
| `key_stats.rs:37` | `KeyStatsStore.target_cpm` | **KEEP** — used by `update_key()` for char confidence |
| `app.rs:330,332,609,611,1320,1322,1897-1898,1964-1965` | `key_stats.target_cpm = ...` | **KEEP** — KeyStatsStore still uses target_cpm |
| `config.rs:142` | `fn target_cpm()` | **KEEP** — still used by KeyStatsStore |

**At all 6 `update_redundancy_streak` call sites** (lines 899, 915, 1044, 1195, 1212, plus rebuild):
- Rename to `update_error_anomaly_streak()`
- Add parallel call to `update_speed_anomaly_streak()` passing the appropriate `&KeyStatsStore`:
  - `&self.key_stats` for `self.bigram_stats` updates
  - `&self.ranked_key_stats` for `self.ranked_bigram_stats` updates

**Update `select_focus_target` calls** in `generate_drill` (line ~663) and drill header in main.rs:
- Add `ranked_key_stats` parameter (already available at call sites)

### 3. `src/ui/components/stats_dashboard.rs` — Two-column anomaly display

**Replace data structs**:
- Remove `EligibleBigramRow` (line 20) and `WatchlistBigramRow` (line 30)
- Add single `AnomalyBigramRow`:
  ```rust
  pub struct AnomalyBigramRow {
      pub pair: String,
      pub anomaly_pct: f64,
      pub sample_count: usize,
      pub streak: u8,
      pub confirmed: bool,
  }
  ```

**Replace `NgramTabData` fields** (line 39):
- Remove `eligible_bigrams: Vec<EligibleBigramRow>` and `watchlist_bigrams: Vec<WatchlistBigramRow>`
- Add `error_anomalies: Vec<AnomalyBigramRow>` and `speed_anomalies: Vec<AnomalyBigramRow>`

**Replace render functions**:
- Remove `render_eligible_bigrams()` (line 1473) and `render_watchlist_bigrams()` (line 1560)
- Add `render_error_anomalies()` and `render_speed_anomalies()`
- Each renders a table with columns: `Pair | Anomaly% | Samples | Streak`
- Confirmed rows (`.confirmed == true`) use highlight/accent color
- Unconfirmed rows use dimmer/warning color
- Column titles: `" Error Anomalies ({}) "` and `" Speed Anomalies ({}) "`
- Empty states: `" No error anomalies detected"` / `" No speed anomalies detected"`

**Narrow-width adaptation**:
- Wide mode (width >= 60): 50/50 horizontal split, full columns `Pair | Anomaly% | Samples | Streak`
- Narrow mode (width < 60): Stack vertically (error on top, speed below). Compact columns: `Pair | Anom% | Smp`
  - Drop `Streak` column
  - Abbreviate headers
  - This mirrors the existing pattern used by the current eligible/watchlist tables
- **Vertical space budget** (stacked mode): Each panel gets a minimum of 3 data rows (+ 1 header + 1 border = 5 lines). Remaining vertical space is split evenly. If total available height < 10 lines, show only error anomalies panel (speed anomalies are less actionable). This prevents one panel from starving the other.

**Update `render_ngram_tab()`** (line 1308):
- Split the bottom section into two horizontal chunks (50/50)
- Left: `render_error_anomalies()`, Right: `render_speed_anomalies()`
- On narrow terminals (width < 60), stack vertically instead

### 4. `src/main.rs` — Bridge adapter

**`build_ngram_tab_data()`** (~line 2232):
- Call `error_anomaly_bigrams()` and `speed_anomaly_bigrams()` instead of old functions
- Map `BigramAnomaly` → `AnomalyBigramRow`
- Pass `&ranked_key_stats` for speed anomaly computation

**Drill header** (~line 1133): `select_focus_target()` signature change (adding `char_stats` param) will require updating the call here.

---

## Files Modified

1. **`src/engine/ngram_stats.rs`** — Core metrics overhaul (remove confidence from NgramStat, remove target_cpm from stores, add two anomaly systems, new query functions)
2. **`src/app.rs`** — Update streak calls, remove target_cpm initialization, update select_focus_target calls
3. **`src/ui/components/stats_dashboard.rs`** — Two-column anomaly display, new data structs, narrow-width adaptation
4. **`src/main.rs`** — Bridge adapter, select_focus_target call update

---

## Test Updates

- **Rewrite `test_focus_eligible_bigrams_gating`** → `test_error_anomaly_bigrams`: Test that bigrams above error threshold with sufficient samples appear; confirmed flag set correctly based on streak + samples
- **Rewrite `test_watchlist_bigrams_gating`** → split into `test_error_anomaly_confirmation` and `test_speed_anomaly_bigrams`
- **New `test_speed_anomaly_pct`**: Verify speed anomaly calculation against mock char stats; verify None returned when char_b has < MIN_CHAR_SAMPLES_FOR_SPEED (10) samples; verify correct result at exactly 10 samples (boundary)
- **New `test_speed_anomaly_streak_holds_when_char_unavailable`**: Verify streak is not reset when char baseline is insufficient (samples 0, 5, 9 — all below threshold)
- **New `test_speed_anomaly_borderline_baseline`**: Verify behavior at sample count transitions (9 → None, 10 → Some) and that early-session noise at exactly 10 samples produces reasonable anomaly values (not extreme outliers from EMA initialization bias)
- **Update `test_weakest_bigram*`** → `test_worst_confirmed_anomaly*`: Verify it picks highest anomaly across both types, deduplicates per bigram preferring higher pct (error on tie), returns None when nothing confirmed
- **Update focus reasoning tests**: Update `FocusReasoning` variants to new names (`BigramWins` now carries `anomaly_pct` and `anomaly_type` instead of `bigram_difficulty`)
- **Update `build_ngram_tab_data_maps_fields_correctly`**: Check `error_anomalies`/`speed_anomalies` fields with `AnomalyBigramRow` assertions

---

## Verification

1. `cargo build` — no compile errors
2. `cargo test` — all tests pass
3. Manual: N-grams tab shows two columns (Error Anomalies / Speed Anomalies)
4. Manual: Confirmed problem bigrams appear highlighted; unconfirmed appear dimmer
5. Manual: Drill header still shows `Focus: "th"` for bigram focus
6. Manual: Bigrams previously stuck on watchlist due to negative difficulty now appear as confirmed error anomalies
7. Manual: On narrow terminal (< 60 cols), columns stack vertically with compact headers
