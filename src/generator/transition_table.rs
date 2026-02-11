use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransitionTable {
    pub order: usize,
    transitions: HashMap<Vec<char>, Vec<(char, f64)>>,
}

impl TransitionTable {
    pub fn new(order: usize) -> Self {
        Self {
            order,
            transitions: HashMap::new(),
        }
    }

    pub fn add(&mut self, prefix: &[char], next: char, weight: f64) {
        self.transitions
            .entry(prefix.to_vec())
            .or_default()
            .push((next, weight));
    }

    pub fn segment(&self, prefix: &[char]) -> Option<&Vec<(char, f64)>> {
        // Try exact prefix match first, then fall back to shorter prefixes
        let key_len = self.order - 1;
        let prefix = if prefix.len() >= key_len {
            &prefix[prefix.len() - key_len..]
        } else {
            prefix
        };

        // Try progressively shorter prefixes for backoff
        for start in 0..prefix.len() {
            let key = prefix[start..].to_vec();
            if let Some(entries) = self.transitions.get(&key) {
                return Some(entries);
            }
        }
        None
    }

    /// Build an order-4 transition table from a word frequency list.
    /// Words earlier in the list are higher frequency and get more weight.
    pub fn build_from_words(words: &[String]) -> Self {
        let mut table = Self::new(4);
        let prefix_len = 3; // order - 1

        for (rank, word) in words.iter().enumerate() {
            if word.len() < 3 {
                continue;
            }
            if !word.chars().all(|c| c.is_ascii_lowercase()) {
                continue;
            }

            // Weight decreases with rank (frequency-based)
            let weight = 1.0 / (1.0 + (rank as f64 / 500.0));

            // Add word start transitions (space prefix -> first chars)
            let chars: Vec<char> = word.chars().collect();

            // Start of word: ' ' prefix
            for i in 0..chars.len() {
                let mut prefix = Vec::new();
                // Build prefix from space + preceding chars
                let start = if i >= prefix_len { i - prefix_len } else { 0 };
                if i < prefix_len {
                    // Pad with spaces
                    for _ in 0..(prefix_len - i) {
                        prefix.push(' ');
                    }
                }
                for j in start..i {
                    prefix.push(chars[j]);
                }

                let next = chars[i];
                table.add(&prefix, next, weight);
            }

            // End of word: last chars -> space
            let end_start = if chars.len() >= prefix_len {
                chars.len() - prefix_len
            } else {
                0
            };
            let mut end_prefix: Vec<char> = Vec::new();
            if chars.len() < prefix_len {
                for _ in 0..(prefix_len - chars.len()) {
                    end_prefix.push(' ');
                }
            }
            for j in end_start..chars.len() {
                end_prefix.push(chars[j]);
            }
            table.add(&end_prefix, ' ', weight);
        }

        table
    }

    /// Legacy order-2 table for fallback
    #[allow(dead_code)]
    pub fn build_english() -> Self {
        let mut table = Self::new(4);

        let common_patterns: &[(&str, f64)] = &[
            ("the", 10.0), ("and", 8.0), ("ing", 7.0), ("tion", 6.0), ("ent", 5.0),
            ("ion", 5.0), ("her", 4.0), ("for", 4.0), ("are", 4.0), ("his", 4.0),
            ("hat", 3.0), ("tha", 3.0), ("ere", 3.0), ("ate", 3.0), ("ith", 3.0),
            ("ver", 3.0), ("all", 3.0), ("not", 3.0), ("ess", 3.0), ("est", 3.0),
            ("rea", 3.0), ("sta", 3.0), ("ted", 3.0), ("com", 3.0), ("con", 3.0),
            ("oun", 2.5), ("pro", 2.5), ("oth", 2.5), ("igh", 2.5), ("ore", 2.5),
            ("our", 2.5), ("ine", 2.5), ("ove", 2.5), ("ome", 2.5), ("use", 2.5),
            ("ble", 2.0), ("ful", 2.0), ("ous", 2.0), ("str", 2.0), ("tri", 2.0),
            ("ght", 2.0), ("whi", 2.0), ("who", 2.0), ("hen", 2.0), ("ter", 2.0),
            ("man", 2.0), ("men", 2.0), ("ner", 2.0), ("per", 2.0), ("pre", 2.0),
            ("ran", 2.0), ("lin", 2.0), ("kin", 2.0), ("din", 2.0), ("sin", 2.0),
            ("out", 2.0), ("ind", 2.0), ("ber", 2.0), ("der", 2.0),
            ("end", 2.0), ("hin", 2.0), ("old", 2.0), ("ear", 2.0), ("ain", 2.0),
            ("ant", 2.0), ("urn", 2.0), ("ell", 2.0), ("ill", 2.0), ("ade", 2.0),
            ("ong", 2.0), ("ung", 2.0), ("ast", 2.0), ("ist", 2.0),
            ("ust", 2.0), ("ost", 2.0), ("ard", 2.0), ("ord", 2.0), ("art", 2.0),
            ("ort", 2.0), ("ect", 2.0), ("act", 2.0), ("ack", 2.0), ("ick", 2.0),
            ("ock", 2.0), ("uck", 2.0), ("ash", 2.0), ("ish", 2.0), ("ush", 2.0),
        ];

        for &(pattern, weight) in common_patterns {
            let chars: Vec<char> = pattern.chars().collect();
            for window in chars.windows(3) {
                let prefix = vec![window[0], window[1]];
                table.add(&prefix, window[2], weight);
            }
            // Also add shorter prefix entries for the start of patterns
            if chars.len() >= 2 {
                table.add(&[' ', chars[0]], chars[1], weight * 0.5);
            }
        }

        let vowels = ['a', 'e', 'i', 'o', 'u'];
        let consonants = [
            'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'm', 'n', 'p', 'r', 's', 't', 'v',
            'w', 'x', 'y', 'z',
        ];

        for &c in &consonants {
            for &v in &vowels {
                table.add(&[' ', c], v, 1.0);
                table.add(&[v, c], 'e', 0.5);
            }
        }

        for &v in &vowels {
            for &c in &consonants {
                table.add(&[' ', v], c, 0.5);
            }
        }

        table
    }
}

impl Default for TransitionTable {
    fn default() -> Self {
        Self::new(4)
    }
}
