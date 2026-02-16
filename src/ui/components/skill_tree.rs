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
    theme: &'a Theme,
}

impl<'a> SkillTreeWidget<'a> {
    pub fn new(
        skill_tree: &'a SkillTreeEngine,
        key_stats: &'a KeyStatsStore,
        selected: usize,
        theme: &'a Theme,
    ) -> Self {
        Self {
            skill_tree,
            key_stats,
            selected,
            theme,
        }
    }
}

/// Get the list of selectable branch IDs (all non-Lowercase branches).
pub fn selectable_branches() -> Vec<BranchId> {
    vec![
        BranchId::Capitals,
        BranchId::Numbers,
        BranchId::ProsePunctuation,
        BranchId::Whitespace,
        BranchId::CodeSymbols,
    ]
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
        let branch_list_height = 3 + branches.len() as u16 * 2 + 1; // root + separator + 5 branches

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
                " Complete a-z to unlock branches "
            } else if bp.status == BranchStatus::Available || bp.status == BranchStatus::InProgress
            {
                " [Enter] Start Drill   [\u{2191}\u{2193}/jk] Navigate   [q] Back "
            } else {
                " [\u{2191}\u{2193}/jk] Navigate   [q] Back "
            }
        } else {
            " [\u{2191}\u{2193}/jk] Navigate   [q] Back "
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

        // Root: Lowercase a-z
        let lowercase_bp = self.skill_tree.branch_progress(BranchId::Lowercase);
        let lowercase_def = get_branch_definition(BranchId::Lowercase);
        let lowercase_total = lowercase_def
            .levels
            .iter()
            .map(|l| l.keys.len())
            .sum::<usize>();
        let lowercase_confident = self
            .skill_tree
            .branch_confident_keys(BranchId::Lowercase, self.key_stats);

        let (prefix, style) = match lowercase_bp.status {
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
            _ => ("  ", Style::default().fg(colors.text_pending())),
        };

        let status_text = match lowercase_bp.status {
            BranchStatus::Complete => "COMPLETE".to_string(),
            BranchStatus::InProgress => {
                let unlocked = self.skill_tree.lowercase_unlocked_count();
                format!("{unlocked}/{lowercase_total}")
            }
            _ => "LOCKED".to_string(),
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("  {prefix}{name}", name = lowercase_def.name),
                style,
            ),
            Span::styled(
                format!("  {status_text}  {lowercase_confident}/{lowercase_total} keys"),
                Style::default().fg(colors.text_pending()),
            ),
        ]));

        // Progress bar for lowercase
        let pct = if lowercase_total > 0 {
            lowercase_confident as f64 / lowercase_total as f64
        } else {
            0.0
        };
        lines.push(Line::from(Span::styled(
            format!("    {}", progress_bar_str(pct, 30)),
            style,
        )));

        // Separator
        lines.push(Line::from(Span::styled(
            "  \u{2500}\u{2500} Branches (unlocked after a-z) \u{2500}\u{2500}",
            Style::default().fg(colors.border()),
        )));

        // Branches
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
                    if is_selected {
                        Style::default()
                            .fg(colors.text_correct())
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::default()
                            .fg(colors.text_correct())
                            .add_modifier(Modifier::BOLD)
                    },
                ),
                BranchStatus::InProgress => (
                    "\u{25b6} ",
                    if is_selected {
                        Style::default()
                            .fg(colors.accent())
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::default()
                            .fg(colors.accent())
                            .add_modifier(Modifier::BOLD)
                    },
                ),
                BranchStatus::Available => (
                    "  ",
                    if is_selected {
                        Style::default()
                            .fg(colors.fg())
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::default().fg(colors.fg())
                    },
                ),
                BranchStatus::Locked => ("  ", Style::default().fg(colors.text_pending())),
            };

            let status_text = match bp.status {
                BranchStatus::Complete => format!("COMPLETE  {confident_keys}/{total_keys} keys"),
                BranchStatus::InProgress => format!(
                    "Lvl {}/{} {confident_keys}/{total_keys} keys",
                    bp.current_level + 1,
                    def.levels.len()
                ),
                BranchStatus::Available => format!("Available  0/{total_keys} keys"),
                BranchStatus::Locked => format!("Locked  0/{total_keys} keys"),
            };

            let sel_indicator = if is_selected { "> " } else { "  " };

            lines.push(Line::from(vec![
                Span::styled(format!("{sel_indicator}{prefix}{}", def.name), style),
                Span::styled(
                    format!("  {status_text}"),
                    Style::default().fg(colors.text_pending()),
                ),
            ]));

            let pct = if total_keys > 0 {
                confident_keys as f64 / total_keys as f64
            } else {
                0.0
            };
            lines.push(Line::from(Span::styled(
                format!("    {}", progress_bar_str(pct, 30)),
                style,
            )));
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
        let level_text = match bp.status {
            BranchStatus::InProgress => {
                format!("Level {}/{}", bp.current_level + 1, def.levels.len())
            }
            BranchStatus::Complete => format!("Level {}/{}", def.levels.len(), def.levels.len()),
            _ => format!("Level 0/{}", def.levels.len()),
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

        // Per-level key breakdown
        let focused = self
            .skill_tree
            .focused_key(DrillScope::Branch(branch_id), self.key_stats);

        for (level_idx, level) in def.levels.iter().enumerate() {
            let level_status =
                if bp.status == BranchStatus::Complete || level_idx < bp.current_level {
                    "complete"
                } else if bp.status == BranchStatus::InProgress && level_idx == bp.current_level {
                    "in progress"
                } else {
                    "locked"
                };

            let mut key_spans: Vec<Span> = Vec::new();
            key_spans.push(Span::styled(
                format!("  L{}: ", level_idx + 1),
                Style::default().fg(colors.fg()),
            ));

            for &key in level.keys {
                let is_confident = self.key_stats.get_confidence(key) >= 1.0;
                let is_focused = focused == Some(key);

                let display = if key == '\n' {
                    "\\n".to_string()
                } else if key == '\t' {
                    "\\t".to_string()
                } else {
                    key.to_string()
                };

                let style = if is_focused {
                    Style::default()
                        .fg(colors.bg())
                        .bg(colors.focused_key())
                        .add_modifier(Modifier::BOLD)
                } else if is_confident {
                    Style::default().fg(colors.text_correct())
                } else if level_status == "locked" {
                    Style::default().fg(colors.text_pending())
                } else {
                    Style::default().fg(colors.fg())
                };

                key_spans.push(Span::styled(display, style));
                key_spans.push(Span::raw(" "));
            }

            key_spans.push(Span::styled(
                format!(" ({level_status})"),
                Style::default().fg(colors.text_pending()),
            ));

            lines.push(Line::from(key_spans));
        }

        // Average confidence
        let total_keys = def.levels.iter().map(|l| l.keys.len()).sum::<usize>();
        let avg_conf = if total_keys > 0 {
            let sum: f64 = def
                .levels
                .iter()
                .flat_map(|l| l.keys.iter())
                .map(|&ch| self.key_stats.get_confidence(ch).min(1.0))
                .sum();
            sum / total_keys as f64
        } else {
            0.0
        };

        lines.push(Line::from(Span::styled(
            format!(
                "  Avg Confidence: {} {:.0}%",
                progress_bar_str(avg_conf, 20),
                avg_conf * 100.0
            ),
            Style::default().fg(colors.text_pending()),
        )));

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }
}

fn progress_bar_str(pct: f64, width: usize) -> String {
    let filled = (pct * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty),)
}
