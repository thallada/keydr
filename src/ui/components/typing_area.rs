use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use crate::session::drill::DrillState;
use crate::session::input::CharStatus;
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

fn color_to_rgb(color: ratatui::style::Color) -> (u8, u8, u8) {
    use ratatui::style::Color;
    match color {
        Color::Reset => (0, 0, 0),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 49, 49),
        Color::Green => (13, 188, 121),
        Color::Yellow => (229, 229, 16),
        Color::Blue => (36, 114, 200),
        Color::Magenta => (188, 63, 188),
        Color::Cyan => (17, 168, 205),
        Color::Gray => (229, 229, 229),
        Color::DarkGray => (102, 102, 102),
        Color::LightRed => (241, 76, 76),
        Color::LightGreen => (35, 209, 139),
        Color::LightYellow => (245, 245, 67),
        Color::LightBlue => (59, 142, 234),
        Color::LightMagenta => (214, 112, 214),
        Color::LightCyan => (41, 184, 219),
        Color::White => (255, 255, 255),
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(i) => {
            if i < 16 {
                const ANSI16: [(u8, u8, u8); 16] = [
                    (0, 0, 0),
                    (128, 0, 0),
                    (0, 128, 0),
                    (128, 128, 0),
                    (0, 0, 128),
                    (128, 0, 128),
                    (0, 128, 128),
                    (192, 192, 192),
                    (128, 128, 128),
                    (255, 0, 0),
                    (0, 255, 0),
                    (255, 255, 0),
                    (0, 0, 255),
                    (255, 0, 255),
                    (0, 255, 255),
                    (255, 255, 255),
                ];
                ANSI16[i as usize]
            } else if i <= 231 {
                let idx = i - 16;
                let r = idx / 36;
                let g = (idx % 36) / 6;
                let b = idx % 6;
                let cv = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
                (cv(r), cv(g), cv(b))
            } else {
                let v = 8 + (i - 232) * 10;
                (v, v, v)
            }
        }
    }
}

fn relative_luminance(color: ratatui::style::Color) -> f64 {
    let (r, g, b) = color_to_rgb(color);
    let to_linear = |c: u8| {
        let x = c as f64 / 255.0;
        if x <= 0.03928 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
}

fn contrast_ratio(a: ratatui::style::Color, b: ratatui::style::Color) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn choose_cursor_colors(colors: &crate::ui::theme::ThemeColors) -> (ratatui::style::Color, ratatui::style::Color) {
    use ratatui::style::Color;

    let base_bg = colors.bg();
    let mut cursor_bg = colors.text_cursor_bg();

    // Ensure cursor block stands out from the typing area's background.
    if contrast_ratio(cursor_bg, base_bg) < 1.8 {
        let mut best_bg = cursor_bg;
        let mut best_ratio = contrast_ratio(cursor_bg, base_bg);
        for candidate in [colors.accent(), colors.focused_key(), colors.warning(), Color::Black, Color::White] {
            let ratio = contrast_ratio(candidate, base_bg);
            if ratio > best_ratio {
                best_bg = candidate;
                best_ratio = ratio;
            }
        }
        cursor_bg = best_bg;
    }

    // Pick most readable fg on top of chosen cursor background.
    let mut cursor_fg = colors.text_cursor_fg();
    let mut best_ratio = contrast_ratio(cursor_fg, cursor_bg);
    for candidate in [colors.fg(), colors.bg(), Color::Black, Color::White] {
        let ratio = contrast_ratio(candidate, cursor_bg);
        if ratio > best_ratio {
            cursor_fg = candidate;
            best_ratio = ratio;
        }
    }

    (cursor_fg, cursor_bg)
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
        let (cursor_fg, cursor_bg) = choose_cursor_colors(colors);
        let tokens = build_render_tokens(&self.drill.target);

        // Group tokens into lines, splitting on line_break tokens
        let mut lines: Vec<Vec<Span>> = vec![Vec::new()];

        for token in &tokens {
            let idx = token.target_idx;
            let target_ch = self.drill.target[idx];

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
                    .fg(cursor_fg)
                    .bg(cursor_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.text_pending())
            };

            // For incorrect chars, show the actual typed char for regular chars,
            // but always show the token display for whitespace markers
            let display = if idx < self.drill.cursor {
                if let CharStatus::Incorrect(actual) = &self.drill.input[idx] {
                    if target_ch == '\n' || target_ch == '\t' {
                        // Show the whitespace marker even when incorrect
                        token.display.clone()
                    } else {
                        actual.to_string()
                    }
                } else {
                    token.display.clone()
                }
            } else if idx == self.drill.cursor && target_ch == ' ' {
                // Keep an actual space at cursor position so soft-wrap break opportunities
                // remain stable at word boundaries.
                " ".to_string()
            } else {
                token.display.clone()
            };

            lines.last_mut().unwrap().push(Span::styled(display, style));

            if token.is_line_break {
                lines.push(Vec::new());
            }
        }

        // Keep cursor visible at end-of-input as an insertion marker.
        if self.drill.cursor >= self.drill.target.len() {
            lines.last_mut().unwrap().push(Span::styled(
                "\u{258f}",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            ));
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
