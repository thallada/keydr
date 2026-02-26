use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::engine::key_stats::KeyStatsStore;
use crate::engine::skill_tree::{DrillScope, SkillTree};
use crate::keyboard::display::BACKSPACE;
use crate::session::result::KeyTime;

const EMA_ALPHA: f64 = 0.1;
const MAX_RECENT: usize = 30;
const ERROR_ANOMALY_RATIO_THRESHOLD: f64 = 1.5;
pub(crate) const ANOMALY_STREAK_REQUIRED: u8 = 3;
pub(crate) const MIN_SAMPLES_FOR_FOCUS: usize = 20;
const ANOMALY_MIN_SAMPLES: usize = 3;
const SPEED_ANOMALY_PCT_THRESHOLD: f64 = 50.0;
const MIN_CHAR_SAMPLES_FOR_SPEED: usize = 10;
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
    pub sample_count: usize,
    pub error_count: usize,
    pub hesitation_count: usize,
    pub recent_times: Vec<f64>,
    #[serde(default = "default_error_rate_ema")]
    pub error_rate_ema: f64,
    pub error_anomaly_streak: u8,
    #[serde(default)]
    pub speed_anomaly_streak: u8,
    #[serde(default)]
    pub last_seen_drill_index: u32,
}

fn default_error_rate_ema() -> f64 {
    0.5
}

impl Default for NgramStat {
    fn default() -> Self {
        Self {
            filtered_time_ms: 1000.0,
            best_time_ms: f64::MAX,
            sample_count: 0,
            error_count: 0,
            hesitation_count: 0,
            recent_times: Vec::new(),
            error_rate_ema: 0.5,
            error_anomaly_streak: 0,
            speed_anomaly_streak: 0,
            last_seen_drill_index: 0,
        }
    }
}

fn update_stat(
    stat: &mut NgramStat,
    time_ms: f64,
    correct: bool,
    hesitation: bool,
    drill_index: u32,
) {
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

    stat.recent_times.push(time_ms);
    if stat.recent_times.len() > MAX_RECENT {
        stat.recent_times.remove(0);
    }

    // Update error rate EMA
    let error_signal = if correct { 0.0 } else { 1.0 };
    if stat.sample_count == 1 {
        stat.error_rate_ema = error_signal;
    } else {
        stat.error_rate_ema = EMA_ALPHA * error_signal + (1.0 - EMA_ALPHA) * stat.error_rate_ema;
    }
}

// ---------------------------------------------------------------------------
// BigramStatsStore
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BigramStatsStore {
    pub stats: HashMap<BigramKey, NgramStat>,
}

impl BigramStatsStore {
    pub fn update(
        &mut self,
        key: BigramKey,
        time_ms: f64,
        correct: bool,
        hesitation: bool,
        drill_index: u32,
    ) {
        let stat = self.stats.entry(key).or_default();
        update_stat(stat, time_ms, correct, hesitation, drill_index);
    }

    pub fn smoothed_error_rate(&self, key: &BigramKey) -> f64 {
        match self.stats.get(key) {
            Some(s) => s.error_rate_ema,
            None => 0.5,
        }
    }

    /// Error anomaly ratio: bigram error rate / expected error rate from char independence.
    /// Values > 1.0 indicate genuine bigram difficulty beyond individual char weakness.
    pub fn error_anomaly_ratio(&self, key: &BigramKey, char_stats: &KeyStatsStore) -> f64 {
        let e_a = char_stats.smoothed_error_rate(key.0[0]);
        let e_b = char_stats.smoothed_error_rate(key.0[1]);
        let e_ab = self.smoothed_error_rate(key);
        let expected_ab = 1.0 - (1.0 - e_a) * (1.0 - e_b);
        e_ab / expected_ab.max(0.01)
    }

    /// Error anomaly as percentage: (ratio - 1.0) * 100.
    /// Returns None if bigram has no stats.
    #[allow(dead_code)]
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
        if char_b_stat.sample_count < MIN_CHAR_SAMPLES_FOR_SPEED {
            return None;
        }
        let ratio = stat.filtered_time_ms / char_b_stat.filtered_time_ms;
        Some((ratio - 1.0) * 100.0)
    }

    /// Update error anomaly streak for a bigram given current char stats.
    /// Call this after updating the bigram stats.
    pub fn update_error_anomaly_streak(&mut self, key: &BigramKey, char_stats: &KeyStatsStore) {
        let ratio = self.error_anomaly_ratio(key, char_stats);
        if let Some(stat) = self.stats.get_mut(key) {
            if ratio > ERROR_ANOMALY_RATIO_THRESHOLD {
                stat.error_anomaly_streak = stat.error_anomaly_streak.saturating_add(1);
            } else {
                stat.error_anomaly_streak = 0;
            }
        }
    }

    /// Update speed anomaly streak for a bigram given current char stats.
    /// If speed_anomaly_pct() returns None (char baseline unavailable), holds previous streak value.
    pub fn update_speed_anomaly_streak(&mut self, key: &BigramKey, char_stats: &KeyStatsStore) {
        let stat = match self.stats.get(key) {
            Some(s) => s,
            None => return,
        };
        if stat.sample_count < ANOMALY_MIN_SAMPLES {
            return;
        }
        match self.speed_anomaly_pct(key, char_stats) {
            Some(pct) => {
                if let Some(stat) = self.stats.get_mut(key) {
                    if pct > SPEED_ANOMALY_PCT_THRESHOLD {
                        stat.speed_anomaly_streak = stat.speed_anomaly_streak.saturating_add(1);
                    } else {
                        stat.speed_anomaly_streak = 0;
                    }
                }
            }
            None => {
                // Hold previous streak — char baseline unavailable
            }
        }
    }

    /// All bigrams with error anomaly above threshold and sufficient samples.
    /// Sorted by anomaly_pct desc. Each entry's `confirmed` flag indicates
    /// streak >= ANOMALY_STREAK_REQUIRED && samples >= MIN_SAMPLES_FOR_FOCUS.
    pub fn error_anomaly_bigrams(
        &self,
        char_stats: &KeyStatsStore,
        unlocked: &[char],
    ) -> Vec<BigramAnomaly> {
        let mut results = Vec::new();

        for (key, stat) in &self.stats {
            if !unlocked.contains(&key.0[0]) || !unlocked.contains(&key.0[1]) {
                continue;
            }
            if stat.sample_count < ANOMALY_MIN_SAMPLES {
                continue;
            }
            let e_a = char_stats.smoothed_error_rate(key.0[0]);
            let e_b = char_stats.smoothed_error_rate(key.0[1]);
            let expected = 1.0 - (1.0 - e_a) * (1.0 - e_b);
            let ratio = self.error_anomaly_ratio(key, char_stats);
            if ratio <= ERROR_ANOMALY_RATIO_THRESHOLD {
                continue;
            }
            let anomaly_pct = (ratio - 1.0) * 100.0;
            let confirmed = stat.error_anomaly_streak >= ANOMALY_STREAK_REQUIRED
                && stat.sample_count >= MIN_SAMPLES_FOR_FOCUS;
            results.push(BigramAnomaly {
                key: key.clone(),
                anomaly_pct,
                sample_count: stat.sample_count,
                error_count: stat.error_count,
                error_rate_ema: stat.error_rate_ema,
                speed_ms: stat.filtered_time_ms,
                expected_baseline: expected,
                confirmed,
            });
        }

        results.sort_by(|a, b| {
            b.anomaly_pct
                .partial_cmp(&a.anomaly_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.key.0.cmp(&b.key.0))
        });

        results
    }

    /// All bigrams with speed anomaly above threshold and sufficient samples.
    /// Sorted by anomaly_pct desc.
    pub fn speed_anomaly_bigrams(
        &self,
        char_stats: &KeyStatsStore,
        unlocked: &[char],
    ) -> Vec<BigramAnomaly> {
        let mut results = Vec::new();

        for (key, stat) in &self.stats {
            if !unlocked.contains(&key.0[0]) || !unlocked.contains(&key.0[1]) {
                continue;
            }
            if stat.sample_count < ANOMALY_MIN_SAMPLES {
                continue;
            }
            let char_b_speed = char_stats
                .stats
                .get(&key.0[1])
                .map(|s| s.filtered_time_ms)
                .unwrap_or(0.0);
            match self.speed_anomaly_pct(key, char_stats) {
                Some(pct) if pct > SPEED_ANOMALY_PCT_THRESHOLD => {
                    let confirmed = stat.speed_anomaly_streak >= ANOMALY_STREAK_REQUIRED
                        && stat.sample_count >= MIN_SAMPLES_FOR_FOCUS;
                    results.push(BigramAnomaly {
                        key: key.clone(),
                        anomaly_pct: pct,
                        sample_count: stat.sample_count,
                        error_count: stat.error_count,
                        error_rate_ema: stat.error_rate_ema,
                        speed_ms: stat.filtered_time_ms,
                        expected_baseline: char_b_speed,
                        confirmed,
                    });
                }
                _ => {}
            }
        }

        results.sort_by(|a, b| {
            b.anomaly_pct
                .partial_cmp(&a.anomaly_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.key.0.cmp(&b.key.0))
        });

        results
    }

    /// Find the worst confirmed anomaly across both error and speed anomalies.
    /// Each bigram gets at most one candidacy (whichever anomaly type is higher; error on tie).
    pub fn worst_confirmed_anomaly(
        &self,
        char_stats: &KeyStatsStore,
        unlocked: &[char],
    ) -> Option<(BigramKey, f64, AnomalyType)> {
        let mut candidates: HashMap<BigramKey, (f64, AnomalyType)> = HashMap::new();

        // Collect confirmed error anomalies
        for a in self.error_anomaly_bigrams(char_stats, unlocked) {
            if a.confirmed {
                candidates.insert(a.key, (a.anomaly_pct, AnomalyType::Error));
            }
        }

        // Collect confirmed speed anomalies, dedup per bigram preferring higher pct (error on tie)
        for a in self.speed_anomaly_bigrams(char_stats, unlocked) {
            if a.confirmed {
                match candidates.get(&a.key) {
                    Some((existing_pct, _)) if *existing_pct >= a.anomaly_pct => {
                        // Keep existing (error wins on tie since >= keeps it)
                    }
                    _ => {
                        candidates.insert(a.key, (a.anomaly_pct, AnomalyType::Speed));
                    }
                }
            }
        }

        candidates
            .into_iter()
            .max_by(|a, b| {
                a.1.0
                    .partial_cmp(&b.1.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(key, (pct, typ))| (key, pct, typ))
    }
}

// ---------------------------------------------------------------------------
// TrigramStatsStore
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrigramStatsStore {
    pub stats: HashMap<TrigramKey, NgramStat>,
}

impl TrigramStatsStore {
    pub fn update(
        &mut self,
        key: TrigramKey,
        time_ms: f64,
        correct: bool,
        hesitation: bool,
        drill_index: u32,
    ) {
        let stat = self.stats.entry(key).or_default();
        update_stat(stat, time_ms, correct, hesitation, drill_index);
    }

    pub fn smoothed_error_rate(&self, key: &TrigramKey) -> f64 {
        match self.stats.get(key) {
            Some(s) => s.error_rate_ema,
            None => 0.5,
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
    pub fn prune(
        &mut self,
        max_entries: usize,
        total_drills: u32,
        bigram_stats: &BigramStatsStore,
        char_stats: &KeyStatsStore,
    ) {
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
                let redundancy = self
                    .redundancy_score(key, bigram_stats, char_stats)
                    .min(3.0);
                let data = (stat.sample_count as f64).ln_1p();

                let utility =
                    recency_weight * recency + signal_weight * redundancy + data_weight * data;
                (key.clone(), utility)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(max_entries);

        let keep: HashMap<TrigramKey, NgramStat> = scored
            .into_iter()
            .filter_map(|(key, _)| self.stats.remove(&key).map(|stat| (key, stat)))
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
// Anomaly types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum AnomalyType {
    Error,
    Speed,
}

pub struct BigramAnomaly {
    pub key: BigramKey,
    pub anomaly_pct: f64,
    pub sample_count: usize,
    pub error_count: usize,
    pub error_rate_ema: f64,
    pub speed_ms: f64,
    pub expected_baseline: f64,
    pub confirmed: bool,
}

// ---------------------------------------------------------------------------
// FocusSelection
// ---------------------------------------------------------------------------

/// Combined focus selection: carries both char and bigram focus independently.
#[derive(Clone, Debug, PartialEq)]
pub struct FocusSelection {
    pub char_focus: Option<char>,
    pub bigram_focus: Option<(BigramKey, f64, AnomalyType)>,
}

/// Select focus targets: weakest char from skill tree + worst confirmed bigram anomaly.
/// Both are independent — neither overrides the other.
pub fn select_focus(
    skill_tree: &SkillTree,
    scope: DrillScope,
    ranked_key_stats: &KeyStatsStore,
    ranked_bigram_stats: &BigramStatsStore,
) -> FocusSelection {
    let unlocked = skill_tree.unlocked_keys(scope);
    let char_focus = skill_tree.focused_key(scope, ranked_key_stats);
    let bigram_focus = ranked_bigram_stats.worst_confirmed_anomaly(ranked_key_stats, &unlocked);
    FocusSelection {
        char_focus,
        bigram_focus,
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
        .filter(|k| {
            trigram_stats.redundancy_score(k, bigram_stats, char_stats)
                > ERROR_ANOMALY_RATIO_THRESHOLD
        })
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

    // --- EMA error rate tests ---

    #[test]
    fn ema_default_is_neutral() {
        let store = BigramStatsStore::default();
        let key = BigramKey(['a', 'b']);
        assert!((store.smoothed_error_rate(&key) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ema_first_sample_sets_directly() {
        let mut store = BigramStatsStore::default();
        let key = BigramKey(['a', 'b']);
        store.update(key.clone(), 200.0, true, false, 0);
        assert!((store.smoothed_error_rate(&key) - 0.0).abs() < f64::EPSILON);

        let mut store2 = BigramStatsStore::default();
        store2.update(key.clone(), 200.0, false, false, 0);
        assert!((store2.smoothed_error_rate(&key) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ema_converges_toward_zero_with_correct() {
        let mut store = BigramStatsStore::default();
        let key = BigramKey(['a', 'b']);
        // Start with an error
        store.update(key.clone(), 200.0, false, false, 0);
        assert!((store.smoothed_error_rate(&key) - 1.0).abs() < f64::EPSILON);
        // 20 correct strokes should bring it down significantly
        for i in 1..=20 {
            store.update(key.clone(), 200.0, true, false, i);
        }
        let rate = store.smoothed_error_rate(&key);
        assert!(
            rate < 0.15,
            "After 20 correct, EMA should be < 0.15, got {rate}"
        );
    }

    #[test]
    fn test_error_rate_ema_decay() {
        // Verify that after N correct strokes, error_rate_ema drops as expected
        let mut store = BigramStatsStore::default();
        let key = BigramKey(['t', 'h']);
        // Simulate 30% error rate: 3 errors in 10 strokes
        for i in 0..10 {
            let correct = i % 3 != 0; // errors at 0, 3, 6, 9
            store.update(key.clone(), 200.0, correct, false, i);
        }
        let rate_before = store.smoothed_error_rate(&key);
        // Now 15 correct strokes
        for i in 10..25 {
            store.update(key.clone(), 200.0, true, false, i);
        }
        let rate_after = store.smoothed_error_rate(&key);
        assert!(
            rate_after < rate_before,
            "EMA should decay: before={rate_before} after={rate_after}"
        );
        assert!(
            rate_after < 0.15,
            "After 15 correct strokes, rate should be < 0.15, got {rate_after}"
        );
    }

    // --- Redundancy tests ---

    #[test]
    fn redundancy_proxy_example() {
        // "is" where 's' is weak — bigram error rate is explained by char weakness
        let mut char_stats = KeyStatsStore::default();
        let s_stat = char_stats.stats.entry('s').or_default();
        s_stat.error_rate_ema = 0.25;
        let i_stat = char_stats.stats.entry('i').or_default();
        i_stat.error_rate_ema = 0.03;

        let mut bigram_stats = BigramStatsStore::default();
        let is_key = BigramKey(['i', 's']);
        let is_stat = bigram_stats.stats.entry(is_key.clone()).or_default();
        is_stat.error_rate_ema = 0.27;
        is_stat.sample_count = 100;

        let e_s = char_stats.smoothed_error_rate('s');
        let e_i = char_stats.smoothed_error_rate('i');
        let e_is = bigram_stats.smoothed_error_rate(&is_key);
        let expected = 1.0 - (1.0 - e_s) * (1.0 - e_i);
        let redundancy = bigram_stats.error_anomaly_ratio(&is_key, &char_stats);

        assert!(
            redundancy < ERROR_ANOMALY_RATIO_THRESHOLD,
            "Proxy bigram 'is' should have redundancy < {ERROR_ANOMALY_RATIO_THRESHOLD}, got {redundancy} (e_s={e_s}, e_i={e_i}, e_is={e_is}, expected={expected})"
        );
    }

    #[test]
    fn redundancy_genuine_difficulty() {
        // "ed" where both chars are fine individually but bigram has high error rate
        let mut char_stats = KeyStatsStore::default();
        let e_stat = char_stats.stats.entry('e').or_default();
        e_stat.error_rate_ema = 0.04;
        let d_stat = char_stats.stats.entry('d').or_default();
        d_stat.error_rate_ema = 0.05;

        let mut bigram_stats = BigramStatsStore::default();
        let ed_key = BigramKey(['e', 'd']);
        let ed_stat = bigram_stats.stats.entry(ed_key.clone()).or_default();
        ed_stat.error_rate_ema = 0.22;
        ed_stat.sample_count = 100;

        let redundancy = bigram_stats.error_anomaly_ratio(&ed_key, &char_stats);
        assert!(
            redundancy > ERROR_ANOMALY_RATIO_THRESHOLD,
            "Genuine difficulty 'ed' should have redundancy > {ERROR_ANOMALY_RATIO_THRESHOLD}, got {redundancy}"
        );
    }

    #[test]
    fn redundancy_trigram_explained_by_bigram() {
        // "the" where "th" bigram explains the difficulty
        let mut char_stats = KeyStatsStore::default();
        for &(ch, ema) in &[('t', 0.03), ('h', 0.04), ('e', 0.04)] {
            let s = char_stats.stats.entry(ch).or_default();
            s.error_rate_ema = ema;
        }

        let mut bigram_stats = BigramStatsStore::default();
        let th_stat = bigram_stats.stats.entry(BigramKey(['t', 'h'])).or_default();
        th_stat.error_rate_ema = 0.15;
        th_stat.sample_count = 100;
        let he_stat = bigram_stats.stats.entry(BigramKey(['h', 'e'])).or_default();
        he_stat.error_rate_ema = 0.04;
        he_stat.sample_count = 100;

        let mut trigram_stats = TrigramStatsStore::default();
        let the_key = TrigramKey(['t', 'h', 'e']);
        let the_stat = trigram_stats.stats.entry(the_key.clone()).or_default();
        the_stat.error_rate_ema = 0.16;
        the_stat.sample_count = 100;

        let redundancy = trigram_stats.redundancy_score(&the_key, &bigram_stats, &char_stats);
        assert!(
            redundancy < ERROR_ANOMALY_RATIO_THRESHOLD,
            "Trigram 'the' explained by 'th' bigram should have redundancy < {ERROR_ANOMALY_RATIO_THRESHOLD}, got {redundancy}"
        );
    }

    // --- Stability gate tests ---

    #[test]
    fn error_anomaly_streak_increments_and_resets() {
        let mut bigram_stats = BigramStatsStore::default();
        let key = BigramKey(['e', 'd']);

        // Set up a bigram with genuine difficulty via EMA
        let stat = bigram_stats.stats.entry(key.clone()).or_default();
        stat.error_rate_ema = 0.25;
        stat.sample_count = 100;

        let mut char_stats = KeyStatsStore::default();
        // Low char error rates
        char_stats.stats.entry('e').or_default().error_rate_ema = 0.03;
        char_stats.stats.entry('d').or_default().error_rate_ema = 0.03;

        // Should increment streak
        bigram_stats.update_error_anomaly_streak(&key, &char_stats);
        assert_eq!(bigram_stats.stats[&key].error_anomaly_streak, 1);
        bigram_stats.update_error_anomaly_streak(&key, &char_stats);
        assert_eq!(bigram_stats.stats[&key].error_anomaly_streak, 2);
        bigram_stats.update_error_anomaly_streak(&key, &char_stats);
        assert_eq!(bigram_stats.stats[&key].error_anomaly_streak, 3);

        // Now simulate char stats getting worse (making anomaly ratio low)
        char_stats.stats.entry('e').or_default().error_rate_ema = 0.30;
        bigram_stats.update_error_anomaly_streak(&key, &char_stats);
        assert_eq!(bigram_stats.stats[&key].error_anomaly_streak, 0); // reset
    }

    #[test]
    fn worst_confirmed_anomaly_requires_all_conditions() {
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();
        let unlocked = vec!['a', 'b', 'c', 'd', 'e'];

        // Set up char stats with low EMA error rates
        for &ch in &['a', 'b'] {
            let s = char_stats.stats.entry(ch).or_default();
            s.error_rate_ema = 0.03;
        }

        let key = BigramKey(['a', 'b']);
        let stat = bigram_stats.stats.entry(key.clone()).or_default();
        stat.error_rate_ema = 0.80;
        stat.sample_count = 25; // enough samples
        stat.error_anomaly_streak = ANOMALY_STREAK_REQUIRED; // stable

        // Should be confirmed
        let result = bigram_stats.worst_confirmed_anomaly(&char_stats, &unlocked);
        assert!(
            result.is_some(),
            "Should be confirmed with all conditions met"
        );

        // Reset streak -> not confirmed
        bigram_stats
            .stats
            .get_mut(&key)
            .unwrap()
            .error_anomaly_streak = 2;
        let result = bigram_stats.worst_confirmed_anomaly(&char_stats, &unlocked);
        assert!(
            result.is_none(),
            "Should NOT be confirmed without stable streak"
        );

        // Restore streak, reduce samples -> not confirmed
        bigram_stats
            .stats
            .get_mut(&key)
            .unwrap()
            .error_anomaly_streak = ANOMALY_STREAK_REQUIRED;
        bigram_stats.stats.get_mut(&key).unwrap().sample_count = 15;
        let result = bigram_stats.worst_confirmed_anomaly(&char_stats, &unlocked);
        assert!(
            result.is_none(),
            "Should NOT be confirmed with < 20 samples"
        );
    }

    // --- Focus selection tests ---

    #[test]
    fn focus_no_bigrams_gives_char_only() {
        let skill_tree = SkillTree::default();
        let key_stats = KeyStatsStore::default();
        let bigram_stats = BigramStatsStore::default();

        let selection = select_focus(&skill_tree, DrillScope::Global, &key_stats, &bigram_stats);

        // No bigram data → bigram_focus should be None
        assert!(
            selection.bigram_focus.is_none(),
            "No bigram data should mean no bigram focus"
        );
    }

    #[test]
    fn focus_both_char_and_bigram_independent() {
        let skill_tree = SkillTree::default();
        let mut key_stats = KeyStatsStore::default();

        for &ch in &['e', 't', 'a', 'o', 'n', 'i'] {
            let stat = key_stats.stats.entry(ch).or_default();
            stat.confidence = 0.95;
            stat.filtered_time_ms = 360.0;
            stat.sample_count = 50;
            stat.total_count = 50;
            stat.error_rate_ema = 0.03;
        }
        key_stats.stats.get_mut(&'n').unwrap().confidence = 0.5;
        key_stats.stats.get_mut(&'n').unwrap().filtered_time_ms = 686.0;

        // Set up a bigram with confirmed error anomaly
        let mut bigram_stats = BigramStatsStore::default();
        let et_key = BigramKey(['e', 't']);
        let stat = bigram_stats.stats.entry(et_key.clone()).or_default();
        stat.sample_count = 30;
        stat.error_rate_ema = 0.80;
        stat.error_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        let selection = select_focus(&skill_tree, DrillScope::Global, &key_stats, &bigram_stats);

        // Both should be populated independently
        assert_eq!(
            selection.char_focus,
            Some('n'),
            "Char focus should be weakest char 'n'"
        );
        assert!(
            selection.bigram_focus.is_some(),
            "Bigram focus should be present"
        );
        let (key, _, _) = selection.bigram_focus.unwrap();
        assert_eq!(key, et_key, "Bigram focus should be 'et'");
    }

    #[test]
    fn focus_char_only_when_no_confirmed_bigram() {
        let skill_tree = SkillTree::default();
        let mut key_stats = KeyStatsStore::default();

        for &ch in &['e', 't', 'a', 'o', 'n', 'i'] {
            let stat = key_stats.stats.entry(ch).or_default();
            stat.confidence = 0.95;
            stat.filtered_time_ms = 360.0;
            stat.sample_count = 50;
            stat.total_count = 50;
            stat.error_rate_ema = 0.03;
        }
        key_stats.stats.get_mut(&'n').unwrap().confidence = 0.1;
        key_stats.stats.get_mut(&'n').unwrap().filtered_time_ms = 3400.0;

        // Bigram with low error rate → no anomaly
        let mut bigram_stats = BigramStatsStore::default();
        let et_key = BigramKey(['e', 't']);
        let stat = bigram_stats.stats.entry(et_key.clone()).or_default();
        stat.sample_count = 30;
        stat.error_rate_ema = 0.02;
        stat.error_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        let selection = select_focus(&skill_tree, DrillScope::Global, &key_stats, &bigram_stats);

        assert_eq!(
            selection.char_focus,
            Some('n'),
            "Should focus on weakest char 'n'"
        );
        assert!(
            selection.bigram_focus.is_none(),
            "No confirmed anomaly → no bigram focus"
        );
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
            stat.error_rate_ema = 0.03;
        }
        key_stats.stats.get_mut(&'n').unwrap().confidence = 0.5;
        key_stats.stats.get_mut(&'n').unwrap().filtered_time_ms = 686.0;

        // Bigram with high error rate but streak only 2 (needs 3)
        let mut bigram_stats = BigramStatsStore::default();
        let et_key = BigramKey(['e', 't']);
        let stat = bigram_stats.stats.entry(et_key.clone()).or_default();
        stat.sample_count = 30;
        stat.error_rate_ema = 0.80;
        stat.error_anomaly_streak = ANOMALY_STREAK_REQUIRED - 1; // not enough

        let selection = select_focus(&skill_tree, DrillScope::Global, &key_stats, &bigram_stats);

        assert!(
            selection.bigram_focus.is_none(),
            "Insufficient streak → no bigram focus"
        );
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
        assert_eq!(
            trigram_marginal_gain(&trigram_stats, &bigram_stats, &char_stats),
            0.0
        );
    }

    // --- Replay invariance ---

    #[test]
    fn replay_produces_correct_error_total_counts() {
        // Simulate a replay: process keystrokes and verify counts + EMA
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

        // Process like rebuild_ngram_stats does (updating EMA for correct strokes too)
        for kt in &keystrokes {
            if kt.correct {
                let stat = key_stats.stats.entry(kt.key).or_default();
                stat.total_count += 1;
                if stat.total_count == 1 {
                    stat.error_rate_ema = 0.0;
                } else {
                    stat.error_rate_ema = 0.1 * 0.0 + 0.9 * stat.error_rate_ema;
                }
            } else {
                key_stats.update_key_error(kt.key);
            }
        }

        let a_stat = key_stats.stats.get(&'a').unwrap();
        assert_eq!(
            a_stat.total_count, 13,
            "a: 10 correct + 3 errors = 13 total"
        );
        assert_eq!(a_stat.error_count, 3, "a: 3 errors");

        let b_stat = key_stats.stats.get(&'b').unwrap();
        assert_eq!(b_stat.total_count, 6, "b: 5 correct + 1 error = 6 total");
        assert_eq!(b_stat.error_count, 1, "b: 1 error");

        // Verify EMA error rate is reasonable (not exact Laplace, but proportional)
        let a_rate = key_stats.smoothed_error_rate('a');
        // 'a' had 3 errors in 13 strokes, last was error → EMA should be moderate
        assert!(
            a_rate > 0.05 && a_rate < 0.5,
            "a rate should be moderate, got {a_rate}"
        );

        let b_rate = key_stats.smoothed_error_rate('b');
        // 'b' had 1 error (the last stroke) → EMA should reflect recent error
        assert!(
            b_rate > 0.05 && b_rate < 0.5,
            "b rate should reflect recent error, got {b_rate}"
        );
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
        assert!(
            trigram_stats.stats.contains_key(&new_key),
            "most recent trigram should survive prune"
        );
        assert!(
            !trigram_stats.stats.contains_key(&old_key),
            "oldest trigram should be pruned"
        );
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
                store.update(
                    ev.key.clone(),
                    ev.total_time_ms,
                    ev.correct,
                    ev.has_hesitation,
                    0,
                );
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
            stat.error_rate_ema = 0.05;
        }

        let mut count: usize = 0;
        for &a in &all_chars {
            for &b in &all_chars {
                if bigram_stats.stats.len() >= 3000 {
                    break;
                }
                let key = BigramKey([a, b]);
                let stat = bigram_stats.stats.entry(key).or_default();
                stat.sample_count = 25 + count % 30;
                stat.error_rate_ema = 0.1 + (count % 10) as f64 * 0.05;
                stat.error_anomaly_streak = if count % 3 == 0 { 3 } else { 1 };
                count += 1;
            }
        }
        assert_eq!(bigram_stats.stats.len(), 3000);

        let unlocked: Vec<char> = all_chars;
        let budget = std::time::Duration::from_millis(5 * DEBUG_MULTIPLIER as u64);

        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = bigram_stats.worst_confirmed_anomaly(&char_stats, &unlocked);
        }
        let elapsed = start.elapsed() / 100;

        assert!(
            elapsed < budget,
            "focus selection took {elapsed:?} per call, budget is {budget:?}"
        );
    }

    #[test]
    fn perf_budget_history_replay_under_500ms() {
        let drills: Vec<Vec<KeyTime>> = (0..500).map(|_| make_bench_keystrokes(300)).collect();

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
                    ev.key.clone(),
                    ev.total_time_ms,
                    ev.correct,
                    ev.has_hesitation,
                    drill_idx as u32,
                );
            }
            for ev in &trigram_events {
                trigram_stats.update(
                    ev.key.clone(),
                    ev.total_time_ms,
                    ev.correct,
                    ev.has_hesitation,
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

    // --- error_anomaly_bigrams tests ---

    fn make_bigram_store_with_char_stats() -> (BigramStatsStore, KeyStatsStore) {
        let mut char_stats = KeyStatsStore::default();
        for ch in 'a'..='z' {
            let s = char_stats.stats.entry(ch).or_default();
            s.error_rate_ema = 0.03;
        }
        let bigram_stats = BigramStatsStore::default();
        (bigram_stats, char_stats)
    }

    #[test]
    fn test_error_anomaly_bigrams() {
        let (mut bigram_stats, char_stats) = make_bigram_store_with_char_stats();
        let unlocked: Vec<char> = ('a'..='z').collect();

        // Confirmed: sample=25, streak=3, high EMA → anomaly ratio > 1.5
        let k1 = BigramKey(['t', 'h']);
        let s1 = bigram_stats.stats.entry(k1.clone()).or_default();
        s1.sample_count = 25;
        s1.error_rate_ema = 0.70;
        s1.error_anomaly_streak = 3;

        // Included but not confirmed: samples < 20
        let k2 = BigramKey(['e', 'd']);
        let s2 = bigram_stats.stats.entry(k2.clone()).or_default();
        s2.sample_count = 15;
        s2.error_rate_ema = 0.60;
        s2.error_anomaly_streak = 3;

        // Excluded: samples < ANOMALY_MIN_SAMPLES (3)
        let k3 = BigramKey(['a', 'b']);
        let s3 = bigram_stats.stats.entry(k3.clone()).or_default();
        s3.sample_count = 2;
        s3.error_rate_ema = 0.80;
        s3.error_anomaly_streak = 3;

        // Excluded: error anomaly ratio <= 1.5 (low EMA)
        let k4 = BigramKey(['i', 's']);
        let s4 = bigram_stats.stats.entry(k4.clone()).or_default();
        s4.sample_count = 25;
        s4.error_rate_ema = 0.02;
        s4.error_anomaly_streak = 3;

        let anomalies = bigram_stats.error_anomaly_bigrams(&char_stats, &unlocked);
        let keys: Vec<BigramKey> = anomalies.iter().map(|a| a.key.clone()).collect();

        assert!(keys.contains(&k1), "k1 should be in error anomalies");
        assert!(
            keys.contains(&k2),
            "k2 should be in error anomalies (above min samples)"
        );
        assert!(
            !keys.contains(&k3),
            "k3 should be excluded (too few samples)"
        );
        assert!(
            !keys.contains(&k4),
            "k4 should be excluded (low anomaly ratio)"
        );

        // k1 should be confirmed (samples >= 20 && streak >= 3)
        let k1_entry = anomalies.iter().find(|a| a.key == k1).unwrap();
        assert!(k1_entry.confirmed, "k1 should be confirmed");

        // k2 should NOT be confirmed (samples < 20)
        let k2_entry = anomalies.iter().find(|a| a.key == k2).unwrap();
        assert!(
            !k2_entry.confirmed,
            "k2 should NOT be confirmed (low samples)"
        );
    }

    #[test]
    fn test_speed_anomaly_pct() {
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();

        // Set up char 'b' with sufficient samples and known time
        let b_stat = char_stats.stats.entry('b').or_default();
        b_stat.sample_count = 10; // exactly at threshold
        b_stat.filtered_time_ms = 200.0;

        // Set up bigram 'a','b' with time 50% slower than char b
        let key = BigramKey(['a', 'b']);
        let stat = bigram_stats.stats.entry(key.clone()).or_default();
        stat.filtered_time_ms = 300.0; // 50% slower than 200
        stat.sample_count = 10;

        let pct = bigram_stats.speed_anomaly_pct(&key, &char_stats);
        assert!(
            pct.is_some(),
            "Should return Some when char has enough samples"
        );
        assert!(
            (pct.unwrap() - 50.0).abs() < f64::EPSILON,
            "Should be 50% slower"
        );

        // Reduce char_b samples below threshold
        char_stats.stats.get_mut(&'b').unwrap().sample_count = 9;
        let pct = bigram_stats.speed_anomaly_pct(&key, &char_stats);
        assert!(
            pct.is_none(),
            "Should return None when char has < 10 samples"
        );
    }

    #[test]
    fn test_speed_anomaly_streak_holds_when_char_unavailable() {
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();

        // Set up char 'b' with insufficient samples
        let b_stat = char_stats.stats.entry('b').or_default();
        b_stat.sample_count = 5; // below MIN_CHAR_SAMPLES_FOR_SPEED
        b_stat.filtered_time_ms = 200.0;

        let key = BigramKey(['a', 'b']);
        let stat = bigram_stats.stats.entry(key.clone()).or_default();
        stat.filtered_time_ms = 400.0;
        stat.sample_count = 10;
        stat.speed_anomaly_streak = 2; // pre-existing streak

        // Update streak — char baseline unavailable, should hold
        bigram_stats.update_speed_anomaly_streak(&key, &char_stats);
        assert_eq!(
            bigram_stats.stats[&key].speed_anomaly_streak, 2,
            "Streak should be held when char unavailable"
        );

        // Now give char_b enough samples
        char_stats.stats.get_mut(&'b').unwrap().sample_count = 10;

        // Speed anomaly = (400/200 - 1) * 100 = 100% > 50% threshold => increment
        bigram_stats.update_speed_anomaly_streak(&key, &char_stats);
        assert_eq!(
            bigram_stats.stats[&key].speed_anomaly_streak, 3,
            "Streak should increment when above threshold"
        );

        // Make speed normal
        bigram_stats.stats.get_mut(&key).unwrap().filtered_time_ms = 220.0;
        // Speed anomaly = (220/200 - 1) * 100 = 10% < 50% threshold => reset
        bigram_stats.update_speed_anomaly_streak(&key, &char_stats);
        assert_eq!(
            bigram_stats.stats[&key].speed_anomaly_streak, 0,
            "Streak should reset when below threshold"
        );
    }

    #[test]
    fn test_speed_anomaly_bigrams() {
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();
        let unlocked = vec!['a', 'b', 'c', 'd'];

        // Set up char stats with enough samples
        for &ch in &['b', 'd'] {
            let s = char_stats.stats.entry(ch).or_default();
            s.sample_count = 15;
            s.filtered_time_ms = 200.0;
        }

        // Bigram with speed anomaly > 50%
        let k1 = BigramKey(['a', 'b']);
        let s1 = bigram_stats.stats.entry(k1.clone()).or_default();
        s1.filtered_time_ms = 400.0; // 100% slower
        s1.sample_count = 25;
        s1.speed_anomaly_streak = 3;

        // Bigram with speed anomaly < 50% (excluded)
        let k2 = BigramKey(['c', 'd']);
        let s2 = bigram_stats.stats.entry(k2.clone()).or_default();
        s2.filtered_time_ms = 250.0; // 25% slower
        s2.sample_count = 25;
        s2.speed_anomaly_streak = 3;

        let anomalies = bigram_stats.speed_anomaly_bigrams(&char_stats, &unlocked);
        let keys: Vec<BigramKey> = anomalies.iter().map(|a| a.key.clone()).collect();

        assert!(
            keys.contains(&k1),
            "k1 should be in speed anomalies (100% slower)"
        );
        assert!(
            !keys.contains(&k2),
            "k2 should be excluded (only 25% slower)"
        );

        let k1_entry = anomalies.iter().find(|a| a.key == k1).unwrap();
        assert!(k1_entry.confirmed, "k1 should be confirmed");
    }

    #[test]
    fn test_worst_confirmed_anomaly_dedup() {
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();
        let unlocked = vec!['a', 'b'];

        // Set up char stats with low EMA error rates
        let b_stat = char_stats.stats.entry('b').or_default();
        b_stat.sample_count = 15;
        b_stat.filtered_time_ms = 200.0;
        b_stat.error_rate_ema = 0.03;

        let a_stat = char_stats.stats.entry('a').or_default();
        a_stat.error_rate_ema = 0.03;

        // Bigram with both error and speed anomalies
        let key = BigramKey(['a', 'b']);
        let stat = bigram_stats.stats.entry(key.clone()).or_default();
        stat.error_rate_ema = 0.70;
        stat.sample_count = 25;
        stat.error_anomaly_streak = ANOMALY_STREAK_REQUIRED;
        stat.filtered_time_ms = 600.0; // 200% slower
        stat.speed_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        let result = bigram_stats.worst_confirmed_anomaly(&char_stats, &unlocked);
        assert!(result.is_some(), "Should find a confirmed anomaly");

        // Should pick whichever anomaly type has higher pct
        let (_, pct, _) = result.unwrap();
        let error_pct = bigram_stats.error_anomaly_pct(&key, &char_stats).unwrap();
        let speed_pct = bigram_stats.speed_anomaly_pct(&key, &char_stats).unwrap();
        let expected_pct = error_pct.max(speed_pct);
        assert!(
            (pct - expected_pct).abs() < f64::EPSILON,
            "Should pick higher anomaly pct"
        );
    }

    #[test]
    fn test_worst_confirmed_anomaly_prefers_error_on_tie() {
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();
        let unlocked = vec!['a', 'b'];

        let b_stat = char_stats.stats.entry('b').or_default();
        b_stat.sample_count = 15;
        b_stat.filtered_time_ms = 200.0;
        b_stat.error_rate_ema = 0.03;

        let a_stat = char_stats.stats.entry('a').or_default();
        a_stat.error_rate_ema = 0.03;

        let key = BigramKey(['a', 'b']);
        let stat = bigram_stats.stats.entry(key.clone()).or_default();
        stat.sample_count = 25;
        stat.error_anomaly_streak = ANOMALY_STREAK_REQUIRED;
        stat.speed_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        // Set EMA so error_anomaly_pct ≈ 150%
        // expected_ab = 1 - (1 - 0.03)^2 ≈ 0.0591
        // For ratio = 2.5: e_ab = 2.5 * 0.0591 ≈ 0.1478
        stat.error_rate_ema = 0.1478;
        // speed_anomaly_pct = (500/200 - 1)*100 = 150%
        stat.filtered_time_ms = 500.0;

        let error_pct = bigram_stats.error_anomaly_pct(&key, &char_stats).unwrap();
        let speed_pct = bigram_stats.speed_anomaly_pct(&key, &char_stats).unwrap();

        let result = bigram_stats.worst_confirmed_anomaly(&char_stats, &unlocked);
        assert!(result.is_some());
        let (_, _pct, typ) = result.unwrap();

        if (error_pct - speed_pct).abs() < 1.0 {
            assert_eq!(
                typ,
                AnomalyType::Error,
                "Error should win on tie or near-tie"
            );
        } else if error_pct > speed_pct {
            assert_eq!(typ, AnomalyType::Error, "Error should win when higher");
        } else {
            assert_eq!(typ, AnomalyType::Speed, "Speed should win when higher");
        }

        // Force exact tie by setting speed to match error exactly
        let exact_speed_time = (error_pct / 100.0 + 1.0) * 200.0;
        bigram_stats.stats.get_mut(&key).unwrap().filtered_time_ms = exact_speed_time;

        let error_pct2 = bigram_stats.error_anomaly_pct(&key, &char_stats).unwrap();
        let speed_pct2 = bigram_stats.speed_anomaly_pct(&key, &char_stats).unwrap();
        assert!(
            (error_pct2 - speed_pct2).abs() < f64::EPSILON,
            "Pcts should be exactly equal: error={error_pct2}, speed={speed_pct2}"
        );

        let result2 = bigram_stats.worst_confirmed_anomaly(&char_stats, &unlocked);
        assert!(result2.is_some());
        let (_, _, typ2) = result2.unwrap();
        assert_eq!(typ2, AnomalyType::Error, "Error should win on exact tie");
    }

    #[test]
    fn test_speed_anomaly_borderline_baseline() {
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();

        let key = BigramKey(['a', 'b']);
        let stat = bigram_stats.stats.entry(key.clone()).or_default();
        stat.filtered_time_ms = 400.0; // 2x char baseline => 100% anomaly
        stat.sample_count = 10;

        // At 9 samples: speed_anomaly_pct should return None
        let b_stat = char_stats.stats.entry('b').or_default();
        b_stat.filtered_time_ms = 200.0;
        b_stat.sample_count = 9;

        assert!(
            bigram_stats.speed_anomaly_pct(&key, &char_stats).is_none(),
            "Should be None at 9 char samples"
        );

        // At exactly 10 samples: should return Some
        char_stats.stats.get_mut(&'b').unwrap().sample_count = 10;
        let pct = bigram_stats.speed_anomaly_pct(&key, &char_stats);
        assert!(pct.is_some(), "Should be Some at exactly 10 char samples");
        assert!(
            (pct.unwrap() - 100.0).abs() < f64::EPSILON,
            "400ms / 200ms => 100% anomaly"
        );

        // Realistic-noise fixture: char baseline is 200ms, bigram is 310ms => 55% anomaly
        // (just above 50% threshold). This should be a mild anomaly, not extreme.
        bigram_stats.stats.get_mut(&key).unwrap().filtered_time_ms = 310.0;
        let pct = bigram_stats.speed_anomaly_pct(&key, &char_stats).unwrap();
        assert!(
            (pct - 55.0).abs() < 1e-10,
            "310ms / 200ms => 55% anomaly, got {pct}"
        );
        assert!(
            pct > SPEED_ANOMALY_PCT_THRESHOLD && pct < 100.0,
            "55% should be above 50% threshold but not extreme"
        );

        // At exactly the threshold: 300ms / 200ms = 50% exactly
        bigram_stats.stats.get_mut(&key).unwrap().filtered_time_ms = 300.0;
        let pct = bigram_stats.speed_anomaly_pct(&key, &char_stats).unwrap();
        assert!(
            (pct - 50.0).abs() < f64::EPSILON,
            "300ms / 200ms => exactly 50%"
        );

        // Verify streak behavior at boundary: at exactly threshold, streak should NOT increment
        // (threshold comparison is >, not >=)
        let stat = bigram_stats.stats.get_mut(&key).unwrap();
        stat.speed_anomaly_streak = 2;
        stat.filtered_time_ms = 300.0; // exactly 50%
        bigram_stats.update_speed_anomaly_streak(&key, &char_stats);
        assert_eq!(
            bigram_stats.stats[&key].speed_anomaly_streak, 0,
            "Streak should reset at exactly threshold (not strictly above)"
        );
    }

    #[test]
    fn test_select_focus_both_active() {
        let skill_tree = SkillTree::default();
        let mut key_stats = KeyStatsStore::default();

        for &ch in &['e', 't', 'a', 'o', 'n', 'i'] {
            let stat = key_stats.stats.entry(ch).or_default();
            stat.confidence = 0.95;
            stat.filtered_time_ms = 360.0;
            stat.sample_count = 50;
            stat.total_count = 50;
            stat.error_rate_ema = 0.03;
        }
        key_stats.stats.get_mut(&'n').unwrap().confidence = 0.5;
        key_stats.stats.get_mut(&'n').unwrap().filtered_time_ms = 686.0;

        let mut bigram_stats = BigramStatsStore::default();
        let et_key = BigramKey(['e', 't']);
        let stat = bigram_stats.stats.entry(et_key.clone()).or_default();
        stat.sample_count = 30;
        stat.error_rate_ema = 0.80;
        stat.error_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        let selection = select_focus(&skill_tree, DrillScope::Global, &key_stats, &bigram_stats);

        assert_eq!(selection.char_focus, Some('n'));
        assert!(selection.bigram_focus.is_some());
        let (key, pct, _) = selection.bigram_focus.unwrap();
        assert_eq!(key, et_key);
        assert!(pct > 0.0);
    }

    #[test]
    fn test_select_focus_bigram_only() {
        // All chars mastered, but bigram anomaly exists
        let skill_tree = SkillTree::default();
        let mut key_stats = KeyStatsStore::default();

        for &ch in &['e', 't', 'a', 'o', 'n', 'i'] {
            let stat = key_stats.stats.entry(ch).or_default();
            stat.confidence = 2.0;
            stat.filtered_time_ms = 100.0;
            stat.sample_count = 200;
            stat.total_count = 200;
            stat.error_rate_ema = 0.01;
        }

        assert!(
            skill_tree
                .focused_key(DrillScope::Global, &key_stats)
                .is_none(),
            "Precondition: focused_key should return None when all chars are mastered"
        );

        let mut bigram_stats = BigramStatsStore::default();
        let et_key = BigramKey(['e', 't']);
        let stat = bigram_stats.stats.entry(et_key.clone()).or_default();
        stat.sample_count = 30;
        stat.error_rate_ema = 0.80;
        stat.error_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        let selection = select_focus(&skill_tree, DrillScope::Global, &key_stats, &bigram_stats);

        assert!(
            selection.char_focus.is_none(),
            "No char weakness → no char focus"
        );
        assert!(
            selection.bigram_focus.is_some(),
            "Bigram anomaly should be present"
        );
    }

    #[test]
    fn test_ema_ranking_stability_during_recovery() {
        // Two bigrams both confirmed. Bigram A has higher anomaly.
        // User corrects bigram A → B becomes worst.
        let mut bigram_stats = BigramStatsStore::default();
        let mut char_stats = KeyStatsStore::default();
        let unlocked = vec!['a', 'b', 'c', 'd'];

        for &ch in &['a', 'b', 'c', 'd'] {
            char_stats.stats.entry(ch).or_default().error_rate_ema = 0.03;
        }

        let key_a = BigramKey(['a', 'b']);
        let sa = bigram_stats.stats.entry(key_a.clone()).or_default();
        sa.error_rate_ema = 0.50;
        sa.sample_count = 30;
        sa.error_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        let key_b = BigramKey(['c', 'd']);
        let sb = bigram_stats.stats.entry(key_b.clone()).or_default();
        sb.error_rate_ema = 0.30;
        sb.sample_count = 30;
        sb.error_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        // Initially A is worst
        let result = bigram_stats.worst_confirmed_anomaly(&char_stats, &unlocked);
        assert!(result.is_some());
        let (worst_key, _, _) = result.unwrap();
        assert_eq!(worst_key, key_a, "A should be worst initially");

        // Simulate A recovering: 20 correct strokes
        for i in 30..50 {
            bigram_stats.update(key_a.clone(), 200.0, true, false, i);
            bigram_stats.update_error_anomaly_streak(&key_a, &char_stats);
        }

        // Now B should be worst (A recovered)
        let result2 = bigram_stats.worst_confirmed_anomaly(&char_stats, &unlocked);
        if let Some((worst_key2, _, _)) = result2 {
            // B should now be the worst (or A dropped out of anomaly entirely)
            if worst_key2 == key_a {
                // A's EMA should be much lower than before
                let a_ema = bigram_stats.stats[&key_a].error_rate_ema;
                assert!(
                    a_ema < 0.30,
                    "A's EMA should have dropped significantly, got {a_ema}"
                );
            }
        }
        // A's EMA should definitely be lower now
        let a_ema = bigram_stats.stats[&key_a].error_rate_ema;
        assert!(
            a_ema < bigram_stats.stats[&key_b].error_rate_ema,
            "After recovery, A's EMA ({a_ema}) should be < B's ({})",
            bigram_stats.stats[&key_b].error_rate_ema
        );
    }
}
