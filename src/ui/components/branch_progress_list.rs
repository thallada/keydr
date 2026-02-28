use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::engine::skill_tree::{BranchId, DrillScope, SkillTree, get_branch_definition};
use crate::ui::theme::Theme;

pub struct BranchProgressList<'a> {
    pub skill_tree: &'a SkillTree,
    pub key_stats: &'a crate::engine::key_stats::KeyStatsStore,
    pub drill_scope: DrillScope,
    pub active_branches: &'a [BranchId],
    pub theme: &'a Theme,
    pub height: u16,
}

const MIN_BRANCH_CELL_WIDTH: usize = 28;
const BRANCH_CELL_GUTTER: usize = 1;

pub fn wrapped_branch_rows(area_width: u16, branch_count: usize) -> u16 {
    if branch_count == 0 {
        return 0;
    }
    let columns = wrapped_branch_columns(area_width, branch_count);
    branch_count.div_ceil(columns) as u16
}

fn wrapped_branch_columns(area_width: u16, branch_count: usize) -> usize {
    if branch_count == 0 {
        return 1;
    }
    let width = area_width as usize;
    let max_cols_by_width =
        ((width + BRANCH_CELL_GUTTER) / (MIN_BRANCH_CELL_WIDTH + BRANCH_CELL_GUTTER)).max(1);
    max_cols_by_width.min(branch_count)
}

impl Widget for BranchProgressList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;
        let mut lines: Vec<Line> = Vec::new();

        let drill_branch = match self.drill_scope {
            DrillScope::Branch(id) => Some(id),
            DrillScope::Global => None,
        };

        let show_all = should_render_branch_rows(self.height, self.active_branches.len());

        if show_all {
            let columns = wrapped_branch_columns(area.width, self.active_branches.len());
            let rows = self.active_branches.len().div_ceil(columns);
            let available_width = area.width as usize;
            let total_gutter = BRANCH_CELL_GUTTER.saturating_mul(columns.saturating_sub(1));
            let cell_width = available_width.saturating_sub(total_gutter) / columns;

            for row in 0..rows {
                if lines.len() as u16 >= self.height.saturating_sub(1) {
                    break;
                }

                let mut spans: Vec<Span> = Vec::new();
                for col in 0..columns {
                    let idx = row * columns + col;
                    if idx >= self.active_branches.len() {
                        break;
                    }
                    if col > 0 {
                        spans.push(Span::raw(" ".repeat(BRANCH_CELL_GUTTER)));
                    }
                    let branch_id = self.active_branches[idx];
                    spans.extend(render_branch_cell(
                        branch_id,
                        drill_branch == Some(branch_id),
                        cell_width,
                        self.skill_tree,
                        self.key_stats,
                        self.theme,
                    ));
                }
                lines.push(Line::from(spans));
            }
        } else if let Some(branch_id) = drill_branch {
            let def = get_branch_definition(branch_id);
            let total = SkillTree::branch_total_keys(branch_id);
            let unlocked = self.skill_tree.branch_unlocked_count(branch_id);
            let mastered = self
                .skill_tree
                .branch_confident_keys(branch_id, self.key_stats);
            let (m_bar, u_bar, e_bar) = compact_dual_bar_parts(mastered, unlocked, total, 12);
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" \u{25b6} {:<14}", def.name),
                    Style::default().fg(colors.accent()),
                ),
                Span::styled(m_bar, Style::default().fg(colors.text_correct())),
                Span::styled(u_bar, Style::default().fg(colors.accent())),
                Span::styled(e_bar, Style::default().fg(colors.text_pending())),
                Span::styled(
                    format!(" {unlocked}/{total}"),
                    Style::default().fg(colors.text_pending()),
                ),
            ]));
        }

        // Overall line
        if lines.len() < self.height as usize {
            if should_insert_overall_separator(lines.len(), self.height as usize) {
                lines.push(Line::from(""));
            }
            let total = self.skill_tree.total_unique_keys;
            let unlocked = self.skill_tree.total_unlocked_count();
            let mastered = self.skill_tree.total_confident_keys(self.key_stats);
            let left_pad = if area.width >= 90 {
                3
            } else if area.width >= 70 {
                2
            } else if area.width >= 55 {
                1
            } else {
                0
            };
            let right_pad = if area.width >= 75 { 2 } else { 0 };
            let label = format!("{}Overall Key Progress  ", " ".repeat(left_pad));
            let suffix = format!(
                " {unlocked}/{total} unlocked ({mastered} mastered){}",
                " ".repeat(right_pad)
            );
            let reserved = label.len() + suffix.len();
            let bar_width = (area.width as usize).saturating_sub(reserved).max(6);
            let (m_bar, u_bar, e_bar) =
                compact_dual_bar_parts(mastered, unlocked, total, bar_width);
            lines.push(Line::from(vec![
                Span::styled(label, Style::default().fg(colors.fg())),
                Span::styled(m_bar, Style::default().fg(colors.text_correct())),
                Span::styled(u_bar, Style::default().fg(colors.accent())),
                Span::styled(e_bar, Style::default().fg(colors.text_pending())),
                Span::styled(suffix, Style::default().fg(colors.text_pending())),
            ]));
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }
}

fn should_render_branch_rows(height: u16, active_branch_count: usize) -> bool {
    active_branch_count > 0 && height > 1
}

fn should_insert_overall_separator(current_lines: usize, total_height: usize) -> bool {
    current_lines > 0 && current_lines + 2 <= total_height
}

fn render_branch_cell<'a>(
    branch_id: BranchId,
    is_active: bool,
    cell_width: usize,
    skill_tree: &SkillTree,
    key_stats: &crate::engine::key_stats::KeyStatsStore,
    theme: &'a Theme,
) -> Vec<Span<'a>> {
    let colors = &theme.colors;
    let def = get_branch_definition(branch_id);
    let total = SkillTree::branch_total_keys(branch_id);
    let unlocked = skill_tree.branch_unlocked_count(branch_id);
    let mastered = skill_tree.branch_confident_keys(branch_id, key_stats);

    let prefix = if is_active { "\u{25b6} " } else { "\u{00b7} " };
    let label_color = if is_active {
        colors.accent()
    } else {
        colors.text_pending()
    };
    let count = format!("{unlocked}/{total}");
    let name_width = if cell_width >= 34 {
        14
    } else if cell_width >= 30 {
        12
    } else {
        10
    };
    let fixed = prefix.len() + name_width + 1 + count.len();
    let bar_width = cell_width.saturating_sub(fixed).max(6);
    let (m_bar, u_bar, e_bar) = compact_dual_bar_parts(mastered, unlocked, total, bar_width);
    let name = truncate_and_pad(def.name, name_width);

    let mut spans: Vec<Span> = vec![
        Span::styled(prefix.to_string(), Style::default().fg(label_color)),
        Span::styled(name, Style::default().fg(label_color)),
        Span::styled(m_bar, Style::default().fg(colors.text_correct())),
        Span::styled(u_bar, Style::default().fg(colors.accent())),
        Span::styled(e_bar, Style::default().fg(colors.text_pending())),
        Span::styled(
            format!(" {count}"),
            Style::default().fg(colors.text_pending()),
        ),
    ];

    let used = prefix.len() + name_width + bar_width + 1 + count.len();
    if cell_width > used {
        spans.push(Span::raw(" ".repeat(cell_width - used)));
    }
    spans
}

fn truncate_and_pad(name: &str, width: usize) -> String {
    let mut text: String = name.chars().take(width).collect();
    let len = text.chars().count();
    if len < width {
        text.push_str(&" ".repeat(width - len));
    }
    text
}

fn compact_dual_bar_parts(
    mastered: usize,
    unlocked: usize,
    total: usize,
    width: usize,
) -> (String, String, String) {
    if total == 0 {
        return (String::new(), String::new(), "\u{2591}".repeat(width));
    }
    let mastered_cells = mastered * width / total;
    let unlocked_cells = (unlocked * width / total).max(mastered_cells);
    let empty_cells = width - unlocked_cells;
    (
        "\u{2588}".repeat(mastered_cells),
        "\u{2593}".repeat(unlocked_cells - mastered_cells),
        "\u{2591}".repeat(empty_cells),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_rows_wraps_when_needed() {
        assert_eq!(wrapped_branch_rows(120, 6), 2);
        assert_eq!(wrapped_branch_rows(70, 6), 3);
        assert_eq!(wrapped_branch_rows(50, 3), 3);
        assert_eq!(wrapped_branch_rows(120, 0), 0);
    }

    #[test]
    fn renders_branch_rows_when_height_is_two() {
        assert!(should_render_branch_rows(2, 6));
        assert!(!should_render_branch_rows(1, 6));
        assert!(!should_render_branch_rows(2, 0));
    }

    #[test]
    fn overall_separator_only_when_space_available() {
        assert!(should_insert_overall_separator(1, 3));
        assert!(should_insert_overall_separator(2, 4));
        assert!(!should_insert_overall_separator(1, 2));
        assert!(!should_insert_overall_separator(0, 4));
    }
}
