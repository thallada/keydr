use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Widget};

use crate::ui::theme::Theme;

pub struct KeyboardDiagram<'a> {
    pub focused_key: Option<char>,
    pub unlocked_keys: &'a [char],
    pub theme: &'a Theme,
}

impl<'a> KeyboardDiagram<'a> {
    pub fn new(
        focused_key: Option<char>,
        unlocked_keys: &'a [char],
        theme: &'a Theme,
    ) -> Self {
        Self {
            focused_key,
            unlocked_keys,
            theme,
        }
    }
}

const ROWS: &[&[char]] = &[
    &['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
    &['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
    &['z', 'x', 'c', 'v', 'b', 'n', 'm'],
];

impl Widget for KeyboardDiagram<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Keyboard ")
            .border_style(Style::default().fg(colors.border()))
            .style(Style::default().bg(colors.bg()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 || inner.width < 20 {
            return;
        }

        let key_width: u16 = 4;
        let offsets: &[u16] = &[1, 2, 4];

        for (row_idx, row) in ROWS.iter().enumerate() {
            let y = inner.y + row_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let offset = offsets.get(row_idx).copied().unwrap_or(0);

            for (col_idx, &key) in row.iter().enumerate() {
                let x = inner.x + offset + col_idx as u16 * key_width;
                if x + 3 > inner.x + inner.width {
                    break;
                }

                let is_unlocked = self.unlocked_keys.contains(&key);
                let is_focused = self.focused_key == Some(key);

                let style = if is_focused {
                    Style::default()
                        .fg(colors.bg())
                        .bg(colors.focused_key())
                } else if is_unlocked {
                    Style::default().fg(colors.fg()).bg(colors.accent_dim())
                } else {
                    Style::default()
                        .fg(colors.text_pending())
                        .bg(colors.bg())
                };

                let display = format!("[{key}]");
                buf.set_string(x, y, &display, style);
            }
        }
    }
}
