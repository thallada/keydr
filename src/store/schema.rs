use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::engine::key_stats::KeyStatsStore;
use crate::engine::skill_tree::SkillTreeProgress;
use crate::session::result::DrillResult;

const SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileData {
    pub schema_version: u32,
    pub skill_tree: SkillTreeProgress,
    pub total_score: f64,
    #[serde(alias = "total_lessons")]
    pub total_drills: u32,
    pub streak_days: u32,
    pub best_streak: u32,
    pub last_practice_date: Option<String>,
}

impl Default for ProfileData {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            skill_tree: SkillTreeProgress::default(),
            total_score: 0.0,
            total_drills: 0,
            streak_days: 0,
            best_streak: 0,
            last_practice_date: None,
        }
    }
}

impl ProfileData {
    /// Check if loaded data has a stale schema version and needs reset.
    pub fn needs_reset(&self) -> bool {
        self.schema_version != SCHEMA_VERSION
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
pub struct DrillHistoryData {
    pub schema_version: u32,
    #[serde(alias = "lessons")]
    pub drills: Vec<DrillResult>,
}

impl Default for DrillHistoryData {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            drills: Vec::new(),
        }
    }
}

pub const EXPORT_VERSION: u32 = 1;

/// Export contract: drill_history is the sole source of truth for n-gram stats.
/// N-gram data is always rebuilt from history on import/startup, so it is not
/// included in the export payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub keydr_export_version: u32,
    pub exported_at: DateTime<Utc>,
    pub config: Config,
    pub profile: ProfileData,
    pub key_stats: KeyStatsData,
    pub ranked_key_stats: KeyStatsData,
    pub drill_history: DrillHistoryData,
}
