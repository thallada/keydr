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

    let focused_upper = focused.filter(|ch| ch.is_ascii_uppercase());
    let mut words: Vec<String> = text.split_whitespace().map(|w| w.to_string()).collect();
    if words.is_empty() {
        return text.to_string();
    }

    // Prefer capitals at starts of words (sentence starts always when possible).
    let mut at_sentence_start = true;
    for i in 0..words.len() {
        if let Some(upper) = word_start_upper(&words[i]) {
            if unlocked_capitals.contains(&upper) {
                let should_cap = if at_sentence_start {
                    true
                } else if focused_upper == Some(upper) {
                    rng.gen_bool(0.55)
                } else {
                    rng.gen_bool(0.22)
                };
                if should_cap {
                    capitalize_word_start(&mut words[i]);
                }
            }
        }
        at_sentence_start = ends_sentence(&words[i]);
    }

    // Occasional mid-word capitals are injected as camelCase joins only.
    // This keeps internal capitals realistic for code contexts.
    let mut i = 0;
    while i + 1 < words.len() {
        if ends_sentence(&words[i]) {
            i += 1;
            continue;
        }
        let next_upper = match word_start_upper(&words[i + 1]) {
            Some(upper) if unlocked_capitals.contains(&upper) => upper,
            _ => {
                i += 1;
                continue;
            }
        };
        let prob = if focused_upper == Some(next_upper) {
            0.35
        } else {
            0.09
        };
        if rng.gen_bool(prob) {
            capitalize_word_start(&mut words[i + 1]);
            let next = words.remove(i + 1);
            words[i].push_str(&next);
        } else {
            i += 1;
        }
    }

    // Focused capitals should show up multiple times for focused drills.
    if let Some(focused_upper) = focused_upper.filter(|ch| unlocked_capitals.contains(ch)) {
        let alpha_words = words
            .iter()
            .filter(|w| w.chars().any(|ch| ch.is_ascii_alphabetic()))
            .count();
        let min_focused = alpha_words.min(4);
        ensure_min_focused_occurrences(&mut words, focused_upper, min_focused);
    }

    // Keep a baseline capital density so branch/global drills with capitals
    // unlocked do not feel too sparse.
    let min_total_caps = words.len().clamp(3, 6) / 2; // ~3 for 6+ words
    ensure_min_total_capitals(&mut words, unlocked_capitals, min_total_caps, rng);

    words.join(" ")
}

fn word_start_upper(word: &str) -> Option<char> {
    word.chars()
        .find(|ch| ch.is_ascii_alphabetic())
        .map(|ch| ch.to_ascii_uppercase())
}

fn capitalize_word_start(word: &mut String) -> Option<char> {
    let mut chars: Vec<char> = word.chars().collect();
    for i in 0..chars.len() {
        if chars[i].is_ascii_lowercase() {
            chars[i] = chars[i].to_ascii_uppercase();
            let upper = chars[i];
            *word = chars.into_iter().collect();
            return Some(upper);
        }
        if chars[i].is_ascii_uppercase() {
            return Some(chars[i]);
        }
    }
    None
}

fn ends_sentence(word: &str) -> bool {
    word.chars()
        .rev()
        .find(|ch| !ch.is_ascii_whitespace())
        .is_some_and(|ch| matches!(ch, '.' | '?' | '!'))
}

fn word_starts_with_lower(word: &str, lower: char) -> bool {
    word.chars()
        .find(|ch| ch.is_ascii_alphabetic())
        .is_some_and(|ch| ch == lower)
}

fn force_word_start_to_upper(word: &mut String, upper: char) -> bool {
    let mut chars: Vec<char> = word.chars().collect();
    for i in 0..chars.len() {
        if chars[i].is_ascii_alphabetic() {
            if chars[i] == upper {
                return false;
            }
            chars[i] = upper;
            *word = chars.into_iter().collect();
            return true;
        }
    }
    false
}

fn ensure_min_focused_occurrences(words: &mut Vec<String>, focused_upper: char, min_count: usize) {
    let focused_lower = focused_upper.to_ascii_lowercase();
    let mut count = words
        .iter()
        .map(|w| w.chars().filter(|&ch| ch == focused_upper).count())
        .sum::<usize>();

    if count >= min_count {
        return;
    }

    // First, capitalize focused matching word starts.
    for word in words.iter_mut() {
        if count >= min_count {
            break;
        }
        if !word_starts_with_lower(word, focused_lower) {
            continue;
        }
        if capitalize_word_start(word) == Some(focused_upper) {
            count += 1;
        }
    }

    // If still short, create camelCase joins where the second word starts
    // with the focused letter.
    let mut i = 0;
    while i + 1 < words.len() {
        if count >= min_count {
            break;
        }
        if ends_sentence(&words[i]) {
            i += 1;
            continue;
        }
        let next_starts_focused = words[i + 1]
            .chars()
            .find(|ch| ch.is_ascii_alphabetic())
            .is_some_and(|ch| ch.eq_ignore_ascii_case(&focused_lower));
        if next_starts_focused {
            capitalize_word_start(&mut words[i + 1]);
            let next = words.remove(i + 1);
            words[i].push_str(&next);
            count += 1;
        } else {
            i += 1;
        }
    }

    // Last resort: force focused uppercase at word starts.
    for word in words.iter_mut() {
        if count >= min_count {
            break;
        }
        if force_word_start_to_upper(word, focused_upper) {
            count += 1;
        }
    }
}

fn ensure_min_total_capitals(
    words: &mut [String],
    unlocked_capitals: &[char],
    min_count: usize,
    rng: &mut SmallRng,
) {
    let mut count = words
        .iter()
        .map(|w| w.chars().filter(|ch| ch.is_ascii_uppercase()).count())
        .sum::<usize>();
    if count >= min_count || unlocked_capitals.is_empty() {
        return;
    }

    // Prefer natural capitalization when the word already starts with an unlocked letter.
    for word in words.iter_mut() {
        if count >= min_count {
            break;
        }
        let Some(upper) = word_start_upper(word) else {
            continue;
        };
        if unlocked_capitals.contains(&upper)
            && word_starts_with_lower(word, upper.to_ascii_lowercase())
        {
            if capitalize_word_start(word) == Some(upper) {
                count += 1;
            }
        }
    }

    // If still short, force additional capitalized starts from unlocked set.
    for word in words.iter_mut() {
        if count >= min_count {
            break;
        }
        let upper = unlocked_capitals[rng.gen_range(0..unlocked_capitals.len())];
        if force_word_start_to_upper(word, upper) {
            count += 1;
        }
    }
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

    #[test]
    fn test_no_interior_focus_caps_without_word_start_or_camel_case_opportunity() {
        let mut rng = SmallRng::seed_from_u64(7);
        let text = "awful claw draw";
        let result = apply_capitalization(text, &['W'], Some('W'), &mut rng);
        assert!(result.starts_with('W') || result.contains(" W"));
        assert!(
            !result.contains("aW"),
            "Should avoid interior non-camel W: {result}"
        );
    }

    #[test]
    fn test_focused_capital_forced_to_multiple_occurrences() {
        let mut rng = SmallRng::seed_from_u64(11);
        let text = "alpha beta gamma delta epsilon zeta eta theta iota";
        let result = apply_capitalization(text, &['Q'], Some('Q'), &mut rng);
        let focused_count = result.chars().filter(|&ch| ch == 'Q').count();
        assert!(
            focused_count >= 4,
            "Expected forced focused Q occurrences, got {focused_count} in: {result}"
        );
    }
}
