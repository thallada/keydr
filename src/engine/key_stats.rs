use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const EMA_ALPHA: f64 = 0.1;
const DEFAULT_TARGET_CPM: f64 = 175.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyStat {
    pub filtered_time_ms: f64,
    pub best_time_ms: f64,
    pub confidence: f64,
    pub sample_count: usize,
    pub recent_times: Vec<f64>,
    #[serde(default)]
    pub error_count: usize,
    #[serde(default)]
    pub total_count: usize,
    #[serde(default = "default_error_rate_ema")]
    pub error_rate_ema: f64,
}

fn default_error_rate_ema() -> f64 {
    0.5
}

impl Default for KeyStat {
    fn default() -> Self {
        Self {
            filtered_time_ms: 1000.0,
            best_time_ms: f64::MAX,
            confidence: 0.0,
            sample_count: 0,
            recent_times: Vec::new(),
            error_count: 0,
            total_count: 0,
            error_rate_ema: 0.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyStatsStore {
    pub stats: HashMap<char, KeyStat>,
    pub target_cpm: f64,
}

impl Default for KeyStatsStore {
    fn default() -> Self {
        Self {
            stats: HashMap::new(),
            target_cpm: DEFAULT_TARGET_CPM,
        }
    }
}

impl KeyStatsStore {
    pub fn update_key(&mut self, key: char, time_ms: f64) {
        let stat = self.stats.entry(key).or_default();
        stat.sample_count += 1;
        stat.total_count += 1;

        if stat.sample_count == 1 {
            stat.filtered_time_ms = time_ms;
        } else {
            stat.filtered_time_ms = EMA_ALPHA * time_ms + (1.0 - EMA_ALPHA) * stat.filtered_time_ms;
        }

        stat.best_time_ms = stat.best_time_ms.min(stat.filtered_time_ms);

        let target_time_ms = 60000.0 / self.target_cpm;
        stat.confidence = target_time_ms / stat.filtered_time_ms;

        stat.recent_times.push(time_ms);
        if stat.recent_times.len() > 30 {
            stat.recent_times.remove(0);
        }

        // Update error rate EMA (correct stroke = 0.0 signal)
        if stat.total_count == 1 {
            stat.error_rate_ema = 0.0;
        } else {
            stat.error_rate_ema = EMA_ALPHA * 0.0 + (1.0 - EMA_ALPHA) * stat.error_rate_ema;
        }
    }

    pub fn get_confidence(&self, key: char) -> f64 {
        self.stats.get(&key).map(|s| s.confidence).unwrap_or(0.0)
    }

    #[allow(dead_code)]
    pub fn get_stat(&self, key: char) -> Option<&KeyStat> {
        self.stats.get(&key)
    }

    /// Record an error for a key (increments error_count and total_count).
    /// Does NOT update timing/confidence (those are only updated for correct strokes).
    pub fn update_key_error(&mut self, key: char) {
        let stat = self.stats.entry(key).or_default();
        stat.error_count += 1;
        stat.total_count += 1;

        // Update error rate EMA (error stroke = 1.0 signal)
        if stat.total_count == 1 {
            stat.error_rate_ema = 1.0;
        } else {
            stat.error_rate_ema = EMA_ALPHA * 1.0 + (1.0 - EMA_ALPHA) * stat.error_rate_ema;
        }
    }

    /// EMA-based error rate for a key.
    pub fn smoothed_error_rate(&self, key: char) -> f64 {
        match self.stats.get(&key) {
            Some(s) => s.error_rate_ema,
            None => 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_confidence_is_zero() {
        let store = KeyStatsStore::default();
        assert_eq!(store.get_confidence('a'), 0.0);
    }

    #[test]
    fn test_update_key_creates_stat() {
        let mut store = KeyStatsStore::default();
        store.update_key('e', 300.0);
        assert!(store.get_confidence('e') > 0.0);
        assert_eq!(store.stats.get(&'e').unwrap().sample_count, 1);
    }

    #[test]
    fn test_ema_converges() {
        let mut store = KeyStatsStore::default();
        // Type key fast many times - confidence should increase
        for _ in 0..50 {
            store.update_key('t', 200.0);
        }
        let conf = store.get_confidence('t');
        // At 175 CPM target, target_time = 60000/175 = 342.8ms
        // With 200ms typing time, confidence = 342.8/200 = 1.71
        assert!(
            conf > 1.0,
            "confidence should be > 1.0 for fast typing, got {conf}"
        );
    }

    #[test]
    fn test_slow_typing_low_confidence() {
        let mut store = KeyStatsStore::default();
        for _ in 0..50 {
            store.update_key('a', 1000.0);
        }
        let conf = store.get_confidence('a');
        // target_time = 342.8ms, typing at 1000ms -> conf = 342.8/1000 = 0.34
        assert!(
            conf < 1.0,
            "confidence should be < 1.0 for slow typing, got {conf}"
        );
    }

    #[test]
    fn test_ema_error_rate_correct_strokes() {
        let mut store = KeyStatsStore::default();
        // All correct strokes → EMA should be 0.0 for first, stay near 0
        store.update_key('a', 200.0);
        assert!((store.smoothed_error_rate('a') - 0.0).abs() < f64::EPSILON);
        for _ in 0..10 {
            store.update_key('a', 200.0);
        }
        assert!(
            store.smoothed_error_rate('a') < 0.01,
            "All correct → EMA near 0"
        );
    }

    #[test]
    fn test_ema_error_rate_error_strokes() {
        let mut store = KeyStatsStore::default();
        // First stroke is error
        store.update_key_error('b');
        assert!((store.smoothed_error_rate('b') - 1.0).abs() < f64::EPSILON);
        // Follow with correct strokes → EMA decays
        for _ in 0..20 {
            store.update_key('b', 200.0);
        }
        let rate = store.smoothed_error_rate('b');
        assert!(
            rate < 0.15,
            "After 20 correct, EMA should be < 0.15, got {rate}"
        );
    }

    #[test]
    fn test_ema_error_rate_default_for_missing_key() {
        let store = KeyStatsStore::default();
        assert!((store.smoothed_error_rate('z') - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ema_error_rate_serde_default() {
        // Verify backward compat: deserializing old data without error_rate_ema gets 0.5
        let json = r#"{"filtered_time_ms":200.0,"best_time_ms":200.0,"confidence":1.0,"sample_count":10,"recent_times":[],"error_count":2,"total_count":10}"#;
        let stat: KeyStat = serde_json::from_str(json).unwrap();
        assert!((stat.error_rate_ema - 0.5).abs() < f64::EPSILON);
    }
}
