mod app;
mod config;
mod engine;
mod event;
mod generator;
mod keyboard;
mod session;
mod store;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};

use app::{App, AppScreen, DrillMode};
use engine::skill_tree::DrillScope;
use event::{AppEvent, EventHandler};
use generator::passage::passage_options;
use ui::components::dashboard::Dashboard;
use ui::components::keyboard_diagram::KeyboardDiagram;
use ui::components::skill_tree::{SkillTreeWidget, detail_line_count, selectable_branches};
use ui::components::stats_dashboard::StatsDashboard;
use ui::components::stats_sidebar::StatsSidebar;
use ui::components::typing_area::TypingArea;
use ui::layout::AppLayout;

#[derive(Parser)]
#[command(
    name = "keydr",
    version,
    about = "Terminal typing tutor with adaptive learning"
)]
struct Cli {
    #[arg(short, long, help = "Theme name")]
    theme: Option<String>,

    #[arg(short, long, help = "Keyboard layout (qwerty, dvorak, colemak)")]
    layout: Option<String>,

    #[arg(short, long, help = "Number of words per drill")]
    words: Option<usize>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut app = App::new();

    if let Some(words) = cli.words {
        app.config.word_count = words;
    }
    if let Some(theme_name) = cli.theme {
        if let Some(theme) = ui::theme::Theme::load(&theme_name) {
            let theme: &'static ui::theme::Theme = Box::leak(Box::new(theme));
            app.theme = theme;
            app.menu.theme = theme;
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Try to enable keyboard enhancement for Release event support
    let keyboard_enhanced = execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
    )
    .is_ok();

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let events = EventHandler::new(Duration::from_millis(100));

    let result = run_app(&mut terminal, &mut app, &events);

    if keyboard_enhanced {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {err:?}");
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    events: &EventHandler,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app))?;

        match events.next()? {
            AppEvent::Key(key) => handle_key(app, key),
            AppEvent::Tick => {
                if (app.screen == AppScreen::PassageIntro
                    || app.screen == AppScreen::PassageDownloadProgress)
                    && app.passage_intro_downloading
                {
                    app.process_passage_download_tick();
                }
                // Fallback: clear depressed keys after 150ms if no Release event received
                if let Some(last) = app.last_key_time {
                    if last.elapsed() > Duration::from_millis(150) && !app.depressed_keys.is_empty()
                    {
                        app.depressed_keys.clear();
                        app.last_key_time = None;
                    }
                    // Clear shift_held after 200ms as fallback
                    if last.elapsed() > Duration::from_millis(200) && app.shift_held {
                        app.shift_held = false;
                    }
                }
            }
            AppEvent::Resize(_, _) => {}
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    // Track depressed keys and shift state for keyboard diagram
    match (&key.code, key.kind) {
        (KeyCode::Char(ch), KeyEventKind::Press) => {
            app.depressed_keys.insert(ch.to_ascii_lowercase());
            app.last_key_time = Some(Instant::now());
            app.shift_held = key.modifiers.contains(KeyModifiers::SHIFT);
        }
        (KeyCode::Char(ch), KeyEventKind::Release) => {
            app.depressed_keys.remove(&ch.to_ascii_lowercase());
            return; // Don't process Release events as input
        }
        (_, KeyEventKind::Release) => return,
        _ => {
            app.shift_held = key.modifiers.contains(KeyModifiers::SHIFT);
        }
    }

    // Only process Press events — ignore Repeat to avoid inflating input
    if key.kind != KeyEventKind::Press {
        return;
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    match app.screen {
        AppScreen::Menu => handle_menu_key(app, key),
        AppScreen::Drill => handle_drill_key(app, key),
        AppScreen::DrillResult => handle_result_key(app, key),
        AppScreen::StatsDashboard => handle_stats_key(app, key),
        AppScreen::Settings => handle_settings_key(app, key),
        AppScreen::SkillTree => handle_skill_tree_key(app, key),
        AppScreen::CodeLanguageSelect => handle_code_language_key(app, key),
        AppScreen::PassageBookSelect => handle_passage_book_key(app, key),
        AppScreen::PassageIntro => handle_passage_intro_key(app, key),
        AppScreen::PassageDownloadProgress => handle_passage_download_progress_key(app, key),
    }
}

fn handle_menu_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('1') => {
            app.drill_mode = DrillMode::Adaptive;
            app.drill_scope = DrillScope::Global;
            app.start_drill();
        }
        KeyCode::Char('2') => {
            app.go_to_code_language_select();
        }
        KeyCode::Char('3') => {
            app.go_to_passage_book_select();
        }
        KeyCode::Char('t') => app.go_to_skill_tree(),
        KeyCode::Char('s') => app.go_to_stats(),
        KeyCode::Char('c') => app.go_to_settings(),
        KeyCode::Up | KeyCode::Char('k') => app.menu.prev(),
        KeyCode::Down | KeyCode::Char('j') => app.menu.next(),
        KeyCode::Enter => match app.menu.selected {
            0 => {
                app.drill_mode = DrillMode::Adaptive;
                app.drill_scope = DrillScope::Global;
                app.start_drill();
            }
            1 => {
                app.go_to_code_language_select();
            }
            2 => {
                app.go_to_passage_book_select();
            }
            3 => app.go_to_skill_tree(),
            4 => app.go_to_stats(),
            5 => app.go_to_settings(),
            _ => {}
        },
        _ => {}
    }
}

fn handle_drill_key(app: &mut App, key: KeyEvent) {
    // Route Enter/Tab as typed characters during active drills
    if app.drill.is_some() {
        match key.code {
            KeyCode::Enter => {
                app.type_char('\n');
                return;
            }
            KeyCode::Tab => {
                app.type_char('\t');
                return;
            }
            KeyCode::BackTab => return, // Ignore Shift+Tab
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => {
            let has_progress = app.drill.as_ref().is_some_and(|d| d.cursor > 0);
            if has_progress {
                app.finish_partial_drill();
            } else {
                app.go_to_menu();
            }
        }
        KeyCode::Backspace => app.backspace(),
        KeyCode::Char(ch) => app.type_char(ch),
        _ => {}
    }
}

fn handle_result_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('r') => app.retry_drill(),
        KeyCode::Char('q') | KeyCode::Esc => app.go_to_menu(),
        KeyCode::Char('s') => app.go_to_stats(),
        _ => {}
    }
}

fn handle_stats_key(app: &mut App, key: KeyEvent) {
    const STATS_TAB_COUNT: usize = 5;

    // Confirmation dialog takes priority
    if app.history_confirm_delete {
        match key.code {
            KeyCode::Char('y') => {
                app.delete_session();
                app.history_confirm_delete = false;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.history_confirm_delete = false;
            }
            _ => {}
        }
        return;
    }

    // History tab has row navigation
    if app.stats_tab == 1 {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
            KeyCode::Char('j') | KeyCode::Down => {
                if !app.drill_history.is_empty() {
                    let max_visible = app.drill_history.len().min(20) - 1;
                    app.history_selected = (app.history_selected + 1).min(max_visible);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.history_selected = app.history_selected.saturating_sub(1);
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if !app.drill_history.is_empty() {
                    app.history_confirm_delete = true;
                }
            }
            KeyCode::Char('1') => app.stats_tab = 0,
            KeyCode::Char('2') => {} // already on history
            KeyCode::Char('3') => app.stats_tab = 2,
            KeyCode::Char('4') => app.stats_tab = 3,
            KeyCode::Char('5') => app.stats_tab = 4,
            KeyCode::Tab => app.stats_tab = (app.stats_tab + 1) % STATS_TAB_COUNT,
            KeyCode::BackTab => {
                app.stats_tab = if app.stats_tab == 0 {
                    STATS_TAB_COUNT - 1
                } else {
                    app.stats_tab - 1
                }
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        KeyCode::Char('1') => app.stats_tab = 0,
        KeyCode::Char('2') => app.stats_tab = 1,
        KeyCode::Char('3') => app.stats_tab = 2,
        KeyCode::Char('4') => app.stats_tab = 3,
        KeyCode::Char('5') => app.stats_tab = 4,
        KeyCode::Tab => app.stats_tab = (app.stats_tab + 1) % STATS_TAB_COUNT,
        KeyCode::BackTab => {
            app.stats_tab = if app.stats_tab == 0 {
                STATS_TAB_COUNT - 1
            } else {
                app.stats_tab - 1
            }
        }
        _ => {}
    }
}

fn handle_settings_key(app: &mut App, key: KeyEvent) {
    if app.settings_editing_download_dir {
        match key.code {
            KeyCode::Esc => {
                app.settings_editing_download_dir = false;
            }
            KeyCode::Backspace => {
                app.config.passage_download_dir.pop();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.config.passage_download_dir.push(ch);
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            let _ = app.config.save();
            app.go_to_menu();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.settings_selected > 0 {
                app.settings_selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.settings_selected < 7 {
                app.settings_selected += 1;
            }
        }
        KeyCode::Enter => {
            if app.settings_selected == 5 {
                app.settings_editing_download_dir = true;
            } else if app.settings_selected == 7 {
                app.start_passage_downloads_from_settings();
            } else {
                app.settings_cycle_forward();
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.settings_selected < 5 {
                app.settings_cycle_forward();
            } else if app.settings_selected == 6 {
                app.settings_cycle_forward();
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.settings_selected < 5 || app.settings_selected == 6 {
                app.settings_cycle_backward();
            }
        }
        _ => {}
    }
}

fn handle_code_language_key(app: &mut App, key: KeyEvent) {
    const LANGS: &[&str] = &["rust", "python", "javascript", "go", "all"];

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        KeyCode::Up | KeyCode::Char('k') => {
            app.code_language_selected = app.code_language_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.code_language_selected + 1 < LANGS.len() {
                app.code_language_selected += 1;
            }
        }
        KeyCode::Char('1') => {
            app.code_language_selected = 0;
            start_code_drill(app, LANGS);
        }
        KeyCode::Char('2') => {
            app.code_language_selected = 1;
            start_code_drill(app, LANGS);
        }
        KeyCode::Char('3') => {
            app.code_language_selected = 2;
            start_code_drill(app, LANGS);
        }
        KeyCode::Char('4') => {
            app.code_language_selected = 3;
            start_code_drill(app, LANGS);
        }
        KeyCode::Char('5') => {
            app.code_language_selected = 4;
            start_code_drill(app, LANGS);
        }
        KeyCode::Enter => {
            start_code_drill(app, LANGS);
        }
        _ => {}
    }
}

fn start_code_drill(app: &mut App, langs: &[&str]) {
    if app.code_language_selected < langs.len() {
        app.config.code_language = langs[app.code_language_selected].to_string();
        let _ = app.config.save();
        app.drill_mode = DrillMode::Code;
        app.drill_scope = DrillScope::Global;
        app.start_drill();
    }
}

fn handle_passage_book_key(app: &mut App, key: KeyEvent) {
    let options = passage_options();
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        KeyCode::Up | KeyCode::Char('k') => {
            app.passage_book_selected = app.passage_book_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.passage_book_selected + 1 < options.len() {
                app.passage_book_selected += 1;
            }
        }
        KeyCode::Char(ch) if ch.is_ascii_digit() => {
            let idx = (ch as usize).saturating_sub('1' as usize);
            if idx < options.len() {
                app.passage_book_selected = idx;
                confirm_passage_book_and_continue(app, &options);
            }
        }
        KeyCode::Enter => {
            confirm_passage_book_and_continue(app, &options);
        }
        _ => {}
    }
}

fn confirm_passage_book_and_continue(app: &mut App, options: &[(&'static str, String)]) {
    if app.passage_book_selected >= options.len() {
        return;
    }
    app.config.passage_book = options[app.passage_book_selected].0.to_string();
    let _ = app.config.save();

    if app.config.passage_onboarding_done {
        app.start_passage_drill();
    } else {
        app.go_to_passage_intro();
    }
}

fn handle_passage_intro_key(app: &mut App, key: KeyEvent) {
    const INTRO_FIELDS: usize = 4;

    if app.passage_intro_downloading {
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        KeyCode::Up | KeyCode::Char('k') => {
            app.passage_intro_selected = app.passage_intro_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.passage_intro_selected + 1 < INTRO_FIELDS {
                app.passage_intro_selected += 1;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => match app.passage_intro_selected {
            0 => app.passage_intro_downloads_enabled = !app.passage_intro_downloads_enabled,
            2 => {
                app.passage_intro_paragraph_limit = match app.passage_intro_paragraph_limit {
                    0 => 500,
                    1 => 0,
                    n => n.saturating_sub(25).max(1),
                };
            }
            _ => {}
        },
        KeyCode::Right | KeyCode::Char('l') => match app.passage_intro_selected {
            0 => app.passage_intro_downloads_enabled = !app.passage_intro_downloads_enabled,
            2 => {
                app.passage_intro_paragraph_limit = match app.passage_intro_paragraph_limit {
                    0 => 1,
                    n if n >= 500 => 0,
                    n => n + 25,
                };
            }
            _ => {}
        },
        KeyCode::Backspace => match app.passage_intro_selected {
            1 => {
                app.passage_intro_download_dir.pop();
            }
            2 => {
                app.passage_intro_paragraph_limit /= 10;
            }
            _ => {}
        },
        KeyCode::Char(ch) => match app.passage_intro_selected {
            1 if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.passage_intro_download_dir.push(ch);
            }
            2 if ch.is_ascii_digit() => {
                let digit = (ch as u8 - b'0') as usize;
                app.passage_intro_paragraph_limit = app
                    .passage_intro_paragraph_limit
                    .saturating_mul(10)
                    .saturating_add(digit)
                    .min(50_000);
            }
            _ => {}
        },
        KeyCode::Enter => {
            if app.passage_intro_selected == 0 {
                app.passage_intro_downloads_enabled = !app.passage_intro_downloads_enabled;
                return;
            }
            if app.passage_intro_selected != 3 {
                return;
            }

            app.config.passage_downloads_enabled = app.passage_intro_downloads_enabled;
            app.config.passage_download_dir = app.passage_intro_download_dir.clone();
            app.config.passage_paragraphs_per_book = app.passage_intro_paragraph_limit;
            app.config.passage_onboarding_done = true;
            let _ = app.config.save();
            app.start_passage_drill();
        }
        _ => {}
    }
}

fn handle_passage_download_progress_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        _ => {}
    }
}

fn handle_skill_tree_key(app: &mut App, key: KeyEvent) {
    const DETAIL_SCROLL_STEP: usize = 10;
    let max_scroll = skill_tree_detail_max_scroll(app);
    app.skill_tree_detail_scroll = app.skill_tree_detail_scroll.min(max_scroll);
    let branches = selectable_branches();
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        KeyCode::Up | KeyCode::Char('k') => {
            app.skill_tree_selected = app.skill_tree_selected.saturating_sub(1);
            app.skill_tree_detail_scroll = 0;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.skill_tree_selected + 1 < branches.len() {
                app.skill_tree_selected += 1;
                app.skill_tree_detail_scroll = 0;
            }
        }
        KeyCode::PageUp => {
            app.skill_tree_detail_scroll = app
                .skill_tree_detail_scroll
                .saturating_sub(DETAIL_SCROLL_STEP);
        }
        KeyCode::PageDown => {
            let max_scroll = skill_tree_detail_max_scroll(app);
            app.skill_tree_detail_scroll = app
                .skill_tree_detail_scroll
                .saturating_add(DETAIL_SCROLL_STEP)
                .min(max_scroll);
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.skill_tree_detail_scroll = app
                .skill_tree_detail_scroll
                .saturating_sub(DETAIL_SCROLL_STEP);
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let max_scroll = skill_tree_detail_max_scroll(app);
            app.skill_tree_detail_scroll = app
                .skill_tree_detail_scroll
                .saturating_add(DETAIL_SCROLL_STEP)
                .min(max_scroll);
        }
        KeyCode::Enter => {
            if app.skill_tree_selected < branches.len() {
                let branch_id = branches[app.skill_tree_selected];
                let status = app.skill_tree.branch_status(branch_id).clone();
                if status == engine::skill_tree::BranchStatus::Available
                    || status == engine::skill_tree::BranchStatus::InProgress
                {
                    app.start_branch_drill(branch_id);
                }
            }
        }
        _ => {}
    }
}

fn skill_tree_detail_max_scroll(app: &App) -> usize {
    let (w, h) = crossterm::terminal::size().unwrap_or((120, 40));
    let screen = Rect::new(0, 0, w, h);
    let centered = ui::layout::centered_rect(70, 90, screen);
    let inner = Rect::new(
        centered.x.saturating_add(1),
        centered.y.saturating_add(1),
        centered.width.saturating_sub(2),
        centered.height.saturating_sub(2),
    );

    let branches = selectable_branches();
    if branches.is_empty() {
        return 0;
    }
    let branch_list_height = branches.len() as u16 * 2 + 1;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(branch_list_height.min(inner.height.saturating_sub(6))),
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(inner);
    let detail_height = layout.get(2).map(|r| r.height as usize).unwrap_or(0);
    let selected = app
        .skill_tree_selected
        .min(branches.len().saturating_sub(1));
    let total_lines = detail_line_count(branches[selected]);
    total_lines.saturating_sub(detail_height)
}

fn render(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let bg = Block::default().style(Style::default().bg(colors.bg()));
    frame.render_widget(bg, area);

    match app.screen {
        AppScreen::Menu => render_menu(frame, app),
        AppScreen::Drill => render_drill(frame, app),
        AppScreen::DrillResult => render_result(frame, app),
        AppScreen::StatsDashboard => render_stats(frame, app),
        AppScreen::Settings => render_settings(frame, app),
        AppScreen::SkillTree => render_skill_tree(frame, app),
        AppScreen::CodeLanguageSelect => render_code_language_select(frame, app),
        AppScreen::PassageBookSelect => render_passage_book_select(frame, app),
        AppScreen::PassageIntro => render_passage_intro(frame, app),
        AppScreen::PassageDownloadProgress => render_passage_download_progress(frame, app),
    }
}

fn render_menu(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let streak_text = if app.profile.streak_days > 0 {
        format!(" | {} day streak", app.profile.streak_days)
    } else {
        String::new()
    };
    let total_keys = app.skill_tree.total_unique_keys;
    let unlocked = app.skill_tree.total_unlocked_count();
    let mastered = app.skill_tree.total_confident_keys(&app.key_stats);
    let header_info = format!(
        " Key Progress {unlocked}/{total_keys} ({mastered} mastered){}",
        streak_text,
    );
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " keydr ",
            Style::default()
                .fg(colors.header_fg())
                .bg(colors.header_bg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            &*header_info,
            Style::default()
                .fg(colors.text_pending())
                .bg(colors.header_bg()),
        ),
    ]))
    .style(Style::default().bg(colors.header_bg()));
    frame.render_widget(header, layout[0]);

    let menu_area = ui::layout::centered_rect(50, 80, layout[1]);
    frame.render_widget(&app.menu, menu_area);

    let footer = Paragraph::new(Line::from(vec![Span::styled(
        " [1-3] Start  [t] Skill Tree  [s] Stats  [c] Settings  [q] Quit ",
        Style::default().fg(colors.text_pending()),
    )]));
    frame.render_widget(footer, layout[2]);
}

fn render_drill(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    if let Some(ref drill) = app.drill {
        let app_layout = AppLayout::new(area);
        let tier = app_layout.tier;

        let mode_name = match app.drill_mode {
            DrillMode::Adaptive => "Adaptive",
            DrillMode::Code => "Code (Unranked)",
            DrillMode::Passage => "Passage (Unranked)",
        };

        // For medium/narrow: show compact stats in header
        if !tier.show_sidebar() {
            let wpm = drill.wpm();
            let accuracy = drill.accuracy();
            let errors = drill.typo_count();
            let header_text =
                format!(" {mode_name} | WPM: {wpm:.0} | Acc: {accuracy:.1}% | Errors: {errors}");
            let header = Paragraph::new(Line::from(Span::styled(
                &*header_text,
                Style::default()
                    .fg(colors.header_fg())
                    .bg(colors.header_bg())
                    .add_modifier(Modifier::BOLD),
            )))
            .style(Style::default().bg(colors.header_bg()));
            frame.render_widget(header, app_layout.header);
        } else {
            let header_title = format!(" {mode_name} Drill ");
            let focus_text = if app.drill_mode == DrillMode::Adaptive {
                let focused = app.skill_tree.focused_key(app.drill_scope, &app.key_stats);
                if let Some(focused) = focused {
                    format!(" | Focus: '{focused}'")
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            let header = Paragraph::new(Line::from(vec![
                Span::styled(
                    &*header_title,
                    Style::default()
                        .fg(colors.header_fg())
                        .bg(colors.header_bg())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    &*focus_text,
                    Style::default()
                        .fg(colors.focused_key())
                        .bg(colors.header_bg()),
                ),
            ]))
            .style(Style::default().bg(colors.header_bg()));
            frame.render_widget(header, app_layout.header);
        }

        // Build main area constraints based on tier
        let show_kbd = tier.show_keyboard(area.height);
        let show_progress = tier.show_progress_bar(area.height);

        // Compute active branch count for progress area height
        let active_branches: Vec<engine::skill_tree::BranchId> =
            engine::skill_tree::BranchId::all()
                .iter()
                .copied()
                .filter(|&id| {
                    matches!(
                        app.skill_tree.branch_status(id),
                        engine::skill_tree::BranchStatus::InProgress
                            | engine::skill_tree::BranchStatus::Complete
                    )
                })
                .collect();

        let progress_height = if show_progress && area.height >= 25 {
            (active_branches.len().min(6) as u16 + 1).max(2) // +1 for overall line
        } else if show_progress && area.height >= 20 {
            2 // active branch + overall
        } else if show_progress {
            1 // active branch only
        } else {
            0
        };

        let kbd_height = if show_kbd {
            if tier.compact_keyboard() {
                5 // 3 rows + 2 border
            } else {
                7 // 4 rows + 2 border + 1 label space
            }
        } else {
            0
        };

        let mut constraints: Vec<Constraint> = vec![Constraint::Min(5)];
        if progress_height > 0 {
            constraints.push(Constraint::Length(progress_height));
        }
        if show_kbd {
            constraints.push(Constraint::Length(kbd_height));
        }

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(app_layout.main);

        let typing = TypingArea::new(drill, app.theme);
        frame.render_widget(typing, main_layout[0]);

        let mut idx = 1;
        if progress_height > 0 {
            if app.drill_mode == DrillMode::Adaptive {
                let progress_widget = ui::components::branch_progress_list::BranchProgressList {
                    skill_tree: &app.skill_tree,
                    key_stats: &app.key_stats,
                    drill_scope: app.drill_scope,
                    active_branches: &active_branches,
                    theme: app.theme,
                    height: progress_height,
                };
                frame.render_widget(progress_widget, main_layout[idx]);
            } else {
                let source = app.drill_source_info.as_deref().unwrap_or("unknown source");
                let label = if app.drill_mode == DrillMode::Code {
                    " Code source "
                } else {
                    " Passage source "
                };
                let source_info = Paragraph::new(Line::from(vec![
                    Span::styled(
                        label,
                        Style::default()
                            .fg(colors.accent())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(source, Style::default().fg(colors.text_pending())),
                ]));
                frame.render_widget(source_info, main_layout[idx]);
            }
            idx += 1;
        }

        if show_kbd {
            let next_char = drill.target.get(drill.cursor).copied();
            let unlocked_keys = app.skill_tree.unlocked_keys(app.drill_scope);
            let focused = app.skill_tree.focused_key(app.drill_scope, &app.key_stats);
            let kbd_height = if tier.compact_keyboard() { 5 } else { 7 };
            let _ = kbd_height; // Height managed by constraints
            let kbd = KeyboardDiagram::new(
                focused,
                next_char,
                &unlocked_keys,
                &app.depressed_keys,
                app.theme,
                &app.keyboard_model,
            )
            .compact(tier.compact_keyboard())
            .shift_held(app.shift_held);
            frame.render_widget(kbd, main_layout[idx]);
        }

        if let Some(sidebar_area) = app_layout.sidebar {
            let sidebar = StatsSidebar::new(
                drill,
                app.last_result.as_ref(),
                &app.drill_history,
                app.theme,
            );
            frame.render_widget(sidebar, sidebar_area);
        }

        let footer = Paragraph::new(Line::from(Span::styled(
            " [ESC] End drill  [Backspace] Delete ",
            Style::default().fg(colors.text_pending()),
        )));
        frame.render_widget(footer, app_layout.footer);
    }
}

fn render_result(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();

    if let Some(ref result) = app.last_result {
        let centered = ui::layout::centered_rect(60, 70, area);
        let dashboard = Dashboard::new(result, app.theme);
        frame.render_widget(dashboard, centered);
    }
}

fn render_stats(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let dashboard = StatsDashboard::new(
        &app.drill_history,
        &app.key_stats,
        app.stats_tab,
        app.config.target_wpm,
        app.skill_tree.total_unlocked_count(),
        app.skill_tree.total_confident_keys(&app.key_stats),
        app.skill_tree.total_unique_keys,
        app.theme,
        app.history_selected,
        app.history_confirm_delete,
        &app.keyboard_model,
    );
    frame.render_widget(dashboard, area);
}

fn render_settings(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let centered = ui::layout::centered_rect(60, 80, area);

    let block = Block::bordered()
        .title(" Settings ")
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let available_themes = ui::theme::Theme::available_themes();
    let languages_all = ["rust", "python", "javascript", "go", "all"];
    let current_lang = &app.config.code_language;

    let fields: Vec<(String, String)> = vec![
        (
            "Target WPM".to_string(),
            format!("{}", app.config.target_wpm),
        ),
        ("Theme".to_string(), app.config.theme.clone()),
        (
            "Word Count".to_string(),
            format!("{}", app.config.word_count),
        ),
        ("Code Language".to_string(), current_lang.clone()),
        (
            "Passage Downloads".to_string(),
            if app.config.passage_downloads_enabled {
                "On".to_string()
            } else {
                "Off".to_string()
            },
        ),
        (
            "Passage Download Dir".to_string(),
            app.config.passage_download_dir.clone(),
        ),
        (
            "Paragraphs per Book".to_string(),
            if app.config.passage_paragraphs_per_book == 0 {
                "Whole book".to_string()
            } else {
                format!("{}", app.config.passage_paragraphs_per_book)
            },
        ),
        (
            "Download Passages Now".to_string(),
            "Run downloader".to_string(),
        ),
    ];

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(fields.len() as u16 * 3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(inner);

    let header = Paragraph::new(Line::from(Span::styled(
        "  Use arrows to navigate, Enter/Right to change, ESC to save & exit",
        Style::default().fg(colors.text_pending()),
    )));
    header.render(layout[0], frame.buffer_mut());

    let field_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            fields
                .iter()
                .map(|_| Constraint::Length(3))
                .collect::<Vec<_>>(),
        )
        .split(layout[1]);

    for (i, (label, value)) in fields.iter().enumerate() {
        let is_selected = i == app.settings_selected;
        let indicator = if is_selected { " > " } else { "   " };

        let label_text = format!("{indicator}{label}:");
        let value_text = if i == 7 {
            format!("  [ {value} ]")
        } else {
            format!("  < {value} >")
        };

        let label_style = Style::default()
            .fg(if is_selected {
                colors.accent()
            } else {
                colors.fg()
            })
            .add_modifier(if is_selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });

        let value_style = Style::default().fg(if is_selected {
            colors.focused_key()
        } else {
            colors.text_pending()
        });

        let lines = if i == 5 {
            let path_line = if app.settings_editing_download_dir && is_selected {
                format!("  {value}_")
            } else {
                format!("  {value}")
            };
            vec![
                Line::from(Span::styled(
                    if app.settings_editing_download_dir && is_selected {
                        format!("{indicator}{label}: (editing)")
                    } else {
                        label_text
                    },
                    label_style,
                )),
                Line::from(Span::styled(path_line, value_style)),
            ]
        } else {
            vec![
                Line::from(Span::styled(label_text, label_style)),
                Line::from(Span::styled(value_text, value_style)),
            ]
        };
        Paragraph::new(lines).render(field_layout[i], frame.buffer_mut());
    }

    let _ = (available_themes, languages_all);

    let footer = Paragraph::new(Line::from(Span::styled(
        if app.settings_editing_download_dir {
            "  Editing path: [Type/Backspace] Modify  [ESC] Done editing"
        } else {
            "  [ESC] Save & back  [Enter/arrows] Change value  [Enter on path] Edit dir"
        },
        Style::default().fg(colors.accent()),
    )));
    footer.render(layout[3], frame.buffer_mut());
}

fn render_code_language_select(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(40, 50, area);

    let block = Block::bordered()
        .title(" Select Code Language ")
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let langs = ["Rust", "Python", "JavaScript", "Go", "All (random)"];
    let lang_keys = ["rust", "python", "javascript", "go", "all"];

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));

    for (i, &lang) in langs.iter().enumerate() {
        let is_selected = i == app.code_language_selected;
        let is_current = lang_keys[i] == app.config.code_language;

        let indicator = if is_selected { " > " } else { "   " };
        let current_marker = if is_current { " (current)" } else { "" };

        let style = if is_selected {
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.fg())
        };

        lines.push(Line::from(Span::styled(
            format!("{indicator}[{}] {lang}{current_marker}", i + 1),
            style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  [1-5] Select  [Enter] Confirm  [ESC] Back",
        Style::default().fg(colors.text_pending()),
    )));

    Paragraph::new(lines).render(inner, frame.buffer_mut());
}

fn render_passage_book_select(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(60, 70, area);

    let block = Block::bordered()
        .title(" Select Passage Source ")
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let options = passage_options();
    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, (_, label)) in options.iter().enumerate() {
        let is_selected = i == app.passage_book_selected;
        let indicator = if is_selected { " > " } else { "   " };
        let style = if is_selected {
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.fg())
        };
        lines.push(Line::from(Span::styled(
            format!("{indicator}[{}] {label}", i + 1),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  [Up/Down] Navigate  [Enter] Confirm  [ESC] Back",
        Style::default().fg(colors.text_pending()),
    )));
    Paragraph::new(lines).render(inner, frame.buffer_mut());
}

fn render_passage_intro(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(75, 80, area);

    let block = Block::bordered()
        .title(" Passage Downloads Setup ")
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let paragraphs_value = if app.passage_intro_paragraph_limit == 0 {
        "whole book".to_string()
    } else {
        app.passage_intro_paragraph_limit.to_string()
    };

    let fields = vec![
        (
            "Enable network downloads",
            if app.passage_intro_downloads_enabled {
                "On".to_string()
            } else {
                "Off".to_string()
            },
        ),
        ("Download directory", app.passage_intro_download_dir.clone()),
        ("Paragraphs per book (0 = whole)", paragraphs_value),
        ("Start passage drill", "Confirm".to_string()),
    ];

    let mut lines = vec![
        Line::from(Span::styled(
            "Configure passage source settings before your first passage drill.",
            Style::default()
                .fg(colors.fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Downloads are lazy: books are fetched only when first needed.",
            Style::default().fg(colors.text_pending()),
        )),
        Line::from(Span::styled(
            "If you exit without confirming, this dialog will appear again next time.",
            Style::default().fg(colors.text_pending()),
        )),
        Line::from(""),
    ];

    for (i, (label, value)) in fields.iter().enumerate() {
        let is_selected = i == app.passage_intro_selected;
        let indicator = if is_selected { " > " } else { "   " };
        let label_style = if is_selected {
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.fg())
        };
        let value_style = if is_selected {
            Style::default().fg(colors.focused_key())
        } else {
            Style::default().fg(colors.text_pending())
        };

        lines.push(Line::from(Span::styled(
            format!("{indicator}{label}"),
            label_style,
        )));
        if i == 1 {
            lines.push(Line::from(Span::styled(format!("   {value}"), value_style)));
        } else if i == 3 {
            lines.push(Line::from(Span::styled(
                format!("   [{value}]"),
                value_style,
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!("   < {value} >"),
                value_style,
            )));
        }
        lines.push(Line::from(""));
    }

    if app.passage_intro_downloading {
        let total_books = app.passage_intro_download_total.max(1);
        let done_books = app.passage_intro_downloaded.min(total_books);
        let total_bytes = app.passage_intro_download_bytes_total;
        let done_bytes = app
            .passage_intro_download_bytes
            .min(total_bytes.max(app.passage_intro_download_bytes));
        let width = 30usize;
        let fill = if total_bytes > 0 {
            ((done_bytes as usize).saturating_mul(width)) / (total_bytes as usize)
        } else {
            0
        };
        let bar = format!(
            "{}{}",
            "=".repeat(fill),
            " ".repeat(width.saturating_sub(fill))
        );
        let progress_text = if total_bytes > 0 {
            format!(" Downloading current book: [{bar}] {done_bytes}/{total_bytes} bytes")
        } else {
            format!(" Downloading current book: {done_bytes} bytes")
        };
        lines.push(Line::from(Span::styled(
            progress_text,
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        )));
        if !app.passage_intro_current_book.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(
                    " Current: {}  (book {}/{})",
                    app.passage_intro_current_book,
                    done_books.saturating_add(1).min(total_books),
                    total_books
                ),
                Style::default().fg(colors.text_pending()),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  [Up/Down] Navigate  [Left/Right] Adjust  [Type/Backspace] Edit  [Enter] Confirm",
            Style::default().fg(colors.text_pending()),
        )));
        lines.push(Line::from(Span::styled(
            "  [ESC] Cancel",
            Style::default().fg(colors.text_pending()),
        )));
    }

    Paragraph::new(lines).render(inner, frame.buffer_mut());
}

fn render_passage_download_progress(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(60, 35, area);

    let block = Block::bordered()
        .title(" Downloading Passage Source ")
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let total_bytes = app.passage_intro_download_bytes_total;
    let done_bytes = app
        .passage_intro_download_bytes
        .min(total_bytes.max(app.passage_intro_download_bytes));
    let width = 36usize;
    let fill = if total_bytes > 0 {
        ((done_bytes as usize).saturating_mul(width)) / (total_bytes as usize)
    } else {
        0
    };
    let bar = format!(
        "{}{}",
        "=".repeat(fill),
        " ".repeat(width.saturating_sub(fill))
    );

    let book_name = if app.passage_intro_current_book.is_empty() {
        "Preparing download...".to_string()
    } else {
        app.passage_intro_current_book.clone()
    };

    let lines = vec![
        Line::from(Span::styled(
            format!(" Book: {book_name}"),
            Style::default()
                .fg(colors.fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            if total_bytes > 0 {
                format!(" [{bar}] {done_bytes}/{total_bytes} bytes")
            } else {
                format!(" Downloaded: {done_bytes} bytes")
            },
            Style::default().fg(colors.accent()),
        )),
    ];

    Paragraph::new(lines).render(inner, frame.buffer_mut());
}

fn render_skill_tree(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let centered = ui::layout::centered_rect(70, 90, area);
    let widget = SkillTreeWidget::new(
        &app.skill_tree,
        &app.key_stats,
        app.skill_tree_selected,
        app.skill_tree_detail_scroll,
        app.theme,
    );
    frame.render_widget(widget, centered);
}
