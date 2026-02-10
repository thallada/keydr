use serde::{Deserialize, Serialize};

use crate::engine::key_stats::KeyStatsStore;
use crate::session::result::LessonResult;

const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileData {
    pub schema_version: u32,
    pub unlocked_letters: Vec<char>,
    pub total_score: f64,
    pub total_lessons: u32,
    pub streak_days: u32,
    pub best_streak: u32,
    pub last_practice_date: Option<String>,
}

impl Default for ProfileData {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            unlocked_letters: Vec::new(),
            total_score: 0.0,
            total_lessons: 0,
            streak_days: 0,
            best_streak: 0,
            last_practice_date: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyStatsData {
    pub schema_version: u32,
    pub stats: KeyStatsStore,
}

impl Default for KeyStatsData {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            stats: KeyStatsStore::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LessonHistoryData {
    pub schema_version: u32,
    pub lessons: Vec<LessonResult>,
}

impl Default for LessonHistoryData {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            lessons: Vec::new(),
        }
    }
}
