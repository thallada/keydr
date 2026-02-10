use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::session::input::KeystrokeEvent;
use crate::session::lesson::LessonState;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LessonResult {
    pub wpm: f64,
    pub cpm: f64,
    pub accuracy: f64,
    pub correct: usize,
    pub incorrect: usize,
    pub total_chars: usize,
    pub elapsed_secs: f64,
    pub timestamp: DateTime<Utc>,
    pub per_key_times: Vec<KeyTime>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyTime {
    pub key: char,
    pub time_ms: f64,
    pub correct: bool,
}

impl LessonResult {
    pub fn from_lesson(lesson: &LessonState, events: &[KeystrokeEvent]) -> Self {
        let per_key_times: Vec<KeyTime> = events
            .windows(2)
            .map(|pair| {
                let dt = pair[1].timestamp.duration_since(pair[0].timestamp);
                KeyTime {
                    key: pair[1].expected,
                    time_ms: dt.as_secs_f64() * 1000.0,
                    correct: pair[1].correct,
                }
            })
            .collect();

        Self {
            wpm: lesson.wpm(),
            cpm: lesson.cpm(),
            accuracy: lesson.accuracy(),
            correct: lesson.correct_count(),
            incorrect: lesson.incorrect_count(),
            total_chars: lesson.target.len(),
            elapsed_secs: lesson.elapsed_secs(),
            timestamp: Utc::now(),
            per_key_times,
        }
    }
}
