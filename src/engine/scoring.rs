use crate::session::result::DrillResult;

pub fn compute_score(result: &DrillResult, complexity: f64) -> f64 {
    let speed = result.cpm;
    let errors = result.incorrect as f64;
    let length = result.total_chars as f64;
    (speed * complexity) / (errors + 1.0) * (length / 50.0)
}

pub fn compute_complexity(unlocked_count: usize, total_keys: usize) -> f64 {
    (unlocked_count as f64 / total_keys as f64).max(0.1)
}

pub fn level_from_score(total_score: f64) -> u32 {
    let level = (total_score / 100.0).sqrt() as u32;
    level.max(1)
}

#[allow(dead_code)]
pub fn score_to_next_level(total_score: f64) -> f64 {
    let current_level = level_from_score(total_score);
    let next_level_score = ((current_level + 1) as f64).powi(2) * 100.0;
    next_level_score - total_score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_starts_at_one() {
        assert_eq!(level_from_score(0.0), 1);
    }

    #[test]
    fn test_level_increases_with_score() {
        assert!(level_from_score(1000.0) > level_from_score(100.0));
    }

    #[test]
    fn test_complexity_scales_with_keys() {
        assert!(compute_complexity(96, 96) > compute_complexity(6, 96));
        assert!((compute_complexity(96, 96) - 1.0).abs() < 0.001);
    }
}
