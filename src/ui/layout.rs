use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutTier {
    Wide,   // ≥100 cols: typing area + sidebar, keyboard, progress bar
    Medium, // 60-99 cols: full-width typing, compact stats header, compact keyboard
    Narrow, // <60 cols: full-width typing, stats header only
}

impl LayoutTier {
    pub fn from_area(area: Rect) -> Self {
        if area.width >= 100 {
            LayoutTier::Wide
        } else if area.width >= 60 {
            LayoutTier::Medium
        } else {
            LayoutTier::Narrow
        }
    }

    pub fn show_keyboard(&self, height: u16) -> bool {
        height >= 20 && *self != LayoutTier::Narrow
    }

    pub fn show_progress_bar(&self, height: u16) -> bool {
        height >= 20 && *self != LayoutTier::Narrow
    }

    pub fn show_sidebar(&self) -> bool {
        *self == LayoutTier::Wide
    }

    pub fn compact_keyboard(&self) -> bool {
        *self == LayoutTier::Medium
    }
}

pub struct AppLayout {
    pub header: Rect,
    pub main: Rect,
    pub sidebar: Option<Rect>,
    pub footer: Rect,
    pub tier: LayoutTier,
}

impl AppLayout {
    pub fn new(area: Rect) -> Self {
        let tier = LayoutTier::from_area(area);

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(area);

        if tier.show_sidebar() {
            let horizontal = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(vertical[1]);

            Self {
                header: vertical[0],
                main: horizontal[0],
                sidebar: Some(horizontal[1]),
                footer: vertical[2],
                tier,
            }
        } else {
            Self {
                header: vertical[0],
                main: vertical[1],
                sidebar: None,
                footer: vertical[2],
                tier,
            }
        }
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
