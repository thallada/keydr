use std::time::Instant;

use crate::session::input::CharStatus;

pub struct LessonState {
    pub target: Vec<char>,
    pub input: Vec<CharStatus>,
    pub cursor: usize,
    pub started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
}

impl LessonState {
    pub fn new(text: &str) -> Self {
        Self {
            target: text.chars().collect(),
            input: Vec::new(),
            cursor: 0,
            started_at: None,
            finished_at: None,
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

    pub fn incorrect_count(&self) -> usize {
        self.input
            .iter()
            .filter(|s| matches!(s, CharStatus::Incorrect(_)))
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

    pub fn accuracy(&self) -> f64 {
        let total = self.input.len();
        if total == 0 {
            return 100.0;
        }
        (self.correct_count() as f64 / total as f64) * 100.0
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
}
