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
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};

use app::{App, AppScreen, DrillMode};
use engine::skill_tree::DrillScope;
use event::{AppEvent, EventHandler};
use generator::code_syntax::{code_language_options, is_language_cached, language_by_key};
use generator::passage::{is_book_cached, passage_options};
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
                if (app.screen == AppScreen::CodeIntro
                    || app.screen == AppScreen::CodeDownloadProgress)
                    && app.code_intro_downloading
                {
                    app.process_code_download_tick();
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
        AppScreen::CodeIntro => handle_code_intro_key(app, key),
        AppScreen::CodeDownloadProgress => handle_code_download_progress_key(app, key),
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
            if app.config.code_onboarding_done {
                app.go_to_code_language_select();
            } else {
                app.go_to_code_intro();
            }
        }
        KeyCode::Char('3') => {
            if app.config.passage_onboarding_done {
                app.go_to_passage_book_select();
            } else {
                app.go_to_passage_intro();
            }
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
                if app.config.code_onboarding_done {
                    app.go_to_code_language_select();
                } else {
                    app.go_to_code_intro();
                }
            }
            2 => {
                if app.config.passage_onboarding_done {
                    app.go_to_passage_book_select();
                } else {
                    app.go_to_passage_intro();
                }
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
    const MAX_SETTINGS: usize = 11;

    if app.settings_editing_download_dir {
        match key.code {
            KeyCode::Esc => {
                app.settings_editing_download_dir = false;
            }
            KeyCode::Backspace => {
                if app.settings_selected == 5 {
                    app.config.code_download_dir.pop();
                } else if app.settings_selected == 9 {
                    app.config.passage_download_dir.pop();
                }
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if app.settings_selected == 5 {
                    app.config.code_download_dir.push(ch);
                } else if app.settings_selected == 9 {
                    app.config.passage_download_dir.push(ch);
                }
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
            if app.settings_selected < MAX_SETTINGS {
                app.settings_selected += 1;
            }
        }
        KeyCode::Enter => {
            match app.settings_selected {
                5 | 9 => app.settings_editing_download_dir = true,
                7 => app.start_code_downloads_from_settings(),
                11 => app.start_passage_downloads_from_settings(),
                _ => app.settings_cycle_forward(),
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            // Allow cycling for non-text, non-button fields
            match app.settings_selected {
                5 | 7 | 9 | 11 => {} // text fields or action buttons
                _ => app.settings_cycle_forward(),
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            match app.settings_selected {
                5 | 7 | 9 | 11 => {} // text fields or action buttons
                _ => app.settings_cycle_backward(),
            }
        }
        _ => {}
    }
}

fn handle_code_language_key(app: &mut App, key: KeyEvent) {
    let options = code_language_options();
    let len = options.len();
    if len == 0 {
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        KeyCode::Up | KeyCode::Char('k') => {
            app.code_language_selected = app.code_language_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.code_language_selected + 1 < len {
                app.code_language_selected += 1;
            }
        }
        KeyCode::PageUp => {
            app.code_language_selected = app.code_language_selected.saturating_sub(10);
        }
        KeyCode::PageDown => {
            app.code_language_selected = (app.code_language_selected + 10).min(len - 1);
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.code_language_selected = 0;
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.code_language_selected = len - 1;
        }
        KeyCode::Enter => {
            if app.code_language_selected >= options.len() {
                return;
            }
            let key = options[app.code_language_selected].0;
            if !is_code_language_disabled(app, key) {
                confirm_code_language_and_continue(app, &options);
            }
        }
        _ => {}
    }

    // Adjust scroll to keep selected item visible.
    // Use a rough viewport estimate; render will use exact terminal size.
    let viewport = 15usize;
    if app.code_language_selected < app.code_language_scroll {
        app.code_language_scroll = app.code_language_selected;
    } else if app.code_language_selected >= app.code_language_scroll + viewport {
        app.code_language_scroll = app.code_language_selected + 1 - viewport;
    }
}

fn code_language_requires_download(app: &App, key: &str) -> bool {
    if key == "all" {
        return false;
    }
    let Some(lang) = language_by_key(key) else {
        return false;
    };
    !lang.has_builtin && !is_language_cached(&app.config.code_download_dir, key)
}

fn is_code_language_disabled(app: &App, key: &str) -> bool {
    !app.config.code_downloads_enabled && code_language_requires_download(app, key)
}

fn confirm_code_language_and_continue(app: &mut App, options: &[(&str, String)]) {
    if app.code_language_selected >= options.len() {
        return;
    }
    app.config.code_language = options[app.code_language_selected].0.to_string();
    let _ = app.config.save();
    if app.config.code_onboarding_done {
        app.start_code_drill();
    } else {
        app.go_to_code_intro();
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
                let key = options[idx].0;
                if !is_passage_option_disabled(app, key) {
                    confirm_passage_book_and_continue(app, &options);
                }
            }
        }
        KeyCode::Enter => {
            if app.passage_book_selected < options.len() {
                let key = options[app.passage_book_selected].0;
                if !is_passage_option_disabled(app, key) {
                    confirm_passage_book_and_continue(app, &options);
                }
            }
        }
        _ => {}
    }
}

fn passage_option_requires_download(app: &App, key: &str) -> bool {
    key != "all" && key != "builtin" && !is_book_cached(&app.config.passage_download_dir, key)
}

fn is_passage_option_disabled(app: &App, key: &str) -> bool {
    !app.config.passage_downloads_enabled && passage_option_requires_download(app, key)
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
            app.go_to_passage_book_select();
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

fn handle_code_intro_key(app: &mut App, key: KeyEvent) {
    const INTRO_FIELDS: usize = 4;

    if app.code_intro_downloading {
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        KeyCode::Up | KeyCode::Char('k') => {
            app.code_intro_selected = app.code_intro_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.code_intro_selected + 1 < INTRO_FIELDS {
                app.code_intro_selected += 1;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => match app.code_intro_selected {
            0 => app.code_intro_downloads_enabled = !app.code_intro_downloads_enabled,
            2 => {
                app.code_intro_snippets_per_repo = match app.code_intro_snippets_per_repo {
                    0 => 200,
                    1 => 0,
                    n => n.saturating_sub(10).max(1),
                };
            }
            _ => {}
        },
        KeyCode::Right | KeyCode::Char('l') => match app.code_intro_selected {
            0 => app.code_intro_downloads_enabled = !app.code_intro_downloads_enabled,
            2 => {
                app.code_intro_snippets_per_repo = match app.code_intro_snippets_per_repo {
                    0 => 1,
                    n if n >= 200 => 0,
                    n => n + 10,
                };
            }
            _ => {}
        },
        KeyCode::Backspace => match app.code_intro_selected {
            1 => {
                app.code_intro_download_dir.pop();
            }
            2 => {
                app.code_intro_snippets_per_repo /= 10;
            }
            _ => {}
        },
        KeyCode::Char(ch) => match app.code_intro_selected {
            1 if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.code_intro_download_dir.push(ch);
            }
            2 if ch.is_ascii_digit() => {
                let digit = (ch as u8 - b'0') as usize;
                app.code_intro_snippets_per_repo = app
                    .code_intro_snippets_per_repo
                    .saturating_mul(10)
                    .saturating_add(digit)
                    .min(10_000);
            }
            _ => {}
        },
        KeyCode::Enter => {
            if app.code_intro_selected == 0 {
                app.code_intro_downloads_enabled = !app.code_intro_downloads_enabled;
                return;
            }
            if app.code_intro_selected != 3 {
                return;
            }

            app.config.code_downloads_enabled = app.code_intro_downloads_enabled;
            app.config.code_download_dir = app.code_intro_download_dir.clone();
            app.config.code_snippets_per_repo = app.code_intro_snippets_per_repo;
            app.config.code_onboarding_done = true;
            let _ = app.config.save();
            app.go_to_code_language_select();
        }
        _ => {}
    }
}

fn handle_code_download_progress_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.cancel_code_download();
            app.go_to_menu();
        }
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
    let centered = skill_tree_popup_rect(screen);
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

fn skill_tree_popup_rect(area: Rect) -> Rect {
    let percent_x = if area.width < 120 { 95 } else { 85 };
    let percent_y = if area.height < 40 { 95 } else { 90 };
    ui::layout::centered_rect(percent_x, percent_y, area)
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
        AppScreen::CodeIntro => render_code_intro(frame, app),
        AppScreen::CodeDownloadProgress => render_code_download_progress(frame, app),
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

    let fields: Vec<(String, String, bool)> = vec![
        (
            "Target WPM".to_string(),
            format!("{}", app.config.target_wpm),
            false,
        ),
        ("Theme".to_string(), app.config.theme.clone(), false),
        (
            "Word Count".to_string(),
            format!("{}", app.config.word_count),
            false,
        ),
        (
            "Code Language".to_string(),
            app.config.code_language.clone(),
            false,
        ),
        (
            "Code Downloads".to_string(),
            if app.config.code_downloads_enabled {
                "On".to_string()
            } else {
                "Off".to_string()
            },
            false,
        ),
        (
            "Code Download Dir".to_string(),
            app.config.code_download_dir.clone(),
            true, // path field
        ),
        (
            "Snippets per Repo".to_string(),
            if app.config.code_snippets_per_repo == 0 {
                "Unlimited".to_string()
            } else {
                format!("{}", app.config.code_snippets_per_repo)
            },
            false,
        ),
        (
            "Download Code Now".to_string(),
            "Run downloader".to_string(),
            false,
        ),
        (
            "Passage Downloads".to_string(),
            if app.config.passage_downloads_enabled {
                "On".to_string()
            } else {
                "Off".to_string()
            },
            false,
        ),
        (
            "Passage Download Dir".to_string(),
            app.config.passage_download_dir.clone(),
            true, // path field
        ),
        (
            "Paragraphs per Book".to_string(),
            if app.config.passage_paragraphs_per_book == 0 {
                "Whole book".to_string()
            } else {
                format!("{}", app.config.passage_paragraphs_per_book)
            },
            false,
        ),
        (
            "Download Passages Now".to_string(),
            "Run downloader".to_string(),
            false,
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

    for (i, (label, value, is_path)) in fields.iter().enumerate() {
        let is_selected = i == app.settings_selected;
        let indicator = if is_selected { " > " } else { "   " };

        let label_text = format!("{indicator}{label}:");
        let is_button = i == 7 || i == 11; // Download Code Now, Download Passages Now
        let value_text = if is_button {
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

        let lines = if *is_path {
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

    let footer_hints: Vec<&str> = if app.settings_editing_download_dir {
        vec!["Editing path:", "[Type/Backspace] Modify", "[ESC] Done editing"]
    } else {
        vec![
            "[ESC] Save & back",
            "[Enter/arrows] Change value",
            "[Enter on path] Edit dir",
        ]
    };
    let footer_lines: Vec<Line> = pack_hint_lines(&footer_hints, layout[3].width as usize)
        .into_iter()
        .map(|line| Line::from(Span::styled(line, Style::default().fg(colors.accent()))))
        .collect();
    Paragraph::new(footer_lines)
        .wrap(Wrap { trim: false })
        .render(layout[3], frame.buffer_mut());
}

fn wrapped_line_count(text: &str, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let chars = text.chars().count().max(1);
    chars.div_ceil(width)
}

fn pack_hint_lines(hints: &[&str], width: usize) -> Vec<String> {
    if width == 0 || hints.is_empty() {
        return Vec::new();
    }

    let prefix = "  ";
    let separator = "  ";
    let mut out: Vec<String> = Vec::new();
    let mut current = prefix.to_string();
    let mut has_hint = false;

    for hint in hints {
        if hint.is_empty() {
            continue;
        }
        let candidate = if has_hint {
            format!("{current}{separator}{hint}")
        } else {
            format!("{current}{hint}")
        };
        if candidate.chars().count() <= width {
            current = candidate;
            has_hint = true;
        } else {
            if has_hint {
                out.push(current);
            }
            current = format!("{prefix}{hint}");
            has_hint = true;
        }
    }

    if has_hint {
        out.push(current);
    }
    out
}

fn render_code_language_select(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(50, 70, area);

    let block = Block::bordered()
        .title(" Select Code Language ")
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let options = code_language_options();
    let cache_dir = &app.config.code_download_dir;
    let footer_hints = [
        "[Up/Down/PgUp/PgDn] Navigate",
        "[Enter] Confirm",
        "[ESC] Back",
    ];
    let disabled_notice =
        "  Some languages are disabled: enable network downloads in intro/settings.";
    let has_disabled = !app.config.code_downloads_enabled
        && options
            .iter()
            .any(|(key, _)| is_code_language_disabled(app, key));
    let width = inner.width as usize;
    let hint_lines_vec = pack_hint_lines(&footer_hints, width);
    let hint_lines = hint_lines_vec.len();
    let notice_lines = wrapped_line_count(disabled_notice, width);
    let total_height = inner.height as usize;
    let show_notice = has_disabled && total_height >= hint_lines + notice_lines + 3;
    let desired_footer_height = hint_lines + if show_notice { notice_lines } else { 0 };
    let footer_height = desired_footer_height.min(total_height.saturating_sub(1)) as u16;
    let (list_area, footer_area) = if footer_height > 0 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner);
        (chunks[0], Some(chunks[1]))
    } else {
        (inner, None)
    };

    let viewport_height = (list_area.height as usize).saturating_sub(2).max(1);
    let scroll = app.code_language_scroll;

    let mut lines: Vec<Line> = Vec::new();

    // Show scroll indicator at top if scrolled down
    if scroll > 0 {
        lines.push(Line::from(Span::styled(
            format!("   ... {} more above ...", scroll),
            Style::default().fg(colors.text_pending()),
        )));
    } else {
        lines.push(Line::from(""));
    }

    let visible_end = (scroll + viewport_height).min(options.len());

    for i in scroll..visible_end {
        let (key, display) = &options[i];
        let is_selected = i == app.code_language_selected;
        let is_current = *key == app.config.code_language;
        let is_disabled = is_code_language_disabled(app, key);

        let indicator = if is_selected { " > " } else { "   " };
        let current_marker = if is_current { " (current)" } else { "" };

        // Determine availability label
        let availability = if *key == "all" {
            String::new()
        } else if let Some(lang) = language_by_key(key) {
            if lang.has_builtin {
                " (built-in)".to_string()
            } else if is_language_cached(cache_dir, key) {
                " (cached)".to_string()
            } else if is_disabled {
                " (disabled: download required)".to_string()
            } else {
                " (download required)".to_string()
            }
        } else {
            String::new()
        };

        let name_style = if is_disabled {
            Style::default().fg(colors.text_pending())
        } else if is_selected {
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.fg())
        };
        let status_style = Style::default()
            .fg(colors.text_pending())
            .add_modifier(Modifier::DIM);

        let mut spans = vec![Span::styled(
            format!("{indicator}{display}{current_marker}"),
            name_style,
        )];
        if !availability.is_empty() {
            spans.push(Span::styled(availability, status_style));
        }
        lines.push(Line::from(spans));
    }

    // Show scroll indicator at bottom if more items below
    if visible_end < options.len() {
        lines.push(Line::from(Span::styled(
            format!("   ... {} more below ...", options.len() - visible_end),
            Style::default().fg(colors.text_pending()),
        )));
    } else {
        lines.push(Line::from(""));
    }

    Paragraph::new(lines).render(list_area, frame.buffer_mut());

    if let Some(footer) = footer_area {
        let mut footer_lines: Vec<Line> = hint_lines_vec
            .iter()
            .map(|line| Line::from(Span::styled(line.clone(), Style::default().fg(colors.text_pending()))))
            .collect();
        if show_notice {
            footer_lines.push(Line::from(Span::styled(
                disabled_notice,
                Style::default().fg(colors.text_pending()),
            )));
        }
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer, frame.buffer_mut());
    }
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
    let footer_hints = ["[Up/Down] Navigate", "[Enter] Confirm", "[ESC] Back"];
    let disabled_notice =
        "  Some sources are disabled: enable network downloads in intro/settings.";
    let has_disabled = !app.config.passage_downloads_enabled
        && options
            .iter()
            .any(|(key, _)| is_passage_option_disabled(app, key));
    let width = inner.width as usize;
    let hint_lines_vec = pack_hint_lines(&footer_hints, width);
    let hint_lines = hint_lines_vec.len();
    let notice_lines = wrapped_line_count(disabled_notice, width);
    let total_height = inner.height as usize;
    let show_notice = has_disabled && total_height >= hint_lines + notice_lines + 3;
    let desired_footer_height = hint_lines + if show_notice { notice_lines } else { 0 };
    let footer_height = desired_footer_height.min(total_height.saturating_sub(1)) as u16;
    let (list_area, footer_area) = if footer_height > 0 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner);
        (chunks[0], Some(chunks[1]))
    } else {
        (inner, None)
    };

    let viewport_height = list_area.height as usize;
    let start = app
        .passage_book_selected
        .saturating_sub(viewport_height.saturating_sub(1));
    let end = (start + viewport_height).min(options.len());
    let mut lines: Vec<Line> = vec![];
    for (i, (key, label)) in options.iter().enumerate().skip(start).take(end - start) {
        let is_selected = i == app.passage_book_selected;
        let is_disabled = is_passage_option_disabled(app, key);
        let indicator = if is_selected { " > " } else { "   " };
        let availability = if *key == "all" {
            String::new()
        } else if *key == "builtin" {
            " (built-in)".to_string()
        } else if is_book_cached(&app.config.passage_download_dir, key) {
            " (cached)".to_string()
        } else if is_disabled {
            " (disabled: download required)".to_string()
        } else {
            " (download required)".to_string()
        };
        let name_style = if is_disabled {
            Style::default().fg(colors.text_pending())
        } else if is_selected {
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.fg())
        };
        let status_style = Style::default()
            .fg(colors.text_pending())
            .add_modifier(Modifier::DIM);
        let mut spans = vec![Span::styled(
            format!("{indicator}[{}] {label}", i + 1),
            name_style,
        )];
        if !availability.is_empty() {
            spans.push(Span::styled(availability, status_style));
        }
        lines.push(Line::from(spans));
    }

    Paragraph::new(lines).render(list_area, frame.buffer_mut());

    if let Some(footer) = footer_area {
        let mut footer_lines: Vec<Line> = hint_lines_vec
            .iter()
            .map(|line| Line::from(Span::styled(line.clone(), Style::default().fg(colors.text_pending()))))
            .collect();
        if show_notice {
            footer_lines.push(Line::from(Span::styled(
                disabled_notice,
                Style::default().fg(colors.text_pending()),
            )));
        }
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer, frame.buffer_mut());
    }
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
    }
    let hint_lines = if app.passage_intro_downloading {
        Vec::new()
    } else {
        pack_hint_lines(
            &[
                "[Up/Down] Navigate",
                "[Left/Right] Adjust",
                "[Type/Backspace] Edit",
                "[Enter] Confirm",
                "[ESC] Cancel",
            ],
            inner.width as usize,
        )
    };
    let footer_height = if hint_lines.is_empty() {
        0
    } else {
        (hint_lines.len() + 1) as u16 // add spacer line above hints
    };
    let (content_area, footer_area) = if footer_height > 0 && footer_height < inner.height {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner);
        (chunks[0], Some(chunks[1]))
    } else {
        (inner, None)
    };
    Paragraph::new(lines).render(content_area, frame.buffer_mut());
    if let Some(footer) = footer_area {
        let mut footer_lines = vec![Line::from("")];
        footer_lines.extend(
            hint_lines
                .into_iter()
                .map(|hint| Line::from(Span::styled(hint, Style::default().fg(colors.text_pending())))),
        );
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer, frame.buffer_mut());
    }
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

fn render_code_intro(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(75, 80, area);

    let block = Block::bordered()
        .title(" Code Downloads Setup ")
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let snippets_value = if app.code_intro_snippets_per_repo == 0 {
        "unlimited".to_string()
    } else {
        app.code_intro_snippets_per_repo.to_string()
    };

    let fields = vec![
        (
            "Enable network downloads",
            if app.code_intro_downloads_enabled {
                "On".to_string()
            } else {
                "Off".to_string()
            },
        ),
        ("Download directory", app.code_intro_download_dir.clone()),
        ("Snippets per repo (0 = unlimited)", snippets_value),
        ("Start code drill", "Confirm".to_string()),
    ];

    let mut lines = vec![
        Line::from(Span::styled(
            "Configure code source settings before your first code drill.",
            Style::default()
                .fg(colors.fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Downloads are lazy: code is fetched only when first needed.",
            Style::default().fg(colors.text_pending()),
        )),
        Line::from(Span::styled(
            "If you exit without confirming, this dialog will appear again next time.",
            Style::default().fg(colors.text_pending()),
        )),
        Line::from(""),
    ];

    for (i, (label, value)) in fields.iter().enumerate() {
        let is_selected = i == app.code_intro_selected;
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

    if app.code_intro_downloading {
        let total_repos = app.code_intro_download_total.max(1);
        let done_repos = app.code_intro_downloaded.min(total_repos);
        let total_bytes = app.code_intro_download_bytes_total;
        let done_bytes = app
            .code_intro_download_bytes
            .min(total_bytes.max(app.code_intro_download_bytes));
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
            format!(" Downloading: [{bar}] {done_bytes}/{total_bytes} bytes")
        } else {
            format!(" Downloading: {done_bytes} bytes")
        };
        lines.push(Line::from(Span::styled(
            progress_text,
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        )));
        if !app.code_intro_current_repo.is_empty() {
            lines.push(Line::from(Span::styled(
                format!(
                    " Current: {}  (repo {}/{})",
                    app.code_intro_current_repo,
                    done_repos.saturating_add(1).min(total_repos),
                    total_repos
                ),
                Style::default().fg(colors.text_pending()),
            )));
        }
    }
    let hint_lines = if app.code_intro_downloading {
        Vec::new()
    } else {
        pack_hint_lines(
            &[
                "[Up/Down] Navigate",
                "[Left/Right] Adjust",
                "[Type/Backspace] Edit",
                "[Enter] Confirm",
                "[ESC] Cancel",
            ],
            inner.width as usize,
        )
    };
    let footer_height = if hint_lines.is_empty() {
        0
    } else {
        (hint_lines.len() + 1) as u16 // add spacer line above hints
    };
    let (content_area, footer_area) = if footer_height > 0 && footer_height < inner.height {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner);
        (chunks[0], Some(chunks[1]))
    } else {
        (inner, None)
    };
    Paragraph::new(lines).render(content_area, frame.buffer_mut());
    if let Some(footer) = footer_area {
        let mut footer_lines = vec![Line::from("")];
        footer_lines.extend(
            hint_lines
                .into_iter()
                .map(|hint| Line::from(Span::styled(hint, Style::default().fg(colors.text_pending())))),
        );
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer, frame.buffer_mut());
    }
}

fn render_code_download_progress(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(60, 35, area);

    let block = Block::bordered()
        .title(" Downloading Code Source ")
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let total_bytes = app.code_intro_download_bytes_total;
    let done_bytes = app
        .code_intro_download_bytes
        .min(total_bytes.max(app.code_intro_download_bytes));
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

    let repo_name = if app.code_intro_current_repo.is_empty() {
        "Preparing download...".to_string()
    } else {
        app.code_intro_current_repo.clone()
    };

    let lines = vec![
        Line::from(Span::styled(
            format!(" Repo: {repo_name}"),
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
        Line::from(""),
        Line::from(Span::styled(
            " [ESC] Cancel",
            Style::default().fg(colors.text_pending()),
        )),
    ];

    Paragraph::new(lines).render(inner, frame.buffer_mut());
}

fn render_skill_tree(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let centered = skill_tree_popup_rect(area);
    let widget = SkillTreeWidget::new(
        &app.skill_tree,
        &app.key_stats,
        app.skill_tree_selected,
        app.skill_tree_detail_scroll,
        app.theme,
    );
    frame.render_widget(widget, centered);
}
