use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use crate::session::input::CharStatus;
use crate::session::drill::DrillState;
use crate::ui::theme::Theme;

pub struct TypingArea<'a> {
    drill: &'a DrillState,
    theme: &'a Theme,
}

impl<'a> TypingArea<'a> {
    pub fn new(drill: &'a DrillState, theme: &'a Theme) -> Self {
        Self { drill, theme }
    }
}

/// A render token maps a single target character to its display representation.
struct RenderToken {
    target_idx: usize,
    display: String,
    is_line_break: bool,
}

/// Expand target chars into render tokens, handling whitespace display.
fn build_render_tokens(target: &[char]) -> Vec<RenderToken> {
    let mut tokens = Vec::new();
    let mut col = 0usize;

    for (i, &ch) in target.iter().enumerate() {
        match ch {
            '\n' => {
                tokens.push(RenderToken {
                    target_idx: i,
                    display: "\u{21b5}".to_string(), // ↵
                    is_line_break: true,
                });
                col = 0;
            }
            '\t' => {
                let tab_width = 4 - (col % 4);
                let mut display = String::from("\u{2192}"); // →
                for _ in 1..tab_width {
                    display.push('\u{00b7}'); // ·
                }
                tokens.push(RenderToken {
                    target_idx: i,
                    display,
                    is_line_break: false,
                });
                col += tab_width;
            }
            _ => {
                tokens.push(RenderToken {
                    target_idx: i,
                    display: ch.to_string(),
                    is_line_break: false,
                });
                col += 1;
            }
        }
    }

    tokens
}

impl Widget for TypingArea<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;
        let tokens = build_render_tokens(&self.drill.target);

        // Group tokens into lines, splitting on line_break tokens
        let mut lines: Vec<Vec<Span>> = vec![Vec::new()];

        for token in &tokens {
            let idx = token.target_idx;

            let style = if idx < self.drill.cursor {
                match &self.drill.input[idx] {
                    CharStatus::Correct => Style::default().fg(colors.text_correct()),
                    CharStatus::Incorrect(_) => Style::default()
                        .fg(colors.text_incorrect())
                        .bg(colors.text_incorrect_bg())
                        .add_modifier(Modifier::UNDERLINED),
                }
            } else if idx == self.drill.cursor {
                Style::default()
                    .fg(colors.text_cursor_fg())
                    .bg(colors.text_cursor_bg())
            } else {
                Style::default().fg(colors.text_pending())
            };

            // For incorrect chars, show the actual typed char for regular chars,
            // but always show the token display for whitespace markers
            let display = if idx < self.drill.cursor {
                if let CharStatus::Incorrect(actual) = &self.drill.input[idx] {
                    let target_ch = self.drill.target[idx];
                    if target_ch == '\n' || target_ch == '\t' {
                        // Show the whitespace marker even when incorrect
                        token.display.clone()
                    } else {
                        actual.to_string()
                    }
                } else {
                    token.display.clone()
                }
            } else {
                token.display.clone()
            };

            lines.last_mut().unwrap().push(Span::styled(display, style));

            if token.is_line_break {
                lines.push(Vec::new());
            }
        }

        let ratatui_lines: Vec<Line> = lines.into_iter().map(Line::from).collect();

        let block = Block::bordered()
            .border_style(Style::default().fg(colors.border()))
            .style(Style::default().bg(colors.bg()));

        let paragraph = Paragraph::new(ratatui_lines)
            .block(block)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_tokens_basic() {
        let target: Vec<char> = "abc".chars().collect();
        let tokens = build_render_tokens(&target);
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].display, "a");
        assert_eq!(tokens[1].display, "b");
        assert_eq!(tokens[2].display, "c");
        assert!(!tokens[0].is_line_break);
    }

    #[test]
    fn test_render_tokens_newline() {
        let target: Vec<char> = "a\nb".chars().collect();
        let tokens = build_render_tokens(&target);
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[1].display, "\u{21b5}"); // ↵
        assert!(tokens[1].is_line_break);
        assert_eq!(tokens[1].target_idx, 1);
    }

    #[test]
    fn test_render_tokens_tab() {
        let target: Vec<char> = "\tx".chars().collect();
        let tokens = build_render_tokens(&target);
        assert_eq!(tokens.len(), 2);
        // Tab at col 0: width = 4 - (0 % 4) = 4 => "→···"
        assert_eq!(tokens[0].display, "\u{2192}\u{00b7}\u{00b7}\u{00b7}");
        assert!(!tokens[0].is_line_break);
        assert_eq!(tokens[0].target_idx, 0);
    }

    #[test]
    fn test_render_tokens_tab_alignment() {
        // "ab\t" -> col 2, tab_width = 4 - (2 % 4) = 2 => "→·"
        let target: Vec<char> = "ab\t".chars().collect();
        let tokens = build_render_tokens(&target);
        assert_eq!(tokens[2].display, "\u{2192}\u{00b7}");
    }

    #[test]
    fn test_render_tokens_newline_resets_column() {
        // "\n\tx" -> after newline, col resets to 0, tab_width = 4
        let target: Vec<char> = "\n\tx".chars().collect();
        let tokens = build_render_tokens(&target);
        assert_eq!(tokens.len(), 3);
        assert!(tokens[0].is_line_break);
        assert_eq!(tokens[1].display, "\u{2192}\u{00b7}\u{00b7}\u{00b7}");
    }
}
