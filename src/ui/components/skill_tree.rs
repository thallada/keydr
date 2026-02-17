use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::engine::key_stats::KeyStatsStore;
use crate::engine::skill_tree::{
    BranchId, BranchStatus, DrillScope, SkillTree as SkillTreeEngine, get_branch_definition,
};
use crate::ui::theme::Theme;

pub struct SkillTreeWidget<'a> {
    skill_tree: &'a SkillTreeEngine,
    key_stats: &'a KeyStatsStore,
    selected: usize,
    detail_scroll: usize,
    theme: &'a Theme,
}

impl<'a> SkillTreeWidget<'a> {
    pub fn new(
        skill_tree: &'a SkillTreeEngine,
        key_stats: &'a KeyStatsStore,
        selected: usize,
        detail_scroll: usize,
        theme: &'a Theme,
    ) -> Self {
        Self {
            skill_tree,
            key_stats,
            selected,
            detail_scroll,
            theme,
        }
    }
}

/// Get the list of selectable branch IDs (Lowercase first, then other branches).
pub fn selectable_branches() -> Vec<BranchId> {
    vec![
        BranchId::Lowercase,
        BranchId::Capitals,
        BranchId::Numbers,
        BranchId::ProsePunctuation,
        BranchId::Whitespace,
        BranchId::CodeSymbols,
    ]
}

pub fn detail_line_count(branch_id: BranchId) -> usize {
    let def = get_branch_definition(branch_id);
    // 1 line branch header + for each level: 1 line level header + 1 line per key
    1 + def
        .levels
        .iter()
        .map(|level| 1 + level.keys.len())
        .sum::<usize>()
}

impl Widget for SkillTreeWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .title(" Skill Tree ")
            .border_style(Style::default().fg(colors.accent()))
            .style(Style::default().bg(colors.bg()));
        let inner = block.inner(area);
        block.render(area, buf);

        // Layout: header (2), branch list (dynamic), separator (1), detail panel (dynamic), footer (2)
        let branches = selectable_branches();
        let branch_list_height = branches.len() as u16 * 2 + 1; // all branches * 2 lines + separator after Lowercase

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(branch_list_height.min(inner.height.saturating_sub(6))),
                Constraint::Length(1),
                Constraint::Min(4),
                Constraint::Length(2),
            ])
            .split(inner);

        // --- Branch list ---
        self.render_branch_list(layout[0], buf, &branches);

        // --- Separator ---
        let sep = Paragraph::new(Line::from(Span::styled(
            "\u{2500}".repeat(layout[1].width as usize),
            Style::default().fg(colors.border()),
        )));
        sep.render(layout[1], buf);

        // --- Detail panel for selected branch ---
        self.render_detail_panel(layout[2], buf, &branches);

        // --- Footer ---
        let footer_text = if self.selected < branches.len() {
            let bp = self.skill_tree.branch_progress(branches[self.selected]);
            if *self.skill_tree.branch_status(branches[self.selected]) == BranchStatus::Locked {
                " Complete a-z to unlock branches   [\u{2191}\u{2193}/jk] Navigate   [PgUp/PgDn or Ctrl+U/Ctrl+D] Scroll   [q] Back "
            } else if bp.status == BranchStatus::Available || bp.status == BranchStatus::InProgress
            {
                " [Enter] Start Drill   [\u{2191}\u{2193}/jk] Navigate   [PgUp/PgDn or Ctrl+U/Ctrl+D] Scroll   [q] Back "
            } else {
                " [\u{2191}\u{2193}/jk] Navigate   [PgUp/PgDn or Ctrl+U/Ctrl+D] Scroll   [q] Back "
            }
        } else {
            " [\u{2191}\u{2193}/jk] Navigate   [PgUp/PgDn or Ctrl+U/Ctrl+D] Scroll   [q] Back "
        };

        let footer = Paragraph::new(Line::from(Span::styled(
            footer_text,
            Style::default().fg(colors.text_pending()),
        )));
        footer.render(layout[3], buf);
    }
}

impl SkillTreeWidget<'_> {
    fn render_branch_list(&self, area: Rect, buf: &mut Buffer, branches: &[BranchId]) {
        let colors = &self.theme.colors;
        let mut lines: Vec<Line> = Vec::new();

        for (i, &branch_id) in branches.iter().enumerate() {
            let bp = self.skill_tree.branch_progress(branch_id);
            let def = get_branch_definition(branch_id);
            let total_keys = def.levels.iter().map(|l| l.keys.len()).sum::<usize>();
            let confident_keys = self
                .skill_tree
                .branch_confident_keys(branch_id, self.key_stats);
            let is_selected = i == self.selected;

            let (prefix, style) = match bp.status {
                BranchStatus::Complete => (
                    "\u{2605} ",
                    Style::default()
                        .fg(colors.text_correct())
                        .add_modifier(Modifier::BOLD),
                ),
                BranchStatus::InProgress => (
                    "\u{25b6} ",
                    Style::default()
                        .fg(colors.accent())
                        .add_modifier(Modifier::BOLD),
                ),
                BranchStatus::Available => (
                    "  ",
                    Style::default().fg(colors.fg()),
                ),
                BranchStatus::Locked => ("  ", Style::default().fg(colors.text_pending())),
            };

            let unlocked = self.skill_tree.branch_unlocked_count(branch_id);
            let mastered_text = if confident_keys > 0 {
                format!(" ({confident_keys} mastered)")
            } else {
                String::new()
            };
            let status_text = match bp.status {
                BranchStatus::Complete => {
                    format!("{unlocked}/{total_keys} unlocked{mastered_text}")
                }
                BranchStatus::InProgress => {
                    if branch_id == BranchId::Lowercase {
                        format!("{unlocked}/{total_keys} unlocked{mastered_text}")
                    } else {
                        format!(
                            "Lvl {}/{}  {unlocked}/{total_keys} unlocked{mastered_text}",
                            bp.current_level + 1,
                            def.levels.len()
                        )
                    }
                }
                BranchStatus::Available => format!("0/{total_keys} unlocked"),
                BranchStatus::Locked => format!("Locked  0/{total_keys}"),
            };

            let sel_indicator = if is_selected { "> " } else { "  " };

            lines.push(Line::from(vec![
                Span::styled(format!("{sel_indicator}{prefix}{}", def.name), style),
                Span::styled(
                    format!("  {status_text}"),
                    Style::default().fg(colors.text_pending()),
                ),
            ]));

            let (mastered_bar, unlocked_bar, empty_bar) =
                dual_progress_bar_parts(confident_keys, unlocked, total_keys, 30);
            lines.push(Line::from(vec![
                Span::styled("    ", style),
                Span::styled(mastered_bar, Style::default().fg(colors.text_correct())),
                Span::styled(unlocked_bar, Style::default().fg(colors.accent())),
                Span::styled(empty_bar, Style::default().fg(colors.text_pending())),
            ]));

            // Add separator after Lowercase (index 0)
            if branch_id == BranchId::Lowercase {
                lines.push(Line::from(Span::styled(
                    "  \u{2500}\u{2500} Branches (unlocked after a-z) \u{2500}\u{2500}",
                    Style::default().fg(colors.border()),
                )));
            }
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }

    fn render_detail_panel(&self, area: Rect, buf: &mut Buffer, branches: &[BranchId]) {
        let colors = &self.theme.colors;

        if self.selected >= branches.len() {
            return;
        }

        let branch_id = branches[self.selected];
        let bp = self.skill_tree.branch_progress(branch_id);
        let def = get_branch_definition(branch_id);

        let mut lines: Vec<Line> = Vec::new();

        // Branch title with level info
        let level_text = if branch_id == BranchId::Lowercase {
            let unlocked = self.skill_tree.branch_unlocked_count(BranchId::Lowercase);
            let total = SkillTreeEngine::branch_total_keys(BranchId::Lowercase);
            format!("Unlocked {unlocked}/{total} letters")
        } else {
            match bp.status {
                BranchStatus::InProgress => {
                    format!("Level {}/{}", bp.current_level + 1, def.levels.len())
                }
                BranchStatus::Complete => {
                    format!("Level {}/{}", def.levels.len(), def.levels.len())
                }
                _ => format!("Level 0/{}", def.levels.len()),
            }
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}", def.name),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {level_text}"),
                Style::default().fg(colors.text_pending()),
            ),
        ]));

        // Per-level key breakdown with per-key mastery bars
        let focused = self
            .skill_tree
            .focused_key(DrillScope::Branch(branch_id), self.key_stats);

        // For Lowercase, determine which keys are unlocked
        let lowercase_unlocked_keys: Vec<char> = if branch_id == BranchId::Lowercase {
            self.skill_tree
                .unlocked_keys(DrillScope::Branch(BranchId::Lowercase))
        } else {
            Vec::new()
        };

        for (level_idx, level) in def.levels.iter().enumerate() {
            let level_status =
                if bp.status == BranchStatus::Complete || level_idx < bp.current_level {
                    "complete"
                } else if bp.status == BranchStatus::InProgress && level_idx == bp.current_level {
                    "in progress"
                } else {
                    "locked"
                };

            // Level header
            lines.push(Line::from(Span::styled(
                format!("  L{}: {} ({level_status})", level_idx + 1, level.name),
                Style::default().fg(colors.fg()),
            )));

            // Per-key mastery bars
            for &key in level.keys {
                let is_focused = focused == Some(key);
                let confidence = self.key_stats.get_confidence(key).min(1.0);
                let is_confident = confidence >= 1.0;

                // For Lowercase, check if this specific key is unlocked
                let is_locked = if branch_id == BranchId::Lowercase {
                    !lowercase_unlocked_keys.contains(&key)
                } else {
                    level_status == "locked"
                };

                let display = if key == '\n' {
                    "\\n".to_string()
                } else if key == '\t' {
                    "\\t".to_string()
                } else {
                    format!(" {key}")
                };

                if is_locked {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("    {display} "),
                            Style::default().fg(colors.text_pending()),
                        ),
                        Span::styled("locked", Style::default().fg(colors.text_pending())),
                    ]));
                } else {
                    let bar_width = 10;
                    let filled = (confidence * bar_width as f64).round() as usize;
                    let empty = bar_width - filled;
                    let bar = format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty));
                    let pct_str = format!("{:>3.0}%", confidence * 100.0);
                    let focus_label = if is_focused { "  in focus" } else { "" };

                    let key_style = if is_focused {
                        Style::default()
                            .fg(colors.bg())
                            .bg(colors.focused_key())
                            .add_modifier(Modifier::BOLD)
                    } else if is_confident {
                        Style::default().fg(colors.text_correct())
                    } else {
                        Style::default().fg(colors.fg())
                    };

                    let bar_color = if is_confident {
                        colors.text_correct()
                    } else {
                        colors.accent()
                    };

                    lines.push(Line::from(vec![
                        Span::styled(format!("    {display} "), key_style),
                        Span::styled(bar, Style::default().fg(bar_color)),
                        Span::styled(
                            format!(" {pct_str}"),
                            Style::default().fg(colors.text_pending()),
                        ),
                        Span::styled(
                            focus_label,
                            Style::default()
                                .fg(colors.focused_key())
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                }
            }
        }

        let visible_height = area.height as usize;
        if visible_height == 0 {
            return;
        }
        let max_scroll = lines.len().saturating_sub(visible_height);
        let scroll = self.detail_scroll.min(max_scroll);
        let visible_lines: Vec<Line> = lines.into_iter().skip(scroll).take(visible_height).collect();
        let paragraph = Paragraph::new(visible_lines);
        paragraph.render(area, buf);
    }
}

fn dual_progress_bar_parts(
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
