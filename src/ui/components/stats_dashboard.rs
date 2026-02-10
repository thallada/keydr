use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::session::result::LessonResult;
use crate::ui::components::chart::WpmChart;
use crate::ui::theme::Theme;

pub struct StatsDashboard<'a> {
    pub history: &'a [LessonResult],
    pub theme: &'a Theme,
}

impl<'a> StatsDashboard<'a> {
    pub fn new(history: &'a [LessonResult], theme: &'a Theme) -> Self {
        Self { history, theme }
    }
}

impl Widget for StatsDashboard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Statistics ")
            .border_style(Style::default().fg(colors.accent()))
            .style(Style::default().bg(colors.bg()));
        let inner = block.inner(area);
        block.render(area, buf);

        if self.history.is_empty() {
            let msg = Paragraph::new(Line::from(Span::styled(
                "No lessons completed yet. Start typing!",
                Style::default().fg(colors.text_pending()),
            )));
            msg.render(inner, buf);
            return;
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),
                Constraint::Min(10),
                Constraint::Length(2),
            ])
            .split(inner);

        let avg_wpm =
            self.history.iter().map(|r| r.wpm).sum::<f64>() / self.history.len() as f64;
        let best_wpm = self
            .history
            .iter()
            .map(|r| r.wpm)
            .fold(0.0f64, f64::max);
        let avg_accuracy =
            self.history.iter().map(|r| r.accuracy).sum::<f64>() / self.history.len() as f64;
        let total_lessons = self.history.len();

        let total_str = format!("{total_lessons}");
        let avg_wpm_str = format!("{avg_wpm:.0}");
        let best_wpm_str = format!("{best_wpm:.0}");
        let avg_acc_str = format!("{avg_accuracy:.1}%");

        let summary = vec![
            Line::from(vec![
                Span::styled("  Lessons:      ", Style::default().fg(colors.fg())),
                Span::styled(
                    &*total_str,
                    Style::default()
                        .fg(colors.accent())
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Avg WPM:      ", Style::default().fg(colors.fg())),
                Span::styled(&*avg_wpm_str, Style::default().fg(colors.accent())),
            ]),
            Line::from(vec![
                Span::styled("  Best WPM:     ", Style::default().fg(colors.fg())),
                Span::styled(
                    &*best_wpm_str,
                    Style::default()
                        .fg(colors.success())
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Avg Accuracy: ", Style::default().fg(colors.fg())),
                Span::styled(
                    &*avg_acc_str,
                    Style::default().fg(if avg_accuracy >= 95.0 {
                        colors.success()
                    } else {
                        colors.warning()
                    }),
                ),
            ]),
        ];

        Paragraph::new(summary).render(layout[0], buf);

        let chart_data: Vec<(f64, f64)> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, r)| (i as f64, r.wpm))
            .collect();
        WpmChart::new(&chart_data, self.theme).render(layout[1], buf);

        let help = Paragraph::new(Line::from(Span::styled(
            "  [ESC] Back to menu",
            Style::default().fg(colors.accent()),
        )));
        help.render(layout[2], buf);
    }
}
