use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::engine::key_stats::KeyStatsStore;
use crate::session::result::DrillResult;
use crate::ui::components::activity_heatmap::ActivityHeatmap;
use crate::ui::theme::Theme;

pub struct StatsDashboard<'a> {
    pub history: &'a [DrillResult],
    pub key_stats: &'a KeyStatsStore,
    pub active_tab: usize,
    pub target_wpm: u32,
    pub theme: &'a Theme,
    pub history_selected: usize,
    pub history_confirm_delete: bool,
}

impl<'a> StatsDashboard<'a> {
    pub fn new(
        history: &'a [DrillResult],
        key_stats: &'a KeyStatsStore,
        active_tab: usize,
        target_wpm: u32,
        theme: &'a Theme,
        history_selected: usize,
        history_confirm_delete: bool,
    ) -> Self {
        Self {
            history,
            key_stats,
            active_tab,
            target_wpm,
            theme,
            history_selected,
            history_confirm_delete,
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
                "No drills completed yet. Start typing!",
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
        let tabs = ["[1] Dashboard", "[2] History", "[3] Keystrokes"];
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

        // Tab content — wide mode shows two panels side by side
        let is_wide = area.width > 170;
        if is_wide {
            let panels = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(layout[1]);

            // Left panel: active tab, Right panel: next tab
            let left_tab = self.active_tab;
            let right_tab = (self.active_tab + 1) % 3;

            self.render_tab(left_tab, panels[0], buf);
            self.render_tab(right_tab, panels[1], buf);
        } else {
            self.render_tab(self.active_tab, layout[1], buf);
        }

        // Footer
        let footer_text = if self.active_tab == 1 {
            "  [ESC] Back  [Tab] Next tab  [j/k] Navigate  [x] Delete"
        } else {
            "  [ESC] Back  [Tab] Next tab  [1/2/3] Switch tab"
        };
        let footer = Paragraph::new(Line::from(Span::styled(
            footer_text,
            Style::default().fg(colors.accent()),
        )));
        footer.render(layout[2], buf);

        // Confirmation dialog overlay
        if self.history_confirm_delete && self.active_tab == 1 {
            let dialog_width = 34u16;
            let dialog_height = 5u16;
            let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
            let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
            let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

            let idx = self.history.len().saturating_sub(self.history_selected);
            let dialog_text = format!("Delete session #{idx}? (y/n)");

            let dialog = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  {dialog_text}  "),
                    Style::default().fg(colors.fg()),
                )),
            ])
            .block(
                Block::bordered()
                    .title(" Confirm ")
                    .border_style(Style::default().fg(colors.error()))
                    .style(Style::default().bg(colors.bg())),
            );
            dialog.render(dialog_area, buf);
        }
    }
}

impl StatsDashboard<'_> {
    fn render_tab(&self, tab: usize, area: Rect, buf: &mut Buffer) {
        match tab {
            0 => self.render_dashboard_tab(area, buf),
            1 => self.render_history_tab(area, buf),
            2 => self.render_keystrokes_tab(area, buf),
            _ => {}
        }
    }

    fn render_dashboard_tab(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6), // summary stats bordered box
                Constraint::Length(3), // progress bars
                Constraint::Min(8),   // charts
            ])
            .split(area);

        // Summary stats as bordered table
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

        let summary_block = Block::bordered()
            .title(" Summary ")
            .border_style(Style::default().fg(colors.border()));
        let summary_inner = summary_block.inner(layout[0]);
        summary_block.render(layout[0], buf);

        let summary = vec![
            Line::from(vec![
                Span::styled("  Drills: ", Style::default().fg(colors.fg())),
                Span::styled(
                    &*total_str,
                    Style::default()
                        .fg(colors.accent())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("    Avg WPM: ", Style::default().fg(colors.fg())),
                Span::styled(&*avg_wpm_str, Style::default().fg(colors.accent())),
                Span::styled("    Best WPM: ", Style::default().fg(colors.fg())),
                Span::styled(
                    &*best_wpm_str,
                    Style::default()
                        .fg(colors.success())
                        .add_modifier(Modifier::BOLD),
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
        Paragraph::new(summary).render(summary_inner, buf);

        // Progress bars
        self.render_progress_bars(layout[1], buf);

        // Charts: WPM bar graph + accuracy trend
        let chart_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(layout[2]);

        self.render_wpm_bar_graph(chart_layout[0], buf);
        self.render_accuracy_chart(chart_layout[1], buf);
    }

    fn render_wpm_bar_graph(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let target_label = format!(" WPM per Drill (Last 20, Target: {}) ", self.target_wpm);
        let block = Block::bordered()
            .title(target_label)
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 10 || inner.height < 3 {
            return;
        }

        let recent: Vec<f64> = self
            .history
            .iter()
            .rev()
            .take(20)
            .map(|r| r.wpm)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        if recent.is_empty() {
            return;
        }

        let max_wpm = recent.iter().fold(0.0f64, |a, &b| a.max(b)).max(10.0);
        let target = self.target_wpm as f64;

        // Reserve left margin for Y-axis labels
        let y_label_width: u16 = 4;
        let chart_x = inner.x + y_label_width;
        let chart_width = inner.width.saturating_sub(y_label_width);

        if chart_width < 5 {
            return;
        }

        let bar_count = (chart_width as usize).min(recent.len());
        let bar_spacing = if bar_count > 0 {
            chart_width / bar_count as u16
        } else {
            return;
        };

        // Y-axis labels (max, mid, 0)
        let max_label = format!("{:.0}", max_wpm);
        let mid_label = format!("{:.0}", max_wpm / 2.0);
        buf.set_string(inner.x, inner.y, &max_label, Style::default().fg(colors.text_pending()));
        if inner.height > 3 {
            let mid_y = inner.y + inner.height / 2;
            buf.set_string(inner.x, mid_y, &mid_label, Style::default().fg(colors.text_pending()));
        }
        buf.set_string(inner.x, inner.y + inner.height - 1, "0", Style::default().fg(colors.text_pending()));

        let bar_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        // Render each bar as a column
        let start_idx = recent.len().saturating_sub(bar_count);
        for (i, &wpm) in recent[start_idx..].iter().enumerate() {
            let x = chart_x + i as u16 * bar_spacing;
            if x >= chart_x + chart_width {
                break;
            }

            let ratio = (wpm / max_wpm).clamp(0.0, 1.0);
            let bar_height = (ratio * (inner.height as f64 - 1.0)).round() as usize;
            let color = if wpm >= target {
                colors.success()
            } else {
                colors.error()
            };

            // Draw bar from bottom up
            for row in 0..inner.height.saturating_sub(1) {
                let y = inner.y + inner.height - 1 - row;
                let row_idx = row as usize;
                if row_idx < bar_height {
                    let ch = if row_idx + 1 == bar_height {
                        // Top of bar - use fractional char
                        let frac = (ratio * (inner.height as f64 - 1.0)) - bar_height as f64 + 1.0;
                        let idx = ((frac * 7.0).round() as usize).min(7);
                        bar_chars[idx]
                    } else {
                        '█'
                    };
                    buf.set_string(x, y, &ch.to_string(), Style::default().fg(color));
                }
            }

            // WPM label on top row
            if bar_spacing >= 3 {
                let label = format!("{wpm:.0}");
                buf.set_string(x, inner.y, &label, Style::default().fg(colors.text_pending()));
            }
        }
    }

    fn render_accuracy_chart(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::symbols;
        use ratatui::widgets::{Axis, Chart, Dataset, GraphType};

        let colors = &self.theme.colors;

        let data: Vec<(f64, f64)> = self
            .history
            .iter()
            .rev()
            .take(50)
            .enumerate()
            .map(|(i, r)| (i as f64, r.accuracy))
            .collect();

        if data.is_empty() {
            let block = Block::bordered()
                .title(" Accuracy % (Last 50 Drills) ")
                .border_style(Style::default().fg(colors.border()));
            block.render(area, buf);
            return;
        }

        let max_x = data.last().map(|(x, _)| *x).unwrap_or(1.0);

        let dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(colors.success()))
            .data(&data);

        let chart = Chart::new(vec![dataset])
            .block(
                Block::bordered()
                    .title(" Accuracy % (Last 50 Drills) ")
                    .border_style(Style::default().fg(colors.border())),
            )
            .x_axis(
                Axis::default()
                    .title("Drill #")
                    .style(Style::default().fg(colors.text_pending()))
                    .bounds([0.0, max_x]),
            )
            .y_axis(
                Axis::default()
                    .title("Accuracy %")
                    .style(Style::default().fg(colors.text_pending()))
                    .bounds([80.0, 100.0]),
            );

        chart.render(area, buf);
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
        let wpm_color = if wpm_pct >= 100.0 {
            colors.success()
        } else {
            colors.accent()
        };
        let wpm_label = format!("  WPM: {avg_wpm:.0}/{} ({wpm_pct:.0}%)", self.target_wpm);
        render_text_bar(
            &wpm_label,
            wpm_pct / 100.0,
            wpm_color,
            colors.bar_empty(),
            layout[0],
            buf,
        );

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
        render_text_bar(
            &acc_label,
            acc_pct / 100.0,
            acc_color,
            colors.bar_empty(),
            layout[1],
            buf,
        );

        // Level progress
        let total_score: f64 = self.history.iter().map(|r| r.wpm * r.accuracy / 100.0).sum();
        let level = ((total_score / 100.0).sqrt() as u32).max(1);
        let next_level_score = ((level + 1) as f64).powi(2) * 100.0;
        let current_level_score = (level as f64).powi(2) * 100.0;
        let level_pct = ((total_score - current_level_score)
            / (next_level_score - current_level_score))
            .clamp(0.0, 1.0);
        let level_label = format!("  Lvl {level} ({:.0}%)", level_pct * 100.0);
        render_text_bar(
            &level_label,
            level_pct,
            colors.focused_key(),
            colors.bar_empty(),
            layout[2],
            buf,
        );
    }

    fn render_history_tab(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(8)])
            .split(area);

        // Recent tests bordered table
        let table_block = Block::bordered()
            .title(" Recent Sessions ")
            .border_style(Style::default().fg(colors.border()));
        let table_inner = table_block.inner(layout[0]);
        table_block.render(layout[0], buf);

        let header = Line::from(vec![Span::styled(
            "   #     WPM    Raw    Acc%    Time      Date",
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        )]);

        let mut lines = vec![
            header,
            Line::from(Span::styled(
                "  ─────────────────────────────────────────────",
                Style::default().fg(colors.border()),
            )),
        ];

        let recent: Vec<&DrillResult> = self.history.iter().rev().take(20).collect();
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

            // WPM indicator
            let wpm_indicator = if result.wpm >= self.target_wpm as f64 {
                "+"
            } else {
                " "
            };

            let row = format!(
                " {wpm_indicator}{idx_str}  {wpm_str}  {raw_str}  {acc_str}  {time_str:>6}  {date_str}"
            );

            let acc_color = if result.accuracy >= 95.0 {
                colors.success()
            } else if result.accuracy >= 85.0 {
                colors.warning()
            } else {
                colors.error()
            };

            let is_selected = i == self.history_selected;
            let style = if is_selected {
                Style::default().fg(acc_color).bg(colors.accent_dim())
            } else {
                Style::default().fg(acc_color)
            };

            lines.push(Line::from(Span::styled(row, style)));
        }

        Paragraph::new(lines).render(table_inner, buf);

        // Per-key speed distribution
        self.render_per_key_speed(layout[1], buf);
    }

    fn render_per_key_speed(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Avg Key Time by Character ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        let columns_per_row: usize = 13;
        let col_width: u16 = 4;
        let row_height: u16 = 3;

        if inner.width < columns_per_row as u16 * col_width || inner.height < row_height {
            return;
        }

        let letters: Vec<char> = ('a'..='z').collect();
        let row_count = if inner.height >= row_height * 2 { 2 } else { 1 };
        let max_time = letters
            .iter()
            .filter_map(|&ch| self.key_stats.stats.get(&ch))
            .map(|s| s.filtered_time_ms)
            .fold(0.0f64, f64::max)
            .max(1.0);

        let bar_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        for (i, &ch) in letters.iter().take(columns_per_row * row_count).enumerate() {
            let row = i / columns_per_row;
            let col = i % columns_per_row;
            let x = inner.x + (col as u16 * col_width);
            let y = inner.y + row as u16 * row_height;

            if x + col_width > inner.x + inner.width || y + 2 >= inner.y + inner.height {
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
            buf.set_string(x, y, &ch.to_string(), Style::default().fg(color));

            // Bar indicator
            let bar_char = if time > 0.0 {
                let idx = ((ratio * 7.0).round() as usize).min(7);
                bar_chars[idx]
            } else {
                ' '
            };
            buf.set_string(x, y + 1, &bar_char.to_string(), Style::default().fg(color));

            // Time label on row 3, render seconds when value exceeds 999ms.
            if time > 0.0 {
                let time_label = if time > 999.0 {
                    format!("({:.0}s)", time / 1000.0)
                } else {
                    format!("{time:.0}")
                };
                let label = if time_label.len() > col_width as usize {
                    let start = time_label.len() - col_width as usize;
                    &time_label[start..]
                } else {
                    &time_label
                };
                let label_x = x + col_width.saturating_sub(label.len() as u16);
                buf.set_string(
                    label_x,
                    y + 2,
                    label,
                    Style::default().fg(colors.text_pending()),
                );
            }
        }
    }

    fn render_keystrokes_tab(&self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(12), // Activity heatmap
                Constraint::Length(7),  // Keyboard accuracy heatmap
                Constraint::Min(5),    // Slowest/Fastest/Stats
                Constraint::Length(5),  // Overall stats
            ])
            .split(area);

        // Activity heatmap
        let heatmap = ActivityHeatmap::new(self.history, self.theme);
        heatmap.render(layout[0], buf);

        // Keyboard accuracy heatmap with percentages
        self.render_keyboard_heatmap(layout[1], buf);

        // Slowest/Fastest/Worst keys
        let key_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(layout[2]);

        self.render_slowest_keys(key_layout[0], buf);
        self.render_fastest_keys(key_layout[1], buf);
        self.render_worst_accuracy_keys(key_layout[2], buf);

        // Overall stats
        self.render_overall_stats(layout[3], buf);
    }

    fn render_keyboard_heatmap(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Keyboard Accuracy % ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 || inner.width < 50 {
            return;
        }

        let rows: &[&[char]] = &[
            &['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
            &['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
            &['z', 'x', 'c', 'v', 'b', 'n', 'm'],
        ];
        let offsets: &[u16] = &[1, 3, 5];
        let key_width: u16 = 5; // wider to fit accuracy %

        for (row_idx, row) in rows.iter().enumerate() {
            let y = inner.y + row_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let offset = offsets.get(row_idx).copied().unwrap_or(0);

            for (col_idx, &key) in row.iter().enumerate() {
                let x = inner.x + offset + col_idx as u16 * key_width;
                if x + key_width > inner.x + inner.width {
                    break;
                }

                let accuracy = self.get_key_accuracy(key);
                let (fg_color, bg_color) = if accuracy <= 0.0 {
                    (colors.text_pending(), colors.bg())
                } else if accuracy >= 98.0 {
                    (colors.success(), colors.bg())
                } else if accuracy >= 90.0 {
                    (colors.warning(), colors.bg())
                } else {
                    (colors.error(), colors.bg())
                };

                let display = if accuracy > 0.0 {
                    let pct = accuracy.round() as u32;
                    format!("{key}{pct:>3}")
                } else {
                    format!("{key}   ")
                };
                buf.set_string(
                    x,
                    y,
                    &display,
                    Style::default().fg(fg_color).bg(bg_color),
                );
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
            .title(" Slowest Keys (ms) ")
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
            .title(" Fastest Keys (ms) ")
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

    fn render_worst_accuracy_keys(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Worst Accuracy Keys (%) ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        // Compute accuracy for each key
        let mut key_accuracies: Vec<(char, f64, usize)> = ('a'..='z')
            .filter_map(|ch| {
                let mut correct = 0usize;
                let mut total = 0usize;
                for result in self.history {
                    for kt in &result.per_key_times {
                        if kt.key == ch {
                            total += 1;
                            if kt.correct {
                                correct += 1;
                            }
                        }
                    }
                }
                if total >= 5 {
                    let acc = correct as f64 / total as f64 * 100.0;
                    Some((ch, acc, total))
                } else {
                    None
                }
            })
            .collect();

        key_accuracies.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        if key_accuracies.is_empty() {
            buf.set_string(
                inner.x,
                inner.y,
                "  Not enough data",
                Style::default().fg(colors.text_pending()),
            );
            return;
        }

        for (i, (ch, acc, _total)) in key_accuracies.iter().take(5).enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let badge = format!("  '{ch}'  {acc:.1}%");
            let color = if *acc >= 95.0 {
                colors.warning()
            } else {
                colors.error()
            };
            buf.set_string(inner.x, y, &badge, Style::default().fg(color));
        }
    }

    fn render_overall_stats(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Overall Totals ")
            .border_style(Style::default().fg(colors.border()));
        let inner = block.inner(area);
        block.render(area, buf);

        let total_chars: usize = self.history.iter().map(|r| r.total_chars).sum();
        let total_correct: usize = self.history.iter().map(|r| r.correct).sum();
        let total_incorrect: usize = self.history.iter().map(|r| r.incorrect).sum();
        let total_time: f64 = self.history.iter().map(|r| r.elapsed_secs).sum();

        let lines = vec![Line::from(vec![
            Span::styled("  Characters: ", Style::default().fg(colors.fg())),
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
            Span::styled("    Time: ", Style::default().fg(colors.fg())),
            Span::styled(
                format_duration(total_time),
                Style::default().fg(colors.text_pending()),
            ),
        ])];

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

    // Bar on second line using ┃ filled / dim ┃ empty
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
