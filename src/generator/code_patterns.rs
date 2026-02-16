use rand::Rng;
use rand::rngs::SmallRng;

/// Post-processing pass that inserts code-like expressions into text.
/// Only uses symbols from `unlocked_symbols`.
pub fn apply_code_symbols(
    text: &str,
    unlocked_symbols: &[char],
    focused: Option<char>,
    rng: &mut SmallRng,
) -> String {
    if unlocked_symbols.is_empty() {
        return text.to_string();
    }

    // If focused key is a code symbol, boost insertion probability
    let focused_symbol = focused.filter(|ch| unlocked_symbols.contains(ch));
    let base_prob = if focused_symbol.is_some() { 0.35 } else { 0.20 };

    let words: Vec<&str> = text.split(' ').collect();
    let mut result = Vec::new();

    for word in &words {
        if rng.gen_bool(base_prob) {
            let expr = generate_code_expr(word, unlocked_symbols, focused_symbol, rng);
            result.push(expr);
        } else {
            result.push(word.to_string());
        }
    }

    result.join(" ")
}

fn generate_code_expr(
    word: &str,
    symbols: &[char],
    focused_symbol: Option<char>,
    rng: &mut SmallRng,
) -> String {
    // Categorize available symbols
    let has = |ch: char| symbols.contains(&ch);

    // Try various patterns based on available symbols
    let mut patterns: Vec<Box<dyn Fn(&mut SmallRng) -> String>> = Vec::new();
    // Track which patterns use the focused symbol for priority selection
    let mut focused_patterns: Vec<usize> = Vec::new();

    // Arithmetic & Assignment patterns
    if has('=') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{w} = val")));
        if focused_symbol == Some('=') {
            focused_patterns.push(idx);
        }
    }
    if has('+') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{w} + num")));
        if focused_symbol == Some('+') {
            focused_patterns.push(idx);
        }
    }
    if has('*') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{w} * cnt")));
        if focused_symbol == Some('*') {
            focused_patterns.push(idx);
        }
    }
    if has('/') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{w} / max")));
        if focused_symbol == Some('/') {
            focused_patterns.push(idx);
        }
    }
    if has('-') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{w} - one")));
        if focused_symbol == Some('-') {
            focused_patterns.push(idx);
        }
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("-{w}")));
        if focused_symbol == Some('-') {
            focused_patterns.push(idx);
        }
    }
    if has('=') && has('+') {
        let w = word.to_string();
        patterns.push(Box::new(move |_| format!("{w} += one")));
    }
    if has('=') && has('-') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{w} -= one")));
        if focused_symbol == Some('-') {
            focused_patterns.push(idx);
        }
    }
    if has('=') && has('=') {
        let w = word.to_string();
        patterns.push(Box::new(move |_| format!("{w} == nil")));
    }

    // Grouping patterns
    if has('{') && has('}') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{{ {w} }}")));
        if matches!(focused_symbol, Some('{') | Some('}')) {
            focused_patterns.push(idx);
        }
    }
    if has('[') && has(']') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{w}[idx]")));
        if matches!(focused_symbol, Some('[') | Some(']')) {
            focused_patterns.push(idx);
        }
    }
    if has('<') && has('>') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("Vec<{w}>")));
        if matches!(focused_symbol, Some('<') | Some('>')) {
            focused_patterns.push(idx);
        }
    }

    // Logic patterns
    if has('&') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("&{w}")));
        if focused_symbol == Some('&') {
            focused_patterns.push(idx);
        }
    }
    if has('|') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{w} | nil")));
        if focused_symbol == Some('|') {
            focused_patterns.push(idx);
        }
    }
    if has('!') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("!{w}")));
        if focused_symbol == Some('!') {
            focused_patterns.push(idx);
        }
    }

    // Special patterns
    if has('@') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("@{w}")));
        if focused_symbol == Some('@') {
            focused_patterns.push(idx);
        }
    }
    if has('#') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("#{w}")));
        if focused_symbol == Some('#') {
            focused_patterns.push(idx);
        }
    }
    if has('_') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("{w}_val")));
        if focused_symbol == Some('_') {
            focused_patterns.push(idx);
        }
    }
    if has('$') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("${w}")));
        if focused_symbol == Some('$') {
            focused_patterns.push(idx);
        }
    }
    if has('\\') {
        let w = word.to_string();
        let idx = patterns.len();
        patterns.push(Box::new(move |_| format!("\\{w}")));
        if focused_symbol == Some('\\') {
            focused_patterns.push(idx);
        }
    }

    if patterns.is_empty() {
        return word.to_string();
    }

    // 50% chance to prefer a pattern that uses the focused symbol
    let idx = if !focused_patterns.is_empty() && rng.gen_bool(0.50) {
        focused_patterns[rng.gen_range(0..focused_patterns.len())]
    } else {
        rng.gen_range(0..patterns.len())
    };
    patterns[idx](rng)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn test_no_symbols_when_empty() {
        let mut rng = SmallRng::seed_from_u64(42);
        let result = apply_code_symbols("hello world", &[], None, &mut rng);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_uses_only_unlocked_symbols() {
        let mut rng = SmallRng::seed_from_u64(42);
        let symbols = ['=', '+'];
        let text = "a b c d e f g h i j";
        let result = apply_code_symbols(text, &symbols, None, &mut rng);
        for ch in result.chars() {
            if !ch.is_alphanumeric() && ch != ' ' {
                assert!(
                    symbols.contains(&ch),
                    "Unexpected symbol '{ch}' in: {result}"
                );
            }
        }
    }

    #[test]
    fn test_dash_patterns_generated() {
        let mut rng = SmallRng::seed_from_u64(42);
        let symbols = ['-', '='];
        let text = "a b c d e f g h i j k l m n o p q r s t";
        let result = apply_code_symbols(text, &symbols, None, &mut rng);
        assert!(result.contains('-'), "Expected dash in: {result}");
    }
}
