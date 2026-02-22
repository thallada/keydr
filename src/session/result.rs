use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::keyboard::display::BACKSPACE;
use crate::session::drill::DrillState;
use crate::session::input::KeystrokeEvent;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrillResult {
    pub wpm: f64,
    pub cpm: f64,
    pub accuracy: f64,
    pub correct: usize,
    pub incorrect: usize,
    pub total_chars: usize,
    pub elapsed_secs: f64,
    pub timestamp: DateTime<Utc>,
    pub per_key_times: Vec<KeyTime>,
    #[serde(default = "default_drill_mode", alias = "lesson_mode")]
    pub drill_mode: String,
    #[serde(default = "default_true")]
    pub ranked: bool,
    #[serde(default)]
    pub partial: bool,
    #[serde(default = "default_completion_percent")]
    pub completion_percent: f64,
}

fn default_drill_mode() -> String {
    "adaptive".to_string()
}

fn default_true() -> bool {
    true
}

fn default_completion_percent() -> f64 {
    100.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyTime {
    pub key: char,
    pub time_ms: f64,
    pub correct: bool,
}

impl DrillResult {
    pub fn from_drill(
        drill: &DrillState,
        events: &[KeystrokeEvent],
        drill_mode: &str,
        ranked: bool,
        partial: bool,
    ) -> Self {
        let mut per_key_times: Vec<KeyTime> = Vec::new();
        let mut pending_backspace = false;
        for pair in events.windows(2) {
            let prev = &pair[0];
            let curr = &pair[1];
            let dt = curr.timestamp.duration_since(prev.timestamp).as_secs_f64() * 1000.0;

            // Track per-key expected-char timing/accuracy for normal typing keys.
            // Backspace attempts are tracked separately below.
            if curr.actual != BACKSPACE {
                per_key_times.push(KeyTime {
                    key: curr.expected,
                    time_ms: dt,
                    correct: curr.correct,
                });
            }

            // Backspace attempt tracking:
            // - Any incorrect non-backspace key creates a pending backspace need.
            // - While pending, every next key press is a backspace attempt.
            // - Backspace press = correct attempt; anything else = incorrect attempt
            //   and the requirement stays pending.
            if pending_backspace {
                if curr.actual == BACKSPACE {
                    per_key_times.push(KeyTime {
                        key: BACKSPACE,
                        time_ms: dt,
                        correct: true,
                    });
                    pending_backspace = false;
                } else {
                    per_key_times.push(KeyTime {
                        key: BACKSPACE,
                        time_ms: dt,
                        correct: false,
                    });
                    pending_backspace = true;
                }
            }

            if curr.actual != BACKSPACE && !curr.correct {
                pending_backspace = true;
            }
        }

        let total_chars = drill.target.len();
        let typo_count = drill.typo_flags.len();
        let accuracy = if total_chars > 0 {
            ((total_chars - typo_count) as f64 / total_chars as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            100.0
        };

        Self {
            wpm: drill.wpm(),
            cpm: drill.cpm(),
            accuracy,
            correct: total_chars - typo_count,
            incorrect: typo_count,
            total_chars,
            elapsed_secs: drill.elapsed_secs(),
            timestamp: Utc::now(),
            per_key_times,
            drill_mode: drill_mode.to_string(),
            ranked,
            partial,
            completion_percent: (drill.progress() * 100.0).clamp(0.0, 100.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn ev(expected: char, actual: char, ms: u64, correct: bool, start: Instant) -> KeystrokeEvent {
        KeystrokeEvent {
            expected,
            actual,
            timestamp: start + Duration::from_millis(ms),
            correct,
        }
    }

    #[test]
    fn tracks_backspace_success_after_incorrect_key() {
        let drill = DrillState::new("ab");
        let t0 = Instant::now();
        let events = vec![
            ev('a', 'a', 0, true, t0),
            ev('b', 'x', 100, false, t0),
            ev(BACKSPACE, BACKSPACE, 220, true, t0),
            ev('b', 'b', 350, true, t0),
        ];

        let result = DrillResult::from_drill(&drill, &events, "adaptive", true, false);
        let backspace: Vec<&KeyTime> = result
            .per_key_times
            .iter()
            .filter(|kt| kt.key == BACKSPACE)
            .collect();
        assert_eq!(backspace.len(), 1);
        assert!(backspace[0].correct);
        assert!((backspace[0].time_ms - 120.0).abs() < 0.1);
    }

    #[test]
    fn tracks_backspace_error_until_user_backspaces() {
        let drill = DrillState::new("abc");
        let t0 = Instant::now();
        let events = vec![
            ev('a', 'a', 0, true, t0),
            ev('b', 'x', 100, false, t0),
            ev('c', 'c', 220, true, t0),
            ev(BACKSPACE, BACKSPACE, 400, true, t0),
        ];

        let result = DrillResult::from_drill(&drill, &events, "adaptive", true, false);
        let backspace: Vec<&KeyTime> = result
            .per_key_times
            .iter()
            .filter(|kt| kt.key == BACKSPACE)
            .collect();
        assert_eq!(backspace.len(), 2);
        assert!(!backspace[0].correct);
        assert!(backspace[1].correct);
        assert!((backspace[0].time_ms - 120.0).abs() < 0.1);
        assert!((backspace[1].time_ms - 180.0).abs() < 0.1);
    }
}
