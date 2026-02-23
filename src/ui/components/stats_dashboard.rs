use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};
use std::collections::{BTreeSet, HashMap};

use crate::engine::key_stats::KeyStatsStore;
use crate::keyboard::display::{self, BACKSPACE, ENTER, MODIFIER_SENTINELS, SPACE, TAB};
use crate::keyboard::model::KeyboardModel;
use crate::session::result::DrillResult;
use crate::ui::components::activity_heatmap::ActivityHeatmap;
use crate::ui::theme::Theme;

pub struct StatsDashboard<'a> {
    pub history: &'a [DrillResult],
    pub key_stats: &'a KeyStatsStore,
    pub active_tab: usize,
    pub target_wpm: u32,
    pub overall_unlocked: usize,
    pub overall_mastered: usize,
    pub overall_total: usize,
    pub theme: &'a Theme,
    pub history_selected: usize,
    pub history_confirm_delete: bool,
    pub keyboard_model: &'a KeyboardModel,
}

impl<'a> StatsDashboard<'a> {
    pub fn new(
        history: &'a [DrillResult],
        key_stats: &'a KeyStatsStore,
        active_tab: usize,
        target_wpm: u32,
        overall_unlocked: usize,
        overall_mastered: usize,
        overall_total: usize,
        theme: &'a Theme,
        history_selected: usize,
        history_confirm_delete: bool,
        keyboard_model: &'a KeyboardModel,
    ) -> Self {
        Self {
            history,
            key_stats,
            active_tab,
            target_wpm,
            overall_unlocked,
            overall_mastered,
            overall_total,
            theme,
            history_selected,
            history_confirm_delete,
            keyboard_model,
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
        let tabs = [
            "[1] Dashboard",
            "[2] History",
            "[3] Activity",
            "[4] Accuracy",
            "[5] Timing",
        ];
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
                vec![Span::styled(format!(" {label} "), style), Span::raw("  ")]
            })
            .collect();
        Paragraph::new(Line::from(tab_spans)).render(layout[0], buf);

        // Render only one tab at a time so each tab gets full breathing room.
        self.render_tab(self.active_tab, layout[1], buf);

        // Footer
        let footer_text = if self.active_tab == 1 {
            "  [ESC] Back  [Tab] Next tab  [1-5] Switch tab  [j/k] Navigate  [x] Delete"
        } else {
            "  [ESC] Back  [Tab] Next tab  [1-5] Switch tab"
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

            Clear.render(dialog_area, buf);
            let dialog = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  {dialog_text}  "),
                    Style::default().fg(colors.fg()),
                )),
            ])
            .style(Style::default().bg(colors.bg()))
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
            2 => self.render_activity_tab(area, buf),
            3 => self.render_accuracy_tab(area, buf),
            4 => self.render_timing_tab(area, buf),
            _ => {}
        }
    }

    fn render_activity_tab(&self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(9), Constraint::Length(6)])
            .split(area);
        ActivityHeatmap::new(self.history, self.theme).render(layout[0], buf);
        self.render_activity_stats(layout[1], buf);
    }

    fn render_accuracy_tab(&self, area: Rect, buf: &mut Buffer) {
        // Give keyboard as much height as available (up to 12), reserving 6 for lists below
        let kbd_height: u16 = area.height.saturating_sub(6).min(12).max(7);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(kbd_height), Constraint::Min(6)])
            .split(area);
        self.render_keyboard_heatmap(layout[0], buf);
        let lists = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(layout[1]);
        self.render_worst_accuracy_keys(lists[0], buf);
        self.render_best_accuracy_keys(lists[1], buf);
    }

    fn render_timing_tab(&self, area: Rect, buf: &mut Buffer) {
        // Give keyboard as much height as available (up to 12), reserving 6 for lists below
        let kbd_height: u16 = area.height.saturating_sub(6).min(12).max(7);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(kbd_height), Constraint::Min(6)])
            .split(area);
        self.render_keyboard_timing(layout[0], buf);

        let lists = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(layout[1]);
        self.render_slowest_keys(lists[0], buf);
        self.render_fastest_keys(lists[1], buf);
    }

    fn render_dashboard_tab(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6), // summary stats bordered box
                Constraint::Length(3), // progress bars
                Constraint::Min(8),    // charts
            ])
            .split(area);

        // Summary stats as bordered table
        let avg_wpm = self.history.iter().map(|r| r.wpm).sum::<f64>() / self.history.len() as f64;
        let best_wpm = self.history.iter().map(|r| r.wpm).fold(0.0f64, f64::max);
        let avg_accuracy =
            self.history.iter().map(|r| r.accuracy).sum::<f64>() / self.history.len() as f64;
        let total_time: f64 = self.history.iter().map(|r| r.elapsed_secs).sum();

        let total_str = format!("{}", self.history.len());
        let avg_wpm_str = format!("{avg_wpm:.0}");
        let best_wpm_str = format!("{best_wpm:.0}");
        let avg_acc_str = format!("{avg_accuracy:.1}%");
        let time_str = format_duration(total_time);

        let summary_block = Block::bordered()
            .title(Line::from(Span::styled(
                " Summary ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
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
            .title(Line::from(Span::styled(
                target_label,
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
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
        buf.set_string(
            inner.x,
            inner.y,
            &max_label,
            Style::default().fg(colors.text_pending()),
        );
        if inner.height > 3 {
            let mid_y = inner.y + inner.height / 2;
            buf.set_string(
                inner.x,
                mid_y,
                &mid_label,
                Style::default().fg(colors.text_pending()),
            );
        }
        buf.set_string(
            inner.x,
            inner.y + inner.height - 1,
            "0",
            Style::default().fg(colors.text_pending()),
        );

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
            if bar_height == 0 {
                let y = inner.y + inner.height - 1;
                buf.set_string(x, y, "▁", Style::default().fg(colors.text_pending()));
            }

            // WPM label on top row
            if bar_spacing >= 3 {
                let label = format!("{wpm:.0}");
                buf.set_string(
                    x,
                    inner.y,
                    &label,
                    Style::default().fg(colors.text_pending()),
                );
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
                .title(Line::from(Span::styled(
                    " Accuracy % (Last 50 Drills) ",
                    Style::default()
                        .fg(colors.accent())
                        .add_modifier(Modifier::BOLD),
                )))
                .border_style(Style::default().fg(colors.accent()))
                .style(Style::default().bg(colors.bg()));
            block.render(area, buf);
            return;
        }

        let max_x = data.last().map(|(x, _)| *x).unwrap_or(1.0);

        let dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(colors.success()).bg(colors.bg()))
            .data(&data);

        let chart = Chart::new(vec![dataset])
            .style(Style::default().fg(colors.fg()).bg(colors.bg()))
            .block(
                Block::bordered()
                    .title(Line::from(Span::styled(
                        " Accuracy % (Last 50 Drills) ",
                        Style::default()
                            .fg(colors.accent())
                            .add_modifier(Modifier::BOLD),
                    )))
                    .border_style(Style::default().fg(colors.accent()))
                    .style(Style::default().bg(colors.bg())),
            )
            .x_axis(
                Axis::default()
                    .title("Drill #")
                    .style(Style::default().fg(colors.text_pending()).bg(colors.bg()))
                    .bounds([0.0, max_x]),
            )
            .y_axis(
                Axis::default()
                    .title("Accuracy %")
                    .style(Style::default().fg(colors.text_pending()).bg(colors.bg()))
                    .labels(vec![
                        Span::styled(
                            "80",
                            Style::default().fg(colors.text_pending()).bg(colors.bg()),
                        ),
                        Span::styled(
                            "90",
                            Style::default().fg(colors.text_pending()).bg(colors.bg()),
                        ),
                        Span::styled(
                            "100",
                            Style::default().fg(colors.text_pending()).bg(colors.bg()),
                        ),
                    ])
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

        let avg_wpm = self.history.iter().map(|r| r.wpm).sum::<f64>() / self.history.len() as f64;
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

        // Overall key progress (unlocked coverage + mastered detail).
        let key_pct = if self.overall_total > 0 {
            self.overall_unlocked as f64 / self.overall_total as f64
        } else {
            0.0
        };
        let level_label = format!(
            "  Keys: {}/{} ({} mastered)",
            self.overall_unlocked, self.overall_total, self.overall_mastered
        );
        render_text_bar(
            &level_label,
            key_pct,
            colors.focused_key(),
            colors.bar_empty(),
            layout[2],
            buf,
        );
    }

    fn render_history_tab(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        // Recent tests bordered table
        let table_block = Block::bordered()
            .title(Line::from(Span::styled(
                " Recent Sessions ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let table_inner = table_block.inner(area);
        table_block.render(area, buf);

        let header = Line::from(vec![Span::styled(
            "   #     WPM    Raw    Acc%    Time      Date       Mode       Ranked  Partial",
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        )]);

        let mut lines = vec![
            header,
            Line::from(Span::styled(
                "  ─────────────────────────────────────────────────────────────────────",
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

            let rank_str = if result.ranked { "yes" } else { "no" };
            let partial_pct = if result.partial {
                result.completion_percent
            } else {
                100.0
            };
            let partial_str = format!("{:>6.0}%", partial_pct);
            let row = format!(
                " {wpm_indicator}{idx_str}  {wpm_str}  {raw_str}  {acc_str}  {time_str:>6}  {date_str}  {mode:<9}  {rank_str:<6}  {partial_str:>7}",
                mode = result.drill_mode,
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
            } else if result.partial {
                Style::default().fg(colors.warning())
            } else if !result.ranked {
                // Muted styling for unranked drills
                Style::default().fg(colors.text_pending())
            } else {
                Style::default().fg(acc_color)
            };

            lines.push(Line::from(Span::styled(row, style)));
        }

        Paragraph::new(lines).render(table_inner, buf);
    }

    fn render_keyboard_heatmap(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(Line::from(Span::styled(
                " Keyboard Accuracy % ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 {
            return;
        }

        let (key_width, key_step) = if inner.width >= required_kbd_width(5, 6) {
            (5, 6)
        } else {
            return;
        };
        let show_shifted = inner.height >= 10; // 4 base + 4 shifted + 1 mod row + 1 spare
        let all_rows = &self.keyboard_model.rows;
        let offsets: &[u16] = &[0, 2, 3, 4];

        for (row_idx, row) in all_rows.iter().enumerate() {
            let base_y = if show_shifted {
                inner.y + row_idx as u16 * 2 + 1 // shifted on top, base below
            } else {
                inner.y + row_idx as u16
            };

            if base_y >= inner.y + inner.height {
                break;
            }

            let offset = offsets.get(row_idx).copied().unwrap_or(0);

            // Shifted row (dimmer)
            if show_shifted {
                let shifted_y = base_y - 1;
                if shifted_y >= inner.y {
                    for (col_idx, physical_key) in row.iter().enumerate() {
                        let x = inner.x + offset + col_idx as u16 * key_step;
                        if x + key_width > inner.x + inner.width {
                            break;
                        }

                        let key = physical_key.shifted;
                        let accuracy = self.get_key_accuracy(key);
                        let fg_color = accuracy_color(accuracy, colors);

                        let display = format_accuracy_cell(key, accuracy, key_width);
                        buf.set_string(
                            x,
                            shifted_y,
                            &display,
                            Style::default().fg(fg_color).add_modifier(Modifier::DIM),
                        );
                    }
                }
            }

            // Base row
            for (col_idx, physical_key) in row.iter().enumerate() {
                let x = inner.x + offset + col_idx as u16 * key_step;
                if x + key_width > inner.x + inner.width {
                    break;
                }

                let key = physical_key.base;
                let accuracy = self.get_key_accuracy(key);
                let fg_color = accuracy_color(accuracy, colors);

                let display = format_accuracy_cell(key, accuracy, key_width);
                buf.set_string(x, base_y, &display, Style::default().fg(fg_color));
            }

        }

        // Modifier key stats row below the keyboard, spread across keyboard width
        let kbd_width = all_rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let off = offsets.get(i).copied().unwrap_or(0);
                off + row.len() as u16 * key_step
            })
            .max()
            .unwrap_or(inner.width)
            .min(inner.width);
        let mod_y = if show_shifted {
            inner.y + all_rows.len() as u16 * 2 + 1
        } else {
            inner.y + all_rows.len() as u16
        };
        if mod_y < inner.y + inner.height {
            let mod_keys: &[(char, &str)] = &[
                (TAB, display::key_short_label(TAB)),
                (SPACE, display::key_short_label(SPACE)),
                (ENTER, display::key_short_label(ENTER)),
                (BACKSPACE, display::key_short_label(BACKSPACE)),
            ];
            let labels: Vec<String> = mod_keys
                .iter()
                .map(|&(key, label)| {
                    let accuracy = self.get_key_accuracy(key);
                    format_accuracy_cell_label(label, accuracy, key_width)
                })
                .collect();
            let positions = spread_labels(&labels, kbd_width);
            for (i, &(key, _)) in mod_keys.iter().enumerate() {
                let accuracy = self.get_key_accuracy(key);
                let fg_color = accuracy_color(accuracy, colors);
                buf.set_string(
                    inner.x + positions[i],
                    mod_y,
                    &labels[i],
                    Style::default().fg(fg_color),
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

    fn get_key_time_ms(&self, key: char) -> f64 {
        self.key_stats
            .stats
            .get(&key)
            .filter(|s| s.sample_count > 0)
            .map(|s| s.filtered_time_ms)
            .unwrap_or(0.0)
    }

    fn render_keyboard_timing(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(Line::from(Span::styled(
                " Keyboard Timing (ms) ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 {
            return;
        }

        let (key_width, key_step) = if inner.width >= required_kbd_width(5, 6) {
            (5, 6)
        } else {
            return;
        };
        let show_shifted = inner.height >= 10; // 4 base + 4 shifted + 1 mod row + 1 spare
        let all_rows = &self.keyboard_model.rows;
        let offsets: &[u16] = &[0, 2, 3, 4];

        for (row_idx, row) in all_rows.iter().enumerate() {
            let base_y = if show_shifted {
                inner.y + row_idx as u16 * 2 + 1
            } else {
                inner.y + row_idx as u16
            };

            if base_y >= inner.y + inner.height {
                break;
            }

            let offset = offsets.get(row_idx).copied().unwrap_or(0);

            if show_shifted {
                let shifted_y = base_y - 1;
                if shifted_y >= inner.y {
                    for (col_idx, physical_key) in row.iter().enumerate() {
                        let x = inner.x + offset + col_idx as u16 * key_step;
                        if x + key_width > inner.x + inner.width {
                            break;
                        }

                        let key = physical_key.shifted;
                        let time_ms = self.get_key_time_ms(key);
                        let fg_color = timing_color(time_ms, colors);
                        let display = format_timing_cell(key, time_ms, key_width);
                        buf.set_string(
                            x,
                            shifted_y,
                            &display,
                            Style::default().fg(fg_color).add_modifier(Modifier::DIM),
                        );
                    }
                }
            }

            for (col_idx, physical_key) in row.iter().enumerate() {
                let x = inner.x + offset + col_idx as u16 * key_step;
                if x + key_width > inner.x + inner.width {
                    break;
                }

                let key = physical_key.base;
                let time_ms = self.get_key_time_ms(key);
                let fg_color = timing_color(time_ms, colors);
                let display = format_timing_cell(key, time_ms, key_width);
                buf.set_string(x, base_y, &display, Style::default().fg(fg_color));
            }

        }

        // Modifier key stats row below the keyboard, spread across keyboard width
        let kbd_width = all_rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let off = offsets.get(i).copied().unwrap_or(0);
                off + row.len() as u16 * key_step
            })
            .max()
            .unwrap_or(inner.width)
            .min(inner.width);
        let mod_y = if show_shifted {
            inner.y + all_rows.len() as u16 * 2 + 1
        } else {
            inner.y + all_rows.len() as u16
        };
        if mod_y < inner.y + inner.height {
            let mod_keys: &[(char, &str)] = &[
                (TAB, display::key_short_label(TAB)),
                (SPACE, display::key_short_label(SPACE)),
                (ENTER, display::key_short_label(ENTER)),
                (BACKSPACE, display::key_short_label(BACKSPACE)),
            ];
            let labels: Vec<String> = mod_keys
                .iter()
                .map(|&(key, label)| {
                    let time_ms = self.get_key_time_ms(key);
                    format_timing_cell_label(label, time_ms, key_width)
                })
                .collect();
            let positions = spread_labels(&labels, kbd_width);
            for (i, &(key, _)) in mod_keys.iter().enumerate() {
                let time_ms = self.get_key_time_ms(key);
                let fg_color = timing_color(time_ms, colors);
                buf.set_string(
                    inner.x + positions[i],
                    mod_y,
                    &labels[i],
                    Style::default().fg(fg_color),
                );
            }
        }
    }

    fn render_slowest_keys(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(Line::from(Span::styled(
                " Slowest Keys (ms) ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        let mut key_times: Vec<(char, f64)> = self
            .key_stats
            .stats
            .iter()
            .filter(|(_, s)| s.sample_count > 0)
            .map(|(&ch, s)| (ch, s.filtered_time_ms))
            .collect();
        key_times.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.0.cmp(&b.0)));

        let max_time = key_times.first().map(|(_, t)| *t).unwrap_or(1.0);

        for (i, (ch, time)) in key_times.iter().take(inner.height as usize).enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let key_name = display_key_short_fixed(*ch);
            let label = format!(" {key_name} {} ", format_ranked_time(*time));
            let label_len = label.len() as u16;
            buf.set_string(inner.x, y, &label, Style::default().fg(colors.error()));
            let bar_space = inner.width.saturating_sub(label_len) as usize;
            if bar_space > 0 {
                let filled = ((time / max_time) * bar_space as f64).round() as usize;
                let bar = "\u{2588}".repeat(filled.min(bar_space));
                buf.set_string(
                    inner.x + label_len,
                    y,
                    &bar,
                    Style::default().fg(colors.error()),
                );
            }
        }
    }

    fn render_fastest_keys(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(Line::from(Span::styled(
                " Fastest Keys (ms) ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        let mut key_times: Vec<(char, f64)> = self
            .key_stats
            .stats
            .iter()
            .filter(|(_, s)| s.sample_count > 0)
            .map(|(&ch, s)| (ch, s.filtered_time_ms))
            .collect();
        key_times.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then_with(|| a.0.cmp(&b.0)));

        let max_time = key_times.last().map(|(_, t)| *t).unwrap_or(1.0);

        for (i, (ch, time)) in key_times.iter().take(inner.height as usize).enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let key_name = display_key_short_fixed(*ch);
            let label = format!(" {key_name} {} ", format_ranked_time(*time));
            let label_len = label.len() as u16;
            buf.set_string(inner.x, y, &label, Style::default().fg(colors.success()));
            let bar_space = inner.width.saturating_sub(label_len) as usize;
            if bar_space > 0 && max_time > 0.0 {
                let filled = ((time / max_time) * bar_space as f64).round() as usize;
                let bar = "\u{2588}".repeat(filled.min(bar_space));
                buf.set_string(
                    inner.x + label_len,
                    y,
                    &bar,
                    Style::default().fg(colors.success()),
                );
            }
        }
    }

    fn render_worst_accuracy_keys(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(Line::from(Span::styled(
                " Worst Accuracy (%) ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        // Collect all keys from keyboard model + modifier keys
        let mut all_keys = std::collections::HashSet::new();
        for row in &self.keyboard_model.rows {
            for pk in row {
                all_keys.insert(pk.base);
                all_keys.insert(pk.shifted);
            }
        }
        // Include modifier/whitespace keys
        all_keys.insert(SPACE);
        for &key in MODIFIER_SENTINELS {
            all_keys.insert(key);
        }

        let mut key_accuracies: Vec<(char, f64)> = all_keys
            .into_iter()
            .filter_map(|ch| {
                let accuracy = self.get_key_accuracy(ch);
                // Only include keys with enough data and imperfect accuracy
                if accuracy > 0.0 && accuracy < 100.0 {
                    Some((ch, accuracy))
                } else {
                    None
                }
            })
            .collect();

        key_accuracies.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then_with(|| a.0.cmp(&b.0)));

        if key_accuracies.is_empty() {
            buf.set_string(
                inner.x,
                inner.y,
                " Not enough data",
                Style::default().fg(colors.text_pending()),
            );
            return;
        }

        for (i, (ch, acc)) in key_accuracies
            .iter()
            .take(inner.height as usize)
            .enumerate()
        {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let key_name = display_key_short_fixed(*ch);
            let label = format!(" {key_name} {acc:>5.1}% ");
            let label_len = label.len() as u16;
            let color = if *acc >= 95.0 {
                colors.warning()
            } else {
                colors.error()
            };
            buf.set_string(inner.x, y, &label, Style::default().fg(color));
            let bar_space = inner.width.saturating_sub(label_len) as usize;
            if bar_space > 0 {
                let filled = ((acc / 100.0) * bar_space as f64).round() as usize;
                let bar = "\u{2588}".repeat(filled.min(bar_space));
                buf.set_string(inner.x + label_len, y, &bar, Style::default().fg(color));
            }
        }
    }

    fn render_best_accuracy_keys(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(Line::from(Span::styled(
                " Best Accuracy (%) ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        let mut all_keys = std::collections::HashSet::new();
        for row in &self.keyboard_model.rows {
            for pk in row {
                all_keys.insert(pk.base);
                all_keys.insert(pk.shifted);
            }
        }
        all_keys.insert(SPACE);
        for &key in MODIFIER_SENTINELS {
            all_keys.insert(key);
        }

        let mut key_accuracies: Vec<(char, f64)> = all_keys
            .into_iter()
            .filter_map(|ch| {
                let accuracy = self.get_key_accuracy(ch);
                if accuracy > 0.0 {
                    Some((ch, accuracy))
                } else {
                    None
                }
            })
            .collect();

        key_accuracies.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.0.cmp(&b.0)));

        if key_accuracies.is_empty() {
            buf.set_string(
                inner.x,
                inner.y,
                " Not enough data",
                Style::default().fg(colors.text_pending()),
            );
            return;
        }

        for (i, (ch, acc)) in key_accuracies
            .iter()
            .take(inner.height as usize)
            .enumerate()
        {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            let key_name = display_key_short_fixed(*ch);
            let label = format!(" {key_name} {acc:>5.1}% ");
            let label_len = label.len() as u16;
            let color = if *acc >= 98.0 {
                colors.success()
            } else {
                colors.warning()
            };
            buf.set_string(inner.x, y, &label, Style::default().fg(color));
            let bar_space = inner.width.saturating_sub(label_len) as usize;
            if bar_space > 0 {
                let filled = ((acc / 100.0) * bar_space as f64).round() as usize;
                let bar = "\u{2588}".repeat(filled.min(bar_space));
                buf.set_string(inner.x + label_len, y, &bar, Style::default().fg(color));
            }
        }
    }

    fn render_activity_stats(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                " Streaks ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        let mut day_counts: HashMap<chrono::NaiveDate, usize> = HashMap::new();
        let mut active_days: BTreeSet<chrono::NaiveDate> = BTreeSet::new();
        for r in self.history.iter().filter(|r| !r.partial) {
            let day = r.timestamp.date_naive();
            active_days.insert(day);
            *day_counts.entry(day).or_insert(0) += 1;
        }
        let (current_streak, best_streak) = compute_streaks(&active_days);
        let active_days_count = active_days.len();
        let mut top_days: Vec<(chrono::NaiveDate, usize)> = day_counts.into_iter().collect();
        top_days.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));

        let mut lines = vec![Line::from(vec![
            Span::styled("  Current: ", Style::default().fg(colors.fg())),
            Span::styled(
                format!("{current_streak}d"),
                Style::default()
                    .fg(colors.success())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("    Best: ", Style::default().fg(colors.fg())),
            Span::styled(
                format!("{best_streak}d"),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("    Active Days: ", Style::default().fg(colors.fg())),
            Span::styled(
                format!("{active_days_count}"),
                Style::default().fg(colors.text_pending()),
            ),
        ])];

        let top_days_text = if top_days.is_empty() {
            "  Top Days: none".to_string()
        } else {
            let parts: Vec<String> = top_days
                .iter()
                .take(3)
                .map(|(d, c)| format!("{} ({})", d.format("%Y-%m-%d"), c))
                .collect();
            format!("  Top Days: {}", parts.join("  |  "))
        };
        lines.push(Line::from(Span::styled(
            top_days_text,
            Style::default().fg(colors.text_pending()),
        )));

        Paragraph::new(lines).render(inner, buf);
    }
}

fn accuracy_color(accuracy: f64, colors: &crate::ui::theme::ThemeColors) -> ratatui::style::Color {
    if accuracy <= 0.0 {
        colors.text_pending()
    } else if accuracy >= 98.0 {
        colors.success()
    } else if accuracy >= 90.0 {
        colors.warning()
    } else {
        colors.error()
    }
}

fn format_accuracy_cell(key: char, accuracy: f64, key_width: u16) -> String {
    if accuracy > 0.0 {
        let pct = accuracy.round() as u32;
        if key_width >= 5 {
            format!("{key} {pct:<3}")
        } else {
            format!("{key}{pct:>2}")
        }
    } else if key_width >= 5 {
        format!("{key}    ")
    } else {
        format!("{key}  ")
    }
}

fn format_accuracy_cell_label(label: &str, accuracy: f64, key_width: u16) -> String {
    if accuracy > 0.0 {
        let pct = accuracy.round() as u32;
        if key_width >= 5 {
            format!("{label} {pct:<3}")
        } else {
            format!("{label}{pct:>2}")
        }
    } else if key_width >= 5 {
        format!("{label}    ")
    } else {
        format!("{label}  ")
    }
}

fn timing_color(time_ms: f64, colors: &crate::ui::theme::ThemeColors) -> ratatui::style::Color {
    if time_ms <= 0.0 {
        colors.text_pending()
    } else if time_ms <= 200.0 {
        colors.success()
    } else if time_ms <= 400.0 {
        colors.warning()
    } else {
        colors.error()
    }
}

fn required_kbd_width(key_width: u16, key_step: u16) -> u16 {
    let max_offset: u16 = 4;
    max_offset + 12 * key_step + key_width
}

fn display_key_short_fixed(ch: char) -> String {
    let special = display::key_short_label(ch);
    let raw = if special.is_empty() {
        ch.to_string()
    } else {
        special.to_string()
    };
    format!("{raw:<4}")
}

fn compute_streaks(active_days: &BTreeSet<chrono::NaiveDate>) -> (usize, usize) {
    if active_days.is_empty() {
        return (0, 0);
    }

    let mut best = 1usize;
    let mut run = 1usize;
    let mut prev = None;
    for &day in active_days {
        if let Some(p) = prev {
            if day.signed_duration_since(p).num_days() == 1 {
                run += 1;
            } else {
                run = 1;
            }
            best = best.max(run);
        }
        prev = Some(day);
    }

    let today = chrono::Utc::now().date_naive();
    let mut current = 0usize;
    let mut cursor = today;
    while active_days.contains(&cursor) {
        current += 1;
        cursor -= chrono::Duration::days(1);
    }

    (current, best)
}

fn format_timing_cell(key: char, time_ms: f64, key_width: u16) -> String {
    if time_ms > 0.0 {
        let value = format_timing_visual_value_3(time_ms);
        if key_width >= 5 {
            format!("{key} {value:<3}")
        } else {
            format!("{key}{value:>3}")
        }
    } else if key_width >= 5 {
        format!("{key}    ")
    } else {
        format!("{key}   ")
    }
}

fn format_timing_cell_label(label: &str, time_ms: f64, key_width: u16) -> String {
    if time_ms > 0.0 {
        let value = format_timing_visual_value_3(time_ms);
        if key_width >= 5 {
            format!("{label} {value:<3}")
        } else {
            format!("{label}{value:>3}")
        }
    } else if key_width >= 5 {
        format!("{label}    ")
    } else {
        format!("{label}   ")
    }
}

fn format_timing_visual_value_3(time_ms: f64) -> String {
    let ms = time_ms.max(0.0).round() as u32;
    if ms <= 999 {
        return format!("{ms:>3}");
    }

    // Keep visualizer values to exactly 3 chars while signaling second units.
    // Example: 1.2s => "1s2", 9.0s => "9s0", 12s => "12s".
    if ms < 10_000 {
        let tenths = ((ms as f64 / 100.0).round() as u32).min(99);
        let whole = tenths / 10;
        let frac = tenths % 10;
        return format!("{whole}s{frac}");
    }

    let secs = ((ms as f64) / 1000.0).round() as u32;
    format!("{:>3}", format!("{}s", secs.min(99)))
}

fn format_ranked_time(time_ms: f64) -> String {
    if time_ms > 59_999.0 {
        return format!("{:.1}m", time_ms / 60_000.0);
    }
    if time_ms > 9_999.0 {
        return format!("{:.1}s", time_ms / 1_000.0);
    }
    format!("{time_ms:>4.0}ms")
}

/// Distribute labels across `total_width`, with the first flush-left
/// and the last flush-right, and equal gaps between the rest.
fn spread_labels(labels: &[String], total_width: u16) -> Vec<u16> {
    let n = labels.len();
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![0];
    }
    let total_label_width: u16 = labels.iter().map(|l| l.len() as u16).sum();
    let last_width = labels.last().map(|l| l.len() as u16).unwrap_or(0);
    let spare = total_width.saturating_sub(total_label_width);
    let gaps = (n - 1) as u16;
    let gap = if gaps > 0 { spare / gaps } else { 0 };
    let remainder = if gaps > 0 { spare % gaps } else { 0 };

    let mut positions = Vec::with_capacity(n);
    let mut x: u16 = 0;
    for (i, label) in labels.iter().enumerate() {
        if i == n - 1 {
            // Last label flush-right
            x = total_width.saturating_sub(last_width);
        }
        positions.push(x);
        x += label.len() as u16 + gap + if (i as u16) < remainder { 1 } else { 0 };
    }
    positions
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
    buf.set_string(area.x, area.y, label, Style::default().fg(fill_color));

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
