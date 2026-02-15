use std::time::Instant;

use crate::session::drill::DrillState;

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

pub fn process_char(drill: &mut DrillState, ch: char) -> Option<KeystrokeEvent> {
    if drill.is_complete() {
        return None;
    }

    if drill.started_at.is_none() {
        drill.started_at = Some(Instant::now());
    }

    let expected = drill.target[drill.cursor];
    let correct = ch == expected;

    let event = KeystrokeEvent {
        expected,
        actual: ch,
        timestamp: Instant::now(),
        correct,
    };

    if correct {
        drill.input.push(CharStatus::Correct);
    } else {
        drill.input.push(CharStatus::Incorrect(ch));
        drill.typo_flags.insert(drill.cursor);
    }
    drill.cursor += 1;

    if drill.is_complete() {
        drill.finished_at = Some(Instant::now());
    }

    Some(event)
}

pub fn process_backspace(drill: &mut DrillState) {
    if drill.cursor > 0 {
        drill.cursor -= 1;
        drill.input.pop();
    }
}
