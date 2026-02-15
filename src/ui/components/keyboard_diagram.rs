use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Widget};

use crate::keyboard::finger::{self, Finger, Hand};
use crate::ui::theme::Theme;

pub struct KeyboardDiagram<'a> {
    pub focused_key: Option<char>,
    pub next_key: Option<char>,
    pub unlocked_keys: &'a [char],
    pub depressed_keys: &'a HashSet<char>,
    pub theme: &'a Theme,
    pub compact: bool,
}

impl<'a> KeyboardDiagram<'a> {
    pub fn new(
        focused_key: Option<char>,
        next_key: Option<char>,
        unlocked_keys: &'a [char],
        depressed_keys: &'a HashSet<char>,
        theme: &'a Theme,
    ) -> Self {
        Self {
            focused_key,
            next_key,
            unlocked_keys,
            depressed_keys,
            theme,
            compact: false,
        }
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

const ROWS: &[&[char]] = &[
    &['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
    &['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
    &['z', 'x', 'c', 'v', 'b', 'n', 'm'],
];

fn finger_color(ch: char) -> Color {
    let assignment = finger::qwerty_finger(ch);
    match (assignment.hand, assignment.finger) {
        (Hand::Left, Finger::Pinky) => Color::Rgb(180, 100, 100),
        (Hand::Left, Finger::Ring) => Color::Rgb(180, 140, 80),
        (Hand::Left, Finger::Middle) => Color::Rgb(120, 160, 80),
        (Hand::Left, Finger::Index) => Color::Rgb(80, 140, 180),
        (Hand::Right, Finger::Index) => Color::Rgb(100, 140, 200),
        (Hand::Right, Finger::Middle) => Color::Rgb(120, 160, 80),
        (Hand::Right, Finger::Ring) => Color::Rgb(180, 140, 80),
        (Hand::Right, Finger::Pinky) => Color::Rgb(180, 100, 100),
        _ => Color::Rgb(120, 120, 120),
    }
}

fn brighten_color(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            r.saturating_add(60),
            g.saturating_add(60),
            b.saturating_add(60),
        ),
        other => other,
    }
}

impl Widget for KeyboardDiagram<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Keyboard ")
            .border_style(Style::default().fg(colors.border()))
            .style(Style::default().bg(colors.bg()));
        let inner = block.inner(area);
        block.render(area, buf);

        let key_width: u16 = if self.compact { 3 } else { 5 };
        let min_width: u16 = if self.compact { 21 } else { 30 };

        if inner.height < 3 || inner.width < min_width {
            return;
        }

        let offsets: &[u16] = if self.compact {
            &[0, 1, 3]
        } else {
            &[1, 3, 5]
        };

        for (row_idx, row) in ROWS.iter().enumerate() {
            let y = inner.y + row_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let offset = offsets.get(row_idx).copied().unwrap_or(0);

            for (col_idx, &key) in row.iter().enumerate() {
                let x = inner.x + offset + col_idx as u16 * key_width;
                if x + key_width > inner.x + inner.width {
                    break;
                }

                let is_depressed = self.depressed_keys.contains(&key);
                let is_unlocked = self.unlocked_keys.contains(&key);
                let is_focused = self.focused_key == Some(key);
                let is_next = self.next_key == Some(key);

                // Priority: depressed > next_expected > focused > unlocked > locked
                let style = if is_depressed {
                    let bg = if is_unlocked {
                        brighten_color(finger_color(key))
                    } else {
                        brighten_color(colors.accent_dim())
                    };
                    Style::default()
                        .fg(Color::White)
                        .bg(bg)
                        .add_modifier(Modifier::BOLD)
                } else if is_next {
                    Style::default()
                        .fg(colors.bg())
                        .bg(colors.accent())
                } else if is_focused {
                    Style::default()
                        .fg(colors.bg())
                        .bg(colors.focused_key())
                } else if is_unlocked {
                    Style::default()
                        .fg(colors.fg())
                        .bg(finger_color(key))
                } else {
                    Style::default()
                        .fg(colors.text_pending())
                        .bg(colors.bg())
                };

                let display = if self.compact {
                    format!("[{key}]")
                } else {
                    format!("[ {key} ]")
                };
                buf.set_string(x, y, &display, style);
            }
        }
    }
}
