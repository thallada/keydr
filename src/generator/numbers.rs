use rand::Rng;
use rand::rngs::SmallRng;

/// Post-processing pass that inserts number expressions into text.
/// Only uses digits from `unlocked_digits`.
pub fn apply_numbers(
    text: &str,
    unlocked_digits: &[char],
    has_dot: bool,
    focused: Option<char>,
    rng: &mut SmallRng,
) -> String {
    if unlocked_digits.is_empty() {
        return text.to_string();
    }

    // If focused key is a digit, boost number insertion probability
    let focused_digit = focused.filter(|ch| ch.is_ascii_digit());
    let base_prob = if focused_digit.is_some() { 0.30 } else { 0.15 };

    let words: Vec<&str> = text.split(' ').collect();
    let mut result = Vec::new();

    for word in &words {
        if rng.gen_bool(base_prob) {
            let expr = generate_number_expr(unlocked_digits, has_dot, focused_digit, rng);
            result.push(expr);
        } else {
            result.push(word.to_string());
        }
    }

    result.join(" ")
}

fn generate_number_expr(
    digits: &[char],
    has_dot: bool,
    focused_digit: Option<char>,
    rng: &mut SmallRng,
) -> String {
    // Determine how many patterns are available (version pattern needs dot)
    let max_pattern = if has_dot { 5 } else { 4 };
    let pattern = rng.gen_range(0..max_pattern);
    let num = match pattern {
        0 => {
            // Simple count: "3" or "42"
            random_number(digits, 1, 3, focused_digit, rng)
        }
        1 => {
            // Measurement: "7 miles" or "42 items"
            let num = random_number(digits, 1, 2, focused_digit, rng);
            let units = ["items", "miles", "days", "lines", "times", "parts"];
            let unit = units[rng.gen_range(0..units.len())];
            return format!("{num} {unit}");
        }
        2 => {
            // Year-like: "2024"
            random_number(digits, 4, 4, focused_digit, rng)
        }
        3 => {
            // ID: "room 42" or "page 7"
            let prefixes = ["room", "page", "step", "item", "line", "port"];
            let prefix = prefixes[rng.gen_range(0..prefixes.len())];
            let num = random_number(digits, 1, 3, focused_digit, rng);
            return format!("{prefix} {num}");
        }
        _ => {
            // Version-like: "3.14" or "2.0" (only when dot is available)
            let major = random_number(digits, 1, 1, focused_digit, rng);
            let minor = random_number(digits, 1, 2, focused_digit, rng);
            return format!("{major}.{minor}");
        }
    };
    num
}

fn random_number(
    digits: &[char],
    min_len: usize,
    max_len: usize,
    focused_digit: Option<char>,
    rng: &mut SmallRng,
) -> String {
    let len = rng.gen_range(min_len..=max_len);
    (0..len)
        .map(|_| {
            // 40% chance to use the focused digit if it's a digit
            if let Some(fd) = focused_digit {
                if rng.gen_bool(0.40) {
                    return fd;
                }
            }
            digits[rng.gen_range(0..digits.len())]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_no_numbers_when_empty() {
        let mut rng = SmallRng::seed_from_u64(42);
        let result = apply_numbers("hello world", &[], false, None, &mut rng);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_numbers_use_only_unlocked_digits() {
        let mut rng = SmallRng::seed_from_u64(42);
        let digits = ['1', '2', '3'];
        let text = "a b c d e f g h i j k l m n o p q r s t";
        let result = apply_numbers(text, &digits, false, None, &mut rng);
        for ch in result.chars() {
            if ch.is_ascii_digit() {
                assert!(digits.contains(&ch), "Unexpected digit {ch} in: {result}");
            }
        }
    }

    #[test]
    fn test_no_dot_without_punctuation() {
        let mut rng = SmallRng::seed_from_u64(42);
        let digits = ['1', '2', '3', '4', '5'];
        let text = "a b c d e f g h i j k l m n o p q r s t";
        let result = apply_numbers(text, &digits, false, None, &mut rng);
        assert!(
            !result.contains('.'),
            "Should not contain dot when has_dot=false: {result}"
        );
    }
}
