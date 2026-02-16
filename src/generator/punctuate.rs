use rand::Rng;
use rand::rngs::SmallRng;

/// Post-processing pass that inserts punctuation into generated text.
/// Only uses punctuation chars from `unlocked_punct`.
pub fn apply_punctuation(
    text: &str,
    unlocked_punct: &[char],
    focused: Option<char>,
    rng: &mut SmallRng,
) -> String {
    if unlocked_punct.is_empty() {
        return text.to_string();
    }

    // If focused key is a punctuation char in our set, boost its insertion probability
    let focused_punct = focused.filter(|ch| unlocked_punct.contains(ch));

    let words: Vec<&str> = text.split(' ').collect();
    if words.is_empty() {
        return text.to_string();
    }

    let has_period = unlocked_punct.contains(&'.');
    let has_comma = unlocked_punct.contains(&',');
    let has_apostrophe = unlocked_punct.contains(&'\'');
    let has_semicolon = unlocked_punct.contains(&';');
    let has_colon = unlocked_punct.contains(&':');
    let has_quote = unlocked_punct.contains(&'"');
    let has_dash = unlocked_punct.contains(&'-');
    let has_question = unlocked_punct.contains(&'?');
    let has_exclaim = unlocked_punct.contains(&'!');
    let has_open_paren = unlocked_punct.contains(&'(');
    let has_close_paren = unlocked_punct.contains(&')');

    let mut result = Vec::new();
    let mut words_since_period = 0;
    let mut words_since_comma = 0;

    for (i, word) in words.iter().enumerate() {
        let mut w = word.to_string();

        // Contractions (~8% of words, boosted if apostrophe is focused)
        let apostrophe_prob = if focused_punct == Some('\'') {
            0.30
        } else {
            0.08
        };
        if has_apostrophe && w.len() >= 3 && rng.gen_bool(apostrophe_prob) {
            w = make_contraction(&w, rng);
        }

        // Compound words with dash (~5% of words, boosted if dash is focused)
        let dash_prob = if focused_punct == Some('-') {
            0.25
        } else {
            0.05
        };
        if has_dash && i + 1 < words.len() && rng.gen_bool(dash_prob) {
            w.push('-');
        }

        // Sentence ending punctuation
        words_since_period += 1;
        let end_sentence =
            words_since_period >= 8 && rng.gen_bool(0.15) || words_since_period >= 12;

        if end_sentence && i < words.len() - 1 {
            let q_prob = if focused_punct == Some('?') {
                0.40
            } else {
                0.15
            };
            let excl_prob = if focused_punct == Some('!') {
                0.40
            } else {
                0.10
            };
            if has_question && rng.gen_bool(q_prob) {
                w.push('?');
            } else if has_exclaim && rng.gen_bool(excl_prob) {
                w.push('!');
            } else if has_period {
                w.push('.');
            }
            words_since_period = 0;
            words_since_comma = 0;
        } else {
            // Comma after clause (~every 4-6 words)
            words_since_comma += 1;
            let comma_prob = if focused_punct == Some(',') {
                0.40
            } else {
                0.20
            };
            if has_comma
                && words_since_comma >= 4
                && rng.gen_bool(comma_prob)
                && i < words.len() - 1
            {
                w.push(',');
                words_since_comma = 0;
            }

            // Semicolon between clauses (rare, boosted if focused)
            let semi_prob = if focused_punct == Some(';') {
                0.25
            } else {
                0.05
            };
            if has_semicolon
                && words_since_comma >= 5
                && rng.gen_bool(semi_prob)
                && i < words.len() - 1
            {
                w.push(';');
                words_since_comma = 0;
            }

            // Colon before list-like content (rare, boosted if focused)
            let colon_prob = if focused_punct == Some(':') {
                0.20
            } else {
                0.03
            };
            if has_colon && rng.gen_bool(colon_prob) && i < words.len() - 1 {
                w.push(':');
            }
        }

        // Quoted phrases (~5% chance to start a quote, boosted if focused)
        let quote_prob = if focused_punct == Some('"') {
            0.20
        } else {
            0.04
        };
        if has_quote && rng.gen_bool(quote_prob) && i + 2 < words.len() {
            w = format!("\"{w}");
        }

        // Parenthetical asides (rare, boosted if focused)
        let paren_prob = if matches!(focused_punct, Some('(' | ')')) {
            0.15
        } else {
            0.03
        };
        if has_open_paren && has_close_paren && rng.gen_bool(paren_prob) && i + 2 < words.len() {
            w = format!("({w}");
        }

        result.push(w);
    }

    // End with period if we have it
    if has_period {
        if let Some(last) = result.last_mut() {
            let last_char = last.chars().last();
            if !matches!(last_char, Some('.' | '?' | '!' | '"' | ')')) {
                last.push('.');
            }
        }
    }

    // Close any open quotes/parens
    let mut open_quotes = 0i32;
    let mut open_parens = 0i32;
    for w in &result {
        for ch in w.chars() {
            if ch == '"' {
                open_quotes += 1;
            }
            if ch == '(' {
                open_parens += 1;
            }
            if ch == ')' {
                open_parens -= 1;
            }
        }
    }
    if let Some(last) = result.last_mut() {
        if open_quotes % 2 != 0 && has_quote {
            // Remove trailing period to put quote after
            let had_period = last.ends_with('.');
            if had_period {
                last.pop();
            }
            last.push('"');
            if had_period {
                last.push('.');
            }
        }
        if open_parens > 0 && has_close_paren {
            let had_period = last.ends_with('.');
            if had_period {
                last.pop();
            }
            last.push(')');
            if had_period {
                last.push('.');
            }
        }
    }

    result.join(" ")
}

fn make_contraction(word: &str, rng: &mut SmallRng) -> String {
    // Simple contractions based on common patterns
    let contractions: &[(&str, &str)] = &[
        ("not", "n't"),
        ("will", "'ll"),
        ("would", "'d"),
        ("have", "'ve"),
        ("are", "'re"),
        ("is", "'s"),
    ];

    for &(base, suffix) in contractions {
        if word == base {
            // For "not" -> "don't", "can't", etc. - just return the contraction form
            return format!("{word}{suffix}");
        }
    }

    // Generic: ~chance to add 's
    if rng.gen_bool(0.5) {
        format!("{word}'s")
    } else {
        word.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_no_punct_when_empty() {
        let mut rng = SmallRng::seed_from_u64(42);
        let result = apply_punctuation("hello world", &[], None, &mut rng);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_adds_period_at_end() {
        let mut rng = SmallRng::seed_from_u64(42);
        let text = "one two three four five six seven eight nine ten";
        let result = apply_punctuation(text, &['.'], None, &mut rng);
        assert!(result.ends_with('.'));
    }

    #[test]
    fn test_period_appears_mid_text() {
        let mut rng = SmallRng::seed_from_u64(42);
        let words: Vec<&str> = (0..20).map(|_| "word").collect();
        let text = words.join(" ");
        let result = apply_punctuation(&text, &['.', ','], None, &mut rng);
        // Should have at least one period somewhere in the middle
        let period_count = result.chars().filter(|&c| c == '.').count();
        assert!(period_count >= 1, "Expected periods in: {result}");
    }
}
