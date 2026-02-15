use std::time::Instant;

use crate::session::lesson::LessonState;

#[derive(Clone, Debug)]
pub enum CharStatus {
    Correct,
    Incorrect(char),
}

#[derive(Clone, Debug)]
pub struct KeystrokeEvent {
    pub expected: char,
    #[allow(dead_code)]
    pub actual: char,
    pub timestamp: Instant,
    pub correct: bool,
}

pub fn process_char(lesson: &mut LessonState, ch: char) -> Option<KeystrokeEvent> {
    if lesson.is_complete() {
        return None;
    }

    if lesson.started_at.is_none() {
        lesson.started_at = Some(Instant::now());
    }

    let expected = lesson.target[lesson.cursor];
    let correct = ch == expected;

    let event = KeystrokeEvent {
        expected,
        actual: ch,
        timestamp: Instant::now(),
        correct,
    };

    if correct {
        lesson.input.push(CharStatus::Correct);
    } else {
        lesson.input.push(CharStatus::Incorrect(ch));
        lesson.typo_flags.insert(lesson.cursor);
    }
    lesson.cursor += 1;

    if lesson.is_complete() {
        lesson.finished_at = Some(Instant::now());
    }

    Some(event)
}

pub fn process_backspace(lesson: &mut LessonState) {
    if lesson.cursor > 0 {
        lesson.cursor -= 1;
        lesson.input.pop();
    }
}
