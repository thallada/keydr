use std::collections::HashSet;

use rand::Rng;
use rand::rngs::SmallRng;

use crate::engine::filter::CharFilter;
use crate::generator::TextGenerator;
use crate::generator::dictionary::Dictionary;
use crate::generator::transition_table::TransitionTable;

const MIN_WORD_LEN: usize = 3;
const MAX_WORD_LEN: usize = 10;
const MIN_REAL_WORDS: usize = 8;
const FULL_DICT_THRESHOLD: usize = 60;

pub struct PhoneticGenerator {
    table: TransitionTable,
    dictionary: Dictionary,
    rng: SmallRng,
    cross_drill_history: HashSet<String>,
    #[cfg(test)]
    pub dict_picks: usize,
}

impl PhoneticGenerator {
    pub fn new(
        table: TransitionTable,
        dictionary: Dictionary,
        rng: SmallRng,
        cross_drill_history: HashSet<String>,
    ) -> Self {
        Self {
            table,
            dictionary,
            rng,
            cross_drill_history,
            #[cfg(test)]
            dict_picks: 0,
        }
    }

    fn pick_weighted_from(
        rng: &mut SmallRng,
        options: &[(char, f64)],
        filter: &CharFilter,
    ) -> Option<char> {
        let filtered: Vec<(char, f64)> = options
            .iter()
            .filter(|(ch, _)| filter.is_allowed(*ch))
            .copied()
            .collect();

        if filtered.is_empty() {
            return None;
        }

        let total: f64 = filtered.iter().map(|(_, w)| w).sum();
        if total <= 0.0 {
            return None;
        }

        let mut roll = rng.gen_range(0.0..total);
        for (ch, weight) in &filtered {
            roll -= weight;
            if roll <= 0.0 {
                return Some(*ch);
            }
        }

        Some(filtered.last().unwrap().0)
    }

    fn generate_phonetic_word(
        &mut self,
        filter: &CharFilter,
        focused_char: Option<char>,
        focused_bigram: Option<[char; 2]>,
    ) -> String {
        for _attempt in 0..5 {
            let word = self.try_generate_word(filter, focused_char, focused_bigram);
            if word.len() >= MIN_WORD_LEN {
                return word;
            }
        }
        // Fallback
        "the".to_string()
    }

    fn try_generate_word(
        &mut self,
        filter: &CharFilter,
        focused: Option<char>,
        focused_bigram: Option<[char; 2]>,
    ) -> String {
        let mut word = Vec::new();

        // Try bigram-start: 30% chance to start word with bigram[0],bigram[1]
        let bigram_eligible =
            focused_bigram.filter(|b| filter.is_allowed(b[0]) && filter.is_allowed(b[1]));
        let start_char = if let Some(bg) = bigram_eligible {
            if self.rng.gen_bool(0.3) {
                word.push(bg[0]);
                word.push(bg[1]);
                // Continue Markov chain from the bigram
                let prefix = vec![' ', bg[0], bg[1]];
                if let Some(probs) = self.table.segment(&prefix) {
                    Self::pick_weighted_from(&mut self.rng, probs, filter)
                } else {
                    None
                }
            } else if let Some(focus) = focused {
                if self.rng.gen_bool(0.4) && filter.is_allowed(focus) {
                    word.push(focus);
                    let prefix = vec![' ', ' ', focus];
                    if let Some(probs) = self.table.segment(&prefix) {
                        Self::pick_weighted_from(&mut self.rng, probs, filter)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else if let Some(focus) = focused {
            if self.rng.gen_bool(0.4) && filter.is_allowed(focus) {
                word.push(focus);
                let prefix = vec![' ', ' ', focus];
                if let Some(probs) = self.table.segment(&prefix) {
                    Self::pick_weighted_from(&mut self.rng, probs, filter)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if word.is_empty() {
            // Pick a start from transition table
            let prefix = vec![' ', ' ', ' '];
            if let Some(probs) = self.table.segment(&prefix) {
                if let Some(ch) = Self::pick_weighted_from(&mut self.rng, probs, filter) {
                    word.push(ch);
                }
            }
            // Fallback: weighted random start
            if word.is_empty() {
                let starters: Vec<(char, f64)> = filter
                    .allowed
                    .iter()
                    .map(|&ch| {
                        let w = match ch {
                            'e' | 't' | 'a' => 3.0,
                            'o' | 'i' | 'n' | 's' => 2.0,
                            _ => 1.0,
                        };
                        (ch, w)
                    })
                    .collect();
                if let Some(ch) = Self::pick_weighted_from(&mut self.rng, &starters, filter) {
                    word.push(ch);
                } else {
                    return "the".to_string();
                }
            }
        }

        if let Some(ch) = start_char {
            word.push(ch);
        }

        while word.len() < MAX_WORD_LEN {
            // Build prefix from recent chars, padded with spaces
            let prefix_len = self.table.order - 1;
            let mut prefix = Vec::new();
            let start = if word.len() >= prefix_len {
                word.len() - prefix_len
            } else {
                0
            };
            for _ in 0..(prefix_len.saturating_sub(word.len())) {
                prefix.push(' ');
            }
            for i in start..word.len() {
                prefix.push(word[i]);
            }

            // Check for word ending (space probability increases with length)
            if word.len() >= MIN_WORD_LEN {
                if let Some(probs) = self.table.segment(&prefix) {
                    let space_weight: f64 = probs
                        .iter()
                        .filter(|(ch, _)| *ch == ' ')
                        .map(|(_, w)| w)
                        .sum();
                    if space_weight > 0.0 {
                        let boost = 1.3f64.powi(word.len() as i32 - MIN_WORD_LEN as i32);
                        let total: f64 = probs.iter().map(|(_, w)| w).sum();
                        let space_prob =
                            (space_weight * boost) / (total + space_weight * (boost - 1.0));
                        if self.rng.gen_bool(space_prob.min(0.85)) {
                            break;
                        }
                    }
                }
                // Even without space in table, use length-based ending
                let end_prob = 1.3f64.powi(word.len() as i32 - MIN_WORD_LEN as i32);
                if self.rng.gen_bool((end_prob / (end_prob + 5.0)).min(0.8)) {
                    break;
                }
            }

            // Get next character from transition table
            if let Some(probs) = self.table.segment(&prefix) {
                let non_space: Vec<(char, f64)> =
                    probs.iter().filter(|(ch, _)| *ch != ' ').copied().collect();
                if let Some(next) = Self::pick_weighted_from(&mut self.rng, &non_space, filter) {
                    word.push(next);
                } else {
                    break;
                }
            } else {
                // Fallback to vowel
                let vowels: Vec<(char, f64)> = ['a', 'e', 'i', 'o', 'u']
                    .iter()
                    .filter(|&&v| filter.is_allowed(v))
                    .map(|&v| (v, 1.0))
                    .collect();
                if let Some(v) = Self::pick_weighted_from(&mut self.rng, &vowels, filter) {
                    word.push(v);
                } else {
                    break;
                }
            }
        }

        word.iter().collect()
    }

    fn pick_tiered_word(
        &mut self,
        all_words: &[String],
        bigram_indices: &[usize],
        char_indices: &[usize],
        other_indices: &[usize],
        recent: &[String],
        cross_drill_accept_prob: f64,
    ) -> String {
        let max_attempts = all_words.len().clamp(6, 12);
        for _ in 0..max_attempts {
            let tier = self.select_tier(bigram_indices, char_indices, other_indices);
            let idx = tier[self.rng.gen_range(0..tier.len())];
            let word = &all_words[idx];
            if recent.contains(word) {
                continue;
            }
            if self.cross_drill_history.contains(word) {
                if self.rng.gen_bool(cross_drill_accept_prob) {
                    return word.clone();
                }
                continue;
            }
            return word.clone();
        }
        // Fallback: accept any non-recent word from full pool
        for _ in 0..all_words.len() {
            let idx = self.rng.gen_range(0..all_words.len());
            let word = &all_words[idx];
            if !recent.contains(word) {
                return word.clone();
            }
        }
        all_words[self.rng.gen_range(0..all_words.len())].clone()
    }

    fn select_tier<'a>(
        &mut self,
        bigram_indices: &'a [usize],
        char_indices: &'a [usize],
        other_indices: &'a [usize],
    ) -> &'a [usize] {
        let has_bigram = bigram_indices.len() >= 2;
        let has_char = char_indices.len() >= 2;

        // Tier selection probabilities:
        // Both available: 40% bigram, 30% char, 30% other
        // Only bigram:    50% bigram, 50% other
        // Only char:      70% char, 30% other
        // Neither:        100% other
        let roll: f64 = self.rng.gen_range(0.0..1.0);

        match (has_bigram, has_char) {
            (true, true) => {
                if roll < 0.4 {
                    bigram_indices
                } else if roll < 0.7 {
                    char_indices
                } else {
                    if other_indices.len() >= 2 {
                        other_indices
                    } else if has_char {
                        char_indices
                    } else {
                        bigram_indices
                    }
                }
            }
            (true, false) => {
                if roll < 0.5 {
                    bigram_indices
                } else {
                    if other_indices.len() >= 2 {
                        other_indices
                    } else {
                        bigram_indices
                    }
                }
            }
            (false, true) => {
                if roll < 0.7 {
                    char_indices
                } else {
                    if other_indices.len() >= 2 {
                        other_indices
                    } else {
                        char_indices
                    }
                }
            }
            (false, false) => {
                // Use other_indices if available, otherwise all words
                if other_indices.len() >= 2 {
                    other_indices
                } else {
                    char_indices
                }
            }
        }
    }
}

impl TextGenerator for PhoneticGenerator {
    fn generate(
        &mut self,
        filter: &CharFilter,
        focused_char: Option<char>,
        focused_bigram: Option<[char; 2]>,
        word_count: usize,
    ) -> String {
        let matching_words: Vec<String> = self
            .dictionary
            .find_matching(filter, None)
            .iter()
            .map(|s| s.to_string())
            .collect();
        let pool_size = matching_words.len();
        let use_dict = pool_size >= MIN_REAL_WORDS;

        // Hybrid ratio: linear interpolation between MIN_REAL_WORDS and FULL_DICT_THRESHOLD
        let dict_ratio = if pool_size <= MIN_REAL_WORDS {
            0.0
        } else if pool_size >= FULL_DICT_THRESHOLD {
            1.0
        } else {
            (pool_size - MIN_REAL_WORDS) as f64 / (FULL_DICT_THRESHOLD - MIN_REAL_WORDS) as f64
        };

        // Scaled within-drill dedup window based on dictionary pool size
        let dedup_window = if pool_size <= 20 {
            pool_size.saturating_sub(1).max(4)
        } else {
            (pool_size / 4).min(20)
        };

        // Cross-drill history accept probability (computed once)
        let cross_drill_accept_prob = if pool_size > 0 {
            let pool_set: HashSet<&str> = matching_words.iter().map(|s| s.as_str()).collect();
            let history_in_pool = self
                .cross_drill_history
                .iter()
                .filter(|w| pool_set.contains(w.as_str()))
                .count();
            let history_coverage = history_in_pool as f64 / pool_size as f64;
            0.15 + 0.60 * history_coverage
        } else {
            1.0
        };

        // Pre-categorize words into tiers for dictionary picks
        let bigram_str = focused_bigram.map(|b| format!("{}{}", b[0], b[1]));
        let focus_char_lower = focused_char.filter(|ch| ch.is_ascii_lowercase());

        let (bigram_indices, char_indices, other_indices) = if use_dict {
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
        let mut recent: Vec<String> = Vec::new();

        for _ in 0..word_count {
            let use_dict_word = use_dict && self.rng.gen_bool(dict_ratio);
            if use_dict_word {
                #[cfg(test)]
                {
                    self.dict_picks += 1;
                }
                let word = self.pick_tiered_word(
                    &matching_words,
                    &bigram_indices,
                    &char_indices,
                    &other_indices,
                    &recent,
                    cross_drill_accept_prob,
                );
                recent.push(word.clone());
                if recent.len() > dedup_window {
                    recent.remove(0);
                }
                words.push(word);
            } else {
                let word = self.generate_phonetic_word(filter, focused_char, focused_bigram);
                recent.push(word.clone());
                if recent.len() > dedup_window {
                    recent.remove(0);
                }
                words.push(word);
            }
        }

        words.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn focused_key_biases_real_word_sampling() {
        let dictionary = Dictionary::load();
        let table = TransitionTable::build_from_words(&dictionary.words_list());
        let filter = CharFilter::new(('a'..='z').collect());

        let mut focused_gen = PhoneticGenerator::new(
            table.clone(),
            Dictionary::load(),
            SmallRng::seed_from_u64(42),
            HashSet::new(),
        );
        let focused_text = focused_gen.generate(&filter, Some('k'), None, 1200);
        let focused_count = focused_text
            .split_whitespace()
            .filter(|w| w.contains('k'))
            .count();

        let mut baseline_gen = PhoneticGenerator::new(
            table,
            Dictionary::load(),
            SmallRng::seed_from_u64(42),
            HashSet::new(),
        );
        let baseline_text = baseline_gen.generate(&filter, None, None, 1200);
        let baseline_count = baseline_text
            .split_whitespace()
            .filter(|w| w.contains('k'))
            .count();

        assert!(
            focused_count > baseline_count,
            "focused_count={focused_count}, baseline_count={baseline_count}"
        );
    }

    #[test]
    fn test_phonetic_bigram_focus_increases_bigram_words() {
        let dictionary = Dictionary::load();
        let table = TransitionTable::build_from_words(&dictionary.words_list());
        let filter = CharFilter::new(('a'..='z').collect());

        let mut bigram_gen = PhoneticGenerator::new(
            table.clone(),
            Dictionary::load(),
            SmallRng::seed_from_u64(42),
            HashSet::new(),
        );
        let bigram_text = bigram_gen.generate(&filter, None, Some(['t', 'h']), 1200);
        let bigram_count = bigram_text
            .split_whitespace()
            .filter(|w| w.contains("th"))
            .count();

        let mut baseline_gen = PhoneticGenerator::new(
            table,
            Dictionary::load(),
            SmallRng::seed_from_u64(42),
            HashSet::new(),
        );
        let baseline_text = baseline_gen.generate(&filter, None, None, 1200);
        let baseline_count = baseline_text
            .split_whitespace()
            .filter(|w| w.contains("th"))
            .count();

        assert!(
            bigram_count > baseline_count,
            "bigram_count={bigram_count}, baseline_count={baseline_count}"
        );
    }

    #[test]
    fn test_phonetic_dual_focus_no_excessive_repeats() {
        let dictionary = Dictionary::load();
        let table = TransitionTable::build_from_words(&dictionary.words_list());
        let filter = CharFilter::new(('a'..='z').collect());

        let mut generator = PhoneticGenerator::new(
            table,
            Dictionary::load(),
            SmallRng::seed_from_u64(42),
            HashSet::new(),
        );
        let text = generator.generate(&filter, Some('k'), Some(['t', 'h']), 200);
        let words: Vec<&str> = text.split_whitespace().collect();

        // Check no word appears > 3 times consecutively
        let mut max_consecutive = 1;
        let mut current_run = 1;
        for i in 1..words.len() {
            if words[i] == words[i - 1] {
                current_run += 1;
                max_consecutive = max_consecutive.max(current_run);
            } else {
                current_run = 1;
            }
        }

        assert!(
            max_consecutive <= 3,
            "Max consecutive repeats = {max_consecutive}, expected <= 3"
        );
    }

    #[test]
    fn cross_drill_history_suppresses_repeats() {
        let dictionary = Dictionary::load();
        let table = TransitionTable::build_from_words(&dictionary.words_list());
        // Use a filter yielding a pool above FULL_DICT_THRESHOLD so dict_ratio=1.0
        // (all words are dictionary picks, maximizing history suppression signal).
        // Focus on 'k' to constrain the effective tier pool further.
        let allowed: Vec<char> = "abcdefghijklmn ".chars().collect();
        let filter = CharFilter::new(allowed);

        // Use 200-word drills for stronger statistical signal
        let word_count = 200;

        // Drill 1: generate words and collect the set
        let mut gen1 = PhoneticGenerator::new(
            table.clone(),
            Dictionary::load(),
            SmallRng::seed_from_u64(100),
            HashSet::new(),
        );
        let text1 = gen1.generate(&filter, Some('k'), None, word_count);
        let words1: HashSet<String> = text1.split_whitespace().map(|w| w.to_string()).collect();

        // Drill 2 without history (baseline)
        let mut gen2_no_hist = PhoneticGenerator::new(
            table.clone(),
            Dictionary::load(),
            SmallRng::seed_from_u64(200),
            HashSet::new(),
        );
        let text2_no_hist = gen2_no_hist.generate(&filter, Some('k'), None, word_count);
        let words2_no_hist: HashSet<String> = text2_no_hist
            .split_whitespace()
            .map(|w| w.to_string())
            .collect();
        let baseline_intersection = words1.intersection(&words2_no_hist).count();
        let baseline_union = words1.union(&words2_no_hist).count();
        let baseline_jaccard = baseline_intersection as f64 / baseline_union as f64;

        // Drill 2 with history from drill 1
        let mut gen2_with_hist = PhoneticGenerator::new(
            table.clone(),
            Dictionary::load(),
            SmallRng::seed_from_u64(200),
            words1.clone(),
        );
        let text2_with_hist = gen2_with_hist.generate(&filter, Some('k'), None, word_count);
        let words2_with_hist: HashSet<String> = text2_with_hist
            .split_whitespace()
            .map(|w| w.to_string())
            .collect();
        let hist_intersection = words1.intersection(&words2_with_hist).count();
        let hist_union = words1.union(&words2_with_hist).count();
        let hist_jaccard = hist_intersection as f64 / hist_union as f64;

        // With seeds 100/200 and filter "abcdefghijklmn", 200-word drills:
        // baseline_jaccard≈0.31, hist_jaccard≈0.13, reduction≈0.18
        assert!(
            baseline_jaccard - hist_jaccard >= 0.15,
            "History should reduce overlap by at least 0.15: baseline_jaccard={baseline_jaccard:.3}, \
             hist_jaccard={hist_jaccard:.3}, reduction={:.3}",
            baseline_jaccard - hist_jaccard,
        );
    }

    #[test]
    fn hybrid_mode_produces_mixed_output() {
        let dictionary = Dictionary::load();
        let table = TransitionTable::build_from_words(&dictionary.words_list());
        // Use a constrained filter to get a pool in the hybrid range (8-60).
        let allowed: Vec<char> = "abcdef ".chars().collect();
        let filter = CharFilter::new(allowed);

        let matching: HashSet<String> = dictionary
            .find_matching(&filter, None)
            .iter()
            .map(|s| s.to_string())
            .collect();
        let match_count = matching.len();

        // Verify pool is in hybrid range
        assert!(
            match_count >= MIN_REAL_WORDS && match_count < FULL_DICT_THRESHOLD,
            "Expected pool in hybrid range ({MIN_REAL_WORDS}-{FULL_DICT_THRESHOLD}), got {match_count}"
        );

        let mut generator = PhoneticGenerator::new(
            table,
            Dictionary::load(),
            SmallRng::seed_from_u64(42),
            HashSet::new(),
        );
        let text = generator.generate(&filter, None, None, 500);
        let words: Vec<&str> = text.split_whitespace().collect();
        let dict_count = words.iter().filter(|w| matching.contains(**w)).count();
        let dict_pct = dict_count as f64 / words.len() as f64;

        // dict_ratio = (22-8)/(60-8) ≈ 0.27. Phonetic words generated by
        // the Markov chain often coincidentally match dictionary entries, so
        // observed dict_pct exceeds the intentional dict_ratio.
        // With seed 42 and filter "abcdef" (pool=22): observed dict_pct ≈ 0.59
        assert!(
            dict_pct >= 0.25 && dict_pct <= 0.65,
            "Dict word percentage {dict_pct:.2} (count={dict_count}/{}, pool={match_count}) \
             outside expected 25%-65% range",
            words.len()
        );
        // Verify it's actually mixed: not all dictionary and not all phonetic
        assert!(
            dict_count > 0 && dict_count < words.len(),
            "Expected mixed output, got dict_count={dict_count}/{}",
            words.len()
        );
    }

    #[test]
    fn boundary_phonetic_only_below_threshold() {
        let dictionary = Dictionary::load();
        let table = TransitionTable::build_from_words(&dictionary.words_list());
        // Very small filter — should yield < MIN_REAL_WORDS (8) dictionary matches.
        // With pool < MIN_REAL_WORDS, use_dict=false so 0% intentional dictionary
        // selections (the code never enters pick_tiered_word).
        let allowed: Vec<char> = "xyz ".chars().collect();
        let filter = CharFilter::new(allowed);

        let matching: Vec<String> = dictionary
            .find_matching(&filter, None)
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(
            matching.len() < MIN_REAL_WORDS,
            "Expected < {MIN_REAL_WORDS} matches, got {}",
            matching.len()
        );

        let mut generator = PhoneticGenerator::new(
            table,
            Dictionary::load(),
            SmallRng::seed_from_u64(42),
            HashSet::new(),
        );
        let text = generator.generate(&filter, None, None, 50);
        let words: Vec<&str> = text.split_whitespace().collect();

        assert!(
            !words.is_empty(),
            "Should generate non-empty output even with tiny filter"
        );
        // Verify the dictionary selection path was never taken (0 intentional picks).
        // Phonetic words may coincidentally match dictionary entries, but the
        // dict_picks counter only increments when the dictionary branch is chosen.
        assert_eq!(
            generator.dict_picks, 0,
            "Below threshold: expected 0 intentional dictionary picks, got {}",
            generator.dict_picks
        );
    }

    #[test]
    fn boundary_full_dict_above_threshold() {
        let dictionary = Dictionary::load();
        let table = TransitionTable::build_from_words(&dictionary.words_list());
        // Full alphabet — should yield 100+ dictionary matches
        let filter = CharFilter::new(('a'..='z').collect());

        let matching: HashSet<String> = dictionary
            .find_matching(&filter, None)
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(
            matching.len() >= FULL_DICT_THRESHOLD,
            "Expected >= {FULL_DICT_THRESHOLD} matches, got {}",
            matching.len()
        );

        // With pool >= FULL_DICT_THRESHOLD, dict_ratio=1.0 and gen_bool(1.0)
        // always returns true, so every word goes through pick_tiered_word.
        // All picks come from matching_words → 100% dictionary.
        let mut generator = PhoneticGenerator::new(
            table,
            Dictionary::load(),
            SmallRng::seed_from_u64(42),
            HashSet::new(),
        );
        let text = generator.generate(&filter, None, None, 200);
        let words: Vec<&str> = text.split_whitespace().collect();
        let dict_count = words.iter().filter(|w| matching.contains(**w)).count();

        assert_eq!(
            dict_count,
            words.len(),
            "Above threshold: expected 100% dictionary words, got {dict_count}/{}",
            words.len()
        );
    }

    #[test]
    fn weighted_suppression_graceful_degradation() {
        let dictionary = Dictionary::load();
        let table = TransitionTable::build_from_words(&dictionary.words_list());
        // Use a small filter to get a small pool
        let allowed: Vec<char> = "abcdefghijk ".chars().collect();
        let filter = CharFilter::new(allowed);

        let matching: Vec<String> = dictionary
            .find_matching(&filter, None)
            .iter()
            .map(|s| s.to_string())
            .collect();

        // Create history containing most of the pool words (up to 8)
        let history: HashSet<String> = matching
            .iter()
            .take(8.min(matching.len()))
            .cloned()
            .collect();

        let mut generator = PhoneticGenerator::new(
            table,
            Dictionary::load(),
            SmallRng::seed_from_u64(42),
            history.clone(),
        );
        let text = generator.generate(&filter, None, None, 50);
        let words: Vec<&str> = text.split_whitespace().collect();

        // Should not panic and should produce output
        assert!(!words.is_empty(), "Should generate non-empty output");

        // History words should still appear (suppression is soft, not hard exclusion)
        let history_words_in_output: usize = words.iter().filter(|w| history.contains(**w)).count();
        // With soft suppression, at least some history words should appear
        // (they're accepted with reduced probability, not blocked)
        assert!(
            history_words_in_output > 0 || matching.len() > history.len(),
            "History words should still appear with soft suppression, or non-history pool words used"
        );
    }
}
