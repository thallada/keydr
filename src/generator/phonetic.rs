use rand::rngs::SmallRng;
use rand::Rng;

use crate::engine::filter::CharFilter;
use crate::generator::transition_table::TransitionTable;
use crate::generator::TextGenerator;

pub struct PhoneticGenerator {
    table: TransitionTable,
    rng: SmallRng,
}

impl PhoneticGenerator {
    pub fn new(table: TransitionTable, rng: SmallRng) -> Self {
        Self { table, rng }
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

    fn generate_word(&mut self, filter: &CharFilter, focused: Option<char>) -> String {
        let min_len = 3;
        let max_len = 10;
        let mut word = String::new();

        let start_char = if let Some(focus) = focused {
            if self.rng.gen_bool(0.4) {
                let probs = self.table.get_next_probs(' ', focus).cloned();
                if let Some(probs) = probs {
                    let filtered: Vec<(char, f64)> = probs
                        .iter()
                        .filter(|(ch, _)| filter.is_allowed(*ch))
                        .copied()
                        .collect();
                    if !filtered.is_empty() {
                        word.push(focus);
                        Self::pick_weighted_from(&mut self.rng, &filtered, filter)
                    } else {
                        None
                    }
                } else {
                    Some(focus)
                }
            } else {
                None
            }
        } else {
            None
        };

        if word.is_empty() {
            let starters: Vec<(char, f64)> = filter
                .allowed
                .iter()
                .map(|&ch| {
                    (
                        ch,
                        if ch == 'e' || ch == 't' || ch == 'a' {
                            3.0
                        } else {
                            1.0
                        },
                    )
                })
                .collect();

            if let Some(ch) = Self::pick_weighted_from(&mut self.rng, &starters, filter) {
                word.push(ch);
            } else {
                return "the".to_string();
            }
        }

        if let Some(ch) = start_char {
            word.push(ch);
        }

        while word.len() < max_len {
            let chars: Vec<char> = word.chars().collect();
            let len = chars.len();

            let (prev, curr) = if len >= 2 {
                (chars[len - 2], chars[len - 1])
            } else {
                (' ', chars[len - 1])
            };

            let space_prob = 1.3f64.powi(word.len() as i32 - min_len as i32);
            if word.len() >= min_len
                && self
                    .rng
                    .gen_bool((space_prob / (space_prob + 5.0)).min(0.8))
            {
                break;
            }

            let probs = self.table.get_next_probs(prev, curr).cloned();
            if let Some(probs) = probs {
                if let Some(next) = Self::pick_weighted_from(&mut self.rng, &probs, filter) {
                    word.push(next);
                } else {
                    break;
                }
            } else {
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

        if word.is_empty() {
            "the".to_string()
        } else {
            word
        }
    }
}

impl TextGenerator for PhoneticGenerator {
    fn generate(
        &mut self,
        filter: &CharFilter,
        focused: Option<char>,
        word_count: usize,
    ) -> String {
        let mut words: Vec<String> = Vec::new();

        for _ in 0..word_count {
            words.push(self.generate_word(filter, focused));
        }

        words.join(" ")
    }
}
