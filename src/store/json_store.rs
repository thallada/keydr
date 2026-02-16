use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};

use crate::store::schema::{DrillHistoryData, KeyStatsData, ProfileData};

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

    pub fn load_drill_history(&self) -> DrillHistoryData {
        self.load("lesson_history.json")
    }

    pub fn save_drill_history(&self, data: &DrillHistoryData) -> Result<()> {
        self.save("lesson_history.json", data)
    }
}
