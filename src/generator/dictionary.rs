use crate::engine::filter::CharFilter;

const WORDS_EN: &str = include_str!("../../assets/words-en.json");

pub struct Dictionary {
    words: Vec<String>,
}

impl Dictionary {
    pub fn load() -> Self {
        let words: Vec<String> = serde_json::from_str(WORDS_EN).unwrap_or_default();

        // Filter to words of length >= 3 (matching keybr)
        let words = words
            .into_iter()
            .filter(|w| w.len() >= 3 && w.chars().all(|c| c.is_ascii_lowercase()))
            .collect();

        Self { words }
    }

    pub fn words_list(&self) -> Vec<String> {
        self.words.clone()
    }

    pub fn find_matching(&self, filter: &CharFilter, focused: Option<char>) -> Vec<&str> {
        let mut matching: Vec<&str> = self
            .words
            .iter()
            .filter(|w| w.chars().all(|c| filter.is_allowed(c)))
            .map(|s| s.as_str())
            .collect();

        // If there's a focused letter, prioritize words containing it
        if let Some(focus) = focused {
            matching.sort_by_key(|w| if w.contains(focus) { 0 } else { 1 });
        }

        matching
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_matching_focused_is_sort_only() {
        let dictionary = Dictionary::load();
        let filter = CharFilter::new(('a'..='z').collect());

        let without_focus = dictionary.find_matching(&filter, None);
        let with_focus = dictionary.find_matching(&filter, Some('k'));

        // Same membership — focused param only reorders, never filters
        let mut sorted_without: Vec<&str> = without_focus.clone();
        let mut sorted_with: Vec<&str> = with_focus.clone();
        sorted_without.sort();
        sorted_with.sort();

        assert_eq!(sorted_without, sorted_with);
        assert_eq!(without_focus.len(), with_focus.len());
    }
}
