use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::session::lesson::LessonState;
use crate::ui::theme::Theme;

pub struct StatsSidebar<'a> {
    lesson: &'a LessonState,
    theme: &'a Theme,
}

impl<'a> StatsSidebar<'a> {
    pub fn new(lesson: &'a LessonState, theme: &'a Theme) -> Self {
        Self { lesson, theme }
    }
}

impl Widget for StatsSidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let wpm = self.lesson.wpm();
        let accuracy = self.lesson.accuracy();
        let progress = self.lesson.progress() * 100.0;
        let correct = self.lesson.correct_count();
        let incorrect = self.lesson.typo_count();
        let elapsed = self.lesson.elapsed_secs();

        let wpm_str = format!("{wpm:.0}");
        let acc_str = format!("{accuracy:.1}%");
        let prog_str = format!("{progress:.0}%");
        let correct_str = format!("{correct}");
        let incorrect_str = format!("{incorrect}");
        let elapsed_str = format!("{elapsed:.1}s");

        let lines = vec![
            Line::from(vec![
                Span::styled("WPM: ", Style::default().fg(colors.fg())),
                Span::styled(&*wpm_str, Style::default().fg(colors.accent())),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Accuracy: ", Style::default().fg(colors.fg())),
                Span::styled(
                    &*acc_str,
                    Style::default().fg(if accuracy >= 95.0 {
                        colors.success()
                    } else if accuracy >= 85.0 {
                        colors.warning()
                    } else {
                        colors.error()
                    }),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Progress: ", Style::default().fg(colors.fg())),
                Span::styled(&*prog_str, Style::default().fg(colors.accent())),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Correct: ", Style::default().fg(colors.fg())),
                Span::styled(&*correct_str, Style::default().fg(colors.success())),
            ]),
            Line::from(vec![
                Span::styled("Errors:  ", Style::default().fg(colors.fg())),
                Span::styled(&*incorrect_str, Style::default().fg(colors.error())),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Time: ", Style::default().fg(colors.fg())),
                Span::styled(&*elapsed_str, Style::default().fg(colors.fg())),
            ]),
        ];

        let block = Block::bordered()
            .title(" Stats ")
            .border_style(Style::default().fg(colors.border()))
            .style(Style::default().bg(colors.bg()));

        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(area, buf);
    }
}
