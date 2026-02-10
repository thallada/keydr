use crate::engine::filter::CharFilter;
use crate::generator::TextGenerator;

const PASSAGES: &[&str] = &[
    "the quick brown fox jumps over the lazy dog and then runs across the field while the sun sets behind the distant hills",
    "it was the best of times it was the worst of times it was the age of wisdom it was the age of foolishness",
    "in the beginning there was nothing but darkness and then the light appeared slowly spreading across the vast empty space",
    "she walked along the narrow path through the forest listening to the birds singing in the trees above her head",
    "the old man sat on the bench watching the children play in the park while the autumn leaves fell softly around him",
    "there is nothing either good or bad but thinking makes it so for the mind is its own place and in itself can make a heaven of hell",
    "to be or not to be that is the question whether it is nobler in the mind to suffer the slings and arrows of outrageous fortune",
    "all that glitters is not gold and not all those who wander are lost for the old that is strong does not wither",
    "the river flowed quietly through the green valley and the mountains rose high on either side covered with trees and snow",
    "a long time ago in a land far away there lived a wise king who ruled his people with kindness and justice",
    "the rain fell steadily on the roof making a soft drumming sound that filled the room and made everything feel calm",
    "she opened the door and stepped outside into the cool morning air breathing deeply as the first light of dawn appeared",
    "he picked up the book and began to read turning the pages slowly as the story drew him deeper and deeper into its world",
    "the stars shone brightly in the clear night sky and the moon cast a silver light over the sleeping town below",
    "they gathered around the fire telling stories and laughing while the wind howled outside and the snow piled up against the door",
];

pub struct PassageGenerator {
    current_idx: usize,
}

impl PassageGenerator {
    pub fn new() -> Self {
        Self { current_idx: 0 }
    }
}

impl Default for PassageGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl TextGenerator for PassageGenerator {
    fn generate(
        &mut self,
        _filter: &CharFilter,
        _focused: Option<char>,
        _word_count: usize,
    ) -> String {
        let passage = PASSAGES[self.current_idx % PASSAGES.len()];
        self.current_idx += 1;
        passage.to_string()
    }
}
