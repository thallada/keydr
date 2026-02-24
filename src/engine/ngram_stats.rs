use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::engine::key_stats::KeyStatsStore;
use crate::engine::skill_tree::{DrillScope, SkillTree};
use crate::keyboard::display::BACKSPACE;
use crate::session::result::KeyTime;

const EMA_ALPHA: f64 = 0.1;
const DEFAULT_TARGET_CPM: f64 = 175.0;
const MAX_RECENT: usize = 30;
const STABILITY_THRESHOLD: f64 = 1.5;
const STABILITY_STREAK_REQUIRED: u8 = 3;
const MIN_SAMPLES_FOR_FOCUS: usize = 20;
const MAX_TRIGRAM_ENTRIES: usize = 5000;

// ---------------------------------------------------------------------------
// N-gram keys
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BigramKey(pub [char; 2]);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrigramKey(pub [char; 3]);

// ---------------------------------------------------------------------------
// NgramStat
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NgramStat {
    pub filtered_time_ms: f64,
    pub best_time_ms: f64,
    pub confidence: f64,
    pub sample_count: usize,
    pub error_count: usize,
    pub hesitation_count: usize,
    pub recent_times: Vec<f64>,
    pub recent_correct: Vec<bool>,
    pub redundancy_streak: u8,
    #[serde(default)]
    pub last_seen_drill_index: u32,
}

impl Default for NgramStat {
    fn default() -> Self {
        Self {
            filtered_time_ms: 1000.0,
            best_time_ms: f64::MAX,
            confidence: 0.0,
            sample_count: 0,
            error_count: 0,
            hesitation_count: 0,
            recent_times: Vec::new(),
            recent_correct: Vec::new(),
            redundancy_streak: 0,
            last_seen_drill_index: 0,
        }
    }
}

fn update_stat(stat: &mut NgramStat, time_ms: f64, correct: bool, hesitation: bool, target_time_ms: f64, drill_index: u32) {
    stat.last_seen_drill_index = drill_index;
    stat.sample_count += 1;
    if !correct {
        stat.error_count += 1;
    }
    if hesitation {
        stat.hesitation_count += 1;
    }

    if stat.sample_count == 1 {
        stat.filtered_time_ms = time_ms;
    } else {
        stat.filtered_time_ms = EMA_ALPHA * time_ms + (1.0 - EMA_ALPHA) * stat.filtered_time_ms;
    }

    stat.best_time_ms = stat.best_time_ms.min(stat.filtered_time_ms);
    stat.confidence = target_time_ms / stat.filtered_time_ms;

    stat.recent_times.push(time_ms);
    if stat.recent_times.len() > MAX_RECENT {
        stat.recent_times.remove(0);
    }
    stat.recent_correct.push(correct);
    if stat.recent_correct.len() > MAX_RECENT {
        stat.recent_correct.remove(0);
    }
}

fn smoothed_error_rate_raw(errors: usize, samples: usize) -> f64 {
    (errors as f64 + 1.0) / (samples as f64 + 2.0)
}

// ---------------------------------------------------------------------------
// BigramStatsStore
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BigramStatsStore {
    pub stats: HashMap<BigramKey, NgramStat>,
    #[serde(default = "default_target_cpm")]
    pub target_cpm: f64,
}

fn default_target_cpm() -> f64 {
    DEFAULT_TARGET_CPM
}

impl BigramStatsStore {
    pub fn update(&mut self, key: BigramKey, time_ms: f64, correct: bool, hesitation: bool, drill_index: u32) {
        let target_time_ms = 2.0 * 60000.0 / self.target_cpm;
        let stat = self.stats.entry(key).or_default();
        update_stat(stat, time_ms, correct, hesitation, target_time_ms, drill_index);
    }

    #[allow(dead_code)]
    pub fn get_confidence(&self, key: &BigramKey) -> f64 {
        self.stats.get(key).map(|s| s.confidence).unwrap_or(0.0)
    }

    pub fn smoothed_error_rate(&self, key: &BigramKey) -> f64 {
        match self.stats.get(key) {
            Some(s) => smoothed_error_rate_raw(s.error_count, s.sample_count),
            None => smoothed_error_rate_raw(0, 0),
        }
    }

    pub fn redundancy_score(&self, key: &BigramKey, char_stats: &KeyStatsStore) -> f64 {
        let e_a = char_stats.smoothed_error_rate(key.0[0]);
        let e_b = char_stats.smoothed_error_rate(key.0[1]);
        let e_ab = self.smoothed_error_rate(key);
        let expected_ab = 1.0 - (1.0 - e_a) * (1.0 - e_b);
        e_ab / expected_ab.max(0.01)
    }

    /// Update redundancy streak for a bigram given current char stats.
    /// Call this after updating the bigram stats.
    pub fn update_redundancy_streak(&mut self, key: &BigramKey, char_stats: &KeyStatsStore) {
        let redundancy = self.redundancy_score(key, char_stats);
        if let Some(stat) = self.stats.get_mut(key) {
            if redundancy > STABILITY_THRESHOLD {
                stat.redundancy_streak = stat.redundancy_streak.saturating_add(1);
            } else {
                stat.redundancy_streak = 0;
            }
        }
    }

    /// Find the weakest eligible bigram (stability-gated).
    /// Only considers bigrams whose chars are all in `unlocked`.
    pub fn weakest_bigram(
        &self,
        char_stats: &KeyStatsStore,
        unlocked: &[char],
    ) -> Option<(BigramKey, f64)> {
        let mut best: Option<(BigramKey, f64)> = None;

        for (key, stat) in &self.stats {
            // Must be composed of unlocked chars
            if !unlocked.contains(&key.0[0]) || !unlocked.contains(&key.0[1]) {
                continue;
            }
            // Minimum samples
            if stat.sample_count < MIN_SAMPLES_FOR_FOCUS {
                continue;
            }
            // Stability gate
            if stat.redundancy_streak < STABILITY_STREAK_REQUIRED {
                continue;
            }
            let redundancy = self.redundancy_score(key, char_stats);
            if redundancy <= STABILITY_THRESHOLD {
                continue;
            }
            // ngram_difficulty = (1.0 - confidence) * redundancy
            let difficulty = (1.0 - stat.confidence) * redundancy;
            if difficulty <= 0.0 {
                continue;
            }
            match best {
                Some((_, best_diff)) if difficulty > best_diff => {
                    best = Some((key.clone(), difficulty));
                }
                None => {
                    best = Some((key.clone(), difficulty));
                }
                _ => {}
            }
        }

        best
    }
}

// ---------------------------------------------------------------------------
// TrigramStatsStore
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrigramStatsStore {
    pub stats: HashMap<TrigramKey, NgramStat>,
    #[serde(default = "default_target_cpm")]
    pub target_cpm: f64,
}

impl TrigramStatsStore {
    pub fn update(&mut self, key: TrigramKey, time_ms: f64, correct: bool, hesitation: bool, drill_index: u32) {
        let target_time_ms = 3.0 * 60000.0 / self.target_cpm;
        let stat = self.stats.entry(key).or_default();
        update_stat(stat, time_ms, correct, hesitation, target_time_ms, drill_index);
    }

    #[allow(dead_code)]
    pub fn get_confidence(&self, key: &TrigramKey) -> f64 {
        self.stats.get(key).map(|s| s.confidence).unwrap_or(0.0)
    }

    pub fn smoothed_error_rate(&self, key: &TrigramKey) -> f64 {
        match self.stats.get(key) {
            Some(s) => smoothed_error_rate_raw(s.error_count, s.sample_count),
            None => smoothed_error_rate_raw(0, 0),
        }
    }

    pub fn redundancy_score(
        &self,
        key: &TrigramKey,
        bigram_stats: &BigramStatsStore,
        char_stats: &KeyStatsStore,
    ) -> f64 {
        let e_a = char_stats.smoothed_error_rate(key.0[0]);
        let e_b = char_stats.smoothed_error_rate(key.0[1]);
        let e_c = char_stats.smoothed_error_rate(key.0[2]);
        let e_abc = self.smoothed_error_rate(key);

        let expected_from_chars = 1.0 - (1.0 - e_a) * (1.0 - e_b) * (1.0 - e_c);

        let e_ab = bigram_stats.smoothed_error_rate(&BigramKey([key.0[0], key.0[1]]));
        let e_bc = bigram_stats.smoothed_error_rate(&BigramKey([key.0[1], key.0[2]]));
        let expected_from_bigrams = e_ab.max(e_bc);

        let expected = expected_from_chars.max(expected_from_bigrams);
        e_abc / expected.max(0.01)
    }

    /// Prune to `max_entries` by composite utility score.
    /// `total_drills` is the current total drill count for recency calculation.
    pub fn prune(&mut self, max_entries: usize, total_drills: u32, bigram_stats: &BigramStatsStore, char_stats: &KeyStatsStore) {
        if self.stats.len() <= max_entries {
            return;
        }

        let recency_weight = 0.3;
        let signal_weight = 0.5;
        let data_weight = 0.2;

        let mut scored: Vec<(TrigramKey, f64)> = self
            .stats
            .iter()
            .map(|(key, stat)| {
                let drills_since = total_drills.saturating_sub(stat.last_seen_drill_index) as f64;
                let recency = 1.0 / (drills_since + 1.0);
                let redundancy = self.redundancy_score(key, bigram_stats, char_stats).min(3.0);
                let data = (stat.sample_count as f64).ln_1p();

                let utility = recency_weight * recency + signal_weight * redundancy + data_weight * data;
                (key.clone(), utility)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_entries);

        let keep: HashMap<TrigramKey, NgramStat> = scored
            .into_iter()
            .filter_map(|(key, _)| {
                self.stats.remove(&key).map(|stat| (key, stat))
            })
            .collect();

        self.stats = keep;
    }
}

// ---------------------------------------------------------------------------
// Extraction events & function
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct BigramEvent {
    pub key: BigramKey,
    pub total_time_ms: f64,
    pub correct: bool,
    pub has_hesitation: bool,
}

#[derive(Debug)]
pub struct TrigramEvent {
    pub key: TrigramKey,
    pub total_time_ms: f64,
    pub correct: bool,
    pub has_hesitation: bool,
}

/// Extract bigram and trigram events from a sequence of per-key times.
///
/// - BACKSPACE entries are filtered out
/// - Space characters split windows (no cross-word n-grams)
/// - For bigram "ab": time = window[1].time_ms
/// - For trigram "abc": time = window[1].time_ms + window[2].time_ms
/// - hesitation = any transition time > hesitation_threshold
pub fn extract_ngram_events(
    per_key_times: &[KeyTime],
    hesitation_threshold: f64,
) -> (Vec<BigramEvent>, Vec<TrigramEvent>) {
    let mut bigrams = Vec::new();
    let mut trigrams = Vec::new();

    // Filter out backspace entries
    let filtered: Vec<&KeyTime> = per_key_times
        .iter()
        .filter(|kt| kt.key != BACKSPACE)
        .collect();

    // Extract bigrams: slide a window of 2
    for window in filtered.windows(2) {
        let a = window[0];
        let b = window[1];

        // Skip cross-word boundaries
        if a.key == ' ' || b.key == ' ' {
            continue;
        }

        let time_ms = b.time_ms;
        let correct = a.correct && b.correct;
        let has_hesitation = b.time_ms > hesitation_threshold;

        bigrams.push(BigramEvent {
            key: BigramKey([a.key, b.key]),
            total_time_ms: time_ms,
            correct,
            has_hesitation,
        });
    }

    // Extract trigrams: slide a window of 3
    for window in filtered.windows(3) {
        let a = window[0];
        let b = window[1];
        let c = window[2];

        // Skip if any is a space (no cross-word)
        if a.key == ' ' || b.key == ' ' || c.key == ' ' {
            continue;
        }

        let time_ms = b.time_ms + c.time_ms;
        let correct = a.correct && b.correct && c.correct;
        let has_hesitation = b.time_ms > hesitation_threshold || c.time_ms > hesitation_threshold;

        trigrams.push(TrigramEvent {
            key: TrigramKey([a.key, b.key, c.key]),
            total_time_ms: time_ms,
            correct,
            has_hesitation,
        });
    }

    (bigrams, trigrams)
}

// ---------------------------------------------------------------------------
// FocusTarget & selection
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum FocusTarget {
    Char(char),
    Bigram(BigramKey),
}

/// Select the best focus target: either a single character or a bigram.
///
/// If the weakest eligible bigram's difficulty score exceeds 80% of the
/// weakest character's difficulty, focus on the bigram. Otherwise fall back
/// to the character.
pub fn select_focus_target(
    skill_tree: &SkillTree,
    scope: DrillScope,
    ranked_key_stats: &KeyStatsStore,
    ranked_bigram_stats: &BigramStatsStore,
) -> FocusTarget {
    let unlocked = skill_tree.unlocked_keys(scope);
    let focused_char = skill_tree.focused_key(scope, ranked_key_stats);

    let bigram_result = ranked_bigram_stats.weakest_bigram(ranked_key_stats, &unlocked);

    match (focused_char, bigram_result) {
        (Some(ch), Some((bigram_key, bigram_difficulty))) => {
            // Compute char difficulty: (1.0 - confidence) — no redundancy multiplier for chars
            let char_conf = ranked_key_stats.get_confidence(ch);
            let char_difficulty = (1.0 - char_conf).max(0.0);

            if bigram_difficulty > char_difficulty * 0.8 {
                FocusTarget::Bigram(bigram_key)
            } else {
                FocusTarget::Char(ch)
            }
        }
        (Some(ch), None) => FocusTarget::Char(ch),
        (None, Some((bigram_key, _))) => FocusTarget::Bigram(bigram_key),
        (None, None) => FocusTarget::Char('e'), // fallback
    }
}

// ---------------------------------------------------------------------------
// Trigram marginal gain analysis
// ---------------------------------------------------------------------------

/// Compute what fraction of trigrams with sufficient samples show genuine
/// redundancy beyond their constituent bigrams. Returns a value in [0.0, 1.0].
pub fn trigram_marginal_gain(
    trigram_stats: &TrigramStatsStore,
    bigram_stats: &BigramStatsStore,
    char_stats: &KeyStatsStore,
) -> f64 {
    let qualified: Vec<&TrigramKey> = trigram_stats
        .stats
        .iter()
        .filter(|(_, s)| s.sample_count >= MIN_SAMPLES_FOR_FOCUS)
        .map(|(k, _)| k)
        .collect();

    if qualified.is_empty() {
        return 0.0;
    }

    let with_signal = qualified
        .iter()
        .filter(|k| trigram_stats.redundancy_score(k, bigram_stats, char_stats) > STABILITY_THRESHOLD)
        .count();

    with_signal as f64 / qualified.len() as f64
}

// ---------------------------------------------------------------------------
// Hesitation helpers
// ---------------------------------------------------------------------------

/// Compute hesitation threshold from user median transition time.
pub fn hesitation_threshold(user_median_transition_ms: f64) -> f64 {
    800.0_f64.max(2.5 * user_median_transition_ms)
}

/// Compute the median of a slice of f64 values. Returns 0.0 if empty.
pub fn compute_median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

/// Constant for max trigram entries (used by App during pruning).
pub const MAX_TRIGRAMS: usize = MAX_TRIGRAM_ENTRIES;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_keytime(key: char, time_ms: f64, correct: bool) -> KeyTime {
        KeyTime {
            key,
            time_ms,
            correct,
        }
    }

    // --- Extraction tests ---

    #[test]
    fn extract_bigrams_from_simple_word() {
        let times = vec![
            make_keytime('h', 100.0, true),
            make_keytime('e', 200.0, true),
            make_keytime('l', 150.0, true),
            make_keytime('l', 180.0, true),
            make_keytime('o', 160.0, true),
        ];
        let (bigrams, trigrams) = extract_ngram_events(&times, 800.0);
        assert_eq!(bigrams.len(), 4); // he, el, ll, lo
        assert_eq!(bigrams[0].key, BigramKey(['h', 'e']));
        assert_eq!(bigrams[0].total_time_ms, 200.0);
        assert!(bigrams[0].correct);

        assert_eq!(trigrams.len(), 3); // hel, ell, llo
        assert_eq!(trigrams[0].key, TrigramKey(['h', 'e', 'l']));
        assert_eq!(trigrams[0].total_time_ms, 200.0 + 150.0); // e.time + l.time
    }

    #[test]
    fn extract_filters_backspace() {
        let times = vec![
            make_keytime('a', 100.0, true),
            make_keytime('x', 200.0, false),
            make_keytime(BACKSPACE, 150.0, true),
            make_keytime('b', 180.0, true),
        ];
        let (bigrams, _) = extract_ngram_events(&times, 800.0);
        // After filtering backspace: a, x, b -> bigrams: ax, xb
        assert_eq!(bigrams.len(), 2);
        assert_eq!(bigrams[0].key, BigramKey(['a', 'x']));
        assert_eq!(bigrams[1].key, BigramKey(['x', 'b']));
    }

    #[test]
    fn extract_splits_on_space() {
        let times = vec![
            make_keytime('a', 100.0, true),
            make_keytime('b', 200.0, true),
            make_keytime(' ', 150.0, true),
            make_keytime('c', 180.0, true),
            make_keytime('d', 160.0, true),
        ];
        let (bigrams, trigrams) = extract_ngram_events(&times, 800.0);
        // ab is valid, b-space skipped, space-c skipped, cd is valid
        assert_eq!(bigrams.len(), 2);
        assert_eq!(bigrams[0].key, BigramKey(['a', 'b']));
        assert_eq!(bigrams[1].key, BigramKey(['c', 'd']));
        // Only trigram with no space: none (ab_space and space_cd both have space)
        assert_eq!(trigrams.len(), 0);
    }

    #[test]
    fn extract_detects_hesitation() {
        let times = vec![
            make_keytime('a', 100.0, true),
            make_keytime('b', 900.0, true), // > 800 threshold
            make_keytime('c', 200.0, true),
        ];
        let (bigrams, _) = extract_ngram_events(&times, 800.0);
        assert!(bigrams[0].has_hesitation); // ab: b.time = 900 > 800
        assert!(!bigrams[1].has_hesitation); // bc: c.time = 200 < 800
    }

    #[test]
    fn extract_marks_incorrect_when_any_char_wrong() {
        let times = vec![
            make_keytime('a', 100.0, true),
            make_keytime('b', 200.0, false), // incorrect
            make_keytime('c', 150.0, true),
        ];
        let (bigrams, trigrams) = extract_ngram_events(&times, 800.0);
        assert!(!bigrams[0].correct); // ab: a correct, b incorrect -> false
        assert!(!bigrams[1].correct); // bc: b incorrect, c correct -> false
        assert!(!trigrams[0].correct); // abc: b incorrect -> false
    }

    // --- Laplace smoothing tests ---

    #[test]
    fn laplace_smoothing_zero_samples() {
        assert!((smoothed_error_rate_raw(0, 0) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn laplace_smoothing_convergence() {
        // With 100 samples and 10 errors, should be close to 0.1
        let rate = smoothed_error_rate_raw(10, 100);
        assert!((rate - 11.0 / 102.0).abs() < f64::EPSILON);
        assert!(rate > 0.1 && rate < 0.12);
    }

    #[test]
    fn laplace_smoothing_all_errors() {
        let rate = smoothed_error_rate_raw(50, 50);
        assert!((rate - 51.0 / 52.0).abs() < f64::EPSILON);
    }

    // --- Redundancy tests ---

    #[test]
    fn redundancy_proxy_example() {
        // Example 1 from plan: "is" where 's' is weak
        let mut char_stats = KeyStatsStore::default();
        // Simulate: s has high error rate
        // We need to set up error_count and total_count
        // s: e_s = 0.25 -> (errors+1)/(samples+2) = 0.25
        // Solve: (e+1)/(s+2) = 0.25 -> at s=50, e=12: (13)/(52) = 0.25
        let s_stat = char_stats.stats.entry('s').or_default();
        s_stat.error_count = 12;
        s_stat.total_count = 50;
        // i: e_i = 0.03 -> (e+1)/(s+2) = 0.03 -> at s=100, e=~2: (3)/(102) = 0.0294
        let i_stat = char_stats.stats.entry('i').or_default();
        i_stat.error_count = 2;
        i_stat.total_count = 100;

        let mut bigram_stats = BigramStatsStore::default();
        let is_key = BigramKey(['i', 's']);
        // e_is = 0.28 -> (e+1)/(s+2) = 0.28 -> at s=50, e=~13: (14)/(52) = 0.269
        // Let's pick s=100, e=~27: (28)/(102) = 0.2745
        // Actually, let's just use values that give close to what we want
        let is_stat = bigram_stats.stats.entry(is_key.clone()).or_default();
        is_stat.error_count = 27;
        is_stat.sample_count = 100;

        let e_s = char_stats.smoothed_error_rate('s');
        let e_i = char_stats.smoothed_error_rate('i');
        let e_is = bigram_stats.smoothed_error_rate(&is_key);
        let expected = 1.0 - (1.0 - e_s) * (1.0 - e_i);
        let redundancy = bigram_stats.redundancy_score(&is_key, &char_stats);

        // The redundancy should be close to 1.0 (proxy, not genuine)
        assert!(
            redundancy < STABILITY_THRESHOLD,
            "Proxy bigram 'is' should have redundancy < {STABILITY_THRESHOLD}, got {redundancy} (e_s={e_s}, e_i={e_i}, e_is={e_is}, expected={expected})"
        );
    }

    #[test]
    fn redundancy_genuine_difficulty() {
        // Example 2 from plan: "ed" where both chars are fine individually
        let mut char_stats = KeyStatsStore::default();
        // e: e_e = 0.04 -> (e+1)/(s+2) ~ 0.04 at s=100, errors=~3: (4)/(102) = 0.039
        let e_stat = char_stats.stats.entry('e').or_default();
        e_stat.error_count = 3;
        e_stat.total_count = 100;
        // d: e_d = 0.05 -> (e+1)/(s+2) ~ 0.05 at s=100, errors=~4: (5)/(102) = 0.049
        let d_stat = char_stats.stats.entry('d').or_default();
        d_stat.error_count = 4;
        d_stat.total_count = 100;

        let mut bigram_stats = BigramStatsStore::default();
        let ed_key = BigramKey(['e', 'd']);
        // e_ed = 0.22 -> at s=100, errors=~21: (22)/(102) = 0.2157
        let ed_stat = bigram_stats.stats.entry(ed_key.clone()).or_default();
        ed_stat.error_count = 21;
        ed_stat.sample_count = 100;

        let redundancy = bigram_stats.redundancy_score(&ed_key, &char_stats);
        assert!(
            redundancy > STABILITY_THRESHOLD,
            "Genuine difficulty 'ed' should have redundancy > {STABILITY_THRESHOLD}, got {redundancy}"
        );
    }

    #[test]
    fn redundancy_trigram_explained_by_bigram() {
        // Example 3: "the" where "th" bigram explains the difficulty
        let mut char_stats = KeyStatsStore::default();
        for &(ch, errors, total) in &[('t', 2, 100), ('h', 3, 100), ('e', 3, 100)] {
            let s = char_stats.stats.entry(ch).or_default();
            s.error_count = errors;
            s.total_count = total;
        }

        let mut bigram_stats = BigramStatsStore::default();
        // th has high error rate: e_th = 0.15 -> at s=100, e=~14: (15)/(102) = 0.147
        let th_stat = bigram_stats.stats.entry(BigramKey(['t', 'h'])).or_default();
        th_stat.error_count = 14;
        th_stat.sample_count = 100;
        // he has low error rate
        let he_stat = bigram_stats.stats.entry(BigramKey(['h', 'e'])).or_default();
        he_stat.error_count = 3;
        he_stat.sample_count = 100;

        let mut trigram_stats = TrigramStatsStore::default();
        let the_key = TrigramKey(['t', 'h', 'e']);
        // e_the = 0.16 -> at s=100, e=~15: (16)/(102) = 0.157
        let the_stat = trigram_stats.stats.entry(the_key.clone()).or_default();
        the_stat.error_count = 15;
        the_stat.sample_count = 100;

        let redundancy = trigram_stats.redundancy_score(&the_key, &bigram_stats, &char_stats);
        assert!(
            redundancy < STABILITY_THRESHOLD,
            "Trigram 'the' explained by 'th' bigram should have redundancy < {STABILITY_THRESHOLD}, got {redundancy}"
        );
    }

    // --- Stability gate tests ---

    #[test]
    fn stability_streak_increments_and_resets() {
        let mut bigram_stats = BigramStatsStore::default();
        let key = BigramKey(['e', 'd']);

        // Set up a bigram with genuine difficulty
        let stat = bigram_stats.stats.entry(key.clone()).or_default();
        stat.error_count = 25;
        stat.sample_count = 100;

        let mut char_stats = KeyStatsStore::default();
        // Low char error rates
        char_stats.stats.entry('e').or_default().error_count = 2;
        char_stats.stats.entry('e').or_default().total_count = 100;
        char_stats.stats.entry('d').or_default().error_count = 2;
        char_stats.stats.entry('d').or_default().total_count = 100;

        // Should increment streak
        bigram_stats.update_redundancy_streak(&key, &char_stats);
        assert_eq!(bigram_stats.stats[&key].redundancy_streak, 1);
        bigram_stats.update_redundancy_streak(&key, &char_stats);
        assert_eq!(bigram_stats.stats[&key].redundancy_streak, 2);
        bigram_stats.update_redundancy_streak(&key, &char_stats);
        assert_eq!(bigram_stats.stats[&key].redundancy_streak, 3);

        // Now simulate char stats getting worse (making redundancy low)
        char_stats.stats.entry('e').or_default().error_count = 30;
        bigram_stats.update_redundancy_streak(&key, &char_stats);
        assert_eq!(bigram_stats.stats[&key].redundancy_streak, 0); // reset
    }

    #[test]
    fn focus_eligibility_requires_all_conditions() {
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();
        let unlocked = vec!['a', 'b', 'c', 'd', 'e'];

        // Set up char stats with low error rates
        for &ch in &['a', 'b'] {
            let s = char_stats.stats.entry(ch).or_default();
            s.error_count = 2;
            s.total_count = 100;
        }

        let key = BigramKey(['a', 'b']);
        let stat = bigram_stats.stats.entry(key.clone()).or_default();
        stat.error_count = 25;
        stat.sample_count = 25; // enough samples
        stat.confidence = 0.5;
        stat.redundancy_streak = STABILITY_STREAK_REQUIRED; // stable

        // Should be eligible
        let result = bigram_stats.weakest_bigram(&char_stats, &unlocked);
        assert!(result.is_some(), "Should be eligible with all conditions met");

        // Reset streak -> not eligible
        bigram_stats.stats.get_mut(&key).unwrap().redundancy_streak = 2;
        let result = bigram_stats.weakest_bigram(&char_stats, &unlocked);
        assert!(result.is_none(), "Should NOT be eligible without stable streak");

        // Restore streak, reduce samples -> not eligible
        bigram_stats.stats.get_mut(&key).unwrap().redundancy_streak = STABILITY_STREAK_REQUIRED;
        bigram_stats.stats.get_mut(&key).unwrap().sample_count = 15;
        let result = bigram_stats.weakest_bigram(&char_stats, &unlocked);
        assert!(result.is_none(), "Should NOT be eligible with < 20 samples");
    }

    // --- Focus selection tests ---

    #[test]
    fn focus_falls_back_to_char_when_no_bigrams() {
        let skill_tree = SkillTree::default();
        let key_stats = KeyStatsStore::default();
        let bigram_stats = BigramStatsStore::default();

        let target = select_focus_target(
            &skill_tree,
            DrillScope::Global,
            &key_stats,
            &bigram_stats,
        );

        // With default skill tree, focused_key may return a char or None
        // Either way, should not be a Bigram
        match target {
            FocusTarget::Char(_) => {} // expected
            FocusTarget::Bigram(_) => panic!("Should not select bigram with no data"),
        }
    }

    #[test]
    fn focus_selects_bigram_when_difficulty_exceeds_threshold() {
        // Set up a skill tree with some unlocked keys and known confidence
        let skill_tree = SkillTree::default();
        let mut key_stats = KeyStatsStore::default();

        // Give all unlocked keys high confidence so focused_key returns
        // the one with lowest confidence
        for &ch in &['e', 't', 'a', 'o', 'n', 'i'] {
            let stat = key_stats.stats.entry(ch).or_default();
            stat.confidence = 0.95;
            stat.filtered_time_ms = 360.0; // slow enough to not be mastered
            stat.sample_count = 50;
            stat.total_count = 50;
            stat.error_count = 2;
        }
        // Make 'n' the weakest char: confidence = 0.5 -> char_difficulty = 0.5
        key_stats.stats.get_mut(&'n').unwrap().confidence = 0.5;
        key_stats.stats.get_mut(&'n').unwrap().filtered_time_ms = 686.0;

        // Set up a bigram 'e','t' with high difficulty that exceeds 0.8 * char_difficulty
        // char_difficulty = 1.0 - 0.5 = 0.5, threshold = 0.5 * 0.8 = 0.4
        // bigram needs ngram_difficulty > 0.4
        // ngram_difficulty = (1.0 - confidence) * redundancy
        // confidence = 0.4, redundancy = 2.0 -> difficulty = 0.6 * 2.0 = 1.2 > 0.4
        let mut bigram_stats = BigramStatsStore::default();
        let et_key = BigramKey(['e', 't']);
        let stat = bigram_stats.stats.entry(et_key.clone()).or_default();
        stat.confidence = 0.4;
        stat.sample_count = 30;
        stat.error_count = 20;
        stat.redundancy_streak = STABILITY_STREAK_REQUIRED;

        let target = select_focus_target(
            &skill_tree,
            DrillScope::Global,
            &key_stats,
            &bigram_stats,
        );

        assert_eq!(
            target,
            FocusTarget::Bigram(et_key),
            "Bigram should win when its difficulty exceeds char_difficulty * 0.8"
        );
    }

    #[test]
    fn focus_selects_char_when_bigram_difficulty_below_threshold() {
        let skill_tree = SkillTree::default();
        let mut key_stats = KeyStatsStore::default();

        for &ch in &['e', 't', 'a', 'o', 'n', 'i'] {
            let stat = key_stats.stats.entry(ch).or_default();
            stat.confidence = 0.95;
            stat.filtered_time_ms = 360.0;
            stat.sample_count = 50;
            stat.total_count = 50;
            stat.error_count = 2;
        }
        // Make 'n' very weak: confidence = 0.1 -> char_difficulty = 0.9
        // threshold = 0.9 * 0.8 = 0.72
        key_stats.stats.get_mut(&'n').unwrap().confidence = 0.1;
        key_stats.stats.get_mut(&'n').unwrap().filtered_time_ms = 3400.0;

        // Bigram 'e','t' with high confidence and low error rate -> low difficulty
        // char error rates: e_e ≈ 0.058, e_t ≈ 0.058
        // expected_et = 1 - (1-0.058)*(1-0.058) ≈ 0.113
        // bigram error: (5+1)/(30+2) = 0.1875 -> redundancy ≈ 1.66
        // ngram_difficulty = (1.0 - 0.85) * 1.66 = 0.249 < 0.72
        let mut bigram_stats = BigramStatsStore::default();
        let et_key = BigramKey(['e', 't']);
        let stat = bigram_stats.stats.entry(et_key.clone()).or_default();
        stat.confidence = 0.85;
        stat.sample_count = 30;
        stat.error_count = 5;
        stat.redundancy_streak = STABILITY_STREAK_REQUIRED;

        let target = select_focus_target(
            &skill_tree,
            DrillScope::Global,
            &key_stats,
            &bigram_stats,
        );

        match target {
            FocusTarget::Char(ch) => {
                assert_eq!(ch, 'n', "Should focus on weakest char 'n'");
            }
            FocusTarget::Bigram(_) => {
                panic!("Should NOT select bigram when its difficulty is below threshold");
            }
        }
    }

    #[test]
    fn focus_ignores_bigram_with_insufficient_streak() {
        let skill_tree = SkillTree::default();
        let mut key_stats = KeyStatsStore::default();

        for &ch in &['e', 't', 'a', 'o', 'n', 'i'] {
            let stat = key_stats.stats.entry(ch).or_default();
            stat.confidence = 0.95;
            stat.filtered_time_ms = 360.0;
            stat.sample_count = 50;
            stat.total_count = 50;
            stat.error_count = 2;
        }
        key_stats.stats.get_mut(&'n').unwrap().confidence = 0.5;
        key_stats.stats.get_mut(&'n').unwrap().filtered_time_ms = 686.0;

        // Bigram with high difficulty but streak only 2 (needs 3)
        let mut bigram_stats = BigramStatsStore::default();
        let et_key = BigramKey(['e', 't']);
        let stat = bigram_stats.stats.entry(et_key.clone()).or_default();
        stat.confidence = 0.3;
        stat.sample_count = 30;
        stat.error_count = 25;
        stat.redundancy_streak = STABILITY_STREAK_REQUIRED - 1; // not enough

        let target = select_focus_target(
            &skill_tree,
            DrillScope::Global,
            &key_stats,
            &bigram_stats,
        );

        match target {
            FocusTarget::Char(_) => {} // expected: bigram filtered by stability gate
            FocusTarget::Bigram(_) => {
                panic!("Should NOT select bigram with insufficient redundancy streak");
            }
        }
    }

    // --- Hesitation tests ---

    #[test]
    fn hesitation_threshold_respects_floor() {
        assert_eq!(hesitation_threshold(100.0), 800.0); // 2.5 * 100 = 250 < 800
        assert_eq!(hesitation_threshold(400.0), 1000.0); // 2.5 * 400 = 1000 > 800
    }

    // --- Median tests ---

    #[test]
    fn median_odd_count() {
        let mut vals = vec![5.0, 1.0, 3.0];
        assert_eq!(compute_median(&mut vals), 3.0);
    }

    #[test]
    fn median_even_count() {
        let mut vals = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(compute_median(&mut vals), 2.5);
    }

    #[test]
    fn median_empty() {
        let mut vals: Vec<f64> = vec![];
        assert_eq!(compute_median(&mut vals), 0.0);
    }

    // --- Trigram marginal gain ---

    #[test]
    fn marginal_gain_zero_when_no_qualified() {
        let trigram_stats = TrigramStatsStore::default();
        let bigram_stats = BigramStatsStore::default();
        let char_stats = KeyStatsStore::default();
        assert_eq!(trigram_marginal_gain(&trigram_stats, &bigram_stats, &char_stats), 0.0);
    }

    // --- Replay invariance ---

    #[test]
    fn replay_produces_correct_error_total_counts() {
        // Simulate a replay: process keystrokes and verify counts
        let mut key_stats = KeyStatsStore::default();

        // Simulate: 10 correct 'a', 3 errors 'a', 5 correct 'b', 1 error 'b'
        let keystrokes = vec![
            make_keytime('a', 200.0, true),
            make_keytime('a', 210.0, true),
            make_keytime('a', 190.0, true),
            make_keytime('a', 220.0, false), // error
            make_keytime('a', 200.0, true),
            make_keytime('a', 200.0, true),
            make_keytime('a', 200.0, true),
            make_keytime('a', 200.0, false), // error
            make_keytime('a', 200.0, true),
            make_keytime('a', 200.0, true),
            make_keytime('a', 200.0, true),
            make_keytime('a', 200.0, true),
            make_keytime('a', 200.0, false), // error
            make_keytime('b', 300.0, true),
            make_keytime('b', 300.0, true),
            make_keytime('b', 300.0, true),
            make_keytime('b', 300.0, true),
            make_keytime('b', 300.0, true),
            make_keytime('b', 300.0, false), // error
        ];

        // Process like rebuild_ngram_stats does
        for kt in &keystrokes {
            if kt.correct {
                let stat = key_stats.stats.entry(kt.key).or_default();
                stat.total_count += 1;
            } else {
                key_stats.update_key_error(kt.key);
            }
        }

        let a_stat = key_stats.stats.get(&'a').unwrap();
        assert_eq!(a_stat.total_count, 13, "a: 10 correct + 3 errors = 13 total");
        assert_eq!(a_stat.error_count, 3, "a: 3 errors");

        let b_stat = key_stats.stats.get(&'b').unwrap();
        assert_eq!(b_stat.total_count, 6, "b: 5 correct + 1 error = 6 total");
        assert_eq!(b_stat.error_count, 1, "b: 1 error");

        // Verify smoothed error rate is reasonable
        let a_rate = key_stats.smoothed_error_rate('a');
        // (3 + 1) / (13 + 2) = 4/15 = 0.2667
        assert!((a_rate - 4.0 / 15.0).abs() < f64::EPSILON);

        let b_rate = key_stats.smoothed_error_rate('b');
        // (1 + 1) / (6 + 2) = 2/8 = 0.25
        assert!((b_rate - 2.0 / 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn last_seen_drill_index_tracks_correctly() {
        let mut bigram_stats = BigramStatsStore::default();
        let key = BigramKey(['a', 'b']);

        bigram_stats.update(key.clone(), 200.0, true, false, 0);
        assert_eq!(bigram_stats.stats[&key].last_seen_drill_index, 0);

        bigram_stats.update(key.clone(), 200.0, true, false, 5);
        assert_eq!(bigram_stats.stats[&key].last_seen_drill_index, 5);

        bigram_stats.update(key.clone(), 200.0, true, false, 42);
        assert_eq!(bigram_stats.stats[&key].last_seen_drill_index, 42);
    }

    #[test]
    fn prune_recency_correct_with_mixed_drill_indices() {
        // Simulate interleaved partial (indices 0,1,3) and full (indices 2,4) drills.
        // The key point: total_drills must match the index space (5, not 2)
        // to avoid artificially inflating recency for partial-drill trigrams.
        let mut trigram_stats = TrigramStatsStore::default();
        let bigram_stats = BigramStatsStore::default();
        let char_stats = KeyStatsStore::default();

        // "Old" trigram last seen at drill index 0 (earliest)
        let old_key = TrigramKey(['o', 'l', 'd']);
        trigram_stats.update(old_key.clone(), 300.0, true, false, 0);
        trigram_stats.stats.get_mut(&old_key).unwrap().sample_count = 5;

        // "Mid" trigram last seen at partial drill index 1
        let mid_key = TrigramKey(['m', 'i', 'd']);
        trigram_stats.update(mid_key.clone(), 300.0, true, false, 1);
        trigram_stats.stats.get_mut(&mid_key).unwrap().sample_count = 5;

        // "New" trigram last seen at drill index 4 (most recent)
        let new_key = TrigramKey(['n', 'e', 'w']);
        trigram_stats.update(new_key.clone(), 300.0, true, false, 4);
        trigram_stats.stats.get_mut(&new_key).unwrap().sample_count = 5;

        // Prune down to 2 entries with total_drills = 5 (matching history length)
        trigram_stats.prune(2, 5, &bigram_stats, &char_stats);

        // "New" (index 4) should survive over "old" (index 0) due to higher recency
        assert!(trigram_stats.stats.contains_key(&new_key), "most recent trigram should survive prune");
        assert!(!trigram_stats.stats.contains_key(&old_key), "oldest trigram should be pruned");
        assert_eq!(trigram_stats.stats.len(), 2);

        // Now verify that using a WRONG total (e.g. 2 completed drills instead of 5)
        // would compress the recency range. We don't assert this breaks ordering here
        // since the fix is in app.rs passing the correct total -- this test just confirms
        // the correct behavior when the right total is used.
    }

    // --- Performance budget tests ---
    // These enforce hard pass/fail limits. Budgets are for release builds;
    // debug builds are ~10-20x slower, so we apply a 20x multiplier.

    const DEBUG_MULTIPLIER: u32 = 20;

    fn make_bench_keystrokes(count: usize) -> Vec<KeyTime> {
        let chars = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
        (0..count)
            .map(|i| KeyTime {
                key: chars[i % chars.len()],
                time_ms: 200.0 + (i % 50) as f64,
                correct: i % 7 != 0,
            })
            .collect()
    }

    #[test]
    fn perf_budget_extraction_under_1ms() {
        let keystrokes = make_bench_keystrokes(500);
        let budget = std::time::Duration::from_millis(1 * DEBUG_MULTIPLIER as u64);

        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = extract_ngram_events(&keystrokes, 800.0);
        }
        let elapsed = start.elapsed() / 100;

        assert!(
            elapsed < budget,
            "extraction took {elapsed:?} per call, budget is {budget:?}"
        );
    }

    #[test]
    fn perf_budget_update_under_1ms() {
        let keystrokes = make_bench_keystrokes(500);
        let (bigram_events, _) = extract_ngram_events(&keystrokes, 800.0);
        let budget = std::time::Duration::from_millis(1 * DEBUG_MULTIPLIER as u64);

        let start = std::time::Instant::now();
        for _ in 0..100 {
            let mut store = BigramStatsStore::default();
            for ev in bigram_events.iter().take(400) {
                store.update(ev.key.clone(), ev.total_time_ms, ev.correct, ev.has_hesitation, 0);
            }
        }
        let elapsed = start.elapsed() / 100;

        assert!(
            elapsed < budget,
            "update took {elapsed:?} per call, budget is {budget:?}"
        );
    }

    #[test]
    fn perf_budget_focus_selection_under_5ms() {
        let all_chars: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();

        for &ch in &all_chars {
            let stat = char_stats.stats.entry(ch).or_default();
            stat.confidence = 0.8;
            stat.filtered_time_ms = 430.0;
            stat.sample_count = 50;
            stat.total_count = 50;
            stat.error_count = 3;
        }

        let mut count: usize = 0;
        for &a in &all_chars {
            for &b in &all_chars {
                if bigram_stats.stats.len() >= 3000 {
                    break;
                }
                let key = BigramKey([a, b]);
                let stat = bigram_stats.stats.entry(key).or_default();
                stat.confidence = 0.5 + (count % 50) as f64 * 0.01;
                stat.sample_count = 25 + count % 30;
                stat.error_count = 5 + count % 10;
                stat.redundancy_streak = if count % 3 == 0 { 3 } else { 1 };
                count += 1;
            }
        }
        assert_eq!(bigram_stats.stats.len(), 3000);

        let unlocked: Vec<char> = all_chars;
        let budget = std::time::Duration::from_millis(5 * DEBUG_MULTIPLIER as u64);

        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = bigram_stats.weakest_bigram(&char_stats, &unlocked);
        }
        let elapsed = start.elapsed() / 100;

        assert!(
            elapsed < budget,
            "focus selection took {elapsed:?} per call, budget is {budget:?}"
        );
    }

    #[test]
    fn perf_budget_history_replay_under_500ms() {
        let drills: Vec<Vec<KeyTime>> = (0..500)
            .map(|_| make_bench_keystrokes(300))
            .collect();

        let budget = std::time::Duration::from_millis(500 * DEBUG_MULTIPLIER as u64);

        let start = std::time::Instant::now();
        let mut bigram_stats = BigramStatsStore::default();
        let mut trigram_stats = TrigramStatsStore::default();
        let mut key_stats = KeyStatsStore::default();

        for (drill_idx, keystrokes) in drills.iter().enumerate() {
            let (bigram_events, trigram_events) = extract_ngram_events(keystrokes, 800.0);

            for kt in keystrokes {
                if kt.correct {
                    let stat = key_stats.stats.entry(kt.key).or_default();
                    stat.total_count += 1;
                } else {
                    key_stats.update_key_error(kt.key);
                }
            }

            for ev in &bigram_events {
                bigram_stats.update(
                    ev.key.clone(), ev.total_time_ms, ev.correct, ev.has_hesitation,
                    drill_idx as u32,
                );
            }
            for ev in &trigram_events {
                trigram_stats.update(
                    ev.key.clone(), ev.total_time_ms, ev.correct, ev.has_hesitation,
                    drill_idx as u32,
                );
            }
        }
        let elapsed = start.elapsed();

        // Sanity: we actually processed data
        assert!(!bigram_stats.stats.is_empty());
        assert!(!trigram_stats.stats.is_empty());

        assert!(
            elapsed < budget,
            "history replay took {elapsed:?}, budget is {budget:?}"
        );
    }
}
