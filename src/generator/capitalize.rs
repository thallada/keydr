use rand::Rng;
use rand::rngs::SmallRng;

/// Post-processing pass that capitalizes words in generated text.
/// Only capitalizes using letters from `unlocked_capitals`.
pub fn apply_capitalization(
    text: &str,
    unlocked_capitals: &[char],
    focused: Option<char>,
    rng: &mut SmallRng,
) -> String {
    if unlocked_capitals.is_empty() {
        return text.to_string();
    }

    // If focused key is an uppercase letter, boost its probability
    let focused_upper = focused.filter(|ch| ch.is_ascii_uppercase());

    let mut result = String::with_capacity(text.len());
    let mut at_sentence_start = true;

    for (i, ch) in text.chars().enumerate() {
        if at_sentence_start && ch.is_ascii_lowercase() {
            let upper = ch.to_ascii_uppercase();
            if unlocked_capitals.contains(&upper) {
                result.push(upper);
                at_sentence_start = false;
                continue;
            }
        }

        // After period/question/exclamation + space, next word starts a sentence
        if ch == ' ' && i > 0 {
            let prev = text.as_bytes().get(i - 1).map(|&b| b as char);
            if matches!(prev, Some('.' | '?' | '!')) {
                at_sentence_start = true;
            }
        }

        // Capitalize word starts: boosted for focused key, ~12% for others
        if ch.is_ascii_lowercase() && !at_sentence_start {
            let is_word_start =
                i == 0 || text.as_bytes().get(i - 1).map(|&b| b as char) == Some(' ');
            if is_word_start {
                let upper = ch.to_ascii_uppercase();
                if unlocked_capitals.contains(&upper) {
                    let prob = if focused_upper == Some(upper) {
                        0.40
                    } else {
                        0.12
                    };
                    if rng.gen_bool(prob) {
                        result.push(upper);
                        continue;
                    }
                }
            }
        }

        if ch != '.' && ch != '?' && ch != '!' {
            at_sentence_start = false;
        }

        result.push(ch);
    }

    // Focused capitals should show up multiple times when possible so they are
    // introduced at a similar density to other focused key types.
    if let Some(focused_upper) = focused_upper.filter(|ch| unlocked_capitals.contains(ch)) {
        return ensure_min_focused_occurrences(&result, focused_upper, 3);
    }

    result
}

fn ensure_min_focused_occurrences(text: &str, focused_upper: char, min_count: usize) -> String {
    let focused_lower = focused_upper.to_ascii_lowercase();
    let mut chars: Vec<char> = text.chars().collect();
    let mut count = chars.iter().filter(|&&ch| ch == focused_upper).count();

    if count >= min_count {
        return text.to_string();
    }

    // First, capitalize matching word starts.
    for i in 0..chars.len() {
        if count >= min_count {
            break;
        }
        if chars[i] != focused_lower {
            continue;
        }
        let is_word_start =
            i == 0 || matches!(chars.get(i.saturating_sub(1)), Some(' ' | '\n' | '\t'));
        if is_word_start {
            chars[i] = focused_upper;
            count += 1;
        }
    }

    // If still short, capitalize matching letters anywhere in the text.
    for ch in &mut chars {
        if count >= min_count {
            break;
        }
        if *ch == focused_lower {
            *ch = focused_upper;
            count += 1;
        }
    }

    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_no_caps_when_empty() {
        let mut rng = SmallRng::seed_from_u64(42);
        let result = apply_capitalization("hello world", &[], None, &mut rng);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_capitalizes_first_word() {
        let mut rng = SmallRng::seed_from_u64(42);
        let result = apply_capitalization("hello world", &['H', 'W'], None, &mut rng);
        assert!(result.starts_with('H'));
    }

    #[test]
    fn test_only_capitalizes_unlocked() {
        let mut rng = SmallRng::seed_from_u64(42);
        // Only 'W' is unlocked, not 'H'
        let result = apply_capitalization("hello world", &['W'], None, &mut rng);
        assert!(result.starts_with('h')); // 'H' not unlocked
    }

    #[test]
    fn test_after_period() {
        let mut rng = SmallRng::seed_from_u64(42);
        let result = apply_capitalization("one. two", &['O', 'T'], None, &mut rng);
        assert!(result.starts_with('O'));
        assert!(result.contains("Two") || result.contains("two"));
        // At minimum, first word should be capitalized
    }

    #[test]
    fn test_focused_capital_boosted() {
        // With focused 'W', W capitalization should happen more often
        let caps = &['H', 'W'];
        let mut focused_count = 0;
        let mut unfocused_count = 0;
        // Run many trials to check statistical boosting
        for seed in 0..200 {
            let mut rng = SmallRng::seed_from_u64(seed);
            let text = "hello world wide web wonder what where who will work";
            let result = apply_capitalization(text, caps, Some('W'), &mut rng);
            // Count W capitalizations (skip first word which is always capitalized if 'H' is available)
            focused_count += result.matches('W').count();
            let mut rng2 = SmallRng::seed_from_u64(seed);
            let result2 = apply_capitalization(text, caps, None, &mut rng2);
            unfocused_count += result2.matches('W').count();
        }
        assert!(
            focused_count > unfocused_count,
            "Focused W count ({focused_count}) should exceed unfocused ({unfocused_count})"
        );
    }

    #[test]
    fn test_focused_capital_has_minimum_presence_when_available() {
        let mut rng = SmallRng::seed_from_u64(123);
        let text = "we will work with weird words while we wait";
        let result = apply_capitalization(text, &['W'], Some('W'), &mut rng);
        let focused_count = result.chars().filter(|&ch| ch == 'W').count();
        assert!(
            focused_count >= 3,
            "Expected at least 3 focused capitals, got {focused_count} in: {result}"
        );
    }
}
