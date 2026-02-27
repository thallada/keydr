use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::session::result::DrillResult;
use crate::ui::layout::pack_hint_lines;
use crate::ui::theme::Theme;

pub struct Dashboard<'a> {
    pub result: &'a DrillResult,
    pub theme: &'a Theme,
    pub input_lock_remaining_ms: Option<u64>,
}

impl<'a> Dashboard<'a> {
    pub fn new(
        result: &'a DrillResult,
        theme: &'a Theme,
        input_lock_remaining_ms: Option<u64>,
    ) -> Self {
        Self {
            result,
            theme,
            input_lock_remaining_ms,
        }
    }
}

impl Widget for Dashboard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Drill Complete ")
            .border_style(Style::default().fg(colors.accent()))
            .style(Style::default().bg(colors.bg()));
        let inner = block.inner(area);
        block.render(area, buf);

        let footer_line_count = if self.input_lock_remaining_ms.is_some() {
            1u16
        } else {
            let hints = [
                "[c/Enter/Space] Continue",
                "[r] Retry",
                "[q] Menu",
                "[s] Stats",
                "[x] Delete",
            ];
            pack_hint_lines(&hints, inner.width as usize).len().max(1) as u16
        };

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(footer_line_count),
            ])
            .split(inner);

        let mut title_spans = vec![Span::styled(
            "Results",
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        )];
        if !self.result.ranked {
            title_spans.push(Span::styled(
                "  (Unranked \u{2014} does not count toward skill tree)",
                Style::default().fg(colors.text_pending()),
            ));
        }
        let title = Paragraph::new(Line::from(title_spans)).alignment(Alignment::Center);
        title.render(layout[0], buf);

        let wpm_text = format!("{:.0} WPM", self.result.wpm);
        let cpm_text = format!("  ({:.0} CPM)", self.result.cpm);
        let wpm_line = Line::from(vec![
            Span::styled("  Speed:    ", Style::default().fg(colors.fg())),
            Span::styled(
                &*wpm_text,
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&*cpm_text, Style::default().fg(colors.text_pending())),
        ]);
        Paragraph::new(wpm_line).render(layout[1], buf);

        let acc_color = if self.result.accuracy >= 95.0 {
            colors.success()
        } else if self.result.accuracy >= 85.0 {
            colors.warning()
        } else {
            colors.error()
        };
        let acc_text = format!("{:.1}%", self.result.accuracy);
        let acc_detail = format!(
            "  ({}/{} correct)",
            self.result.correct, self.result.total_chars
        );
        let acc_line = Line::from(vec![
            Span::styled("  Accuracy: ", Style::default().fg(colors.fg())),
            Span::styled(
                &*acc_text,
                Style::default().fg(acc_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(&*acc_detail, Style::default().fg(colors.text_pending())),
        ]);
        Paragraph::new(acc_line).render(layout[2], buf);

        let time_text = format!("{:.1}s", self.result.elapsed_secs);
        let time_line = Line::from(vec![
            Span::styled("  Time:     ", Style::default().fg(colors.fg())),
            Span::styled(&*time_text, Style::default().fg(colors.fg())),
        ]);
        Paragraph::new(time_line).render(layout[3], buf);

        let error_text = format!("{}", self.result.incorrect);
        let chars_line = Line::from(vec![
            Span::styled("  Errors:   ", Style::default().fg(colors.fg())),
            Span::styled(
                &*error_text,
                Style::default().fg(if self.result.incorrect == 0 {
                    colors.success()
                } else {
                    colors.error()
                }),
            ),
        ]);
        Paragraph::new(chars_line).render(layout[4], buf);

        let help = if let Some(ms) = self.input_lock_remaining_ms {
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "  Input temporarily blocked ",
                    Style::default().fg(colors.warning()),
                ),
                Span::styled(
                    format!("({ms}ms remaining)"),
                    Style::default()
                        .fg(colors.warning())
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
        } else {
            let hints = [
                "[c/Enter/Space] Continue",
                "[r] Retry",
                "[q] Menu",
                "[s] Stats",
                "[x] Delete",
            ];
            let lines: Vec<Line> = pack_hint_lines(&hints, inner.width as usize)
                .into_iter()
                .map(|line| {
                    Line::from(Span::styled(
                        line,
                        Style::default().fg(colors.accent()),
                    ))
                })
                .collect();
            Paragraph::new(lines)
        };
        help.render(layout[6], buf);
    }
}
