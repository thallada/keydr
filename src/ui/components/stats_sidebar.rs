use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::session::drill::DrillState;
use crate::session::result::DrillResult;
use crate::ui::theme::Theme;

pub struct StatsSidebar<'a> {
    drill: &'a DrillState,
    last_result: Option<&'a DrillResult>,
    history: &'a [DrillResult],
    theme: &'a Theme,
}

impl<'a> StatsSidebar<'a> {
    pub fn new(
        drill: &'a DrillState,
        last_result: Option<&'a DrillResult>,
        history: &'a [DrillResult],
        theme: &'a Theme,
    ) -> Self {
        Self {
            drill,
            last_result,
            history,
            theme,
        }
    }
}

/// Format a delta value with arrow indicator
fn format_delta(delta: f64, suffix: &str) -> String {
    if delta > 0.0 {
        format!("\u{2191}+{:.1}{suffix}", delta)
    } else if delta < 0.0 {
        format!("\u{2193}{:.1}{suffix}", delta)
    } else {
        format!("={suffix}")
    }
}

impl Widget for StatsSidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let has_last = self.last_result.is_some();

        // Split sidebar into current stats and last drill sections
        let sections = if has_last {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Min(12)])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(10), Constraint::Length(0)])
                .split(area)
        };

        // Current drill stats
        {
            let wpm = self.drill.wpm();
            let accuracy = self.drill.accuracy();
            let progress = self.drill.progress() * 100.0;
            let correct = self.drill.correct_count();
            let incorrect = self.drill.typo_count();
            let elapsed = self.drill.elapsed_secs();

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

        // Last drill stats with session impact deltas
        if let Some(last) = self.last_result {
            let wpm_str = format!("{:.0}", last.wpm);
            let acc_str = format!("{:.1}%", last.accuracy);
            let time_str = format!("{:.1}s", last.elapsed_secs);
            let errors_str = format!("{}", last.incorrect);

            // Compute deltas: compare last drill to the average of all prior drills
            // (excluding the last one which is the current result)
            let prior_count = self.history.len().saturating_sub(1);
            let (wpm_delta, acc_delta) = if prior_count > 0 {
                let prior = &self.history[..prior_count];
                let avg_wpm = prior.iter().map(|r| r.wpm).sum::<f64>() / prior.len() as f64;
                let avg_acc = prior.iter().map(|r| r.accuracy).sum::<f64>() / prior.len() as f64;
                (last.wpm - avg_wpm, last.accuracy - avg_acc)
            } else {
                (0.0, 0.0)
            };

            let wpm_delta_str = format_delta(wpm_delta, "");
            let acc_delta_str = format_delta(acc_delta, "%");

            let wpm_delta_color = if wpm_delta > 0.0 {
                colors.success()
            } else if wpm_delta < 0.0 {
                colors.error()
            } else {
                colors.text_pending()
            };

            let acc_delta_color = if acc_delta > 0.0 {
                colors.success()
            } else if acc_delta < 0.0 {
                colors.error()
            } else {
                colors.text_pending()
            };

            let mut lines = vec![Line::from(vec![
                Span::styled("WPM: ", Style::default().fg(colors.fg())),
                Span::styled(wpm_str, Style::default().fg(colors.accent())),
            ])];

            if prior_count > 0 {
                lines.push(Line::from(vec![
                    Span::styled("  vs avg: ", Style::default().fg(colors.text_pending())),
                    Span::styled(wpm_delta_str, Style::default().fg(wpm_delta_color)),
                ]));
            }

            lines.push(Line::from(""));

            lines.push(Line::from(vec![
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
            ]));

            if prior_count > 0 {
                lines.push(Line::from(vec![
                    Span::styled("  vs avg: ", Style::default().fg(colors.text_pending())),
                    Span::styled(acc_delta_str, Style::default().fg(acc_delta_color)),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Errors:  ", Style::default().fg(colors.fg())),
                Span::styled(errors_str, Style::default().fg(colors.error())),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Time: ", Style::default().fg(colors.fg())),
                Span::styled(time_str, Style::default().fg(colors.fg())),
            ]));

            let block = Block::bordered()
                .title(" Last Drill ")
                .border_style(Style::default().fg(colors.border()))
                .style(Style::default().bg(colors.bg()));

            let paragraph = Paragraph::new(lines).block(block);
            paragraph.render(sections[1], buf);
        }
    }
}
