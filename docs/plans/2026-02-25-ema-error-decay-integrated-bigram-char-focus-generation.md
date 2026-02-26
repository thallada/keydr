# Plan: EMA Error Decay + Integrated Bigram/Char Focus Generation

## Context

Two problems with the current n-gram focus system:

1. **Focus stickiness**: Bigram anomaly uses cumulative `(error_count+1)/(sample_count+2)` Laplace smoothing. A bigram with 20 errors / 25 samples would need ~54 consecutive correct strokes to drop below the 1.5x threshold. Once confirmed, a bigram dominates focus for many drills even as the user visibly improves, while worse bigrams can't take over.

2. **Post-processing bigram focus causes repetition**: When a bigram is in focus, `apply_bigram_focus()` post-processes finished text by replacing 40% of words with dictionary words containing the bigram. This selects randomly from candidates with no duplicate tracking, causing repeated words. It also means the bigram doesn't influence the actual word selection — it's bolted on after generation and overrides the focused char (the weakest char gets replaced by bigram[0]).

This plan addresses both: (A) switch error rate to EMA so anomalies respond to recent performance, and (B) integrate bigram focus directly into the word selection algorithm alongside char focus, enabling both to be active simultaneously.

---

## Part A: EMA Error Rate Decay

### Approach

Add an `error_rate_ema: f64` field to both `NgramStat` and `KeyStat`, updated via exponential moving average on each keystroke (same pattern as existing `filtered_time_ms`). Use this EMA for all anomaly computations instead of cumulative `(error_count+1)/(sample_count+2)`.

Both bigram AND char error rates must use EMA — `error_anomaly_ratio` divides one by the other, so asymmetric decay would distort the comparison.

**Alpha = 0.1** (same as timing EMA). Half-life ~7 samples. A bigram at 30% error rate recovering with all-correct strokes: drops below 1.5x threshold after ~15 correct (~2 drills). This is responsive without being twitchy.

### Changes

#### `src/engine/ngram_stats.rs`

**NgramStat struct** (line 34):
- Add `error_rate_ema: f64` with `#[serde(default = "default_error_rate_ema")]` and default value `0.5`
- Add `fn default_error_rate_ema() -> f64 { 0.5 }` (Laplace-equivalent neutral prior)
- Remove `recent_correct: Vec<bool>` — superseded by EMA and never read

**`update_stat()`** (line 67):
- After existing `error_count` increment, add EMA update:
  ```rust
  let error_signal = if correct { 0.0 } else { 1.0 };
  if stat.sample_count == 1 {
      stat.error_rate_ema = error_signal;
  } else {
      stat.error_rate_ema = EMA_ALPHA * error_signal + (1.0 - EMA_ALPHA) * stat.error_rate_ema;
  }
  ```
- Remove `recent_correct` push/trim logic (lines 89-92)
- Keep `error_count` and `sample_count` (needed for gating thresholds and display)

**`smoothed_error_rate_raw()`** (line 95): Remove. After `smoothed_error_rate()` on both BigramStatsStore and TrigramStatsStore switch to `error_rate_ema`, this function has no callers.

**`BigramStatsStore::smoothed_error_rate()`** (line 120): Change to return `stat.error_rate_ema` instead of `smoothed_error_rate_raw(stat.error_count, stat.sample_count)`.

**`TrigramStatsStore::smoothed_error_rate()`** (line 333): Same change — return `stat.error_rate_ema`.

**`error_anomaly_ratio()`** (line 123): No changes needed — it calls `self.smoothed_error_rate()` and `char_stats.smoothed_error_rate()`, which now both return EMA values.

**Default for NgramStat** (line 50): Set `error_rate_ema: 0.5` (neutral — same as Laplace `(0+1)/(0+2)`).

#### `src/engine/key_stats.rs`

**KeyStat struct** (line 7):
- Add `error_rate_ema: f64` with `#[serde(default = "default_error_rate_ema")]` and default value `0.5`
- Add `fn default_error_rate_ema() -> f64 { 0.5 }` helper
- **Note**: KeyStat IS persisted to disk. The `#[serde(default)]` ensures backward compat — existing data without the field gets 0.5.

**`update_key()`** (line 50) — called for correct strokes:
- Add EMA update: `stat.error_rate_ema = if stat.total_count == 1 { 0.0 } else { EMA_ALPHA * 0.0 + (1.0 - EMA_ALPHA) * stat.error_rate_ema }`
- Use `total_count` (already incremented on the line before) to detect first sample

**`update_key_error()`** (line 83) — called for error strokes:
- Add EMA update: `stat.error_rate_ema = if stat.total_count == 1 { 1.0 } else { EMA_ALPHA * 1.0 + (1.0 - EMA_ALPHA) * stat.error_rate_ema }`

**`smoothed_error_rate()`** (line 90): Change to return `stat.error_rate_ema` (or 0.5 for missing keys).

#### `src/app.rs`

**`rebuild_ngram_stats()`** (line 1155):
- Reset `error_rate_ema` to `0.5` alongside `error_count` and `total_count` for KeyStat stores (lines 1165-1172)
- NgramStat stores already reset to `Default` which has `error_rate_ema: 0.5`
- The replay loop (line 1177) naturally rebuilds EMA by calling `update_stat()` and `update_key()`/`update_key_error()` in order

No other app.rs changes needed — the streak update and focus selection code reads through `error_anomaly_ratio()` which now uses EMA values transparently.

---

## Part B: Integrated Bigram + Char Focus Generation

### Approach

Replace the exclusive `FocusTarget` enum (either char OR bigram) with a `FocusSelection` struct that carries both independently. The weakest char comes from skill_tree progression; the worst bigram anomaly comes from the anomaly system. Both feed into the `PhoneticGenerator` simultaneously. Remove `apply_bigram_focus()` post-processing entirely.

### Changes

#### `src/engine/ngram_stats.rs` — Focus selection

**Replace `FocusTarget` enum** (line 510):
```rust
// Old
pub enum FocusTarget { Char(char), Bigram(BigramKey) }

// New
#[derive(Clone, Debug, PartialEq)]
pub struct FocusSelection {
    pub char_focus: Option<char>,
    pub bigram_focus: Option<(BigramKey, f64, AnomalyType)>,
}
```

**Replace `FocusReasoning` enum** (line 523):
```rust
// Old
pub enum FocusReasoning {
    BigramWins { bigram_anomaly_pct: f64, anomaly_type: AnomalyType, char_key: Option<char> },
    CharWins { char_key: char, bigram_best: Option<(BigramKey, f64)> },
    NoBigrams { char_key: char },
    Fallback,
}

// New — reasoning is now just the selection itself (both fields self-describe)
// FocusReasoning is removed; FocusSelection carries all needed info.
```

**Simplify `select_focus_target_with_reasoning()`** → **`select_focus()`**:
```rust
pub fn select_focus(
    skill_tree: &SkillTree,
    scope: DrillScope,
    ranked_key_stats: &KeyStatsStore,
    ranked_bigram_stats: &BigramStatsStore,
) -> FocusSelection {
    let unlocked = skill_tree.unlocked_keys(scope);
    let char_focus = skill_tree.focused_key(scope, ranked_key_stats);
    let bigram_focus = ranked_bigram_stats.worst_confirmed_anomaly(ranked_key_stats, &unlocked);
    FocusSelection { char_focus, bigram_focus }
}
```

Remove `select_focus_target()` and `select_focus_target_with_reasoning()` — replaced by `select_focus()`.

#### `src/generator/mod.rs` — Trait update

**Update `TextGenerator` trait** (line 14):
```rust
pub trait TextGenerator {
    fn generate(
        &mut self,
        filter: &CharFilter,
        focused_char: Option<char>,
        focused_bigram: Option<[char; 2]>,
        word_count: usize,
    ) -> String;
}
```

#### `src/generator/phonetic.rs` — Integrated word selection

**`generate()` method** — rewrite word selection with tiered approach:

Note: `find_matching(filter, None)` is used (not `focused_char`) because we do our own tiering below. `find_matching` returns ALL words matching the CharFilter — the `focused` param only sorts, never filters — but passing `None` avoids an unnecessary sort we'd discard anyway.

```rust
fn generate(
    &mut self,
    filter: &CharFilter,
    focused_char: Option<char>,
    focused_bigram: Option<[char; 2]>,
    word_count: usize,
) -> String {
    let matching_words: Vec<String> = self.dictionary
        .find_matching(filter, None)  // no char-sort; we tier ourselves
        .iter().map(|s| s.to_string()).collect();
    let use_real_words = matching_words.len() >= MIN_REAL_WORDS;

    // Pre-categorize words into tiers for real-word mode
    let bigram_str = focused_bigram.map(|b| format!("{}{}", b[0], b[1]));
    let focus_char_lower = focused_char.filter(|ch| ch.is_ascii_lowercase());

    let (bigram_indices, char_indices, other_indices) = if use_real_words {
        let mut bi = Vec::new();
        let mut ci = Vec::new();
        let mut oi = Vec::new();
        for (i, w) in matching_words.iter().enumerate() {
            if bigram_str.as_ref().is_some_and(|b| w.contains(b.as_str())) {
                bi.push(i);
            } else if focus_char_lower.is_some_and(|ch| w.contains(ch)) {
                ci.push(i);
            } else {
                oi.push(i);
            }
        }
        (bi, ci, oi)
    } else {
        (vec![], vec![], vec![])
    };

    let mut words: Vec<String> = Vec::new();
    let mut recent: Vec<String> = Vec::new(); // anti-repeat window

    for _ in 0..word_count {
        if use_real_words {
            let word = self.pick_tiered_word(
                &matching_words,
                &bigram_indices,
                &char_indices,
                &other_indices,
                &recent,
            );
            recent.push(word.clone());
            if recent.len() > 4 { recent.remove(0); }
            words.push(word);
        } else {
            let word = self.generate_phonetic_word(
                filter, focused_char, focused_bigram,
            );
            words.push(word);
        }
    }
    words.join(" ")
}
```

**New `pick_tiered_word()` method**:
```rust
fn pick_tiered_word(
    &mut self,
    all_words: &[String],
    bigram_indices: &[usize],
    char_indices: &[usize],
    other_indices: &[usize],
    recent: &[String],
) -> String {
    // Tier selection probabilities:
    // Both available: 40% bigram, 30% char, 30% other
    // Only bigram:    50% bigram, 50% other
    // Only char:      70% char, 30% other (matches current behavior)
    // Neither:        100% other
    //
    // Try up to 6 times to avoid repeating a recent word.
    for _ in 0..6 {
        let tier = self.select_tier(bigram_indices, char_indices, other_indices);
        let idx = tier[self.rng.gen_range(0..tier.len())];
        let word = &all_words[idx];
        if !recent.contains(word) {
            return word.clone();
        }
    }
    // Fallback: accept any non-recent word from full pool
    let idx = self.rng.gen_range(0..all_words.len());
    all_words[idx].clone()
}
```

**`select_tier()` helper**: Returns reference to the tier to sample from based on availability and probability roll. Only considers a tier "available" if it has >= 2 words (prevents unavoidable repeats when a tier has just 1 word and the anti-repeat window rejects it). Falls through to the next tier when the selected tier is too small.

**`try_generate_word()` / `generate_phonetic_word()`** — add bigram awareness for Markov fallback:
- Accept `focused_bigram: Option<[char; 2]>` parameter
- Only attempt bigram forcing when both chars pass the CharFilter (avoids pathological starts when bigram chars are rare/unavailable in current filter scope)
- When eligible: 30% chance to start word with bigram[0] and force bigram[1] as second char, then continue Markov chain from `[' ', bigram[0], bigram[1]]` prefix
- Falls back to existing focused_char logic otherwise

#### `src/generator/code_syntax.rs` + `src/generator/passage.rs`

Add `_focused_bigram: Option<[char; 2]>` parameter to their `generate()` signatures (ignored, matching trait).

#### `src/app.rs` — Pipeline update

**`generate_text()`** (line 653):
- Call `select_focus()` (new function) instead of `select_focus_target()`
- Extract `focused_char` from `selection.char_focus` (the actual weakest char)
- Extract `focused_bigram` from `selection.bigram_focus.map(|(k, _, _)| k.0)`
- Pass both to `generator.generate(filter, focused_char, focused_bigram, word_count)`
- **Remove** the `apply_bigram_focus()` call (lines 784-787)
- Post-processing passes (capitalize, punctuate, numbers, code_patterns) continue to receive `focused_char` — this is now the real weakest char, not the bigram's first char

**Remove `apply_bigram_focus()`** method (lines 1087-1131) entirely.

**Store `FocusSelection`** on App:
- Add `pub current_focus: Option<FocusSelection>` field to App (default `None`)
- Set in `generate_text()` right after `select_focus()` — captures the focus that was actually used to generate the current drill's text
- **Lifecycle**: Set when drill starts (in `generate_text()`). Persists through the drill result screen (so the user sees what was in focus for the drill they just completed). Cleared to `None` when: starting the next drill (overwritten), leaving drill screen, changing drill scope/mode, or on import/reset. This is a snapshot, not live-recomputed — the header always shows what generated the current text.
- Used by drill header display in main.rs (reads `app.current_focus` instead of re-calling `select_focus()`)

#### `src/main.rs` — Drill header + stats adapter

**Drill header** (line 1134):
- Read `app.current_focus` to build focus_text (no re-computation — shows what generated the text)
- Display format: `Focus: 'n' + "th"` (both), `Focus: 'n'` (char only), `Focus: "th"` (bigram only)
- Replace the current `select_focus_target()` call with reading the stored selection
- When `current_focus` is `None`, show no focus text

**`build_ngram_tab_data()`** (line 2253):
- Call `select_focus()` instead of `select_focus_target_with_reasoning()`
- Update `NgramTabData` struct: replace `focus_target: FocusTarget` and `focus_reasoning: FocusReasoning` with `focus: FocusSelection`

#### `src/ui/components/stats_dashboard.rs` — Focus panel

**`NgramTabData`** (line 28):
- Replace `focus_target: FocusTarget` and `focus_reasoning: FocusReasoning` with `focus: FocusSelection`
- Remove `FocusTarget` and `FocusReasoning` imports

**`render_ngram_focus()`** (line 1352):
- Show both focus targets when both active:
  - Line 1: `Focus: Char 'n' + Bigram "th"` (or just one if only one active)
  - Line 2: Details — `Char 'n': weakest key | Bigram "th": error anomaly 250%`
- When neither active: show fallback message
- Rendering adapts based on which focuses are present

---

## Files Modified

1. **`src/engine/ngram_stats.rs`** — EMA field on NgramStat, EMA-based smoothed_error_rate, `FocusSelection` struct, `select_focus()`, remove old FocusTarget/FocusReasoning
2. **`src/engine/key_stats.rs`** — EMA field on KeyStat, EMA updates in update_key/update_key_error, EMA-based smoothed_error_rate
3. **`src/generator/mod.rs`** — TextGenerator trait: add `focused_bigram` parameter
4. **`src/generator/phonetic.rs`** — Tiered word selection with bigram+char, anti-repeat window, Markov bigram awareness
5. **`src/generator/code_syntax.rs`** — Add ignored `focused_bigram` parameter
6. **`src/generator/passage.rs`** — Add ignored `focused_bigram` parameter
7. **`src/app.rs`** — Use `select_focus()`, pass both focuses to generator, remove `apply_bigram_focus()`, store `current_focus`
8. **`src/main.rs`** — Update drill header, update `build_ngram_tab_data()` adapter
9. **`src/ui/components/stats_dashboard.rs`** — Update NgramTabData, render_ngram_focus for dual focus display

---

## Test Updates

### Part A (EMA)
- **Update `test_error_anomaly_bigrams`**: Set `error_rate_ema` directly instead of relying on cumulative error_count/sample_count for anomaly ratio computation
- **Update `test_worst_confirmed_anomaly_dedup`** and **`_prefers_error_on_tie`**: Same — set EMA values
- **New `test_error_rate_ema_decay`**: Verify that after N correct strokes, error_rate_ema drops as expected. Verify anomaly ratio crosses below threshold after reasonable recovery (~15 correct strokes from 30% error rate).
- **New `test_error_rate_ema_rebuild_from_history`**: Verify that rebuilding from drill history produces same EMA as live updates (deterministic replay)
- **New `test_ema_ranking_stability_during_recovery`**: Two bigrams both confirmed. Bigram A has higher anomaly. User corrects bigram A over several drills while bigram B stays bad. Verify that A's anomaly drops below B's and B becomes the new worst_confirmed_anomaly — clean handoff without oscillation.
- **Update key_stats tests**: Verify EMA updates in `update_key()` and `update_key_error()`, backward compat (serde default)

### Part B (Integrated focus)
- **Replace focus reasoning tests** (`test_select_focus_with_reasoning_*`): Replace with `test_select_focus_*` testing `FocusSelection` struct — verify both char_focus and bigram_focus are populated independently
- **New `test_phonetic_bigram_focus_increases_bigram_words`**: Generate 1200 words with focused_bigram, verify significantly more words contain the bigram than without
- **New `test_phonetic_dual_focus_no_excessive_repeats`**: Generate text with both focuses, verify no word appears > 3 times consecutively
- **Update `build_ngram_tab_data_maps_fields_correctly`**: Update for `FocusSelection` struct instead of FocusTarget/FocusReasoning
- **New `test_find_matching_focused_is_sort_only`** (in `dictionary.rs` or `phonetic.rs`): Verify that `find_matching(filter, Some('k'))` and `find_matching(filter, None)` return the same set of words (same membership, potentially different order). Guards against future regressions where focused param accidentally becomes a filter.
- No `apply_bigram_focus` tests exist to remove (method was untested)

---

## Verification

1. `cargo build` — no compile errors
2. `cargo test` — all tests pass
3. Manual: Start adaptive drill, observe both char and bigram appearing in focus header
4. Manual: Verify drill text contains focused bigram words AND focused char words mixed naturally
5. Manual: Verify no excessive word repetition (the old apply_bigram_focus problem)
6. Manual: Practice a bigram focus target correctly for 2-3 drills → verify it drops out of focus and a different bigram (or char-only) takes over
7. Manual: N-grams tab shows both focuses in the Active Focus panel
8. Manual: Narrow terminal (<60 cols) stacks anomaly panels vertically; very short terminal (<10 rows available for panels) shows only error anomalies panel; focus panel always shows at least line 1
