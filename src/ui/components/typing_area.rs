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

impl Widget for TypingArea<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;
        let mut spans: Vec<Span> = Vec::new();

        for (i, &target_ch) in self.drill.target.iter().enumerate() {
            if i < self.drill.cursor {
                let style = match &self.drill.input[i] {
                    CharStatus::Correct => Style::default().fg(colors.text_correct()),
                    CharStatus::Incorrect(_) => Style::default()
                        .fg(colors.text_incorrect())
                        .bg(colors.text_incorrect_bg())
                        .add_modifier(Modifier::UNDERLINED),
                };
                let display = match &self.drill.input[i] {
                    CharStatus::Incorrect(actual) => *actual,
                    _ => target_ch,
                };
                spans.push(Span::styled(display.to_string(), style));
            } else if i == self.drill.cursor {
                let style = Style::default()
                    .fg(colors.text_cursor_fg())
                    .bg(colors.text_cursor_bg());
                spans.push(Span::styled(target_ch.to_string(), style));
            } else {
                let style = Style::default().fg(colors.text_pending());
                spans.push(Span::styled(target_ch.to_string(), style));
            }
        }

        let line = Line::from(spans);
        let block = Block::bordered()
            .border_style(Style::default().fg(colors.border()))
            .style(Style::default().bg(colors.bg()));

        let paragraph = Paragraph::new(line).block(block).wrap(Wrap { trim: false });

        paragraph.render(area, buf);
    }
}
