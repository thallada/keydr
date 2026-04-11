use chrono::{Datelike, Utc};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Widget};
use std::collections::{BTreeSet, HashMap};

use crate::engine::key_stats::KeyStatsStore;
use crate::engine::ngram_stats::{AnomalyType, FocusSelection};
use crate::keyboard::display::{self, BACKSPACE, ENTER, MODIFIER_SENTINELS, SPACE, TAB};
use crate::keyboard::model::KeyboardModel;
use crate::session::result::DrillResult;
use crate::ui::components::activity_heatmap::ActivityHeatmap;
use crate::i18n::t;
use crate::ui::layout::pack_hint_lines;
use crate::ui::theme::Theme;

// ---------------------------------------------------------------------------
// N-grams tab view models
// ---------------------------------------------------------------------------

pub struct AnomalyBigramRow {
    pub bigram: String,
    pub anomaly_pct: f64,
    pub sample_count: usize,
    pub error_count: usize,
    pub error_rate_ema: f64,
    pub speed_ms: f64,
    pub expected_baseline: f64,
    pub confirmed: bool,
}

pub struct NgramTabData {
    pub focus: FocusSelection,
    pub error_anomalies: Vec<AnomalyBigramRow>,
    pub speed_anomalies: Vec<AnomalyBigramRow>,
    pub total_bigrams: usize,
    pub hesitation_threshold_ms: f64,
    pub scope_label: String,
}

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
    pub history_scroll: usize,
    pub history_confirm_delete: bool,
    pub keyboard_model: &'a KeyboardModel,
    pub ngram_data: Option<&'a NgramTabData>,
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
        history_scroll: usize,
        history_confirm_delete: bool,
        keyboard_model: &'a KeyboardModel,
        ngram_data: Option<&'a NgramTabData>,
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
            history_scroll,
            history_confirm_delete,
            keyboard_model,
            ngram_data,
        }
    }
}

impl Widget for StatsDashboard<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let title = t!("stats.title");
        let block = Block::bordered()
            .title(title.as_ref())
            .border_style(Style::default().fg(colors.accent()))
            .style(Style::default().bg(colors.bg()));
        let inner = block.inner(area);
        block.render(area, buf);

        if self.history.is_empty() {
            let msg = Paragraph::new(Line::from(Span::styled(
                t!("stats.empty").to_string(),
                Style::default().fg(colors.text_pending()),
            )));
            msg.render(inner, buf);
            return;
        }

        // Tab header — width-aware wrapping
        let width = inner.width as usize;
        let labels = tab_labels();
        let mut tab_lines: Vec<Line> = Vec::new();
        let mut current_spans: Vec<Span> = Vec::new();
        let mut current_width: usize = 0;
        for (i, label) in labels.iter().enumerate() {
            let styled_label = format!(" {label} ");
            let item_width = styled_label.chars().count() + TAB_SEPARATOR.len();
            if current_width > 0 && current_width + item_width > width {
                tab_lines.push(Line::from(current_spans));
                current_spans = Vec::new();
                current_width = 0;
            }
            let style = if i == self.active_tab {
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default().fg(colors.text_pending())
            };
            current_spans.push(Span::styled(styled_label, style));
            current_spans.push(Span::raw(TAB_SEPARATOR));
            current_width += item_width;
        }
        if !current_spans.is_empty() {
            tab_lines.push(Line::from(current_spans));
        }
        let tab_line_count = tab_lines.len().max(1) as u16;

        // Footer — width-aware wrapping
        let footer_hints = if self.active_tab == 1 {
            footer_hints_history()
        } else {
            footer_hints_default()
        };
        let hint_refs: Vec<&str> = footer_hints.iter().map(|s| s.as_str()).collect();
        let footer_lines_vec = pack_hint_lines(&hint_refs, width);
        let footer_line_count = footer_lines_vec.len().max(1) as u16;

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(tab_line_count),
                Constraint::Min(10),
                Constraint::Length(footer_line_count),
            ])
            .split(inner);

        Paragraph::new(tab_lines).render(layout[0], buf);

        // Render only one tab at a time so each tab gets full breathing room.
        self.render_tab(self.active_tab, layout[1], buf);

        // Footer
        let footer_lines: Vec<Line> = footer_lines_vec
            .into_iter()
            .map(|line| Line::from(Span::styled(line, Style::default().fg(colors.accent()))))
            .collect();
        Paragraph::new(footer_lines).render(layout[2], buf);

        // Confirmation dialog overlay
        if self.history_confirm_delete && self.active_tab == 1 {
            let dialog_width = 34u16;
            let dialog_height = 5u16;
            let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
            let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
            let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

            let idx = self.history.len().saturating_sub(self.history_selected);
            let dialog_text = t!("stats.delete_confirm", idx = idx);
            let confirm_title = t!("stats.confirm_title");

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
                    .title(confirm_title.as_ref())
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
            5 => self.render_ngram_tab(area, buf),
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

        let summary_title = t!("stats.summary_title");
        let summary_block = Block::bordered()
            .title(Line::from(Span::styled(
                summary_title.to_string(),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let summary_inner = summary_block.inner(layout[0]);
        summary_block.render(layout[0], buf);

        let drills_label = t!("stats.drills");
        let avg_wpm_label = t!("stats.avg_wpm");
        let best_wpm_label = t!("stats.best_wpm");
        let accuracy_label = t!("stats.accuracy_label");
        let total_time_label = t!("stats.total_time");
        let summary = vec![
            Line::from(vec![
                Span::styled(drills_label.to_string(), Style::default().fg(colors.fg())),
                Span::styled(
                    &*total_str,
                    Style::default()
                        .fg(colors.accent())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(avg_wpm_label.to_string(), Style::default().fg(colors.fg())),
                Span::styled(&*avg_wpm_str, Style::default().fg(colors.accent())),
                Span::styled(best_wpm_label.to_string(), Style::default().fg(colors.fg())),
                Span::styled(
                    &*best_wpm_str,
                    Style::default()
                        .fg(colors.success())
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled(accuracy_label.to_string(), Style::default().fg(colors.fg())),
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
                Span::styled(total_time_label.to_string(), Style::default().fg(colors.fg())),
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

        let target_label = t!("stats.wpm_chart_title", target = self.target_wpm);
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                target_label.to_string(),
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
                    t!("stats.accuracy_chart_title").to_string(),
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
                        t!("stats.accuracy_chart_title").to_string(),
                        Style::default()
                            .fg(colors.accent())
                            .add_modifier(Modifier::BOLD),
                    )))
                    .border_style(Style::default().fg(colors.accent()))
                    .style(Style::default().bg(colors.bg())),
            )
            .x_axis(
                Axis::default()
                    .title(t!("stats.chart_drill").to_string())
                    .style(Style::default().fg(colors.text_pending()).bg(colors.bg()))
                    .bounds([0.0, max_x]),
            )
            .y_axis(
                Axis::default()
                    .title(t!("stats.chart_accuracy_pct").to_string())
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
        let wpm_label = t!("stats.wpm_label", avg = format!("{avg_wpm:.0}"), target = self.target_wpm, pct = format!("{wpm_pct:.0}")).to_string();
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
        let acc_label = t!("stats.acc_label", pct = format!("{acc_pct:.1}")).to_string();
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
        let level_label = t!("stats.keys_label", unlocked = self.overall_unlocked, total = self.overall_total, mastered = self.overall_mastered).to_string();
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
        let sessions_title = t!("stats.sessions_title");
        let table_block = Block::bordered()
            .title(Line::from(Span::styled(
                sessions_title.to_string(),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let table_inner = table_block.inner(area);
        table_block.render(area, buf);

        let header = Line::from(vec![Span::styled(
            t!("stats.session_header").to_string(),
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        )]);

        let mut lines = vec![
            header,
            Line::from(Span::styled(
                t!("stats.session_separator").to_string(),
                Style::default().fg(colors.border()),
            )),
        ];

        let visible_rows = history_visible_rows(table_inner);
        let total = self.history.len();
        let max_scroll = total.saturating_sub(visible_rows);
        let scroll = self.history_scroll.min(max_scroll);
        let end = (scroll + visible_rows).min(total);
        let current_year = Utc::now().year();

        for display_idx in scroll..end {
            let result = &self.history[total - 1 - display_idx];
            let idx = total - display_idx;
            let raw_wpm = result.cpm / 5.0;
            let time_str = format!("{:.1}s", result.elapsed_secs);
            let date_str = format_history_timestamp(result.timestamp, current_year);

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

            let rank_label = if result.ranked { t!("stats.yes") } else { t!("stats.no") };
            let rank_str = rank_label.as_ref();
            let partial_pct = if result.partial {
                result.completion_percent
            } else {
                100.0
            };
            let partial_str = format!("{:>6.0}%", partial_pct);
            let row = format!(
                " {wpm_indicator}{idx_str}  {wpm_str}  {raw_str}  {acc_str}  {time_str:>6}  {date_str:<14}  {mode:<9}  {rank_str:<6}  {partial_str:>7}",
                mode = result.drill_mode,
            );

            let acc_color = if result.accuracy >= 95.0 {
                colors.success()
            } else if result.accuracy >= 85.0 {
                colors.warning()
            } else {
                colors.error()
            };

            let is_selected = display_idx == self.history_selected;
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

        let kbd_acc_title = t!("stats.keyboard_accuracy_title");
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                kbd_acc_title.to_string(),
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
        let offsets = self.keyboard_model.geometry_hints.row_offsets;
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
        let keyboard_x = inner.x + inner.width.saturating_sub(kbd_width) / 2;

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
                        let x = keyboard_x + offset + col_idx as u16 * key_step;
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
                let x = keyboard_x + offset + col_idx as u16 * key_step;
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
                    keyboard_x + positions[i],
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

        let kbd_timing_title = t!("stats.keyboard_timing_title");
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                kbd_timing_title.to_string(),
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
        let offsets = self.keyboard_model.geometry_hints.row_offsets;
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
        let keyboard_x = inner.x + inner.width.saturating_sub(kbd_width) / 2;

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
                        let x = keyboard_x + offset + col_idx as u16 * key_step;
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
                let x = keyboard_x + offset + col_idx as u16 * key_step;
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
                    keyboard_x + positions[i],
                    mod_y,
                    &labels[i],
                    Style::default().fg(fg_color),
                );
            }
        }
    }

    fn render_slowest_keys(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let slowest_title = t!("stats.slowest_keys_title");
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                slowest_title.to_string(),
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

        let fastest_title = t!("stats.fastest_keys_title");
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                fastest_title.to_string(),
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

        let worst_title = t!("stats.worst_accuracy_title");
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                worst_title.to_string(),
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
            let no_data = t!("stats.not_enough_data");
            buf.set_string(
                inner.x,
                inner.y,
                no_data.as_ref(),
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

        let best_title = t!("stats.best_accuracy_title");
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                best_title.to_string(),
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
            let no_data = t!("stats.not_enough_data");
            buf.set_string(
                inner.x,
                inner.y,
                no_data.as_ref(),
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
        let streaks_title = t!("stats.streaks_title");
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                streaks_title.to_string(),
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

        let current_label = t!("stats.current_streak");
        let best_label = t!("stats.best_streak");
        let active_days_label = t!("stats.active_days");
        let mut lines = vec![Line::from(vec![
            Span::styled(current_label.to_string(), Style::default().fg(colors.fg())),
            Span::styled(
                format!("{current_streak}d"),
                Style::default()
                    .fg(colors.success())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(best_label.to_string(), Style::default().fg(colors.fg())),
            Span::styled(
                format!("{best_streak}d"),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(active_days_label.to_string(), Style::default().fg(colors.fg())),
            Span::styled(
                format!("{active_days_count}"),
                Style::default().fg(colors.text_pending()),
            ),
        ])];

        let top_days_text = if top_days.is_empty() {
            t!("stats.top_days_none").to_string()
        } else {
            let parts: Vec<String> = top_days
                .iter()
                .take(3)
                .map(|(d, c)| format!("{} ({})", d.format("%Y-%m-%d"), c))
                .collect();
            t!("stats.top_days", days = parts.join("  |  ")).to_string()
        };
        lines.push(Line::from(Span::styled(
            top_days_text,
            Style::default().fg(colors.text_pending()),
        )));

        Paragraph::new(lines).render(inner, buf);
    }

    // --- N-grams tab ---

    fn render_ngram_tab(&self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let data = match self.ngram_data {
            Some(d) => d,
            None => {
                let msg = Paragraph::new(Line::from(Span::styled(
                    t!("stats.ngram_empty").to_string(),
                    Style::default().fg(colors.text_pending()),
                )));
                msg.render(area, buf);
                return;
            }
        };

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // focus box
                Constraint::Min(5),    // lists
                Constraint::Length(2), // summary
            ])
            .split(area);

        self.render_ngram_focus(data, layout[0], buf);

        let wide = layout[1].width >= 60;
        if wide {
            let lists = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(layout[1]);
            self.render_error_anomalies(data, lists[0], buf);
            self.render_speed_anomalies(data, lists[1], buf);
        } else {
            // Stacked vertically for narrow terminals
            let available = layout[1].height;
            if available < 10 {
                // Only show error anomalies if very little space
                self.render_error_anomalies(data, layout[1], buf);
            } else {
                let half = available / 2;
                let lists = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(half), Constraint::Min(0)])
                    .split(layout[1]);
                self.render_error_anomalies(data, lists[0], buf);
                self.render_speed_anomalies(data, lists[1], buf);
            }
        }
        self.render_ngram_summary(data, layout[2], buf);
    }

    fn render_ngram_focus(&self, data: &NgramTabData, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let focus_title = t!("stats.focus_title");
        let block = Block::bordered()
            .title(Line::from(Span::styled(
                focus_title.to_string(),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 {
            return;
        }

        let mut lines = Vec::new();

        match (&data.focus.char_focus, &data.focus.bigram_focus) {
            (Some(ch), Some((key, anomaly_pct, anomaly_type))) => {
                let bigram_label = format!("\"{}{}\"", key.0[0], key.0[1]);
                // Line 1: both focuses
                lines.push(Line::from(vec![
                    Span::styled(t!("stats.focus_char_label").to_string(), Style::default().fg(colors.fg())),
                    Span::styled(
                        t!("stats.focus_char_value", ch = ch).to_string(),
                        Style::default()
                            .fg(colors.focused_key())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(t!("stats.focus_plus").to_string(), Style::default().fg(colors.fg())),
                    Span::styled(
                        t!("stats.focus_bigram_value", label = &bigram_label).to_string(),
                        Style::default()
                            .fg(colors.focused_key())
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                // Line 2: details
                if inner.height >= 2 {
                    let type_label = match anomaly_type {
                        AnomalyType::Error => t!("stats.anomaly_error").to_string(),
                        AnomalyType::Speed => t!("stats.anomaly_speed").to_string(),
                    };
                    let detail = t!("stats.focus_detail_both", ch = ch, label = &bigram_label, r#type = &type_label, pct = format!("{anomaly_pct:.0}"));
                    lines.push(Line::from(Span::styled(
                        detail.to_string(),
                        Style::default().fg(colors.text_pending()),
                    )));
                }
            }
            (Some(ch), None) => {
                lines.push(Line::from(vec![
                    Span::styled(t!("stats.focus_char_label").to_string(), Style::default().fg(colors.fg())),
                    Span::styled(
                        t!("stats.focus_char_value", ch = ch).to_string(),
                        Style::default()
                            .fg(colors.focused_key())
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                if inner.height >= 2 {
                    lines.push(Line::from(Span::styled(
                        t!("stats.focus_detail_char_only", ch = ch).to_string(),
                        Style::default().fg(colors.text_pending()),
                    )));
                }
            }
            (None, Some((key, anomaly_pct, anomaly_type))) => {
                let bigram_label = format!("\"{}{}\"", key.0[0], key.0[1]);
                let type_label = match anomaly_type {
                    AnomalyType::Error => t!("stats.anomaly_error").to_string(),
                    AnomalyType::Speed => t!("stats.anomaly_speed").to_string(),
                };
                lines.push(Line::from(vec![
                    Span::styled(t!("stats.focus_char_label").to_string(), Style::default().fg(colors.fg())),
                    Span::styled(
                        t!("stats.focus_bigram_value", label = &bigram_label).to_string(),
                        Style::default()
                            .fg(colors.focused_key())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        t!("stats.focus_detail_bigram_only", r#type = &type_label, pct = format!("{anomaly_pct:.0}")).to_string(),
                        Style::default().fg(colors.text_pending()),
                    ),
                ]));
            }
            (None, None) => {
                lines.push(Line::from(Span::styled(
                    t!("stats.focus_empty").to_string(),
                    Style::default().fg(colors.text_pending()),
                )));
            }
        }

        Paragraph::new(lines).render(inner, buf);
    }

    fn render_anomaly_panel(
        &self,
        title: &str,
        empty_msg: &str,
        rows: &[AnomalyBigramRow],
        is_speed: bool,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(Line::from(Span::styled(
                title.to_string(),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 {
            return;
        }

        if rows.is_empty() {
            buf.set_string(
                inner.x,
                inner.y,
                empty_msg,
                Style::default().fg(colors.text_pending()),
            );
            return;
        }

        let narrow = inner.width < 30;

        // Error table: Bigram  Anom%  Rate  Errors  Smp  Strk
        // Speed table: Bigram  Anom%  Speed  Smp  Strk
        let header = if narrow {
            if is_speed {
                t!("stats.ngram_header_speed_narrow").to_string()
            } else {
                t!("stats.ngram_header_error_narrow").to_string()
            }
        } else if is_speed {
            t!("stats.ngram_header_speed").to_string()
        } else {
            t!("stats.ngram_header_error").to_string()
        };
        buf.set_string(
            inner.x,
            inner.y,
            header,
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        );

        let max_rows = (inner.height as usize).saturating_sub(1);
        for (i, row) in rows.iter().take(max_rows).enumerate() {
            let y = inner.y + 1 + i as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let line = if narrow {
                if is_speed {
                    format!(
                        " {:>4} {:>3.0}ms {:>3.0}ms {:>4.0}%",
                        row.bigram, row.speed_ms, row.expected_baseline, row.anomaly_pct,
                    )
                } else {
                    format!(
                        " {:>4} {:>3} {:>3} {:>3.0}% {:>2.0}% {:>4.0}%",
                        row.bigram,
                        row.error_count,
                        row.sample_count,
                        row.error_rate_ema * 100.0,
                        row.expected_baseline * 100.0,
                        row.anomaly_pct,
                    )
                }
            } else if is_speed {
                format!(
                    " {:>6}  {:>4.0}ms  {:>4.0}ms  {:>5}  {:>4.0}%",
                    row.bigram,
                    row.speed_ms,
                    row.expected_baseline,
                    row.sample_count,
                    row.anomaly_pct,
                )
            } else {
                format!(
                    " {:>6}  {:>5}  {:>5}  {:>4.0}%  {:>4.0}%  {:>5.0}%",
                    row.bigram,
                    row.error_count,
                    row.sample_count,
                    row.error_rate_ema * 100.0,
                    row.expected_baseline * 100.0,
                    row.anomaly_pct,
                )
            };

            let color = if row.confirmed {
                colors.error()
            } else {
                colors.warning()
            };

            buf.set_string(inner.x, y, &line, Style::default().fg(color));
        }
    }

    fn render_error_anomalies(&self, data: &NgramTabData, area: Rect, buf: &mut Buffer) {
        let title = t!("stats.error_anomalies_title", count = data.error_anomalies.len());
        let empty_msg = t!("stats.no_error_anomalies");
        self.render_anomaly_panel(
            title.as_ref(),
            empty_msg.as_ref(),
            &data.error_anomalies,
            false,
            area,
            buf,
        );
    }

    fn render_speed_anomalies(&self, data: &NgramTabData, area: Rect, buf: &mut Buffer) {
        let title = t!("stats.speed_anomalies_title", count = data.speed_anomalies.len());
        let empty_msg = t!("stats.no_speed_anomalies");
        self.render_anomaly_panel(
            title.as_ref(),
            empty_msg.as_ref(),
            &data.speed_anomalies,
            true,
            area,
            buf,
        );
    }

    fn render_ngram_summary(&self, data: &NgramTabData, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;
        let w = area.width as usize;

        // Build segments from most to least important, progressively drop from the right
        let scope = t!("stats.scope_label_prefix", ).to_string() + &data.scope_label;
        let bigrams = t!("stats.bi_label", count = data.total_bigrams).to_string();
        let hesitation = t!("stats.hes_label", ms = format!("{:.0}", data.hesitation_threshold_ms)).to_string();

        let segments: &[&str] = &[&scope, &bigrams, &hesitation];
        let mut line = String::new();
        for seg in segments {
            if line.len() + seg.len() <= w {
                line.push_str(seg);
            } else {
                break;
            }
        }

        buf.set_string(
            area.x,
            area.y,
            &line,
            Style::default().fg(colors.text_pending()),
        );
    }
}

fn tab_labels() -> Vec<String> {
    vec![
        t!("stats.tab_dashboard").to_string(),
        t!("stats.tab_history").to_string(),
        t!("stats.tab_activity").to_string(),
        t!("stats.tab_accuracy").to_string(),
        t!("stats.tab_timing").to_string(),
        t!("stats.tab_ngrams").to_string(),
    ]
}

const TAB_SEPARATOR: &str = "  ";

fn footer_hints_default() -> Vec<String> {
    vec![
        t!("stats.hint_back").to_string(),
        t!("stats.hint_next_tab").to_string(),
        t!("stats.hint_switch_tab").to_string(),
    ]
}

fn footer_hints_history() -> Vec<String> {
    vec![
        t!("stats.hint_back").to_string(),
        t!("stats.hint_next_tab").to_string(),
        t!("stats.hint_switch_tab").to_string(),
        t!("stats.hint_navigate").to_string(),
        t!("stats.hint_page").to_string(),
        t!("stats.hint_delete").to_string(),
    ]
}

fn history_visible_rows(table_inner: Rect) -> usize {
    table_inner.height.saturating_sub(2) as usize
}

fn wrapped_tab_line_count(width: usize) -> usize {
    let labels = tab_labels();
    let mut lines = 1usize;
    let mut current_width = 0usize;
    for label in &labels {
        let item_width = format!(" {label} ").chars().count() + TAB_SEPARATOR.len();
        if current_width > 0 && current_width + item_width > width {
            lines += 1;
            current_width = 0;
        }
        current_width += item_width;
    }
    lines.max(1)
}

fn footer_line_count_for_history(width: usize) -> usize {
    let hints = footer_hints_history();
    let hint_refs: Vec<&str> = hints.iter().map(|s| s.as_str()).collect();
    pack_hint_lines(&hint_refs, width).len().max(1)
}

pub fn history_page_size_for_terminal(width: u16, height: u16) -> usize {
    let inner_width = width.saturating_sub(2) as usize;
    let inner_height = height.saturating_sub(2);
    let tab_lines = wrapped_tab_line_count(inner_width) as u16;
    let footer_lines = footer_line_count_for_history(inner_width) as u16;
    let tab_area_height = inner_height.saturating_sub(tab_lines + footer_lines);
    let table_inner_height = tab_area_height.saturating_sub(2);
    table_inner_height.saturating_sub(2).max(1) as usize
}

fn format_history_timestamp(ts: chrono::DateTime<Utc>, current_year: i32) -> String {
    if ts.year() < current_year {
        ts.format("%m/%d/%y %H:%M").to_string()
    } else {
        ts.format("%m/%d %H:%M").to_string()
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

/// Compute the ngram tab panel layout for the given terminal area.
/// Returns `(wide, lists_area_height)` where:
/// - `wide` = true means side-by-side anomaly panels (width >= 60)
/// - `lists_area_height` = height available for the anomaly panels region
///
/// When `!wide && lists_area_height < 10`, only error anomalies should render.
#[cfg(test)]
fn ngram_panel_layout(area: Rect) -> (bool, u16) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // focus box
            Constraint::Min(5),    // lists
            Constraint::Length(2), // summary
        ])
        .split(area);
    let wide = layout[1].width >= 60;
    (wide, layout[1].height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn narrow_short_terminal_shows_only_error_panel() {
        // 50 cols × 15 rows: narrow (<60) so panels stack vertically.
        // lists area = 15 - 4 (focus) - 2 (summary) = 9 rows → < 10 → error only.
        let area = Rect::new(0, 0, 50, 15);
        let (wide, lists_height) = ngram_panel_layout(area);
        assert!(!wide, "50 cols should be narrow layout");
        assert!(
            lists_height < 10,
            "lists_height={lists_height}, expected < 10 so only error panel renders"
        );
    }

    #[test]
    fn narrow_tall_terminal_stacks_both_panels() {
        // 50 cols × 30 rows: narrow (<60) so panels stack vertically.
        // lists area = 30 - 4 - 2 = 24 rows → >= 10 → both panels stacked.
        let area = Rect::new(0, 0, 50, 30);
        let (wide, lists_height) = ngram_panel_layout(area);
        assert!(!wide, "50 cols should be narrow layout");
        assert!(
            lists_height >= 10,
            "lists_height={lists_height}, expected >= 10 so both panels stack vertically"
        );
    }

    #[test]
    fn wide_terminal_shows_side_by_side_panels() {
        // 80 cols × 24 rows: wide (>= 60) so panels render side by side.
        let area = Rect::new(0, 0, 80, 24);
        let (wide, _) = ngram_panel_layout(area);
        assert!(
            wide,
            "80 cols should be wide layout with side-by-side panels"
        );
    }

    #[test]
    fn boundary_width_59_is_narrow() {
        let area = Rect::new(0, 0, 59, 24);
        let (wide, _) = ngram_panel_layout(area);
        assert!(!wide, "59 cols should be narrow");
    }

    #[test]
    fn boundary_width_60_is_wide() {
        let area = Rect::new(0, 0, 60, 24);
        let (wide, _) = ngram_panel_layout(area);
        assert!(wide, "60 cols should be wide");
    }

    #[test]
    fn history_page_size_is_positive() {
        let page = history_page_size_for_terminal(80, 24);
        assert!(page >= 1, "history page size should be at least 1");
    }

    #[test]
    fn history_page_size_grows_with_terminal_height() {
        let short_page = history_page_size_for_terminal(100, 20);
        let tall_page = history_page_size_for_terminal(100, 40);
        assert!(
            tall_page > short_page,
            "expected taller terminal to show more rows ({short_page} -> {tall_page})"
        );
    }

    #[test]
    fn history_date_shows_year_for_previous_year_sessions() {
        let ts = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 0).unwrap();
        let display = format_history_timestamp(ts, 2026);
        assert!(
            display.starts_with("12/31/25"),
            "expected MM/DD/YY format for prior-year session: {display}"
        );
    }

    #[test]
    fn history_date_omits_year_for_current_year_sessions() {
        let ts = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 0).unwrap();
        let display = format_history_timestamp(ts, 2026);
        assert!(
            !display.starts_with("2026-"),
            "did not expect year prefix for current-year session: {display}"
        );
    }
}
