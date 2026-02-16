use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
}

fn default_drill_mode() -> String {
    "adaptive".to_string()
}

fn default_true() -> bool {
    true
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
    ) -> Self {
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
        }
    }
}
