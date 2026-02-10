pub struct CharFilter {
    pub allowed: Vec<char>,
}

impl CharFilter {
    pub fn new(allowed: Vec<char>) -> Self {
        Self { allowed }
    }

    pub fn is_allowed(&self, ch: char) -> bool {
        self.allowed.contains(&ch) || ch == ' '
    }

    #[allow(dead_code)]
    pub fn filter_text(&self, text: &str) -> String {
        text.chars()
            .filter(|&ch| self.is_allowed(ch))
            .collect()
    }
}
