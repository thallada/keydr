use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransitionTable {
    pub transitions: HashMap<(char, char), Vec<(char, f64)>>,
}

impl TransitionTable {
    pub fn new() -> Self {
        Self {
            transitions: HashMap::new(),
        }
    }

    pub fn add(&mut self, prev: char, curr: char, next: char, weight: f64) {
        self.transitions
            .entry((prev, curr))
            .or_default()
            .push((next, weight));
    }

    pub fn get_next_probs(&self, prev: char, curr: char) -> Option<&Vec<(char, f64)>> {
        self.transitions.get(&(prev, curr))
    }

    pub fn build_english() -> Self {
        let mut table = Self::new();

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
            ("out", 2.0), ("ind", 2.0), ("ith", 2.0), ("ber", 2.0), ("der", 2.0),
            ("end", 2.0), ("hin", 2.0), ("old", 2.0), ("ear", 2.0), ("ain", 2.0),
            ("ant", 2.0), ("urn", 2.0), ("ell", 2.0), ("ill", 2.0), ("ade", 2.0),
            ("igh", 2.0), ("ong", 2.0), ("ung", 2.0), ("ast", 2.0), ("ist", 2.0),
            ("ust", 2.0), ("ost", 2.0), ("ard", 2.0), ("ord", 2.0), ("art", 2.0),
            ("ort", 2.0), ("ect", 2.0), ("act", 2.0), ("ack", 2.0), ("ick", 2.0),
            ("ock", 2.0), ("uck", 2.0), ("ash", 2.0), ("ish", 2.0), ("ush", 2.0),
            ("anc", 1.5), ("enc", 1.5), ("inc", 1.5), ("onc", 1.5), ("unc", 1.5),
            ("unt", 1.5), ("int", 1.5), ("ont", 1.5), ("ent", 1.5), ("ment", 1.5),
            ("ness", 1.5), ("less", 1.5), ("able", 1.5), ("ible", 1.5), ("ting", 1.5),
            ("ring", 1.5), ("sing", 1.5), ("king", 1.5), ("ning", 1.5), ("ling", 1.5),
            ("wing", 1.5), ("ding", 1.5), ("ping", 1.5), ("ging", 1.5), ("ving", 1.5),
            ("bing", 1.5), ("ming", 1.5), ("fing", 1.0), ("hing", 1.0), ("cing", 1.0),
        ];

        for &(pattern, weight) in common_patterns {
            let chars: Vec<char> = pattern.chars().collect();
            for window in chars.windows(3) {
                table.add(window[0], window[1], window[2], weight);
            }
        }

        let vowels = ['a', 'e', 'i', 'o', 'u'];
        let consonants = [
            'b', 'c', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'm', 'n', 'p', 'r', 's', 't', 'v',
            'w', 'x', 'y', 'z',
        ];

        for &c in &consonants {
            for &v in &vowels {
                table.add(' ', c, v, 1.0);
                table.add(v, c, 'e', 0.5);
                for &v2 in &vowels {
                    table.add(c, v, v2.to_ascii_lowercase(), 0.3);
                }
                for &c2 in &consonants {
                    table.add(v, c, c2, 0.2);
                }
            }
        }

        for &v in &vowels {
            for &c in &consonants {
                table.add(' ', v, c, 0.5);
            }
        }

        table
    }
}

impl Default for TransitionTable {
    fn default() -> Self {
        Self::new()
    }
}
