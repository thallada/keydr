use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::i18n::t;
use crate::ui::theme::Theme;

const MENU_ITEMS: &[(&str, &str, &str)] = &[
    ("1", "menu.adaptive_drill", "menu.adaptive_drill_desc"),
    ("2", "menu.code_drill", "menu.code_drill_desc"),
    ("3", "menu.passage_drill", "menu.passage_drill_desc"),
    ("t", "menu.skill_tree", "menu.skill_tree_desc"),
    ("b", "menu.keyboard", "menu.keyboard_desc"),
    ("s", "menu.statistics", "menu.statistics_desc"),
    ("c", "menu.settings", "menu.settings_desc"),
];

pub struct Menu<'a> {
    pub selected: usize,
    pub theme: &'a Theme,
}

impl<'a> Menu<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            selected: 0,
            theme,
        }
    }

    pub fn item_count() -> usize {
        MENU_ITEMS.len()
    }

    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % MENU_ITEMS.len();
    }

    pub fn prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else {
            self.selected = MENU_ITEMS.len() - 1;
        }
    }
}

impl Widget for &Menu<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let colors = &self.theme.colors;

        let block = Block::bordered()
            .border_style(Style::default().fg(colors.border()))
            .style(Style::default().bg(colors.bg()));
        let inner = block.inner(area);
        block.render(area, buf);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);

        let subtitle = t!("menu.subtitle");
        let title_lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "keydr",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                subtitle.as_ref(),
                Style::default().fg(colors.fg()),
            )),
            Line::from(""),
        ];

        let title = Paragraph::new(title_lines).alignment(Alignment::Center);
        title.render(layout[0], buf);

        let menu_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                MENU_ITEMS
                    .iter()
                    .map(|_| Constraint::Length(3))
                    .collect::<Vec<_>>(),
            )
            .split(layout[2]);
        let key_width = MENU_ITEMS
            .iter()
            .map(|(key, _, _)| key.len())
            .max()
            .unwrap_or(1);

        for (i, &(key, label_key, desc_key)) in MENU_ITEMS.iter().enumerate() {
            let is_selected = i == self.selected;
            let indicator = if is_selected { ">" } else { " " };
            let label = t!(label_key);
            let description = t!(desc_key);

            let label_text = format!(
                " {indicator} [{key:<key_width$}] {label}",
                key_width = key_width,
            );
            let desc_text = format!(
                "   {:indent$}{description}",
                "",
                indent = key_width + 4
            );

            let lines = vec![
                Line::from(Span::styled(
                    &*label_text,
                    Style::default()
                        .fg(if is_selected {
                            colors.accent()
                        } else {
                            colors.fg()
                        })
                        .add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                )),
                Line::from(Span::styled(
                    &*desc_text,
                    Style::default().fg(colors.text_pending()),
                )),
            ];

            let p = Paragraph::new(lines);
            if i < menu_layout.len() {
                p.render(menu_layout[i], buf);
            }
        }
    }
}
