use std::collections::HashSet;
use std::time::Instant;

use crate::session::input::CharStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyntheticSpan {
    pub start: usize,
    pub end: usize,
}

pub struct DrillState {
    pub target: Vec<char>,
    pub input: Vec<CharStatus>,
    pub cursor: usize,
    pub started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
    pub typo_flags: HashSet<usize>,
    pub synthetic_spans: Vec<SyntheticSpan>,
}

impl DrillState {
    pub fn new(text: &str) -> Self {
        Self {
            target: text.chars().collect(),
            input: Vec::new(),
            cursor: 0,
            started_at: None,
            finished_at: None,
            typo_flags: HashSet::new(),
            synthetic_spans: Vec::new(),
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
    fn test_new_drill() {
        let drill = DrillState::new("hello");
        assert_eq!(drill.target.len(), 5);
        assert_eq!(drill.cursor, 0);
        assert!(!drill.is_complete());
        assert_eq!(drill.progress(), 0.0);
    }

    #[test]
    fn test_accuracy_starts_at_100() {
        let drill = DrillState::new("test");
        assert_eq!(drill.accuracy(), 100.0);
    }

    #[test]
    fn test_empty_drill_progress() {
        let drill = DrillState::new("");
        assert!(drill.is_complete());
        assert_eq!(drill.progress(), 0.0);
    }

    #[test]
    fn test_correct_typing_no_typos() {
        let mut drill = DrillState::new("abc");
        input::process_char(&mut drill, 'a');
        input::process_char(&mut drill, 'b');
        input::process_char(&mut drill, 'c');
        assert!(drill.typo_flags.is_empty());
        assert_eq!(drill.accuracy(), 100.0);
    }

    #[test]
    fn test_wrong_then_backspace_then_correct_counts_as_error() {
        let mut drill = DrillState::new("abc");
        // Type wrong at pos 0
        input::process_char(&mut drill, 'x');
        assert!(drill.typo_flags.contains(&0));
        // Backspace
        input::process_backspace(&mut drill);
        // Typo flag persists
        assert!(drill.typo_flags.contains(&0));
        // Type correct
        input::process_char(&mut drill, 'a');
        assert!(drill.typo_flags.contains(&0));
        assert_eq!(drill.typo_count(), 1);
        assert!(drill.accuracy() < 100.0);
    }

    #[test]
    fn test_multiple_errors_same_position_counts_as_one() {
        let mut drill = DrillState::new("abc");
        // Wrong, backspace, wrong again, backspace, correct
        input::process_char(&mut drill, 'x');
        input::process_backspace(&mut drill);
        input::process_char(&mut drill, 'y');
        input::process_backspace(&mut drill);
        input::process_char(&mut drill, 'a');
        assert_eq!(drill.typo_count(), 1);
    }

    #[test]
    fn test_wrong_char_without_backspace() {
        let mut drill = DrillState::new("abc");
        input::process_char(&mut drill, 'x'); // wrong at pos 0
        input::process_char(&mut drill, 'b'); // correct at pos 1
        assert_eq!(drill.typo_count(), 1);
        assert!(drill.typo_flags.contains(&0));
    }

    #[test]
    fn test_wrong_enter_skips_line_and_backspace_collapses() {
        let mut drill = DrillState::new("abcd\nef");
        input::process_char(&mut drill, 'a');
        assert_eq!(drill.cursor, 1);

        // Wrong newline while expecting 'b' should skip to next line start.
        input::process_char(&mut drill, '\n');
        assert_eq!(drill.cursor, 5); // index after '\n'
        assert!(drill.typo_count() >= 4);
        for pos in 1..5 {
            assert!(drill.typo_flags.contains(&pos));
        }

        // Backspacing at jump boundary collapses span to a single typo.
        input::process_backspace(&mut drill);
        assert_eq!(drill.cursor, 1);
        assert_eq!(drill.typo_count(), 1);
        assert!(drill.typo_flags.contains(&1));
    }

    #[test]
    fn test_wrong_tab_skips_tab_width_and_backspace_collapses() {
        let mut drill = DrillState::new("abcdef");
        input::process_char(&mut drill, 'a');
        assert_eq!(drill.cursor, 1);

        // Tab jumps 4 chars (or to end of line).
        input::process_char(&mut drill, '\t');
        assert_eq!(drill.cursor, 5);
        for pos in 1..5 {
            assert!(drill.typo_flags.contains(&pos));
        }

        input::process_backspace(&mut drill);
        assert_eq!(drill.cursor, 1);
        assert_eq!(drill.typo_count(), 1);
        assert!(drill.typo_flags.contains(&1));
    }

    #[test]
    fn test_wrong_tab_near_line_end_clamps_to_end_of_line() {
        let mut drill = DrillState::new("ab\ncd");
        input::process_char(&mut drill, 'a');
        input::process_char(&mut drill, 'b');
        // At newline position, a wrong tab should consume just this line remainder.
        input::process_char(&mut drill, '\t');
        assert_eq!(drill.cursor, 3);
        assert_eq!(drill.typo_count(), 1);
    }

    #[test]
    fn test_tab_counts_as_four_spaces() {
        let mut drill = DrillState::new("    pass");
        let start = drill.cursor;
        input::process_char(&mut drill, '\t');
        assert_eq!(drill.cursor, start + 4);
        assert_eq!(drill.typo_count(), 0);
    }

    #[test]
    fn test_tab_counts_as_two_spaces() {
        let mut drill = DrillState::new("  echo");
        let start = drill.cursor;
        input::process_char(&mut drill, '\t');
        assert_eq!(drill.cursor, start + 2);
        assert_eq!(drill.typo_count(), 0);
    }

    #[test]
    fn test_tab_not_accepted_for_non_four_space_prefix() {
        let mut drill = DrillState::new("abc   def");
        for ch in "abc".chars() {
            input::process_char(&mut drill, ch);
        }
        let start = drill.cursor;
        input::process_char(&mut drill, '\t');
        // Falls back to synthetic incorrect span behavior.
        assert!(drill.cursor > start);
        assert!(drill.typo_count() >= 1);
    }

    #[test]
    fn test_correct_enter_auto_indents_next_line() {
        let mut drill = DrillState::new("if x:\n    pass");
        for ch in "if x:".chars() {
            input::process_char(&mut drill, ch);
        }
        // Correct newline should also consume the 4-space indent.
        input::process_char(&mut drill, '\n');
        let expected_cursor = "if x:\n    ".chars().count();
        assert_eq!(drill.cursor, expected_cursor);
        assert_eq!(drill.typo_count(), 0);
        assert_eq!(drill.accuracy(), 100.0);
    }

    #[test]
    fn test_nested_synthetic_spans_collapse_to_single_error() {
        let mut drill = DrillState::new("abcd\nefgh");
        input::process_char(&mut drill, 'a');
        input::process_char(&mut drill, '\n');
        let after_newline = drill.cursor;
        input::process_char(&mut drill, '\t');
        assert!(drill.typo_count() > 1);

        input::process_backspace(&mut drill); // collapse tab span
        assert_eq!(drill.cursor, after_newline);
        input::process_backspace(&mut drill); // collapse newline span
        assert_eq!(drill.cursor, 1);
        assert_eq!(drill.typo_count(), 1);
        assert!(drill.typo_flags.contains(&1));
    }
}
