pub mod cache;
pub mod capitalize;
pub mod code_patterns;
pub mod code_syntax;
pub mod dictionary;
pub mod numbers;
pub mod passage;
pub mod phonetic;
pub mod punctuate;
pub mod transition_table;

use crate::engine::filter::CharFilter;

pub trait TextGenerator {
    fn generate(
        &mut self,
        filter: &CharFilter,
        focused_char: Option<char>,
        focused_bigram: Option<[char; 2]>,
        word_count: usize,
    ) -> String;
}
