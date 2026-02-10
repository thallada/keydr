use crate::engine::filter::CharFilter;
use crate::generator::TextGenerator;

#[allow(dead_code)]
pub struct GitHubCodeGenerator {
    cached_snippets: Vec<String>,
    current_idx: usize,
}

impl GitHubCodeGenerator {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            cached_snippets: Vec::new(),
            current_idx: 0,
        }
    }
}

impl Default for GitHubCodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TextGenerator for GitHubCodeGenerator {
    fn generate(
        &mut self,
        _filter: &CharFilter,
        _focused: Option<char>,
        _word_count: usize,
    ) -> String {
        if self.cached_snippets.is_empty() {
            return "// GitHub code fetching not yet configured. Use settings to add a repository."
                .to_string();
        }
        let snippet = self.cached_snippets[self.current_idx % self.cached_snippets.len()].clone();
        self.current_idx += 1;
        snippet
    }
}
