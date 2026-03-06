use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::config::Config;
use crate::engine::key_stats::KeyStatsStore;
use crate::engine::skill_tree::SkillTreeProgress;
use crate::session::result::DrillResult;

pub const SCHEMA_VERSION: u32 = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileData {
    pub schema_version: u32,
    /// Legacy single-scope progress mirror retained for import/export compatibility.
    /// Always write this via `set_skill_tree_for_language`, never directly.
    pub skill_tree: SkillTreeProgress,
    /// Language-scoped skill tree progression state keyed by dictionary language.
    #[serde(default)]
    pub skill_tree_by_language: HashMap<String, SkillTreeProgress>,
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
            skill_tree_by_language: HashMap::new(),
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

    pub fn skill_tree_for_language(&self, language_key: &str) -> SkillTreeProgress {
        self.skill_tree_by_language
            .get(language_key)
            .cloned()
            .unwrap_or_else(|| self.skill_tree.clone())
    }

    pub fn set_skill_tree_for_language(&mut self, language_key: &str, progress: SkillTreeProgress) {
        self.skill_tree_by_language
            .insert(language_key.to_string(), progress.clone());
        // Keep legacy mirror aligned with the current active scope.
        self.skill_tree = progress;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_skill_tree_for_language_falls_back_to_legacy() {
        let profile = ProfileData::default();
        let scoped = profile.skill_tree_for_language("de");
        let lowercase = scoped
            .branches
            .get("lowercase")
            .expect("lowercase branch should exist");
        assert_eq!(lowercase.current_level, 0);
    }

    #[test]
    fn profile_set_skill_tree_for_language_updates_scoped_map() {
        let mut profile = ProfileData::default();
        let mut progress = SkillTreeProgress::default();
        progress
            .branches
            .get_mut("lowercase")
            .expect("lowercase branch should exist")
            .current_level = 3;
        profile.set_skill_tree_for_language("de", progress.clone());

        let loaded = profile.skill_tree_for_language("de");
        assert_eq!(
            loaded
                .branches
                .get("lowercase")
                .expect("lowercase branch should exist")
                .current_level,
            3
        );
        assert_eq!(
            profile
                .skill_tree
                .branches
                .get("lowercase")
                .expect("lowercase branch should exist")
                .current_level,
            3
        );
    }
}
