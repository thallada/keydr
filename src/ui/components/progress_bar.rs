use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Widget};

use crate::ui::theme::Theme;

pub struct ProgressBar<'a> {
    pub label: String,
    pub ratio: f64,
    pub theme: &'a Theme,
}

impl<'a> ProgressBar<'a> {
    pub fn new(label: &str, ratio: f64, theme: &'a Theme) -> Self {
        Self {
            label: label.to_string(),
            ratio: ratio.clamp(0.0, 1.0),
            theme,
        }
    }
}

impl Widget for ProgressBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(format!(" {} ", self.label))
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width == 0 || inner.height == 0 {
            return;
        }

        let filled_width = (self.ratio * inner.width as f64) as u16;
        let label = format!("{:.0}%", self.ratio * 100.0);

        for x in inner.x..inner.x + inner.width {
            let style = if x < inner.x + filled_width {
                Style::default().fg(colors.bg()).bg(colors.bar_filled())
            } else {
                Style::default().fg(colors.fg()).bg(colors.bar_empty())
            };
            buf[(x, inner.y)].set_style(style);
        }

        let label_x = inner.x + (inner.width.saturating_sub(label.len() as u16)) / 2;
        buf.set_string(label_x, inner.y, &label, Style::default().fg(colors.fg()));
    }
}
