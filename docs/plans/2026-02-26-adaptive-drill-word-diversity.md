# Adaptive Drill Word Diversity

## Context

When adaptive drills focus on characters/bigrams with few matching dictionary words, the same words repeat excessively both within and across drills. Currently:

- **Within-drill dedup** uses a sliding window of only 4 words — too small when the matching word pool is small
- **Cross-drill**: no tracking at all — each drill creates a fresh `PhoneticGenerator` with no memory of previous drills
- **Dictionary vs phonetic is binary**: if `matching_words >= 15` use dictionary only, if `< 15` use phonetic only. A pool of 16 words gets 100% dictionary (lots of repeats), while 14 gets 0% dictionary

## Changes

### 1. Cross-drill word history

Add `adaptive_word_history: VecDeque<HashSet<String>>` to `App` that tracks words from the last 5 adaptive drills. Pass a flattened `HashSet<String>` into `PhoneticGenerator::new()`.

**Word normalization**: Capture words from the generator output *before* capitalization/punctuation/numbers post-processing (the `generator.generate()` call in `generate_text()` produces lowercase-only text). This means words in history are always lowercase ASCII with no punctuation — no normalization function needed since the generator already guarantees this format.

**`src/app.rs`**:
- Add `adaptive_word_history: VecDeque<HashSet<String>>` to `App` struct, initialize empty
- In `generate_text()`, before creating the generator: flatten history into `HashSet` and pass to constructor
- After `generator.generate()` returns (before capitalization/punctuation): `split_whitespace()` into a `HashSet`, push to history, pop front if `len > 5`

**Lifecycle/reset rules**:
- Clear `adaptive_word_history` when `drill_mode` changes away from `Adaptive` (i.e., switching to Code/Passage mode)
- Clear when `drill_scope` changes (switching between branches or global/branch)
- Do NOT persist across app restarts — session-local only (it's a `VecDeque`, not serialized)
- Do NOT clear on gradual key unlocks — as the skill tree progresses one key at a time, history should carry over to maintain cross-drill diversity within the same learning progression
- The effective "adaptive context key" is `(drill_mode, drill_scope)` — history clears when either changes. Other parameters (focus char, focus bigram, filter) change naturally within a learning progression and should not trigger resets
- This prevents cross-contamination between unrelated drill contexts while preserving continuity during normal adaptive flow

**`src/generator/phonetic.rs`**:
- Add `cross_drill_history: HashSet<String>` field to `PhoneticGenerator`
- Update constructor to accept it
- In `pick_tiered_word()`, use weighted suppression instead of hard exclusion:
  - When selecting a candidate word, if it's in within-drill `recent`, always reject
  - If it's in `cross_drill_history`, accept it with reduced probability based on pool coverage:
    - Guard: if pool is empty, skip suppression logic entirely (fall through to phonetic generation in hybrid mode)
    - `history_coverage = cross_drill_history.intersection(pool).count() as f64 / pool.len() as f64`
    - `accept_prob = 0.15 + 0.60 * history_coverage` (range: 15% when history covers few pool words → 75% when history covers most of the pool)
    - This prevents over-suppression in small pools where history covers most words, while still penalizing repeats in large pools
  - Scale attempt count to `pool_size.clamp(6, 12)` with final fallback accepting any non-recent word
  - Compute `accept_prob` once at the start of `generate()` alongside tier categorization (not per-attempt)

### 2. Hybrid dictionary + phonetic mode

Replace the binary threshold with a gradient that mixes dictionary and phonetic words.

**`src/generator/phonetic.rs`**:
- Change constants: `MIN_REAL_WORDS = 8` (below: phonetic only), add `FULL_DICT_THRESHOLD = 60` (above: dictionary only)
- Calculate `dict_ratio` as linear interpolation: `(count - 8) / (60 - 8)` clamped to `[0.0, 1.0]`
- In the word generation loop, for each word: roll against `dict_ratio` to decide dictionary vs phonetic
- Tier categorization still happens when `count >= MIN_REAL_WORDS` (needed for dictionary picks)
- Phonetic words also participate in the `recent` dedup window (already handled since all words push to `recent`)

### 3. Scale within-drill dedup window

Replace the fixed window of 4 with a window proportional to the **filtered dictionary match count** (the `matching_words` vec computed at the top of `generate()`):
- `pool_size <= 20`: window = `pool_size.saturating_sub(1).max(4)`
- `pool_size > 20`: window = `(pool_size / 4).min(20)`
- In hybrid mode, this is based on the dictionary pool size regardless of phonetic mixing — phonetic words add diversity naturally, so the window governs dictionary repeat pressure

### 4. Tests

All tests use seeded `SmallRng::seed_from_u64()` for determinism (existing pattern in codebase).

**Update existing tests**: Add `HashSet::new()` to `PhoneticGenerator::new()` constructor calls (3 tests).

**New tests** (all use `SmallRng::seed_from_u64()` for determinism):

1. **Cross-drill history suppresses repeats**: Generate drill 1 with seeded RNG and constrained filter (~20 matching words), collect word set. Generate drill 2 with same filter but different seed, no history — compute Jaccard index as baseline. Generate drill 2 again with drill 1's words as history — compute Jaccard index. Assert history Jaccard is at least 0.15 lower than baseline Jaccard (i.e., measurably less overlap). Use 100-word drills.

2. **Hybrid mode produces mixed output**: Use a filter that yields ~30 dictionary matches. Generate 500 words with seeded RNG. Collect output words and check against the dictionary match set. With ~30 matches, `dict_ratio ≈ 0.42`. Since the seed is fixed, the output is deterministic — the band of 25%-65% accommodates potential future seed changes rather than runtime variance. Assert dictionary word percentage is within this range, and document the actual observed value for the chosen seed in a comment.

3. **Boundary conditions**: With 5 matching words → assert 0% dictionary words (all phonetic). With 100+ matching words → assert 100% dictionary words. Seeded RNG.

4. **Weighted suppression graceful degradation**: Create a pool of 10 words with history containing 8 of them. Generate 50 words. Verify no panics, output is non-empty, and history words still appear (suppression is soft, not hard exclusion).

## Files to modify

- `src/generator/phonetic.rs` — core changes: hybrid mixing, cross-drill history field, weighted suppression in `pick_tiered_word`, dedup window scaling
- `src/app.rs` — add `adaptive_word_history` field, wire through `generate_text()`, add reset logic on mode/scope changes
- `src/generator/mod.rs` — no changes (`TextGenerator` trait signature unchanged for API stability; the `cross_drill_history` parameter is internal to `PhoneticGenerator`'s constructor, not the trait interface)

## Verification

1. `cargo test` — all existing and new tests pass
2. Manual test: start adaptive drill on an early skill tree branch (few unlocked letters, ~15-30 matching words). Run 5+ consecutive drills. Measure: unique words across 5 drills should be notably higher than before (target: >70% unique across 5 drills for pools of 20+ words)
3. Full alphabet test: with all keys unlocked, behavior should be essentially unchanged (dict_ratio ≈ 1.0, large pool, no phonetic mixing)
4. Scope change test: switch between branch drill and global drill, verify no stale history leaks
