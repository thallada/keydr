use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::ui::theme::Theme;

pub struct MenuItem {
    pub key: String,
    pub label: String,
    pub description: String,
}

pub struct Menu<'a> {
    pub items: Vec<MenuItem>,
    pub selected: usize,
    pub theme: &'a Theme,
}

impl<'a> Menu<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            items: vec![
                MenuItem {
                    key: "1".to_string(),
                    label: "Adaptive Drill".to_string(),
                    description: "Phonetic words with adaptive letter unlocking".to_string(),
                },
                MenuItem {
                    key: "2".to_string(),
                    label: "Code Drill".to_string(),
                    description: "Practice typing code syntax".to_string(),
                },
                MenuItem {
                    key: "3".to_string(),
                    label: "Passage Drill".to_string(),
                    description: "Type passages from books".to_string(),
                },
                MenuItem {
                    key: "s".to_string(),
                    label: "Statistics".to_string(),
                    description: "View your typing statistics".to_string(),
                },
                MenuItem {
                    key: "c".to_string(),
                    label: "Settings".to_string(),
                    description: "Configure keydr".to_string(),
                },
            ],
            selected: 0,
            theme,
        }
    }

    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % self.items.len();
    }

    pub fn prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else {
            self.selected = self.items.len() - 1;
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

        let title_lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "keydr",
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Terminal Typing Tutor",
                Style::default().fg(colors.fg()),
            )),
            Line::from(""),
        ];

        let title = Paragraph::new(title_lines).alignment(Alignment::Center);
        title.render(layout[0], buf);

        let menu_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                self.items
                    .iter()
                    .map(|_| Constraint::Length(3))
                    .collect::<Vec<_>>(),
            )
            .split(layout[2]);

        for (i, item) in self.items.iter().enumerate() {
            let is_selected = i == self.selected;
            let indicator = if is_selected { ">" } else { " " };

            let label_text = format!(" {indicator} [{key}] {label}", key = item.key, label = item.label);
            let desc_text = format!("     {}", item.description);

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
