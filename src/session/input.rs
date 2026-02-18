use std::time::Instant;

use crate::session::drill::{DrillState, SyntheticSpan};

#[derive(Clone, Debug)]
pub enum CharStatus {
    Correct,
    Incorrect(char),
}

#[derive(Clone, Debug)]
pub struct KeystrokeEvent {
    pub expected: char,
    #[allow(dead_code)]
    pub actual: char,
    pub timestamp: Instant,
    pub correct: bool,
}

pub fn process_char(drill: &mut DrillState, ch: char) -> Option<KeystrokeEvent> {
    if drill.is_complete() {
        return None;
    }

    if drill.started_at.is_none() {
        drill.started_at = Some(Instant::now());
    }

    let expected = drill.target[drill.cursor];
    let correct = ch == expected;

    let event = KeystrokeEvent {
        expected,
        actual: ch,
        timestamp: Instant::now(),
        correct,
    };

    if correct {
        drill.input.push(CharStatus::Correct);
        drill.cursor += 1;
    } else if ch == '\n' {
        apply_newline_span(drill, ch);
    } else if ch == '\t' {
        apply_tab_span(drill, ch);
    } else {
        drill.input.push(CharStatus::Incorrect(ch));
        drill.typo_flags.insert(drill.cursor);
        drill.cursor += 1;
    }

    if drill.is_complete() {
        drill.finished_at = Some(Instant::now());
    }

    Some(event)
}

pub fn process_backspace(drill: &mut DrillState) {
    if drill.cursor == 0 {
        return;
    }

    if let Some(span) = drill
        .synthetic_spans
        .last()
        .copied()
        .filter(|s| s.end == drill.cursor)
    {
        let span_len = span.end.saturating_sub(span.start);
        if span_len > 0 {
            let has_chained_prev = drill
                .synthetic_spans
                .iter()
                .rev()
                .nth(1)
                .is_some_and(|prev| prev.end == span.start);
            let new_len = drill.input.len().saturating_sub(span_len);
            drill.input.truncate(new_len);
            drill.cursor = span.start;
            for pos in span.start..span.end {
                drill.typo_flags.remove(&pos);
            }
            if !has_chained_prev {
                drill.typo_flags.insert(span.start);
            }
            drill.synthetic_spans.pop();
            return;
        }
    }

    drill.cursor -= 1;
    drill.input.pop();
}

fn apply_newline_span(drill: &mut DrillState, typed: char) {
    let start = drill.cursor;
    let line_end = drill.target[start..]
        .iter()
        .position(|&c| c == '\n')
        .map(|offset| start + offset + 1)
        .unwrap_or(drill.target.len());
    let end = line_end.max(start + 1).min(drill.target.len());
    apply_synthetic_span(drill, start, end, typed, None);
}

fn apply_tab_span(drill: &mut DrillState, typed: char) {
    let start = drill.cursor;
    let line_end = drill.target[start..]
        .iter()
        .position(|&c| c == '\n')
        .map(|offset| start + offset)
        .unwrap_or(drill.target.len());
    let mut end = (start + 4).min(line_end);
    if end <= start {
        end = (start + 1).min(drill.target.len());
    }
    let first_actual = drill.target.get(start).copied();
    apply_synthetic_span(drill, start, end, typed, first_actual);
}

fn apply_synthetic_span(
    drill: &mut DrillState,
    start: usize,
    end: usize,
    typed: char,
    first_actual: Option<char>,
) {
    if start >= end || start >= drill.target.len() {
        drill.input.push(CharStatus::Incorrect(typed));
        drill.typo_flags.insert(drill.cursor);
        drill.cursor += 1;
        return;
    }

    for idx in start..end {
        let actual = if idx == start {
            first_actual.unwrap_or(typed)
        } else {
            drill.target[idx]
        };
        drill.input.push(CharStatus::Incorrect(actual));
        drill.typo_flags.insert(idx);
    }
    drill.cursor = end;
    drill.synthetic_spans.push(SyntheticSpan { start, end });
}
