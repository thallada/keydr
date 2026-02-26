# Plan: N-grams Statistics Tab

## Context

The n-gram error tracking system (last commit `e7f57dd`) tracks bigram/trigram transition difficulties and uses them to adapt drill selection. However, there's no visibility into what the system has identified as weak or how it's influencing drills. This plan adds a **[6] N-grams** tab to the Statistics page to surface this data.

---

## Layout

```
 [1] Dashboard  [2] History  [3] Activity  [4] Accuracy  [5] Timing  [6] N-grams

 ┌─ Active Focus ──────────────────────────────────────────────────────────────┐
 │  Focus: Bigram "th"  (difficulty: 1.24)                                    │
 │  Bigram diff 1.24 > char 'n' diff 0.50 x 0.8 threshold                    │
 └─────────────────────────────────────────────────────────────────────────────┘
 ┌─ Eligible Bigrams (3) ────────────────┐┌─ Watchlist ─────────────────────────┐
 │ Pair  Diff  Err%  Exp%  Red  Conf  N  ││ Pair  Red   Samples   Streak       │
 │  th   1.24  18%    7%  2.10  0.41  32 ││  er   1.82   14/20     2/3         │
 │  ed   0.89  22%    9%  1.90  0.53  28 ││  in   1.61    8/20     1/3         │
 │  ng   0.72  14%    8%  1.72  0.58  24 ││  ou   1.53   18/20     1/3         │
 └────────────────────────────────────────┘└───────────────────────────────────┘
 Scope: Global | Bigrams: 142 | Trigrams: 387 | Hesitation: >832ms | Tri-gain: 12.0%

 [ESC] Back  [Tab] Next tab  [1-6] Switch tab
```

---

## Scope Decisions

- **Drill scope**: Tab shows data for `app.drill_scope` (current adaptive scope). A scope label in the summary line makes this explicit (e.g., "Scope: Global" or "Scope: Branch: lowercase").
- **Trigram gain**: Sourced from `app.trigram_gain_history` (computed every 50 ranked drills). Always from ranked stats, consistent with bigram/trigram counts shown. The value is a fraction in `[0.0, 1.0]` (count of signal trigrams / total qualified trigrams), so it is mathematically non-negative. Format: `X.X%` (one decimal). When empty: `--` with note "(computed every 50 drills)".
- **Eligible vs Watchlist**: Strictly disjoint by construction. Watchlist filter explicitly excludes bigrams that pass all eligibility gates.

---

## Layer Boundaries

Domain logic (engine) and presentation (UI) are separated:

- **Engine** (`ngram_stats.rs`): Owns `FocusReasoning` (domain decision explanation), `select_focus_target_with_reasoning()`, filtering/gating/sorting logic for eligible and watchlist bigrams. Returns domain-oriented results.
- **UI** (`stats_dashboard.rs`): Owns `NgramTabData`, `EligibleBigramRow`, `WatchlistBigramRow` (view model structs tailored for rendering columns).
- **Adapter** (`main.rs`): `build_ngram_tab_data()` is the single point that translates engine output → UI view models. All stats store lookups for display columns happen here.

---

## Files to Modify

### 1. `src/engine/ngram_stats.rs` — Domain logic + focus reasoning

**`FocusReasoning` enum** (domain concept — why the target was selected):
```rust
pub enum FocusReasoning {
    BigramWins {
        bigram_difficulty: f64,
        char_difficulty: f64,
        char_key: Option<char>,  // None when no focused char exists
    },
    CharWins {
        char_key: char,
        char_difficulty: f64,
        bigram_best: Option<(BigramKey, f64)>,
    },
    NoBigrams { char_key: char },
    Fallback,
}
```

**`select_focus_target_with_reasoning()`** — Unified function returning `(FocusTarget, FocusReasoning)`. Internally calls `focused_key()` and `weakest_bigram()` once. Handles all four match arms without synthetic values.

**`focus_eligible_bigrams()`** on `BigramStatsStore` — Returns `Vec<(BigramKey, f64 /*difficulty*/, f64 /*redundancy*/)>` sorted by `(difficulty desc, redundancy desc, key lexical asc)`. Same gating as `weakest_bigram()`: sample >= `MIN_SAMPLES_FOR_FOCUS`, streak >= `STABILITY_STREAK_REQUIRED`, redundancy > `STABILITY_THRESHOLD`, difficulty > 0. Returns ALL qualifying entries (no truncation — UI handles truncation to available height).

**`watchlist_bigrams()`** on `BigramStatsStore` — Returns `Vec<(BigramKey, f64 /*redundancy*/)>` sorted by `(redundancy desc, key lexical asc)`. Criteria: redundancy > `STABILITY_THRESHOLD`, sample_count >= 3 (noise floor), AND NOT fully eligible. Returns ALL qualifying entries.

**Export constants** — Make `MIN_SAMPLES_FOR_FOCUS` and `STABILITY_STREAK_REQUIRED` `pub(crate)` so the adapter in `main.rs` can pass them into `NgramTabData` without duplicating values.

### 2. `src/ui/components/stats_dashboard.rs` — View models + rendering

**View model structs** (presentation-oriented, mapped from engine data by adapter):

```rust
pub struct EligibleBigramRow {
    pub pair: String,          // e.g., "th"
    pub difficulty: f64,
    pub error_rate_pct: f64,   // smoothed, as percentage
    pub expected_rate_pct: f64,// from char independence, as percentage
    pub redundancy: f64,
    pub confidence: f64,
    pub sample_count: usize,
}

pub struct WatchlistBigramRow {
    pub pair: String,
    pub redundancy: f64,
    pub sample_count: usize,
    pub redundancy_streak: u8,
}
```

**`NgramTabData` struct** (assembled by `build_ngram_tab_data()` in main.rs):
```rust
pub struct NgramTabData {
    pub focus_target: FocusTarget,
    pub focus_reasoning: FocusReasoning,
    pub eligible: Vec<EligibleBigramRow>,
    pub watchlist: Vec<WatchlistBigramRow>,
    pub total_bigrams: usize,
    pub total_trigrams: usize,
    pub hesitation_threshold_ms: f64,
    pub latest_trigram_gain: Option<f64>,
    pub scope_label: String,
    // Engine thresholds for watchlist progress denominators:
    pub min_samples_for_focus: usize,     // from ngram_stats::MIN_SAMPLES_FOR_FOCUS
    pub stability_streak_required: u8,    // from ngram_stats::STABILITY_STREAK_REQUIRED
}
```

**Add field** to `StatsDashboard`: `ngram_data: Option<&'a NgramTabData>`

**Update constructor**, tab header (add `"[6] N-grams"`), footer (`[1-6]`), `render_tab()` dispatch.

**Rendering methods:**

- **`render_ngram_tab()`** — Vertical layout: focus (4 lines), lists (Min 5), summary (2 lines).

- **`render_ngram_focus()`** — Bordered "Active Focus" block.
  - Line 1: target name in `colors.focused_key()` + bold
  - Line 2: reasoning in `colors.text_pending()`
  - When BigramWins + char_key is None: "Bigram selected (no individual char weakness found)"
  - Empty state: "Complete some adaptive drills to see focus data"

- **`render_eligible_bigrams()`** — Bordered "Eligible Bigrams (N)" block.
  - Header in `colors.accent()` + bold
  - Rows colored by difficulty: `error()` (>1.0), `warning()` (>0.5), `success()` (<=0.5)
  - Columns: `Pair  Diff  Err%  Exp%  Red  Conf  N`
  - Narrow (<38 inner): drop Exp% and Conf
  - Truncate rows to available height
  - Empty state: "No bigrams meet focus criteria yet"

- **`render_watchlist_bigrams()`** — Bordered "Watchlist" block.
  - Columns: `Pair  Red  Samples  Streak`
  - Samples rendered as `n/{data.min_samples_for_focus}`, Streak as `n/{data.stability_streak_required}` — denominators sourced from `NgramTabData` (engine constants), never hardcoded in UI
  - All rows in `colors.warning()`
  - Truncate rows to available height
  - Empty state: "No approaching bigrams"

- **`render_ngram_summary()`** — Single line: scope label, bigram/trigram counts, hesitation threshold, trigram gain.

### 3. `src/main.rs` — Input handling + adapter

**`handle_stats_key()`**:
- `STATS_TAB_COUNT`: 5 → 6
- Add `KeyCode::Char('6') => app.stats_tab = 5` in both branches

**`build_ngram_tab_data(app: &App) -> NgramTabData`** — Dedicated adapter function (single point of engine→UI translation):
- Calls `select_focus_target_with_reasoning()`
- Calls `focus_eligible_bigrams()` and `watchlist_bigrams()`
- Maps engine results to `EligibleBigramRow`/`WatchlistBigramRow` by looking up additional per-bigram stats (error rate, expected rate, confidence, streak) from `app.ranked_bigram_stats` and `app.ranked_key_stats`
- Builds scope label from `app.drill_scope`
- Only called when `app.stats_tab == 5`

**`render_stats()`**: Call `build_ngram_tab_data()` when on tab 5, pass `Some(&data)` to StatsDashboard.

---

## Implementation Order

1. Add `FocusReasoning` enum and `select_focus_target_with_reasoning()` to `ngram_stats.rs`
2. Add `focus_eligible_bigrams()` and `watchlist_bigrams()` to `BigramStatsStore`
3. Add unit tests for steps 1-2
4. Add view model structs (`EligibleBigramRow`, `WatchlistBigramRow`, `NgramTabData`) and `ngram_data` field to `stats_dashboard.rs`
5. Add all rendering methods to `stats_dashboard.rs`
6. Update tab header, footer, `render_tab()` dispatch in `stats_dashboard.rs`
7. Add `build_ngram_tab_data()` adapter + update `render_stats()` in `main.rs`
8. Update `handle_stats_key()` in `main.rs`

---

## Verification

### Unit Tests (in `ngram_stats.rs` test module)

**`test_focus_eligible_bigrams_gating`** — BigramStatsStore with bigrams at boundary conditions:
- sample=25, streak=3, redundancy=2.0 → eligible
- sample=15, streak=3, redundancy=2.0 → excluded (samples < 20)
- sample=25, streak=2, redundancy=2.0 → excluded (streak < 3)
- sample=25, streak=3, redundancy=1.2 → excluded (redundancy <= 1.5)
- sample=25, streak=3, redundancy=2.0, confidence=1.5 → excluded (difficulty <= 0)

**`test_focus_eligible_bigrams_ordering_and_tiebreak`** — 3 eligible bigrams: two with same difficulty but different redundancy, one with lower difficulty. Verify sorted by (difficulty desc, redundancy desc, key lexical asc).

**`test_watchlist_bigrams_gating`** — Bigrams at boundary:
- Fully eligible (sample=25, streak=3) → excluded (goes to eligible list)
- High redundancy, low samples (sample=10) → included
- High redundancy, low streak (sample=25, streak=1) → included
- Low redundancy (1.3) → excluded
- Very few samples (sample=2) → excluded (< 3 noise floor)

**`test_watchlist_bigrams_ordering_and_tiebreak`** — 3 watchlist entries: two with same redundancy. Verify sorted by (redundancy desc, key lexical asc).

**`test_select_focus_with_reasoning_bigram_wins`** — Bigram difficulty > char difficulty * 0.8. Returns `BigramWins` with correct values and `char_key: Some(ch)`.

**`test_select_focus_with_reasoning_char_wins`** — Char difficulty high, bigram < threshold. Returns `CharWins` with `bigram_best` populated.

**`test_select_focus_with_reasoning_no_bigrams`** — No eligible bigrams. Returns `NoBigrams`.

**`test_select_focus_with_reasoning_bigram_only`** — No focused char, bigram exists. Returns `BigramWins` with `char_key: None`.

### Build & Existing Tests
- `cargo build` — no compile errors
- `cargo test` — all existing + new tests pass

### Manual Testing
- Navigate to Statistics → press [6] → see N-grams tab
- Tab/BackTab cycles through all 6 tabs
- With no drill history: empty states shown for all panels
- After several adaptive drills: eligible bigrams appear with plausible data
- Scope label reflects current drill scope
- Verify layout at 80x24 terminal size — confirm column drop at narrow widths keeps header/data aligned
