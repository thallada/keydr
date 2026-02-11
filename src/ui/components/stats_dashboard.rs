use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::engine::key_stats::KeyStatsStore;
use crate::session::result::LessonResult;
use crate::ui::components::chart::WpmChart;
use crate::ui::theme::Theme;

pub struct StatsDashboard<'a> {
    pub history: &'a [LessonResult],
    pub key_stats: &'a KeyStatsStore,
    pub active_tab: usize,
    pub target_wpm: u32,
    pub theme: &'a Theme,
}

impl<'a> StatsDashboard<'a> {
    pub fn new(
        history: &'a [LessonResult],
        key_stats: &'a KeyStatsStore,
        active_tab: usize,
        target_wpm: u32,
        theme: &'a Theme,
    ) -> Self {
        Self {
            history,
            key_stats,
            active_tab,
            target_wpm,
            theme,
        }
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
                Constraint::Length(2),
                Constraint::Min(10),
                Constraint::Length(2),
            ])
            .split(inner);

        // Tab header
        let tabs = ["[D] Dashboard", "[H] History", "[K] Keystrokes"];
        let tab_spans: Vec<Span> = tabs
            .iter()
            .enumerate()
            .flat_map(|(i, &label)| {
                let style = if i == self.active_tab {
                    Style::default()
                        .fg(colors.accent())
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(colors.text_pending())
                };
                vec![
                    Span::styled(format!(" {label} "), style),
                    Span::raw("  "),
                ]
            })
            .collect();
        Paragraph::new(Line::from(tab_spans)).render(layout[0], buf);

        // Tab content
        match self.active_tab {
            0 => self.render_dashboard_tab(layout[1], buf),
            1 => self.render_history_tab(layout[1], buf),
            2 => self.render_keystrokes_tab(layout[1], buf),
            _ => {}
        }

        // Footer
        let footer = Paragraph::new(Line::from(Span::styled(
            "  [ESC] Back  [Tab] Next tab  [D/H/K] Switch tab",
            Style::default().fg(colors.accent()),
        )));
        footer.render(layout[2], buf);
    }
}

impl StatsDashboard<'_> {
    fn render_dashboard_tab(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Min(8),
            ])
            .split(area);

        // Summary stats
        let avg_wpm =
            self.history.iter().map(|r| r.wpm).sum::<f64>() / self.history.len() as f64;
        let best_wpm = self
            .history
            .iter()
            .map(|r| r.wpm)
            .fold(0.0f64, f64::max);
        let avg_accuracy =
            self.history.iter().map(|r| r.accuracy).sum::<f64>() / self.history.len() as f64;
        let total_time: f64 = self.history.iter().map(|r| r.elapsed_secs).sum();

        let total_str = format!("{}", self.history.len());
        let avg_wpm_str = format!("{avg_wpm:.0}");
        let best_wpm_str = format!("{best_wpm:.0}");
        let avg_acc_str = format!("{avg_accuracy:.1}%");
        let time_str = format_duration(total_time);

        let summary = vec![
            Line::from(vec![
                Span::styled("  Lessons: ", Style::default().fg(colors.fg())),
                Span::styled(
                    &*total_str,
                    Style::default().fg(colors.accent()).add_modifier(Modifier::BOLD),
                ),
                Span::styled("    Avg WPM: ", Style::default().fg(colors.fg())),
                Span::styled(&*avg_wpm_str, Style::default().fg(colors.accent())),
                Span::styled("    Best WPM: ", Style::default().fg(colors.fg())),
                Span::styled(
                    &*best_wpm_str,
                    Style::default().fg(colors.success()).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Accuracy: ", Style::default().fg(colors.fg())),
                Span::styled(
                    &*avg_acc_str,
                    Style::default().fg(if avg_accuracy >= 95.0 {
                        colors.success()
                    } else if avg_accuracy >= 85.0 {
                        colors.warning()
                    } else {
                        colors.error()
                    }),
                ),
                Span::styled("    Total time: ", Style::default().fg(colors.fg())),
                Span::styled(&*time_str, Style::default().fg(colors.text_pending())),
            ]),
        ];
        Paragraph::new(summary).render(layout[0], buf);

        // Progress bars
        self.render_progress_bars(layout[1], buf);

        // Charts
        let chart_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(layout[2]);

        // WPM chart
        let wpm_data: Vec<(f64, f64)> = self
            .history
            .iter()
            .rev()
            .take(50)
            .enumerate()
            .map(|(i, r)| (i as f64, r.wpm))
            .collect();
        WpmChart::new(&wpm_data, self.theme).render(chart_layout[0], buf);

        // Accuracy chart
        let acc_data: Vec<(f64, f64)> = self
            .history
            .iter()
            .rev()
            .take(50)
            .enumerate()
            .map(|(i, r)| (i as f64, r.accuracy))
            .collect();
        render_accuracy_chart(&acc_data, self.theme, chart_layout[1], buf);
    }

    fn render_progress_bars(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(area);

        let avg_wpm =
            self.history.iter().map(|r| r.wpm).sum::<f64>() / self.history.len() as f64;
        let avg_accuracy =
            self.history.iter().map(|r| r.accuracy).sum::<f64>() / self.history.len() as f64;

        // WPM progress
        let wpm_pct = (avg_wpm / self.target_wpm as f64 * 100.0).min(100.0);
        let wpm_label = format!("  WPM: {avg_wpm:.0}/{} ({wpm_pct:.0}%)", self.target_wpm);
        render_text_bar(&wpm_label, wpm_pct / 100.0, colors.accent(), colors.bar_empty(), layout[0], buf);

        // Accuracy progress
        let acc_pct = avg_accuracy.min(100.0);
        let acc_label = format!("  Acc: {acc_pct:.1}%");
        let acc_color = if acc_pct >= 95.0 {
            colors.success()
        } else if acc_pct >= 85.0 {
            colors.warning()
        } else {
            colors.error()
        };
        render_text_bar(&acc_label, acc_pct / 100.0, acc_color, colors.bar_empty(), layout[1], buf);

        // Level progress
        let total_score: f64 = self.history.iter().map(|r| r.wpm * r.accuracy / 100.0).sum();
        let level = ((total_score / 100.0).sqrt() as u32).max(1);
        let next_level_score = ((level + 1) as f64).powi(2) * 100.0;
        let current_level_score = (level as f64).powi(2) * 100.0;
        let level_pct = ((total_score - current_level_score) / (next_level_score - current_level_score)).clamp(0.0, 1.0);
        let level_label = format!("  Lvl {level} ({:.0}%)", level_pct * 100.0);
        render_text_bar(&level_label, level_pct, colors.focused_key(), colors.bar_empty(), layout[2], buf);
    }

    fn render_history_tab(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(8),
            ])
            .split(area);

        // Recent tests table
        let header = Line::from(vec![
            Span::styled(
                "  #     WPM    Raw    Acc%    Time      Date",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let mut lines = vec![header, Line::from(Span::styled(
            "  ─────────────────────────────────────────────",
            Style::default().fg(colors.border()),
        ))];

        let recent: Vec<&LessonResult> = self.history.iter().rev().take(20).collect();
        let total = self.history.len();

        for (i, result) in recent.iter().enumerate() {
            let idx = total - i;
            let raw_wpm = result.cpm / 5.0;
            let time_str = format!("{:.1}s", result.elapsed_secs);
            let date_str = result.timestamp.format("%m/%d %H:%M").to_string();

            let idx_str = format!("{idx:>3}");
            let wpm_str = format!("{:>6.0}", result.wpm);
            let raw_str = format!("{:>6.0}", raw_wpm);
            let acc_str = format!("{:>6.1}%", result.accuracy);
            let row = format!("  {idx_str}  {wpm_str}  {raw_str}  {acc_str}  {time_str:>6}  {date_str}");

            let acc_color = if result.accuracy >= 95.0 {
                colors.success()
            } else if result.accuracy >= 85.0 {
                colors.warning()
            } else {
                colors.error()
            };

            lines.push(Line::from(Span::styled(row, Style::default().fg(acc_color))));
        }

        Paragraph::new(lines).render(layout[0], buf);

        // Per-key speed
        self.render_per_key_speed(layout[1], buf);
    }

    fn render_per_key_speed(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Per-Key Average Speed (ms) ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 52 || inner.height < 2 {
            return;
        }

        let letters: Vec<char> = ('a'..='z').collect();
        let max_time = letters
            .iter()
            .filter_map(|&ch| self.key_stats.stats.get(&ch))
            .map(|s| s.filtered_time_ms)
            .fold(0.0f64, f64::max)
            .max(1.0);

        // Render bar chart: letter label on row 0, bar on row 1
        let bar_width = (inner.width as usize).min(52) / 26;
        let bar_width = bar_width.max(1) as u16;

        for (i, &ch) in letters.iter().enumerate() {
            let x = inner.x + (i as u16 * 2).min(inner.width.saturating_sub(1));
            if x >= inner.x + inner.width {
                break;
            }

            let time = self
                .key_stats
                .stats
                .get(&ch)
                .map(|s| s.filtered_time_ms)
                .unwrap_or(0.0);

            let ratio = time / max_time;
            let color = if ratio < 0.3 {
                colors.success()
            } else if ratio < 0.6 {
                colors.accent()
            } else {
                colors.error()
            };

            // Letter label
            buf.set_string(x, inner.y, &ch.to_string(), Style::default().fg(color));

            // Simple bar indicator
            if inner.height >= 2 {
                let bar_char = if time > 0.0 {
                    match (ratio * 8.0) as u8 {
                        0 => '▁',
                        1 => '▂',
                        2 => '▃',
                        3 => '▄',
                        4 => '▅',
                        5 => '▆',
                        6 => '▇',
                        _ => '█',
                    }
                } else {
                    ' '
                };
                buf.set_string(
                    x,
                    inner.y + 1,
                    &bar_char.to_string(),
                    Style::default().fg(color),
                );
            }
        }

        let _ = bar_width;
    }

    fn render_keystrokes_tab(&self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),
                Constraint::Min(5),
                Constraint::Length(6),
            ])
            .split(area);

        // Keyboard accuracy heatmap
        self.render_keyboard_heatmap(layout[0], buf);

        // Slowest/Fastest keys
        let key_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(layout[1]);

        self.render_slowest_keys(key_layout[0], buf);
        self.render_fastest_keys(key_layout[1], buf);
        self.render_char_stats(key_layout[2], buf);

        // Word/Character stats summary
        self.render_overall_stats(layout[2], buf);
    }

    fn render_keyboard_heatmap(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Keyboard Accuracy ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 || inner.width < 40 {
            return;
        }

        let rows: &[&[char]] = &[
            &['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
            &['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
            &['z', 'x', 'c', 'v', 'b', 'n', 'm'],
        ];
        let offsets: &[u16] = &[1, 3, 5];
        let key_width: u16 = 4;

        for (row_idx, row) in rows.iter().enumerate() {
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

                let accuracy = self.get_key_accuracy(key);
                let color = if accuracy >= 100.0 {
                    colors.text_pending()
                } else if accuracy >= 90.0 {
                    colors.warning()
                } else if accuracy > 0.0 {
                    colors.error()
                } else {
                    colors.text_pending()
                };

                let display = format!("[{key}]");
                buf.set_string(x, y, &display, Style::default().fg(color).bg(colors.bg()));
            }
        }
    }

    fn get_key_accuracy(&self, key: char) -> f64 {
        let mut correct = 0usize;
        let mut total = 0usize;

        for result in self.history {
            for kt in &result.per_key_times {
                if kt.key == key {
                    total += 1;
                    if kt.correct {
                        correct += 1;
                    }
                }
            }
        }

        if total == 0 {
            return 0.0;
        }
        correct as f64 / total as f64 * 100.0
    }

    fn render_slowest_keys(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Slowest ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        let mut key_times: Vec<(char, f64)> = self
            .key_stats
            .stats
            .iter()
            .filter(|(_, s)| s.sample_count > 0)
            .map(|(&ch, s)| (ch, s.filtered_time_ms))
            .collect();
        key_times.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        for (i, (ch, time)) in key_times.iter().take(5).enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let text = format!("  '{ch}'  {time:.0}ms");
            buf.set_string(
                inner.x,
                y,
                &text,
                Style::default().fg(colors.error()),
            );
        }
    }

    fn render_fastest_keys(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Fastest ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        let mut key_times: Vec<(char, f64)> = self
            .key_stats
            .stats
            .iter()
            .filter(|(_, s)| s.sample_count > 0)
            .map(|(&ch, s)| (ch, s.filtered_time_ms))
            .collect();
        key_times.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        for (i, (ch, time)) in key_times.iter().take(5).enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let text = format!("  '{ch}'  {time:.0}ms");
            buf.set_string(
                inner.x,
                y,
                &text,
                Style::default().fg(colors.success()),
            );
        }
    }

    fn render_char_stats(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Key Stats ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        let mut total_correct = 0usize;
        let mut total_incorrect = 0usize;

        for result in self.history {
            total_correct += result.correct;
            total_incorrect += result.incorrect;
        }

        let total = total_correct + total_incorrect;
        let overall_acc = if total > 0 {
            total_correct as f64 / total as f64 * 100.0
        } else {
            0.0
        };

        let lines = [
            format!("  Total:   {total}"),
            format!("  Correct: {total_correct}"),
            format!("  Wrong:   {total_incorrect}"),
            format!("  Acc:     {overall_acc:.1}%"),
        ];

        for (i, line) in lines.iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            buf.set_string(inner.x, y, line, Style::default().fg(colors.fg()));
        }
    }

    fn render_overall_stats(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Overall ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        let total_chars: usize = self.history.iter().map(|r| r.total_chars).sum();
        let total_correct: usize = self.history.iter().map(|r| r.correct).sum();
        let total_incorrect: usize = self.history.iter().map(|r| r.incorrect).sum();
        let total_time: f64 = self.history.iter().map(|r| r.elapsed_secs).sum();

        let lines = vec![
            Line::from(vec![
                Span::styled("  Characters typed: ", Style::default().fg(colors.fg())),
                Span::styled(
                    format!("{total_chars}"),
                    Style::default().fg(colors.accent()),
                ),
                Span::styled("    Correct: ", Style::default().fg(colors.fg())),
                Span::styled(
                    format!("{total_correct}"),
                    Style::default().fg(colors.success()),
                ),
                Span::styled("    Errors: ", Style::default().fg(colors.fg())),
                Span::styled(
                    format!("{total_incorrect}"),
                    Style::default().fg(if total_incorrect > 0 {
                        colors.error()
                    } else {
                        colors.success()
                    }),
                ),
                Span::styled("    Total time: ", Style::default().fg(colors.fg())),
                Span::styled(
                    format_duration(total_time),
                    Style::default().fg(colors.text_pending()),
                ),
            ]),
        ];

        Paragraph::new(lines).render(inner, buf);
    }
}

fn render_text_bar(
    label: &str,
    ratio: f64,
    fill_color: ratatui::style::Color,
    empty_color: ratatui::style::Color,
    area: Rect,
    buf: &mut Buffer,
) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    // Label on first line
    buf.set_string(
        area.x,
        area.y,
        label,
        Style::default().fg(fill_color),
    );

    // Bar on second line
    let bar_width = (area.width as usize).saturating_sub(4);
    let filled = (ratio * bar_width as f64) as usize;

    let bar_y = area.y + 1;
    buf.set_string(area.x, bar_y, "  ", Style::default());

    for i in 0..bar_width {
        let x = area.x + 2 + i as u16;
        if x >= area.x + area.width {
            break;
        }
        let (ch, color) = if i < filled {
            ('█', fill_color)
        } else {
            ('░', empty_color)
        };
        buf.set_string(x, bar_y, &ch.to_string(), Style::default().fg(color));
    }
}

fn render_accuracy_chart(
    data: &[(f64, f64)],
    theme: &Theme,
    area: Rect,
    buf: &mut Buffer,
) {
    use ratatui::symbols;
    use ratatui::widgets::{Axis, Chart, Dataset, GraphType};

    let colors = &theme.colors;

    if data.is_empty() {
        let block = Block::bordered()
            .title(" Accuracy Over Time ")
            .border_style(Style::default().fg(colors.border()));
        block.render(area, buf);
        return;
    }

    let max_x = data.last().map(|(x, _)| *x).unwrap_or(1.0);

    let dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(colors.success()))
        .data(data);

    let chart = Chart::new(vec![dataset])
        .block(
            Block::bordered()
                .title(" Accuracy Over Time ")
                .border_style(Style::default().fg(colors.border())),
        )
        .x_axis(
            Axis::default()
                .title("Lesson")
                .style(Style::default().fg(colors.text_pending()))
                .bounds([0.0, max_x]),
        )
        .y_axis(
            Axis::default()
                .title("%")
                .style(Style::default().fg(colors.text_pending()))
                .bounds([80.0, 100.0]),
        );

    chart.render(area, buf);
}

fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let hours = total / 3600;
    let mins = (total % 3600) / 60;
    let s = total % 60;
    if hours > 0 {
        format!("{hours}h {mins}m {s}s")
    } else if mins > 0 {
        format!("{mins}m {s}s")
    } else {
        format!("{s}s")
    }
}
