use rand::rngs::SmallRng;
use rand::Rng;

use crate::engine::filter::CharFilter;
use crate::generator::dictionary::Dictionary;
use crate::generator::transition_table::TransitionTable;
use crate::generator::TextGenerator;

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

    fn generate_phonetic_word(&mut self, filter: &CharFilter, focused: Option<char>) -> String {
        for _attempt in 0..5 {
            let word = self.try_generate_word(filter, focused);
            if word.len() >= MIN_WORD_LEN {
                return word;
            }
        }
        // Fallback
        "the".to_string()
    }

    fn try_generate_word(&mut self, filter: &CharFilter, focused: Option<char>) -> String {
        let mut word = Vec::new();

        // Start with space prefix
        let start_char = if let Some(focus) = focused {
            if self.rng.gen_bool(0.4) && filter.is_allowed(focus) {
                word.push(focus);
                // Get next char from transition table
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
                        let space_prob = (space_weight * boost) / (total + space_weight * (boost - 1.0));
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
                let non_space: Vec<(char, f64)> = probs
                    .iter()
                    .filter(|(ch, _)| *ch != ' ')
                    .copied()
                    .collect();
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
}

impl TextGenerator for PhoneticGenerator {
    fn generate(
        &mut self,
        filter: &CharFilter,
        focused: Option<char>,
        word_count: usize,
    ) -> String {
        // keybr's approach: prefer real words when enough match the filter
        // Collect matching words into owned Vec to avoid borrow conflict
        let matching_words: Vec<String> = self
            .dictionary
            .find_matching(filter, focused)
            .iter()
            .map(|s| s.to_string())
            .collect();
        let use_real_words = matching_words.len() >= MIN_REAL_WORDS;

        let mut words: Vec<String> = Vec::new();
        let mut last_word = String::new();

        for _ in 0..word_count {
            if use_real_words {
                // Pick a real word (avoid consecutive duplicates)
                let mut picked = None;
                for _ in 0..3 {
                    let idx = self.rng.gen_range(0..matching_words.len());
                    let word = matching_words[idx].clone();
                    if word != last_word {
                        picked = Some(word);
                        break;
                    }
                }
                let word = match picked {
                    Some(w) => w,
                    None => self.generate_phonetic_word(filter, focused),
                };
                last_word.clone_from(&word);
                words.push(word);
            } else {
                // Fall back to phonetic pseudo-words
                let word = self.generate_phonetic_word(filter, focused);
                words.push(word);
            }
        }

        words.join(" ")
    }
}
