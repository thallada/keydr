use rand::Rng;
use rand::rngs::SmallRng;

use crate::engine::filter::CharFilter;
use crate::generator::TextGenerator;
use crate::generator::dictionary::Dictionary;
use crate::generator::transition_table::TransitionTable;

const MIN_WORD_LEN: usize = 3;
const MAX_WORD_LEN: usize = 10;
const MIN_REAL_WORDS: usize = 15;

pub struct PhoneticGenerator {
    table: TransitionTable,
    dictionary: Dictionary,
    rng: SmallRng,
}

impl PhoneticGenerator {
    pub fn new(table: TransitionTable, dictionary: Dictionary, rng: SmallRng) -> Self {
        Self {
            table,
            dictionary,
            rng,
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
    ) -> String {
        for _ in 0..6 {
            let tier = self.select_tier(bigram_indices, char_indices, other_indices);
            let idx = tier[self.rng.gen_range(0..tier.len())];
            let word = &all_words[idx];
            if !recent.contains(word) {
                return word.clone();
            }
        }
        // Fallback: accept any word from full pool
        let idx = self.rng.gen_range(0..all_words.len());
        all_words[idx].clone()
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
        let mut recent: Vec<String> = Vec::new();

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
                if recent.len() > 4 {
                    recent.remove(0);
                }
                words.push(word);
            } else {
                let word = self.generate_phonetic_word(filter, focused_char, focused_bigram);
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
        );
        let focused_text = focused_gen.generate(&filter, Some('k'), None, 1200);
        let focused_count = focused_text
            .split_whitespace()
            .filter(|w| w.contains('k'))
            .count();

        let mut baseline_gen =
            PhoneticGenerator::new(table, Dictionary::load(), SmallRng::seed_from_u64(42));
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
        );
        let bigram_text = bigram_gen.generate(&filter, None, Some(['t', 'h']), 1200);
        let bigram_count = bigram_text
            .split_whitespace()
            .filter(|w| w.contains("th"))
            .count();

        let mut baseline_gen =
            PhoneticGenerator::new(table, Dictionary::load(), SmallRng::seed_from_u64(42));
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

        let mut generator =
            PhoneticGenerator::new(table, Dictionary::load(), SmallRng::seed_from_u64(42));
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
}
