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
}

impl Default for KeyStat {
    fn default() -> Self {
        Self {
            filtered_time_ms: 1000.0,
            best_time_ms: f64::MAX,
            confidence: 0.0,
            sample_count: 0,
            recent_times: Vec::new(),
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
    }

    pub fn get_confidence(&self, key: char) -> f64 {
        self.stats.get(&key).map(|s| s.confidence).unwrap_or(0.0)
    }

    #[allow(dead_code)]
    pub fn get_stat(&self, key: char) -> Option<&KeyStat> {
        self.stats.get(&key)
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
}
