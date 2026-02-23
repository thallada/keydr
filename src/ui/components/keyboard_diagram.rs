use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Widget};

use crate::keyboard::display::{self, BACKSPACE, ENTER, SPACE, TAB};
use crate::keyboard::model::KeyboardModel;
use crate::ui::theme::Theme;

pub struct KeyboardDiagram<'a> {
    pub selected_key: Option<char>,
    pub next_key: Option<char>,
    pub unlocked_keys: &'a [char],
    pub depressed_keys: &'a HashSet<char>,
    pub theme: &'a Theme,
    pub compact: bool,
    pub model: &'a KeyboardModel,
    pub shift_held: bool,
    pub caps_lock: bool,
}

impl<'a> KeyboardDiagram<'a> {
    pub fn new(
        next_key: Option<char>,
        unlocked_keys: &'a [char],
        depressed_keys: &'a HashSet<char>,
        theme: &'a Theme,
        model: &'a KeyboardModel,
    ) -> Self {
        Self {
            selected_key: None,
            next_key,
            unlocked_keys,
            depressed_keys,
            theme,
            compact: false,
            model,
            shift_held: false,
            caps_lock: false,
        }
    }

    pub fn caps_lock(mut self, caps_lock: bool) -> Self {
        self.caps_lock = caps_lock;
        self
    }

    pub fn selected_key(mut self, key: Option<char>) -> Self {
        self.selected_key = key;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn shift_held(mut self, shift_held: bool) -> Self {
        self.shift_held = shift_held;
        self
    }

    /// Check if a key (by display or base char) matches the selected key.
    fn is_key_selected(&self, display_char: char, base_char: char) -> bool {
        self.selected_key == Some(display_char) || self.selected_key == Some(base_char)
    }

    /// Check if a sentinel/modifier key matches the selected key.
    fn is_sentinel_selected(&self, sentinel: char) -> bool {
        self.selected_key == Some(sentinel)
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

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
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

fn relative_luminance(color: Color) -> f64 {
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

fn contrast_ratio(a: Color, b: Color) -> f64 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn readable_fg(bg: Color, preferred: Color) -> Color {
    let mut best = preferred;
    let mut best_ratio = contrast_ratio(preferred, bg);
    for candidate in [Color::White, Color::Black] {
        let ratio = contrast_ratio(candidate, bg);
        if ratio > best_ratio {
            best = candidate;
            best_ratio = ratio;
        }
    }
    best
}

/// Blend a color toward the background at the given ratio (0.0 = full bg, 1.0 = full color).
fn blend_toward_bg(color: Color, bg: Color, ratio: f32) -> Color {
    match (color, bg) {
        (Color::Rgb(r, g, b), Color::Rgb(br, bg_g, bb)) => {
            let mix = |c: u8, base: u8| -> u8 {
                (base as f32 + (c as f32 - base as f32) * ratio).round() as u8
            };
            Color::Rgb(mix(r, br), mix(g, bg_g), mix(b, bb))
        }
        _ => color,
    }
}

/// Compute style for a modifier key box (Tab, Enter, Shift, Space, Backspace).
fn modifier_key_style(
    is_depressed: bool,
    is_next: bool,
    is_selected: bool,
    colors: &crate::ui::theme::ThemeColors,
) -> Style {
    if is_depressed {
        let bg = brighten_color(colors.accent_dim());
        Style::default()
            .fg(readable_fg(bg, colors.fg()))
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    } else if is_next {
        let bg = blend_toward_bg(colors.accent(), colors.bg(), 0.35);
        Style::default().fg(readable_fg(bg, colors.accent())).bg(bg)
    } else if is_selected {
        let bg = colors.accent_dim();
        Style::default().fg(readable_fg(bg, colors.fg())).bg(bg)
    } else {
        Style::default().fg(colors.fg()).bg(colors.bg())
    }
}

fn key_style(
    is_depressed: bool,
    is_next: bool,
    is_selected: bool,
    is_unlocked: bool,
    colors: &crate::ui::theme::ThemeColors,
) -> Style {
    if is_depressed {
        let bg = brighten_color(colors.accent_dim());
        Style::default()
            .fg(readable_fg(bg, colors.fg()))
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    } else if is_next {
        let bg = blend_toward_bg(colors.accent(), colors.bg(), 0.35);
        Style::default().fg(readable_fg(bg, colors.accent())).bg(bg)
    } else if is_selected {
        let bg = colors.accent_dim();
        Style::default().fg(readable_fg(bg, colors.fg())).bg(bg)
    } else if is_unlocked {
        Style::default().fg(colors.fg()).bg(colors.bg())
    } else {
        Style::default().fg(colors.text_pending()).bg(colors.bg())
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
            self.render_compact(inner, buf);
        } else {
            self.render_full(inner, buf);
        }
    }
}

impl KeyboardDiagram<'_> {
    fn render_compact(&self, inner: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;
        let letter_rows = self.model.letter_rows();
        let key_width: u16 = 3;
        let min_width: u16 = 21;

        if inner.height < 3 || inner.width < min_width {
            return;
        }

        let offsets: &[u16] = &[3, 4, 6];

        for (row_idx, row) in letter_rows.iter().enumerate() {
            let y = inner.y + row_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let offset = offsets.get(row_idx).copied().unwrap_or(0);

            // Render leading modifier key
            match row_idx {
                0 => {
                    let is_dep = self.depressed_keys.contains(&TAB);
                    let is_next = self.next_key == Some(TAB);
                    let is_sel = self.is_sentinel_selected(TAB);
                    let style = modifier_key_style(is_dep, is_next, is_sel, colors);
                    buf.set_string(inner.x, y, "[T]", style);
                }
                2 => {
                    let is_dep = self.shift_held;
                    let style = modifier_key_style(is_dep, false, false, colors);
                    buf.set_string(inner.x, y, "[S]", style);
                }
                _ => {}
            }

            for (col_idx, physical_key) in row.iter().enumerate() {
                let x = inner.x + offset + col_idx as u16 * key_width;
                if x + key_width > inner.x + inner.width {
                    break;
                }

                // Caps lock inverts shift for alpha keys only
                let show_shifted = if physical_key.base.is_ascii_alphabetic() {
                    self.shift_held ^ self.caps_lock
                } else {
                    self.shift_held
                };
                let display_char = if show_shifted {
                    physical_key.shifted
                } else {
                    physical_key.base
                };
                let base_char = physical_key.base;

                let is_depressed = self.depressed_keys.contains(&base_char);
                let is_unlocked = self.unlocked_keys.contains(&display_char)
                    || self.unlocked_keys.contains(&base_char);
                let is_next =
                    self.next_key == Some(display_char) || self.next_key == Some(base_char);
                let is_sel = self.is_key_selected(display_char, base_char);

                let style = key_style(is_depressed, is_next, is_sel, is_unlocked, colors);

                let display = format!("[{display_char}]");
                buf.set_string(x, y, &display, style);
            }

            // Render trailing modifier key
            let row_end_x = inner.x + offset + row.len() as u16 * key_width;
            match row_idx {
                1 => {
                    if row_end_x + 3 <= inner.x + inner.width {
                        let is_dep = self.depressed_keys.contains(&ENTER);
                        let is_next = self.next_key == Some(ENTER);
                        let is_sel = self.is_sentinel_selected(ENTER);
                        let style = modifier_key_style(is_dep, is_next, is_sel, colors);
                        buf.set_string(row_end_x, y, "[E]", style);
                    }
                }
                2 => {
                    if row_end_x + 3 <= inner.x + inner.width {
                        let is_dep = self.shift_held;
                        let style = modifier_key_style(is_dep, false, false, colors);
                        buf.set_string(row_end_x, y, "[S]", style);
                    }
                }
                _ => {}
            }
        }

        // Backspace at end of first row
        if inner.height >= 3 {
            let y = inner.y;
            let row_end_x = inner.x + offsets[0] + letter_rows[0].len() as u16 * key_width;
            if row_end_x + 3 <= inner.x + inner.width {
                let is_dep = self.depressed_keys.contains(&BACKSPACE);
                let is_next = self.next_key == Some(BACKSPACE);
                let is_sel = self.is_sentinel_selected(BACKSPACE);
                let style = modifier_key_style(is_dep, is_next, is_sel, colors);
                buf.set_string(row_end_x, y, "[B]", style);
            }
        }
    }

    fn render_full(&self, inner: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;
        let key_width: u16 = 5;
        let min_width: u16 = 75;

        if inner.height < 4 || inner.width < min_width {
            self.render_full_fallback(inner, buf);
            return;
        }

        let offsets: &[u16] = &[0, 5, 5, 6];

        for (row_idx, row) in self.model.rows.iter().enumerate() {
            let y = inner.y + row_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let offset = offsets.get(row_idx).copied().unwrap_or(0);

            // Render leading modifier keys
            match row_idx {
                1 => {
                    if offset >= 5 {
                        let is_dep = self.depressed_keys.contains(&TAB);
                        let is_next = self.next_key == Some(TAB);
                        let is_sel = self.is_sentinel_selected(TAB);
                        let style = modifier_key_style(is_dep, is_next, is_sel, colors);
                        let label = format!("[{}]", display::key_short_label(TAB));
                        buf.set_string(inner.x, y, &label, style);
                    }
                }
                2 => {
                    if offset >= 5 {
                        if self.caps_lock {
                            let bg = colors.accent_dim();
                            let style = Style::default()
                                .fg(readable_fg(bg, colors.warning()))
                                .bg(bg);
                            buf.set_string(inner.x, y, "[Cap]", style);
                        } else {
                            let style = Style::default().fg(colors.text_pending()).bg(colors.bg());
                            buf.set_string(inner.x, y, "[   ]", style);
                        }
                    }
                }
                3 => {
                    if offset >= 6 {
                        let is_dep = self.shift_held;
                        let style = modifier_key_style(is_dep, false, false, colors);
                        buf.set_string(inner.x, y, "[Shft]", style);
                    }
                }
                _ => {}
            }

            for (col_idx, physical_key) in row.iter().enumerate() {
                let x = inner.x + offset + col_idx as u16 * key_width;
                if x + key_width > inner.x + inner.width {
                    break;
                }

                // Caps lock inverts shift for alpha keys only
                let show_shifted = if physical_key.base.is_ascii_alphabetic() {
                    self.shift_held ^ self.caps_lock
                } else {
                    self.shift_held
                };
                let display_char = if show_shifted {
                    physical_key.shifted
                } else {
                    physical_key.base
                };
                let base_char = physical_key.base;

                let is_depressed = self.depressed_keys.contains(&base_char);
                let is_unlocked = self.unlocked_keys.contains(&display_char)
                    || self.unlocked_keys.contains(&base_char);
                let is_next =
                    self.next_key == Some(display_char) || self.next_key == Some(base_char);
                let is_sel = self.is_key_selected(display_char, base_char);

                let style = key_style(is_depressed, is_next, is_sel, is_unlocked, colors);

                let display = format!("[ {display_char} ]");
                buf.set_string(x, y, &display, style);
            }

            // Render trailing modifier keys
            let after_x = inner.x + offset + row.len() as u16 * key_width;
            match row_idx {
                0 => {
                    if after_x + 6 <= inner.x + inner.width {
                        let is_dep = self.depressed_keys.contains(&BACKSPACE);
                        let is_next = self.next_key == Some(BACKSPACE);
                        let is_sel = self.is_sentinel_selected(BACKSPACE);
                        let style = modifier_key_style(is_dep, is_next, is_sel, colors);
                        let label = format!("[{}]", display::key_short_label(BACKSPACE));
                        buf.set_string(after_x, y, &label, style);
                    }
                }
                2 => {
                    if after_x + 7 <= inner.x + inner.width {
                        let is_dep = self.depressed_keys.contains(&ENTER);
                        let is_next = self.next_key == Some(ENTER);
                        let is_sel = self.is_sentinel_selected(ENTER);
                        let style = modifier_key_style(is_dep, is_next, is_sel, colors);
                        let label = format!("[{}]", display::key_display_name(ENTER));
                        buf.set_string(after_x, y, &label, style);
                    }
                }
                3 => {
                    if after_x + 6 <= inner.x + inner.width {
                        let is_dep = self.shift_held;
                        let style = modifier_key_style(is_dep, false, false, colors);
                        buf.set_string(after_x, y, "[Shft]", style);
                    }
                }
                _ => {}
            }
        }

        // Compute full keyboard width from rendered rows (including trailing modifier keys),
        // so the space bar centers relative to the keyboard, not the container.
        let keyboard_width = self
            .model
            .rows
            .iter()
            .enumerate()
            .map(|(row_idx, row)| {
                let offset = offsets.get(row_idx).copied().unwrap_or(0);
                let row_end = offset + row.len() as u16 * key_width;
                match row_idx {
                    0 => row_end + 6, // [Bksp]
                    2 => row_end + 7, // [Enter]
                    3 => row_end + 6, // [Shft]
                    _ => row_end,
                }
            })
            .max()
            .unwrap_or(0)
            .min(inner.width);

        // Space bar row (row 4)
        let space_y = inner.y + 4;
        if space_y < inner.y + inner.height {
            let space_name = display::key_display_name(SPACE);
            let space_label = format!("[       {space_name}       ]");
            let space_width = space_label.len() as u16;
            let space_x = inner.x + (keyboard_width.saturating_sub(space_width)) / 2;
            if space_x + space_width <= inner.x + inner.width {
                let is_dep = self.depressed_keys.contains(&SPACE);
                let is_next = self.next_key == Some(SPACE);
                let is_sel = self.is_sentinel_selected(SPACE);
                let style = modifier_key_style(is_dep, is_next, is_sel, colors);
                buf.set_string(space_x, space_y, space_label, style);
            }
        }
    }

    fn render_full_fallback(&self, inner: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;
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

                // Caps lock inverts shift for alpha keys only
                let show_shifted = if physical_key.base.is_ascii_alphabetic() {
                    self.shift_held ^ self.caps_lock
                } else {
                    self.shift_held
                };
                let display_char = if show_shifted {
                    physical_key.shifted
                } else {
                    physical_key.base
                };
                let base_char = physical_key.base;

                let is_depressed = self.depressed_keys.contains(&base_char);
                let is_unlocked = self.unlocked_keys.contains(&display_char)
                    || self.unlocked_keys.contains(&base_char);
                let is_next =
                    self.next_key == Some(display_char) || self.next_key == Some(base_char);
                let is_sel = self.is_key_selected(display_char, base_char);

                let style = key_style(is_depressed, is_next, is_sel, is_unlocked, colors);

                let display = format!("[ {display_char} ]");
                buf.set_string(x, y, &display, style);
            }
        }
    }
}
