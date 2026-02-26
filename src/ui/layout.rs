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
    const MIN_POPUP_WIDTH: u16 = 72;
    const MIN_POPUP_HEIGHT: u16 = 18;

    let requested_w = area.width.saturating_mul(percent_x.min(100)) / 100;
    let requested_h = area.height.saturating_mul(percent_y.min(100)) / 100;

    let target_w = requested_w.max(MIN_POPUP_WIDTH).min(area.width);
    let target_h = requested_h.max(MIN_POPUP_HEIGHT).min(area.height);

    let left = area
        .x
        .saturating_add((area.width.saturating_sub(target_w)) / 2);
    let top = area
        .y
        .saturating_add((area.height.saturating_sub(target_h)) / 2);

    Rect::new(left, top, target_w, target_h)
}
