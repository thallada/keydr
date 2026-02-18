use std::fs;
#[cfg(feature = "network")]
use std::io::Read;
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

#[cfg(feature = "network")]
pub fn fetch_url_bytes_with_progress<F>(url: &str, mut on_progress: F) -> Option<Vec<u8>>
where
    F: FnMut(u64, Option<u64>),
{
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .ok()?;
    let mut response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }

    let total = response.content_length();
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    let mut downloaded = 0u64;

    loop {
        let n = response.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        downloaded = downloaded.saturating_add(n as u64);
        on_progress(downloaded, total);
    }

    Some(out)
}

#[cfg(not(feature = "network"))]
pub fn fetch_url_bytes_with_progress<F>(_url: &str, _on_progress: F) -> Option<Vec<u8>>
where
    F: FnMut(u64, Option<u64>),
{
    None
}
