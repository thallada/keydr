use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Result, bail};
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};

use crate::config::Config;
use crate::store::schema::{
    DrillHistoryData, EXPORT_VERSION, ExportData, KeyStatsData, ProfileData,
};

pub struct JsonStore {
    base_dir: PathBuf,
}

impl JsonStore {
    pub fn new() -> Result<Self> {
        let base_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("keydr");
        fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    #[allow(dead_code)] // Used by integration tests
    pub fn with_base_dir(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    fn file_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(name)
    }

    fn load<T: DeserializeOwned + Default>(&self, name: &str) -> T {
        let path = self.file_path(name);
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => T::default(),
            }
        } else {
            T::default()
        }
    }

    fn save<T: Serialize>(&self, name: &str, data: &T) -> Result<()> {
        let path = self.file_path(name);
        let tmp_path = path.with_extension("tmp");

        let json = serde_json::to_string_pretty(data)?;
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;

        fs::rename(&tmp_path, &path)?;
        Ok(())
    }

    /// Load and deserialize profile. Returns None if file exists but
    /// cannot be parsed (schema mismatch / corruption).
    pub fn load_profile(&self) -> Option<ProfileData> {
        let path = self.file_path("profile.json");
        if path.exists() {
            let content = fs::read_to_string(&path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            // No file yet — return fresh default (not a schema mismatch)
            Some(ProfileData::default())
        }
    }

    pub fn save_profile(&self, data: &ProfileData) -> Result<()> {
        self.save("profile.json", data)
    }

    pub fn load_key_stats(&self) -> KeyStatsData {
        self.load("key_stats.json")
    }

    pub fn save_key_stats(&self, data: &KeyStatsData) -> Result<()> {
        self.save("key_stats.json", data)
    }

    pub fn load_ranked_key_stats(&self) -> KeyStatsData {
        self.load("key_stats_ranked.json")
    }

    pub fn save_ranked_key_stats(&self, data: &KeyStatsData) -> Result<()> {
        self.save("key_stats_ranked.json", data)
    }

    pub fn load_drill_history(&self) -> DrillHistoryData {
        self.load("lesson_history.json")
    }

    pub fn save_drill_history(&self, data: &DrillHistoryData) -> Result<()> {
        self.save("lesson_history.json", data)
    }

    /// Bundle all persisted data + config into an ExportData struct.
    /// N-gram stats are not included — they are always rebuilt from drill history.
    pub fn export_all(&self, config: &Config) -> ExportData {
        let profile = self.load_profile().unwrap_or_default();
        let key_stats = self.load_key_stats();
        let ranked_key_stats = self.load_ranked_key_stats();
        let drill_history = self.load_drill_history();

        ExportData {
            keydr_export_version: EXPORT_VERSION,
            exported_at: Utc::now(),
            config: config.clone(),
            profile,
            key_stats,
            ranked_key_stats,
            drill_history,
        }
    }

    /// Transactional import: two-phase commit with best-effort .bak rollback.
    ///
    /// Stage phase: write all data to .tmp files. If any fails, clean up and bail.
    /// Commit phase: for each file, rename original to .bak, then .tmp to final.
    /// On commit failure, attempt to restore .bak files and clean up .tmp files.
    /// After success, delete .bak files.
    pub fn import_all(&self, data: &ExportData) -> Result<()> {
        if data.keydr_export_version != EXPORT_VERSION {
            bail!(
                "Unsupported export version: {} (expected {})",
                data.keydr_export_version,
                EXPORT_VERSION
            );
        }

        let files: Vec<(&str, String)> = vec![
            ("profile.json", serde_json::to_string_pretty(&data.profile)?),
            (
                "key_stats.json",
                serde_json::to_string_pretty(&data.key_stats)?,
            ),
            (
                "key_stats_ranked.json",
                serde_json::to_string_pretty(&data.ranked_key_stats)?,
            ),
            (
                "lesson_history.json",
                serde_json::to_string_pretty(&data.drill_history)?,
            ),
        ];

        // Stage phase: write .tmp files
        let mut staged: Vec<PathBuf> = Vec::new();
        for (name, json) in &files {
            let tmp_path = self.file_path(name).with_extension("json.tmp");
            match (|| -> Result<()> {
                let mut file = fs::File::create(&tmp_path)?;
                file.write_all(json.as_bytes())?;
                file.sync_all()?;
                Ok(())
            })() {
                Ok(()) => staged.push(tmp_path),
                Err(e) => {
                    // Clean up staged .tmp files
                    for tmp in &staged {
                        let _ = fs::remove_file(tmp);
                    }
                    bail!("Import failed during staging: {e}");
                }
            }
        }

        // Commit phase: .bak then rename .tmp to final
        // Track (final_path, had_original) so rollback can restore absence
        let mut committed: Vec<(PathBuf, PathBuf, bool)> = Vec::new();
        for (i, (name, _)) in files.iter().enumerate() {
            let final_path = self.file_path(name);
            let bak_path = self.file_path(name).with_extension("json.bak");
            let tmp_path = &staged[i];
            let had_original = final_path.exists();

            // Back up existing file if it exists
            if had_original && let Err(e) = fs::rename(&final_path, &bak_path) {
                // Rollback: restore already committed files
                for (committed_final, committed_bak, committed_had) in &committed {
                    if *committed_had {
                        let _ = fs::rename(committed_bak, committed_final);
                    } else {
                        let _ = fs::remove_file(committed_final);
                    }
                }
                // Clean up all .tmp files
                for tmp in &staged {
                    let _ = fs::remove_file(tmp);
                }
                bail!("Import failed during commit (backup): {e}");
            }

            // Rename .tmp to final
            if let Err(e) = fs::rename(tmp_path, &final_path) {
                // Restore this file's backup or remove if it didn't exist
                if had_original && bak_path.exists() {
                    let _ = fs::rename(&bak_path, &final_path);
                } else {
                    let _ = fs::remove_file(&final_path);
                }
                // Rollback previously committed files
                for (committed_final, committed_bak, committed_had) in &committed {
                    if *committed_had {
                        let _ = fs::rename(committed_bak, committed_final);
                    } else {
                        let _ = fs::remove_file(committed_final);
                    }
                }
                // Clean up remaining .tmp files
                for tmp in &staged[i + 1..] {
                    let _ = fs::remove_file(tmp);
                }
                bail!("Import failed during commit (rename): {e}");
            }

            committed.push((final_path, bak_path, had_original));
        }

        // Success: clean up .bak files
        for (_, bak_path, had_original) in &committed {
            if *had_original {
                let _ = fs::remove_file(bak_path);
            }
        }

        Ok(())
    }

    /// Check for leftover .bak files from an interrupted import.
    /// Returns true if recovery files were found (and cleaned up).
    pub fn check_interrupted_import(&self) -> bool {
        let bak_names = [
            "profile.json.bak",
            "key_stats.json.bak",
            "key_stats_ranked.json.bak",
            "lesson_history.json.bak",
        ];
        let mut found = false;
        for name in &bak_names {
            let bak_path = self.base_dir.join(name);
            if bak_path.exists() {
                found = true;
                let _ = fs::remove_file(&bak_path);
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::store::schema::EXPORT_VERSION;
    use tempfile::TempDir;

    fn make_test_store() -> (TempDir, JsonStore) {
        let dir = TempDir::new().unwrap();
        let store = JsonStore::with_base_dir(dir.path().to_path_buf()).unwrap();
        (dir, store)
    }

    fn make_test_export(config: &Config) -> ExportData {
        ExportData {
            keydr_export_version: EXPORT_VERSION,
            exported_at: Utc::now(),
            config: config.clone(),
            profile: ProfileData::default(),
            key_stats: KeyStatsData::default(),
            ranked_key_stats: KeyStatsData::default(),
            drill_history: DrillHistoryData::default(),
        }
    }

    #[test]
    fn test_round_trip_export_import() {
        let (_dir, store) = make_test_store();
        let config = Config::default();

        // Save some initial data
        store.save_profile(&ProfileData::default()).unwrap();

        let export = store.export_all(&config);
        assert_eq!(export.keydr_export_version, EXPORT_VERSION);

        // Create a second store and import into it
        let (_dir2, store2) = make_test_store();
        store2.import_all(&export).unwrap();

        // Verify data matches
        let imported_profile = store2.load_profile().unwrap();
        assert_eq!(imported_profile.total_drills, export.profile.total_drills);
        assert!((imported_profile.total_score - export.profile.total_score).abs() < f64::EPSILON);
    }

    #[test]
    fn test_version_rejection() {
        let (_dir, store) = make_test_store();
        let config = Config::default();
        let mut export = make_test_export(&config);
        export.keydr_export_version = 99;

        let result = store.import_all(&export);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Unsupported export version"));
        assert!(err_msg.contains("99"));
    }

    #[test]
    fn test_config_validate_clamps_values() {
        let mut config = Config::default();
        config.target_wpm = 0;
        config.word_count = 999;
        config.code_language = "nonexistent".to_string();

        let valid_keys = vec!["rust", "python", "javascript"];
        config.validate(&valid_keys);

        assert_eq!(config.target_wpm, 10);
        assert_eq!(config.word_count, 100);
        assert_eq!(config.code_language, "rust"); // falls back to default
    }

    #[test]
    fn test_import_staging_failure_preserves_originals() {
        let (_dir, store) = make_test_store();

        // Save known good data
        let mut profile = ProfileData::default();
        profile.total_drills = 42;
        store.save_profile(&profile).unwrap();
        let original_content = fs::read_to_string(store.file_path("profile.json")).unwrap();

        // Now create a store that points to a nonexistent subdir of the same tmpdir
        // so that staging .tmp writes will fail
        let bad_dir = _dir.path().join("nonexistent_subdir");
        let bad_store = JsonStore {
            base_dir: bad_dir.clone(),
        };
        let config = Config::default();
        let export = make_test_export(&config);
        let result = bad_store.import_all(&export);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Import failed during staging")
        );

        // Original file in the real store is unchanged
        let after_content = fs::read_to_string(store.file_path("profile.json")).unwrap();
        assert_eq!(original_content, after_content);

        // No .tmp files left in the bad dir (dir doesn't exist, so nothing to clean)
        assert!(!bad_dir.exists());

        // No .tmp files left in the real store dir either
        let tmp_files: Vec<_> = fs::read_dir(_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("tmp"))
            .collect();
        assert!(tmp_files.is_empty(), "no residual .tmp files");
    }

    #[test]
    fn test_import_into_empty_store_then_verify_files_created() {
        let (_dir, store) = make_test_store();

        // No files initially
        assert!(!store.file_path("profile.json").exists());

        let config = Config::default();
        let export = make_test_export(&config);
        store.import_all(&export).unwrap();

        // All files should now exist
        assert!(store.file_path("profile.json").exists());
        assert!(store.file_path("key_stats.json").exists());
        assert!(store.file_path("key_stats_ranked.json").exists());
        assert!(store.file_path("lesson_history.json").exists());
    }

    #[test]
    fn test_check_interrupted_import_detects_bak_files() {
        let (_dir, store) = make_test_store();

        // No .bak files initially
        assert!(!store.check_interrupted_import());

        // Create a .bak file
        fs::write(store.file_path("profile.json.bak"), "{}").unwrap();
        assert!(store.check_interrupted_import());

        // Should have been cleaned up
        assert!(!store.file_path("profile.json.bak").exists());
    }
}
