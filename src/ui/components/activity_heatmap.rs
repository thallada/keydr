use std::collections::HashMap;

use chrono::{Datelike, NaiveDate, Utc};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Widget};

use crate::session::result::DrillResult;
use crate::ui::theme::Theme;

pub struct ActivityHeatmap<'a> {
    history: &'a [DrillResult],
    theme: &'a Theme,
}

impl<'a> ActivityHeatmap<'a> {
    pub fn new(history: &'a [DrillResult], theme: &'a Theme) -> Self {
        Self { history, theme }
    }
}

impl Widget for ActivityHeatmap<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(Line::from(Span::styled(
                " Daily Activity (Sessions per Day) ",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )))
            .border_style(Style::default().fg(colors.accent()));
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 9 || inner.width < 30 {
            return;
        }

        // Count sessions per day
        let mut day_counts: HashMap<NaiveDate, usize> = HashMap::new();
        for result in self.history.iter().filter(|r| !r.partial) {
            let date = result.timestamp.date_naive();
            *day_counts.entry(date).or_insert(0) += 1;
        }

        let today = Utc::now().date_naive();
        let end_date = today;
        // Show ~26 weeks (half a year)
        let weeks_to_show = ((inner.width as usize).saturating_sub(3)) / 2;
        let weeks_to_show = weeks_to_show.min(26);
        let start_date = end_date - chrono::Duration::weeks(weeks_to_show as i64);
        // Align to Monday
        let start_date =
            start_date - chrono::Duration::days(start_date.weekday().num_days_from_monday() as i64);

        // Day-of-week labels
        let day_labels = ["M", " ", "W", " ", "F", " ", "S"];
        for (row, label) in day_labels.iter().enumerate() {
            let y = inner.y + 1 + row as u16;
            if y < inner.y + inner.height {
                buf.set_string(
                    inner.x,
                    y,
                    label,
                    Style::default().fg(colors.text_pending()),
                );
            }
        }

        // Render weeks as columns
        let mut current_date = start_date;
        let mut col = 0u16;

        // Month labels
        let mut last_month = 0u32;

        while current_date <= end_date {
            let x = inner.x + 2 + col * 2;
            if x + 1 >= inner.x + inner.width {
                break;
            }

            // Month label on first row
            let month = current_date.month();
            if month != last_month {
                let month_name = match month {
                    1 => "Jan",
                    2 => "Feb",
                    3 => "Mar",
                    4 => "Apr",
                    5 => "May",
                    6 => "Jun",
                    7 => "Jul",
                    8 => "Aug",
                    9 => "Sep",
                    10 => "Oct",
                    11 => "Nov",
                    12 => "Dec",
                    _ => "",
                };
                // Only show if we have space (3 chars)
                if x + 3 <= inner.x + inner.width {
                    buf.set_string(
                        x,
                        inner.y,
                        month_name,
                        Style::default().fg(colors.text_pending()),
                    );
                }
                last_month = month;
            }

            // Render 7 days in this week column
            for day_offset in 0..7u16 {
                let date = current_date + chrono::Duration::days(day_offset as i64);
                if date > end_date {
                    break;
                }
                let y = inner.y + 1 + day_offset;
                if y >= inner.y + inner.height {
                    break;
                }

                let count = day_counts.get(&date).copied().unwrap_or(0);
                let color = intensity_cell_bg(count, colors);
                // Fill both columns so low-activity cells render as blocks instead of glyphs.
                // This avoids cursor-like artifacts in some terminal fonts.
                buf.set_string(x, y, "  ", Style::default().bg(color).fg(colors.bg()));
            }

            current_date += chrono::Duration::weeks(1);
            col += 1;
        }
    }
}

fn scale_color(base: Color, factor: f64) -> Color {
    match base {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f64 * factor).min(255.0) as u8,
            (g as f64 * factor).min(255.0) as u8,
            (b as f64 * factor).min(255.0) as u8,
        ),
        other => other,
    }
}

fn intensity_cell_bg(count: usize, colors: &crate::ui::theme::ThemeColors) -> Color {
    let success = colors.success();
    match count {
        0 => scale_color(colors.accent_dim(), 0.35),
        1..=2 => scale_color(success, 0.35),
        3..=5 => scale_color(success, 0.6),
        6..=15 => scale_color(success, 0.8),
        _ => success,
    }
}
