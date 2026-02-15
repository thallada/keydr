use std::collections::HashSet;
use std::time::Instant;

use crate::session::input::CharStatus;

pub struct LessonState {
    pub target: Vec<char>,
    pub input: Vec<CharStatus>,
    pub cursor: usize,
    pub started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
    pub typo_flags: HashSet<usize>,
}

impl LessonState {
    pub fn new(text: &str) -> Self {
        Self {
            target: text.chars().collect(),
            input: Vec::new(),
            cursor: 0,
            started_at: None,
            finished_at: None,
            typo_flags: HashSet::new(),
        }
    }

    pub fn is_complete(&self) -> bool {
        self.cursor >= self.target.len()
    }

    pub fn elapsed_secs(&self) -> f64 {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => end.duration_since(start).as_secs_f64(),
            (Some(start), None) => start.elapsed().as_secs_f64(),
            _ => 0.0,
        }
    }

    pub fn correct_count(&self) -> usize {
        self.input
            .iter()
            .filter(|s| matches!(s, CharStatus::Correct))
            .count()
    }

    pub fn wpm(&self) -> f64 {
        let elapsed = self.elapsed_secs();
        if elapsed < 0.1 {
            return 0.0;
        }
        let chars = self.correct_count() as f64;
        (chars / 5.0) / (elapsed / 60.0)
    }

    pub fn typo_count(&self) -> usize {
        self.typo_flags.len()
    }

    pub fn accuracy(&self) -> f64 {
        if self.cursor == 0 {
            return 100.0;
        }
        let typos_before_cursor = self
            .typo_flags
            .iter()
            .filter(|&&pos| pos < self.cursor)
            .count();
        ((self.cursor - typos_before_cursor) as f64 / self.cursor as f64 * 100.0).clamp(0.0, 100.0)
    }

    pub fn cpm(&self) -> f64 {
        let elapsed = self.elapsed_secs();
        if elapsed < 0.1 {
            return 0.0;
        }
        self.correct_count() as f64 / (elapsed / 60.0)
    }

    pub fn progress(&self) -> f64 {
        if self.target.is_empty() {
            return 0.0;
        }
        self.cursor as f64 / self.target.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::input;

    #[test]
    fn test_new_lesson() {
        let lesson = LessonState::new("hello");
        assert_eq!(lesson.target.len(), 5);
        assert_eq!(lesson.cursor, 0);
        assert!(!lesson.is_complete());
        assert_eq!(lesson.progress(), 0.0);
    }

    #[test]
    fn test_accuracy_starts_at_100() {
        let lesson = LessonState::new("test");
        assert_eq!(lesson.accuracy(), 100.0);
    }

    #[test]
    fn test_empty_lesson_progress() {
        let lesson = LessonState::new("");
        assert!(lesson.is_complete());
        assert_eq!(lesson.progress(), 0.0);
    }

    #[test]
    fn test_correct_typing_no_typos() {
        let mut lesson = LessonState::new("abc");
        input::process_char(&mut lesson, 'a');
        input::process_char(&mut lesson, 'b');
        input::process_char(&mut lesson, 'c');
        assert!(lesson.typo_flags.is_empty());
        assert_eq!(lesson.accuracy(), 100.0);
    }

    #[test]
    fn test_wrong_then_backspace_then_correct_counts_as_error() {
        let mut lesson = LessonState::new("abc");
        // Type wrong at pos 0
        input::process_char(&mut lesson, 'x');
        assert!(lesson.typo_flags.contains(&0));
        // Backspace
        input::process_backspace(&mut lesson);
        // Typo flag persists
        assert!(lesson.typo_flags.contains(&0));
        // Type correct
        input::process_char(&mut lesson, 'a');
        assert!(lesson.typo_flags.contains(&0));
        assert_eq!(lesson.typo_count(), 1);
        assert!(lesson.accuracy() < 100.0);
    }

    #[test]
    fn test_multiple_errors_same_position_counts_as_one() {
        let mut lesson = LessonState::new("abc");
        // Wrong, backspace, wrong again, backspace, correct
        input::process_char(&mut lesson, 'x');
        input::process_backspace(&mut lesson);
        input::process_char(&mut lesson, 'y');
        input::process_backspace(&mut lesson);
        input::process_char(&mut lesson, 'a');
        assert_eq!(lesson.typo_count(), 1);
    }

    #[test]
    fn test_wrong_char_without_backspace() {
        let mut lesson = LessonState::new("abc");
        input::process_char(&mut lesson, 'x'); // wrong at pos 0
        input::process_char(&mut lesson, 'b'); // correct at pos 1
        assert_eq!(lesson.typo_count(), 1);
        assert!(lesson.typo_flags.contains(&0));
    }
}
