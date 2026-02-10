use crate::engine::key_stats::KeyStatsStore;

pub const FREQUENCY_ORDER: &[char] = &[
    'e', 't', 'a', 'o', 'i', 'n', 's', 'h', 'r', 'd', 'l', 'c', 'u', 'm', 'w', 'f', 'g', 'y',
    'p', 'b', 'v', 'k', 'j', 'x', 'q', 'z',
];

const MIN_LETTERS: usize = 6;

#[derive(Clone, Debug)]
pub struct LetterUnlock {
    pub included: Vec<char>,
    pub focused: Option<char>,
}

impl LetterUnlock {
    pub fn new() -> Self {
        let included = FREQUENCY_ORDER[..MIN_LETTERS].to_vec();
        Self {
            included,
            focused: None,
        }
    }

    pub fn from_included(included: Vec<char>) -> Self {
        let mut lu = Self {
            included,
            focused: None,
        };
        lu.focused = None;
        lu
    }

    pub fn update(&mut self, stats: &KeyStatsStore) {
        let all_confident = self
            .included
            .iter()
            .all(|&ch| stats.get_confidence(ch) >= 1.0);

        if all_confident {
            for &letter in FREQUENCY_ORDER {
                if !self.included.contains(&letter) {
                    self.included.push(letter);
                    break;
                }
            }
        }

        while self.included.len() < MIN_LETTERS {
            for &letter in FREQUENCY_ORDER {
                if !self.included.contains(&letter) {
                    self.included.push(letter);
                    break;
                }
            }
        }

        self.focused = self
            .included
            .iter()
            .filter(|&&ch| stats.get_confidence(ch) < 1.0)
            .min_by(|&&a, &&b| {
                stats
                    .get_confidence(a)
                    .partial_cmp(&stats.get_confidence(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied();
    }

    #[allow(dead_code)]
    pub fn is_unlocked(&self, ch: char) -> bool {
        self.included.contains(&ch)
    }

    pub fn unlocked_count(&self) -> usize {
        self.included.len()
    }

    pub fn total_letters(&self) -> usize {
        FREQUENCY_ORDER.len()
    }

    pub fn progress(&self) -> f64 {
        self.unlocked_count() as f64 / self.total_letters() as f64
    }
}

impl Default for LetterUnlock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::key_stats::KeyStatsStore;

    #[test]
    fn test_initial_unlock_has_min_letters() {
        let lu = LetterUnlock::new();
        assert_eq!(lu.unlocked_count(), 6);
        assert_eq!(&lu.included, &['e', 't', 'a', 'o', 'i', 'n']);
    }

    #[test]
    fn test_no_unlock_without_confidence() {
        let mut lu = LetterUnlock::new();
        let stats = KeyStatsStore::default();
        lu.update(&stats);
        assert_eq!(lu.unlocked_count(), 6);
    }

    #[test]
    fn test_unlock_when_all_confident() {
        let mut lu = LetterUnlock::new();
        let mut stats = KeyStatsStore::default();
        // Make all included keys confident by typing fast
        for &ch in &['e', 't', 'a', 'o', 'i', 'n'] {
            for _ in 0..50 {
                stats.update_key(ch, 200.0);
            }
        }
        lu.update(&stats);
        assert_eq!(lu.unlocked_count(), 7);
        assert!(lu.included.contains(&'s'));
    }

    #[test]
    fn test_focused_key_is_weakest() {
        let mut lu = LetterUnlock::new();
        let mut stats = KeyStatsStore::default();
        // Make most keys confident except 'o'
        for &ch in &['e', 't', 'a', 'i', 'n'] {
            for _ in 0..50 {
                stats.update_key(ch, 200.0);
            }
        }
        stats.update_key('o', 1000.0); // slow on 'o'
        lu.update(&stats);
        assert_eq!(lu.focused, Some('o'));
    }

    #[test]
    fn test_progress_ratio() {
        let lu = LetterUnlock::new();
        let expected = 6.0 / 26.0;
        assert!((lu.progress() - expected).abs() < 0.001);
    }
}
