use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Widget};

use crate::keyboard::finger::{Finger, Hand};
use crate::keyboard::model::KeyboardModel;
use crate::ui::theme::Theme;

pub struct KeyboardDiagram<'a> {
    pub focused_key: Option<char>,
    pub next_key: Option<char>,
    pub unlocked_keys: &'a [char],
    pub depressed_keys: &'a HashSet<char>,
    pub theme: &'a Theme,
    pub compact: bool,
    pub model: &'a KeyboardModel,
    pub shift_held: bool,
}

impl<'a> KeyboardDiagram<'a> {
    pub fn new(
        focused_key: Option<char>,
        next_key: Option<char>,
        unlocked_keys: &'a [char],
        depressed_keys: &'a HashSet<char>,
        theme: &'a Theme,
        model: &'a KeyboardModel,
    ) -> Self {
        Self {
            focused_key,
            next_key,
            unlocked_keys,
            depressed_keys,
            theme,
            compact: false,
            model,
            shift_held: false,
        }
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn shift_held(mut self, shift_held: bool) -> Self {
        self.shift_held = shift_held;
        self
    }
}

fn finger_color(model: &KeyboardModel, ch: char) -> Color {
    let assignment = model.finger_for_char(ch);
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

        if self.compact {
            // Compact mode: letter rows only (rows 1-3 of the model)
            let letter_rows = self.model.letter_rows();
            let key_width: u16 = 3;
            let min_width: u16 = 21;

            if inner.height < 3 || inner.width < min_width {
                return;
            }

            let offsets: &[u16] = &[0, 1, 3];

            for (row_idx, row) in letter_rows.iter().enumerate() {
                let y = inner.y + row_idx as u16;
                if y >= inner.y + inner.height {
                    break;
                }

                let offset = offsets.get(row_idx).copied().unwrap_or(0);

                for (col_idx, physical_key) in row.iter().enumerate() {
                    let x = inner.x + offset + col_idx as u16 * key_width;
                    if x + key_width > inner.x + inner.width {
                        break;
                    }

                    let display_char = if self.shift_held {
                        physical_key.shifted
                    } else {
                        physical_key.base
                    };
                    let base_char = physical_key.base;

                    let is_depressed = self.depressed_keys.contains(&base_char);
                    let is_unlocked = self.unlocked_keys.contains(&display_char)
                        || self.unlocked_keys.contains(&base_char);
                    let is_focused = self.focused_key == Some(display_char)
                        || self.focused_key == Some(base_char);
                    let is_next =
                        self.next_key == Some(display_char) || self.next_key == Some(base_char);

                    let style = key_style(
                        is_depressed,
                        is_next,
                        is_focused,
                        is_unlocked,
                        base_char,
                        self.model,
                        colors,
                    );

                    let display = format!("[{display_char}]");
                    buf.set_string(x, y, &display, style);
                }
            }
        } else {
            // Full mode: all 4 rows
            let key_width: u16 = 5;
            let min_width: u16 = 69;

            if inner.height < 4 || inner.width < min_width {
                // Fallback to compact-style if too narrow for full
                let letter_rows = self.model.letter_rows();
                let key_width: u16 = 5;
                let offsets: &[u16] = &[1, 3, 5];

                if inner.height < 3 || inner.width < 30 {
                    return;
                }

                for (row_idx, row) in letter_rows.iter().enumerate() {
                    let y = inner.y + row_idx as u16;
                    if y >= inner.y + inner.height {
                        break;
                    }

                    let offset = offsets.get(row_idx).copied().unwrap_or(0);

                    for (col_idx, physical_key) in row.iter().enumerate() {
                        let x = inner.x + offset + col_idx as u16 * key_width;
                        if x + key_width > inner.x + inner.width {
                            break;
                        }

                        let display_char = if self.shift_held {
                            physical_key.shifted
                        } else {
                            physical_key.base
                        };
                        let base_char = physical_key.base;

                        let is_depressed = self.depressed_keys.contains(&base_char);
                        let is_unlocked = self.unlocked_keys.contains(&display_char)
                            || self.unlocked_keys.contains(&base_char);
                        let is_focused = self.focused_key == Some(display_char)
                            || self.focused_key == Some(base_char);
                        let is_next =
                            self.next_key == Some(display_char) || self.next_key == Some(base_char);

                        let style = key_style(
                            is_depressed,
                            is_next,
                            is_focused,
                            is_unlocked,
                            base_char,
                            self.model,
                            colors,
                        );

                        let display = format!("[ {display_char} ]");
                        buf.set_string(x, y, &display, style);
                    }
                }
                return;
            }

            // Row offsets for full layout (staggered keyboard)
            let offsets: &[u16] = &[0, 2, 3, 4];

            for (row_idx, row) in self.model.rows.iter().enumerate() {
                let y = inner.y + row_idx as u16;
                if y >= inner.y + inner.height {
                    break;
                }

                let offset = offsets.get(row_idx).copied().unwrap_or(0);

                for (col_idx, physical_key) in row.iter().enumerate() {
                    let x = inner.x + offset + col_idx as u16 * key_width;
                    if x + key_width > inner.x + inner.width {
                        break;
                    }

                    let display_char = if self.shift_held {
                        physical_key.shifted
                    } else {
                        physical_key.base
                    };
                    let base_char = physical_key.base;

                    let is_depressed = self.depressed_keys.contains(&base_char);
                    let is_unlocked = self.unlocked_keys.contains(&display_char)
                        || self.unlocked_keys.contains(&base_char);
                    let is_focused = self.focused_key == Some(display_char)
                        || self.focused_key == Some(base_char);
                    let is_next =
                        self.next_key == Some(display_char) || self.next_key == Some(base_char);

                    let style = key_style(
                        is_depressed,
                        is_next,
                        is_focused,
                        is_unlocked,
                        base_char,
                        self.model,
                        colors,
                    );

                    let display = format!("[ {display_char} ]");
                    buf.set_string(x, y, &display, style);
                }

                // Modifier labels at row edges (visual only)
                let label_style = Style::default().fg(colors.text_pending());
                let after_x = inner.x + offset + row.len() as u16 * key_width + 1;
                match row_idx {
                    0 => {
                        // Backspace after number row
                        if after_x + 4 <= inner.x + inner.width {
                            buf.set_string(after_x, y, "Bksp", label_style);
                        }
                    }
                    1 => {
                        // Tab before top row, backslash already in row
                        if offset >= 3 {
                            buf.set_string(inner.x, y, "Tab", label_style);
                        }
                    }
                    2 => {
                        // Enter after home row
                        if after_x + 5 <= inner.x + inner.width {
                            buf.set_string(after_x, y, "Enter", label_style);
                        }
                    }
                    3 => {
                        // Shift before and after bottom row
                        if offset >= 5 {
                            buf.set_string(inner.x, y, "Shft", label_style);
                        }
                        if after_x + 4 <= inner.x + inner.width {
                            buf.set_string(after_x, y, "Shft", label_style);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn key_style(
    is_depressed: bool,
    is_next: bool,
    is_focused: bool,
    is_unlocked: bool,
    base_char: char,
    model: &KeyboardModel,
    colors: &crate::ui::theme::ThemeColors,
) -> Style {
    if is_depressed {
        let bg = if is_unlocked {
            brighten_color(finger_color(model, base_char))
        } else {
            brighten_color(colors.accent_dim())
        };
        Style::default()
            .fg(Color::White)
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    } else if is_next {
        Style::default().fg(colors.bg()).bg(colors.accent())
    } else if is_focused {
        Style::default().fg(colors.bg()).bg(colors.focused_key())
    } else if is_unlocked {
        Style::default()
            .fg(colors.fg())
            .bg(finger_color(model, base_char))
    } else {
        Style::default().fg(colors.text_pending()).bg(colors.bg())
    }
}
