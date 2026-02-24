# N-gram Error Tracking for Adaptive Drill Selection

## Context

keydr currently tracks typing errors at the single-character level only. The adaptive algorithm picks the weakest character by confidence score and biases drill text to include words containing that character. This misses **transition difficulties** -- sequences where individual characters are easy but the combination is hard (e.g., same-finger bigrams, awkward hand transitions). Research strongly supports that these transition effects are real and distinct from single-character difficulty.

**Goal:** Add bigram (n=2) and trigram (n=3) error tracking, with a redundancy detection formula that distinguishes genuine transition difficulties from errors that are just proxies for single-character weakness. Integrate problematic bigrams into the adaptive drill selection pipeline. Trigrams are tracked for observation only and not used for drill generation until empirically proven useful.

---

## Research Summary

1. **N-gram tracking is genuinely novel** -- No existing typing tutor does comprehensive n-gram *error* tracking with adaptive drill selection.

2. **Bigrams capture real, distinct information** -- The 136M Keystrokes study (Dhakal et al., CHI 2018) found letter pairs typed by different hands are more predictive of speed than character repetitions. This cannot be inferred from single-char data.

3. **Motor chunking is real** -- The motor cortex plans keystrokes in chunks, not individually. Single-character optimization misses this.

4. **Bigrams are the sweet spot** -- Nearly all keyboard layout research focuses on bigrams. Trigrams likely offer diminishing returns.

---

## Core Innovation: Redundancy Detection

The key question: "Is a high-error bigram just a proxy for a high-error character?"

### Error Rate Estimation (Laplace-smoothed)

Raw error rates are unstable at low sample counts. All error rates use Laplace smoothing:

```
smoothed_error_rate(errors, samples) = (errors + 1) / (samples + 2)
```

This gives a Bayesian prior of 50% error rate that gets pulled toward the true rate as samples accumulate. At 10 samples with 3 errors, this yields 0.333 instead of raw 0.3 -- a small correction. At 2 samples with 1 error, it yields 0.5 instead of raw 0.5 -- stabilizing the estimate.

### Bigram Redundancy Formula

For bigram "ab" with characters `a` and `b`:

```
e_a = smoothed_error_rate(char_a.errors, char_a.samples)
e_b = smoothed_error_rate(char_b.errors, char_b.samples)
e_ab = smoothed_error_rate(bigram_ab.errors, bigram_ab.samples)

expected_ab = 1.0 - (1.0 - e_a) * (1.0 - e_b)
redundancy_ab = e_ab / max(expected_ab, 0.01)
```

### Trigram Redundancy Formula

For trigram "abc", redundancy is computed against BOTH individual chars AND constituent bigrams:

```
// Expected from chars alone (independence assumption)
expected_from_chars = 1.0 - (1.0 - e_a) * (1.0 - e_b) * (1.0 - e_c)

// Expected from bigrams (takes the max -- if either bigram explains the error, no trigram signal)
expected_from_bigrams = max(e_ab, e_bc)

// Use the higher expectation (harder to exceed = more conservative)
expected_abc = max(expected_from_chars, expected_from_bigrams)
redundancy_abc = e_abc / max(expected_abc, 0.01)
```

This ensures trigrams only flag as informative when NEITHER the individual characters NOR constituent bigrams explain the difficulty.

### Focus Eligibility (Stability-Gated)

An n-gram becomes eligible for focus only when ALL conditions hold:

1. `sample_count >= 20` -- minimum statistical reliability
2. `redundancy > 1.5` -- genuine transition difficulty, not a proxy
3. `redundancy_stable == true` -- the redundancy score has been > 1.5 for the last 3 consecutive update checks (prevents focus flapping from noisy estimates)

The **difficulty score** for ranking eligible n-grams:

```
ngram_difficulty = (1.0 - confidence) * redundancy
```

### Worked Examples

**Example 1 -- Proxy (should NOT focus):** User struggles with 's'. `e_s = 0.25`, `e_i = 0.03`. Expected bigram "is" error: `1 - 0.75 * 0.97 = 0.273`. Observed "is" error: `0.28`. Redundancy: `0.28 / 0.273 = 1.03`. This is ~1.0, confirming "is" errors are just 's' errors. Not eligible.

**Example 2 -- Genuine difficulty (should focus):** User is fine with 'e' and 'd' individually. `e_e = 0.04`, `e_d = 0.05`. Expected "ed" error: `1 - 0.96 * 0.95 = 0.088`. Observed "ed" error: `0.22`. Redundancy: `0.22 / 0.088 = 2.5`. This exceeds 1.5 -- the "ed" transition is genuinely hard. Eligible for focus.

**Example 3 -- Trigram vs bigram:** `e_t = 0.03`, `e_h = 0.04`, `e_e = 0.04`. Bigram `e_th = 0.15` (genuine difficulty). Expected trigram "the" from chars: `0.107`. Expected from bigrams: `max(0.15, 0.04) = 0.15`. Observed "the" error: `0.16`. Redundancy: `0.16 / 0.15 = 1.07`. Not significant -- the "th" bigram already explains the trigram difficulty. Trigram NOT eligible.

---

## Confidence Scale

`NgramStat.confidence` uses the same formula as `KeyStat.confidence`:

```
target_time_ms = 60000.0 / target_cpm    // 342.86ms at 175 CPM
confidence = target_time_ms / filtered_time_ms
```

- `confidence < 1.0`: Slower than target (needs practice)
- `confidence == 1.0`: Exactly at target speed
- `confidence > 1.0`: Faster than target (mastered)

For n-grams, `target_time_ms` scales linearly with order: a bigram target is `2 * single_char_target`, a trigram target is `3 * single_char_target`. This is approximate but consistent.

---

## Hesitation Tracking

Hesitations indicate cognitive uncertainty even when the correct key is pressed. The threshold is **relative to the user's rolling baseline**:

```
hesitation_threshold = max(800.0, 2.5 * user_median_transition_ms)
```

Where `user_median_transition_ms` is the median of the user's last 200 inter-keystroke intervals across all drills. The 800ms absolute floor prevents the threshold from being too low for fast typists. The 2.5x multiplier flags transitions that are notably slower than the user's norm.

`user_median_transition_ms` is stored as a single rolling value on the App struct, updated from `per_key_times` after each drill.

---

## N-gram Key Representation

N-gram keys use typed arrays instead of strings to avoid encoding/canonicalization issues:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BigramKey(pub [char; 2]);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrigramKey(pub [char; 3]);
```

**Normalization rules** (applied at extraction boundary in `extract_ngram_events`):
- All characters are Unicode scalar values (Rust `char`) -- no grapheme cluster handling needed since the app only supports ASCII typing
- No case folding -- 'A' and 'a' are distinct (they require different motor actions: shift+a vs a)
- Punctuation is included (transitions to/from punctuation are legitimate motor sequences)
- BACKSPACE characters are filtered out before windowing
- Space characters split windows (no cross-word-boundary n-grams)

---

## Implementation

### Phase 1: Core Data Structures & Extraction

**New file: `src/engine/ngram_stats.rs`**

- `BigramKey(pub [char; 2])` and `TrigramKey(pub [char; 3])` -- typed keys with Hash/Eq/Serialize
- `NgramStat` struct:
  - `filtered_time_ms: f64` -- EMA-smoothed transition time (alpha=0.1)
  - `best_time_ms: f64` -- personal best EMA time
  - `confidence: f64` -- `(target_time_ms * order) / filtered_time_ms`
  - `sample_count: usize` -- total observations
  - `error_count: usize` -- total errors (mistype or hesitation)
  - `hesitation_count: usize` -- total hesitations specifically
  - `recent_times: Vec<f64>` -- last 30 observations
  - `recent_correct: Vec<bool>` -- last 30 correctness values
  - `redundancy_streak: u8` -- consecutive updates where redundancy > 1.5 (for stability gate, max 255)
- `BigramStatsStore` -- `HashMap<BigramKey, NgramStat>` (concrete, not generic)
  - `update(&mut self, key: BigramKey, time_ms: f64, correct: bool, hesitation: bool)`
  - `get_confidence(&self, key: &BigramKey) -> f64`
  - `smoothed_error_rate(&self, key: &BigramKey) -> f64` -- Laplace-smoothed
  - `redundancy_score(&self, key: &BigramKey, char_stats: &KeyStatsStore) -> f64`
  - `weakest_bigram(&self, char_stats: &KeyStatsStore, unlocked: &[char]) -> Option<(BigramKey, f64)>` -- stability-gated
- `TrigramStatsStore` -- `HashMap<TrigramKey, NgramStat>` (concrete, not generic)
  - Same update/query methods as BigramStatsStore
  - `prune(&mut self, max_entries: usize)` -- composite utility pruning (see below)
- Internal: shared helper functions/trait for the common EMA update logic to avoid duplication between bigram and trigram stores
- `BigramEvent` / `TrigramEvent` structs -- `{ key, total_time_ms, correct, has_hesitation }`
- `extract_ngram_events(per_key_times: &[KeyTime], hesitation_threshold: f64) -> (Vec<BigramEvent>, Vec<TrigramEvent>)` -- single pass, returns both orders
- `FocusTarget` enum -- `Char(char) | Bigram(BigramKey)` -- lives in `src/engine/ngram_stats.rs`, re-exported from `src/engine/mod.rs`

**Note:** `KeyStatsStore` needs a new method `smoothed_error_rate(key: char) -> f64` to provide Laplace-smoothed error rates. This requires adding `error_count` to `KeyStat`. Currently `KeyStat` only tracks timing for correct keystrokes -- we need to also count errors. Add `error_count: usize` and `total_count: usize` fields to `KeyStat`, increment in `update_key()`. Use `#[serde(default)]` for backward compat on deserialization.

**Modify: `src/engine/key_stats.rs`** (additive)
- Add `error_count: usize` and `total_count: usize` to `KeyStat` with `#[serde(default)]`
- Add `update_key_error(&mut self, key: char)` -- increments error/total counts without updating timing
- Add `smoothed_error_rate(&self, key: char) -> f64` -- Laplace-smoothed

**Modify: `src/engine/mod.rs`** (additive) -- add `pub mod ngram_stats`, re-export `FocusTarget`

**Extraction detail:** For bigram "th", transition time = `window[1].time_ms`. For trigram "the", transition time = `window[1].time_ms + window[2].time_ms`. The first element's `time_ms` is the transition FROM the previous character and is NOT part of this n-gram.

### Phase 2: Persistence (Replay-Only, No Caching)

**Architecture:** `drill_history` (lesson_history.json) is the **sole source of truth**. N-gram stats are **always rebuilt from drill history** on startup. There are no separate n-gram cache files in this initial implementation. This eliminates all cache coherency concerns at the cost of ~200-500ms startup replay. Caching can be added later as an optimization if rebuild latency becomes problematic.

**Modify: `src/store/schema.rs`** (additive)
- Add concrete `BigramStatsData { stats: BigramStatsStore }` with Default impl
- Add concrete `TrigramStatsData { stats: TrigramStatsStore }` with Default impl
- These types are used for export/import serialization only, not for runtime caching

**Modify: `src/app.rs`** (additive + modify existing)
- Add 4 fields to `App`: `bigram_stats`, `ranked_bigram_stats`, `trigram_stats`, `ranked_trigram_stats`
- Add `user_median_transition_ms: f64` and `transition_buffer: Vec<f64>` (rolling last 200 intervals)
- On startup: rebuild all n-gram stats + hesitation baseline by replaying `drill_history`
- `save_data()`: no n-gram files to save (stats are always derived)

**Trigram pruning:** Max 5,000 entries. Prune by composite utility score after history replay:
```
utility = recency_weight * (1.0 / (drills_since_last_seen + 1))
        + signal_weight * redundancy_score.min(3.0)
        + data_weight * (sample_count as f64).ln()
```
Where `recency_weight=0.3`, `signal_weight=0.5`, `data_weight=0.2`. Entries with highest utility are kept. This preserves rare-but-informative trigrams over frequent-but-noisy ones.

### Phase 3: Drill Integration

**Modify: `src/app.rs` -- `finish_drill()`** (modify existing, after line 847)
- Compute `hesitation_threshold = max(800.0, 2.5 * self.user_median_transition_ms)`
- Call `extract_ngram_events(&result.per_key_times, hesitation_threshold)`
- Update `bigram_stats` and `trigram_stats` with each event
- For incorrect keystrokes: also call `self.key_stats.update_key_error(kt.key)` to build char-level error counts
- Same pattern for ranked stats in the ranked block (after line 854)
- Update `transition_buffer` and recompute `user_median_transition_ms`

**Modify: `src/app.rs` -- `finish_partial_drill()`** -- same pattern

**Hesitation baseline rebuild:** During startup history replay, also accumulate transition times into `transition_buffer` to rebuild `user_median_transition_ms`. This ensures the hesitation threshold is consistent across restarts.

### Phase 4: Adaptive Focus Selection (Bigram Only)

The focus pipeline uses a **thin adapter at the App boundary** rather than changing generator signatures directly. This minimizes cross-cutting risk.

**Modify: `src/app.rs` -- `generate_text()`** (modify existing, line 628)

```rust
// Adapter: compute focus target, then decompose into existing generator knobs
let focus_target = select_focus_target(
    &self.skill_tree, scope, &self.ranked_key_stats, &self.ranked_bigram_stats
);

let (focused_char, focused_bigram) = match &focus_target {
    FocusTarget::Char(ch) => (Some(*ch), None),
    FocusTarget::Bigram(key) => (Some(key.0[0]), Some(key.clone())),
};

// Existing generators use focused_char unchanged
let mut text = generator.generate(&filter, lowercase_focused_char, word_count);
// ... existing capitalize/punctuate/numbers pipeline unchanged ...

// After all generation: if bigram focus, swap some words for bigram-containing words
if let Some(ref bigram) = focused_bigram {
    text = self.apply_bigram_focus(&text, &filter, bigram);
}
```

**New method on `App`: `apply_bigram_focus()`**
- Scans generated words, replaces up to 40% with dictionary words containing the target bigram
- Only replaces when suitable alternatives exist and pass the CharFilter
- Maintains word count and approximate text length
- **Diversity cap:** No more than 3 consecutive bigram-focused words to prevent repetitive feel

This approach keeps ALL existing generator APIs unchanged. If the adapter proves insufficient (e.g., bigram-focused words are too rare in dictionary), we can widen generator APIs in a follow-up.

**Focus selection logic** (new function `select_focus_target()` in `src/engine/ngram_stats.rs`):
1. Compute weakest single character via existing `focused_key()`
2. Compute weakest eligible bigram via `weakest_bigram()` (stability-gated: sample >= 20, redundancy > 1.5 for 3 consecutive checks)
3. If bigram `ngram_difficulty > char_difficulty * 0.8`, focus on bigram
4. Otherwise, fall back to single-char focus

### Phase 5: Information Gain Analysis (Trigram Observation)

**Add to `src/engine/ngram_stats.rs`:**

```rust
pub fn trigram_marginal_gain(
    trigram_stats: &TrigramStatsStore,
    bigram_stats: &BigramStatsStore,
    char_stats: &KeyStatsStore,
) -> f64
```

Computes what fraction of trigrams with >= 20 samples have `redundancy > 1.5` vs their constituent bigrams. Returns a value in `[0.0, 1.0]`.

- Called every 50 drills, result logged to a `trigram_gain_history: Vec<f64>` on the App
- If the most recent 3 measurements all show gain > 10%, trigrams could be promoted to active focus (future work)
- This metric is primarily for analysis -- it answers "are trigrams adding value beyond bigrams for this user?"

### Phase 6: Export/Import

**Modify: `src/store/schema.rs`** (additive) -- add n-gram fields to `ExportData` with `#[serde(default)]`
**Modify: `src/store/json_store.rs`** (additive) -- update `export_all()` to serialize n-gram stats from memory; `import_all()` imports them into drill_history replay pipeline

---

## Performance Budgets

| Operation | Budget | Notes |
|-----------|--------|-------|
| N-gram extraction per drill | < 1ms | Linear scan of ~200-500 keystrokes |
| Stats update per drill | < 1ms | ~400 bigram + ~300 trigram hash map inserts |
| Focus selection | < 5ms | Iterate all bigrams (~2K), filter + rank |
| History replay (full rebuild) | < 500ms | Replay 500 drills x extraction + update (fixture: 500 drills, 300 keystrokes each) |
| Memory for n-gram stores | < 5MB | ~3K bigrams + 5K trigrams x ~200 bytes each |

Benchmark tests enforce extraction (<1ms for 500 keystrokes), update (<1ms for 400 events), and focus selection (<5ms for 3K bigrams) budgets.

---

## Files Summary

| File | Action | Breaking? | What Changes |
|------|--------|-----------|-------------|
| `src/engine/ngram_stats.rs` | **New** | No | All n-gram structs, extraction, redundancy formula, FocusTarget, focus selection |
| `src/engine/mod.rs` | Modify | No (additive) | Add `pub mod ngram_stats`, re-export `FocusTarget` |
| `src/engine/key_stats.rs` | Modify | No (additive) | Add `error_count`/`total_count` to `KeyStat` with `#[serde(default)]`, add `smoothed_error_rate()` |
| `src/store/schema.rs` | Modify | No (additive) | `BigramStatsData`/`TrigramStatsData` types, `ExportData` update with `#[serde(default)]` |
| `src/store/json_store.rs` | Modify | No (additive) | Export/import n-gram data |
| `src/app.rs` | Modify | No (internal) | App fields, `finish_drill()` n-gram extraction, `generate_text()` adapter + `apply_bigram_focus()`, startup replay |
| `src/generator/dictionary.rs` | Unchanged | - | Existing `find_matching` used as-is via adapter |
| `src/generator/phonetic.rs` | Unchanged | - | Existing API used as-is via adapter |

---

## Verification

1. **Unit tests** for `extract_ngram_events` -- verify bigram/trigram extraction from known keystroke sequences, BACKSPACE filtering, space-boundary skipping, hesitation detection at threshold boundary
2. **Unit tests** for `redundancy_score` -- the 3 worked examples above as test cases, plus edge cases (zero samples, all errors, no errors)
3. **Unit tests** for Laplace smoothing -- verify convergence behavior at low and high sample counts
4. **Unit tests** for stability gate -- verify `redundancy_streak` increments/resets correctly, focus eligibility requires 3 consecutive hits
5. **Deterministic integration tests** for focus selection -- seed `SmallRng` with fixed seed, verify tie-breaking behavior between char and bigram focus, verify fallback when no bigrams are eligible
6. **Regression test** -- verify existing single-character focus works unchanged when no bigrams have sufficient samples (cold start path)
7. **Benchmark tests** (non-blocking, `#[bench]` or criterion):
   - Extraction: < 1ms for 500 `KeyTime` entries
   - Update: < 1ms for 400 bigram events
   - Focus selection: < 5ms for 3,000 bigram entries
   - History replay: < 500ms for 500 drills of 300 keystrokes each
8. **Manual test** -- deliberately mistype a specific bigram repeatedly, verify it becomes the focus target and subsequent drills contain words with that bigram

## Future Considerations (Not in Scope)

- **N-gram cache files** for faster startup if replay latency becomes problematic (hybrid append-only cursor approach)
- **Per-order empirical confidence targets** instead of linear scaling (calibrate from user data, log diagnostics)
- **Bigram placement control** in phonetic generator (prefix/medial/suffix weighting) if adapter approach proves insufficient
- **Trigram-driven focus** if marginal gain metric consistently shows > 10% incremental value
