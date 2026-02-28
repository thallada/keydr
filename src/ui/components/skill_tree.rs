use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use crate::engine::key_stats::KeyStatsStore;
use crate::engine::skill_tree::{
    BranchId, BranchStatus, DrillScope, SkillTree as SkillTreeEngine, get_branch_definition,
};
use crate::ui::layout::{pack_hint_lines, wrapped_line_count};
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

pub fn detail_line_count_with_level_spacing(branch_id: BranchId, level_spacing: bool) -> usize {
    let base = detail_line_count(branch_id);
    if !level_spacing {
        return base;
    }
    let def = get_branch_definition(branch_id);
    base + def.levels.len().saturating_sub(1)
}

pub fn use_expanded_level_spacing(detail_area_height: u16, branch_id: BranchId) -> bool {
    let def = get_branch_definition(branch_id);
    let base = detail_line_count(branch_id);
    let extra = def.levels.len().saturating_sub(1);
    (detail_area_height as usize) >= base + extra
}

pub fn use_side_by_side_layout(inner_width: u16) -> bool {
    inner_width >= 100
}

pub fn branch_list_spacing_flags(branch_area_height: u16, branch_count: usize) -> (bool, bool) {
    if branch_count == 0 {
        return (false, false);
    }
    // Base lines: 2 per branch + 1 separator after lowercase.
    let base_lines = branch_count * 2 + 1;
    let extra_lines = (branch_area_height as usize).saturating_sub(base_lines);
    // Priority 1: one spacer between each progress bar and following branch title.
    let inter_branch_needed = branch_count.saturating_sub(1);
    let inter_branch_spacing = extra_lines >= inter_branch_needed;
    // Priority 2: one extra line above and below "Branches (...)" separator.
    let separator_padding = inter_branch_spacing && extra_lines >= inter_branch_needed + 2;
    (inter_branch_spacing, separator_padding)
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

        // Layout: main split (branch list + detail) and footer (adaptive height)
        let branches = selectable_branches();
        let (footer_hints, footer_notice) = if self.selected < branches.len() {
            let bp = self.skill_tree.branch_progress(branches[self.selected]);
            if *self.skill_tree.branch_status(branches[self.selected]) == BranchStatus::Locked {
                (
                    vec![
                        "[↑↓/jk] Navigate",
                        "[PgUp/PgDn or Ctrl+U/Ctrl+D] Scroll",
                        "[q] Back",
                    ],
                    Some("Complete a-z to unlock branches"),
                )
            } else if bp.status == BranchStatus::Available {
                (
                    vec![
                        "[Enter] Unlock",
                        "[↑↓/jk] Navigate",
                        "[PgUp/PgDn or Ctrl+U/Ctrl+D] Scroll",
                        "[q] Back",
                    ],
                    None,
                )
            } else if bp.status == BranchStatus::InProgress {
                (
                    vec![
                        "[Enter] Start Drill",
                        "[↑↓/jk] Navigate",
                        "[PgUp/PgDn or Ctrl+U/Ctrl+D] Scroll",
                        "[q] Back",
                    ],
                    None,
                )
            } else {
                (
                    vec![
                        "[↑↓/jk] Navigate",
                        "[PgUp/PgDn or Ctrl+U/Ctrl+D] Scroll",
                        "[q] Back",
                    ],
                    None,
                )
            }
        } else {
            (
                vec![
                    "[↑↓/jk] Navigate",
                    "[PgUp/PgDn or Ctrl+U/Ctrl+D] Scroll",
                    "[q] Back",
                ],
                None,
            )
        };
        let hint_lines = pack_hint_lines(&footer_hints, inner.width as usize);
        let notice_lines = footer_notice
            .map(|text| wrapped_line_count(text, inner.width as usize))
            .unwrap_or(0);
        let show_notice = footer_notice.is_some()
            && (inner.height as usize >= hint_lines.len() + notice_lines + 8);
        let footer_needed = hint_lines.len() + if show_notice { notice_lines } else { 0 } + 1;
        let footer_height = footer_needed
            .min(inner.height.saturating_sub(5) as usize)
            .max(1) as u16;

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(footer_height)])
            .split(inner);

        if use_side_by_side_layout(inner.width) {
            let main = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(42),
                    Constraint::Length(1),
                    Constraint::Percentage(58),
                ])
                .split(layout[0]);

            // --- Branch list (left pane) ---
            let (inter_branch_spacing, separator_padding) =
                branch_list_spacing_flags(main[0].height, branches.len());
            self.render_branch_list(
                main[0],
                buf,
                &branches,
                inter_branch_spacing,
                separator_padding,
            );

            // --- Vertical separator ---
            let sep_lines: Vec<Line> = (0..main[1].height)
                .map(|_| {
                    Line::from(Span::styled(
                        "\u{2502}",
                        Style::default().fg(colors.border()),
                    ))
                })
                .collect();
            Paragraph::new(sep_lines).render(main[1], buf);

            // --- Detail panel for selected branch (right pane) ---
            self.render_detail_panel(main[2], buf, &branches, true);
        } else {
            let branch_list_height = branches.len() as u16 * 2 + 1;
            let main = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(branch_list_height.min(layout[0].height.saturating_sub(4))),
                    Constraint::Length(1),
                    Constraint::Min(3),
                ])
                .split(layout[0]);

            // --- Branch list (top pane) ---
            self.render_branch_list(main[0], buf, &branches, false, false);

            // --- Horizontal separator ---
            let sep = Paragraph::new(Line::from(Span::styled(
                "\u{2500}".repeat(main[1].width as usize),
                Style::default().fg(colors.border()),
            )));
            sep.render(main[1], buf);

            // --- Detail panel (bottom pane) ---
            self.render_detail_panel(main[2], buf, &branches, true);
        }

        // --- Footer ---
        let mut footer_lines: Vec<Line> = Vec::new();
        if show_notice {
            if let Some(notice) = footer_notice {
                footer_lines.push(Line::from(Span::styled(
                    format!(" {notice}"),
                    Style::default().fg(colors.text_pending()),
                )));
            }
        }
        footer_lines.extend(hint_lines.into_iter().map(|line| {
            Line::from(Span::styled(
                line,
                Style::default().fg(colors.text_pending()),
            ))
        }));
        let footer = Paragraph::new(footer_lines).wrap(Wrap { trim: false });
        footer.render(layout[1], buf);
    }
}

impl SkillTreeWidget<'_> {
    fn render_branch_list(
        &self,
        area: Rect,
        buf: &mut Buffer,
        branches: &[BranchId],
        inter_branch_spacing: bool,
        separator_padding: bool,
    ) {
        let colors = &self.theme.colors;
        let mut lines: Vec<Line> = Vec::new();

        for (i, &branch_id) in branches.iter().enumerate() {
            if i > 0 && inter_branch_spacing {
                lines.push(Line::from(""));
            }

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
                BranchStatus::Available => ("  ", Style::default().fg(colors.fg())),
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
                if separator_padding {
                    lines.push(Line::from(""));
                }
                lines.push(Line::from(Span::styled(
                    "  \u{2500}\u{2500} Branches (available after a-z) \u{2500}\u{2500}",
                    Style::default().fg(colors.text_pending()),
                )));
                // If inter-branch spacing is enabled, the next branch will already
                // insert one blank line before its title.
                if separator_padding && !inter_branch_spacing {
                    lines.push(Line::from(""));
                }
            }
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }

    fn render_detail_panel(
        &self,
        area: Rect,
        buf: &mut Buffer,
        branches: &[BranchId],
        allow_expanded_level_spacing: bool,
    ) {
        let colors = &self.theme.colors;

        if self.selected >= branches.len() {
            return;
        }

        let branch_id = branches[self.selected];
        let bp = self.skill_tree.branch_progress(branch_id);
        let def = get_branch_definition(branch_id);
        let expanded_level_spacing =
            allow_expanded_level_spacing && use_expanded_level_spacing(area.height, branch_id);

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

            if expanded_level_spacing && level_idx + 1 < def.levels.len() {
                lines.push(Line::from(""));
            }
        }

        let visible_height = area.height as usize;
        if visible_height == 0 {
            return;
        }
        let max_scroll = lines.len().saturating_sub(visible_height);
        let scroll = self.detail_scroll.min(max_scroll);
        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(scroll)
            .take(visible_height)
            .collect();
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
