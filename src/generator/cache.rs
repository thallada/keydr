use std::fs;
use std::path::PathBuf;

pub struct DiskCache {
    base_dir: PathBuf,
}

impl DiskCache {
    pub fn new(subdir: &str) -> Option<Self> {
        let base = dirs::data_dir()?.join("keydr").join(subdir);
        fs::create_dir_all(&base).ok()?;
        Some(Self { base_dir: base })
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let path = self.base_dir.join(Self::sanitize_key(key));
        fs::read_to_string(path).ok()
    }

    pub fn put(&self, key: &str, content: &str) -> bool {
        let path = self.base_dir.join(Self::sanitize_key(key));
        fs::write(path, content).is_ok()
    }

    fn sanitize_key(key: &str) -> String {
        key.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }
}

#[cfg(feature = "network")]
pub fn fetch_url(url: &str) -> Option<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;
    let response = client.get(url).send().ok()?;
    if response.status().is_success() {
        response.text().ok()
    } else {
        None
    }
}

#[cfg(not(feature = "network"))]
pub fn fetch_url(_url: &str) -> Option<String> {
    None
}
