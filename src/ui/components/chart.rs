use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::widgets::{Axis, Block, Chart, Dataset, GraphType, Widget};

use crate::i18n::t;
use crate::ui::theme::Theme;

#[allow(dead_code)]
pub struct WpmChart<'a> {
    pub data: &'a [(f64, f64)],
    pub theme: &'a Theme,
}

#[allow(dead_code)]
impl<'a> WpmChart<'a> {
    pub fn new(data: &'a [(f64, f64)], theme: &'a Theme) -> Self {
        Self { data, theme }
    }
}

impl Widget for WpmChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        if self.data.is_empty() {
            let wpm_title = t!("chart.wpm_over_time");
            let block = Block::bordered()
                .title(wpm_title.to_string())
                .border_style(Style::default().fg(colors.border()));
            block.render(area, buf);
            return;
        }

        let max_x = self.data.last().map(|(x, _)| *x).unwrap_or(1.0);
        let max_y = self
            .data
            .iter()
            .map(|(_, y)| *y)
            .fold(0.0f64, f64::max)
            .max(10.0);

        let dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(colors.accent()))
            .data(self.data);

        let wpm_title = t!("chart.wpm_over_time");
        let drill_number_label = t!("chart.drill_number");
        let wpm_label = t!("common.wpm");
        let chart = Chart::new(vec![dataset])
            .block(
                Block::bordered()
                    .title(wpm_title.to_string())
                    .border_style(Style::default().fg(colors.border())),
            )
            .x_axis(
                Axis::default()
                    .title(drill_number_label.to_string())
                    .style(Style::default().fg(colors.text_pending()))
                    .bounds([0.0, max_x]),
            )
            .y_axis(
                Axis::default()
                    .title(wpm_label.to_string())
                    .style(Style::default().fg(colors.text_pending()))
                    .bounds([0.0, max_y * 1.1]),
            );

        chart.render(area, buf);
    }
}
