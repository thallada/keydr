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

impl Widget for BranchProgressList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;
        let mut lines: Vec<Line> = Vec::new();

        let drill_branch = match self.drill_scope {
            DrillScope::Branch(id) => Some(id),
            DrillScope::Global => None,
        };

        let show_all = self.height > 2;

        if show_all {
            for &branch_id in self.active_branches {
                if lines.len() as u16 >= self.height.saturating_sub(1) {
                    break;
                }
                let def = get_branch_definition(branch_id);
                let total = SkillTree::branch_total_keys(branch_id);
                let unlocked = self.skill_tree.branch_unlocked_count(branch_id);
                let mastered = self
                    .skill_tree
                    .branch_confident_keys(branch_id, self.key_stats);
                let is_active = drill_branch == Some(branch_id);
                let prefix = if is_active {
                    " \u{25b6} "
                } else {
                    " \u{00b7} "
                };
                let (m_bar, u_bar, e_bar) = compact_dual_bar_parts(mastered, unlocked, total, 12);
                let name = format!("{:<14}", def.name);
                let label_color = if is_active {
                    colors.accent()
                } else {
                    colors.text_pending()
                };
                lines.push(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(label_color)),
                    Span::styled(name, Style::default().fg(label_color)),
                    Span::styled(m_bar, Style::default().fg(colors.text_correct())),
                    Span::styled(u_bar, Style::default().fg(colors.accent())),
                    Span::styled(e_bar, Style::default().fg(colors.text_pending())),
                    Span::styled(
                        format!(" {unlocked}/{total}"),
                        Style::default().fg(colors.text_pending()),
                    ),
                ]));
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
