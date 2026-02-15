use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::session::lesson::LessonState;
use crate::session::result::LessonResult;
use crate::ui::theme::Theme;

pub struct StatsSidebar<'a> {
    lesson: &'a LessonState,
    last_result: Option<&'a LessonResult>,
    theme: &'a Theme,
}

impl<'a> StatsSidebar<'a> {
    pub fn new(lesson: &'a LessonState, last_result: Option<&'a LessonResult>, theme: &'a Theme) -> Self {
        Self { lesson, last_result, theme }
    }
}

impl Widget for StatsSidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let has_last = self.last_result.is_some();

        // Split sidebar into current stats and last lesson sections
        let sections = if has_last {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Min(10)])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(0)])
                .split(area)
        };

        // Current lesson stats
        {
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
                    Span::styled(wpm_str, Style::default().fg(colors.accent())),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Accuracy: ", Style::default().fg(colors.fg())),
                    Span::styled(
                        acc_str,
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
                    Span::styled(prog_str, Style::default().fg(colors.accent())),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Correct: ", Style::default().fg(colors.fg())),
                    Span::styled(correct_str, Style::default().fg(colors.success())),
                ]),
                Line::from(vec![
                    Span::styled("Errors:  ", Style::default().fg(colors.fg())),
                    Span::styled(incorrect_str, Style::default().fg(colors.error())),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Time: ", Style::default().fg(colors.fg())),
                    Span::styled(elapsed_str, Style::default().fg(colors.fg())),
                ]),
            ];

            let block = Block::bordered()
                .title(" Stats ")
                .border_style(Style::default().fg(colors.border()))
                .style(Style::default().bg(colors.bg()));

            let paragraph = Paragraph::new(lines).block(block);
            paragraph.render(sections[0], buf);
        }

        // Last lesson stats
        if let Some(last) = self.last_result {
            let wpm_str = format!("{:.0}", last.wpm);
            let acc_str = format!("{:.1}%", last.accuracy);
            let chars_str = format!("{}", last.total_chars);
            let time_str = format!("{:.1}s", last.elapsed_secs);
            let errors_str = format!("{}", last.incorrect);

            let lines = vec![
                Line::from(vec![
                    Span::styled("WPM: ", Style::default().fg(colors.fg())),
                    Span::styled(wpm_str, Style::default().fg(colors.accent())),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Accuracy: ", Style::default().fg(colors.fg())),
                    Span::styled(
                        acc_str,
                        Style::default().fg(if last.accuracy >= 95.0 {
                            colors.success()
                        } else if last.accuracy >= 85.0 {
                            colors.warning()
                        } else {
                            colors.error()
                        }),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Chars: ", Style::default().fg(colors.fg())),
                    Span::styled(chars_str, Style::default().fg(colors.fg())),
                ]),
                Line::from(vec![
                    Span::styled("Errors:  ", Style::default().fg(colors.fg())),
                    Span::styled(errors_str, Style::default().fg(colors.error())),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Time: ", Style::default().fg(colors.fg())),
                    Span::styled(time_str, Style::default().fg(colors.fg())),
                ]),
            ];

            let block = Block::bordered()
                .title(" Last Lesson ")
                .border_style(Style::default().fg(colors.border()))
                .style(Style::default().bg(colors.bg()));

            let paragraph = Paragraph::new(lines).block(block);
            paragraph.render(sections[1], buf);
        }
    }
}
