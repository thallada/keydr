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
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};
use ratatui::Terminal;

use app::{App, AppScreen, LessonMode};
use session::result::LessonResult;
use event::{AppEvent, EventHandler};
use ui::components::dashboard::Dashboard;
use ui::components::keyboard_diagram::KeyboardDiagram;
use ui::components::progress_bar::ProgressBar;
use ui::components::stats_dashboard::StatsDashboard;
use ui::components::stats_sidebar::StatsSidebar;
use ui::components::typing_area::TypingArea;
use ui::layout::AppLayout;

#[derive(Parser)]
#[command(name = "keydr", version, about = "Terminal typing tutor with adaptive learning")]
struct Cli {
    #[arg(short, long, help = "Theme name")]
    theme: Option<String>,

    #[arg(short, long, help = "Keyboard layout (qwerty, dvorak, colemak)")]
    layout: Option<String>,

    #[arg(short, long, help = "Number of words per lesson")]
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
                // Fallback: clear depressed keys after 150ms if no Release event received
                if let Some(last) = app.last_key_time {
                    if last.elapsed() > Duration::from_millis(150) && !app.depressed_keys.is_empty()
                    {
                        app.depressed_keys.clear();
                        app.last_key_time = None;
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
    // Track depressed keys for keyboard diagram
    match (&key.code, key.kind) {
        (KeyCode::Char(ch), KeyEventKind::Press) => {
            app.depressed_keys.insert(ch.to_ascii_lowercase());
            app.last_key_time = Some(Instant::now());
        }
        (KeyCode::Char(ch), KeyEventKind::Release) => {
            app.depressed_keys.remove(&ch.to_ascii_lowercase());
            return; // Don't process Release events as input
        }
        (_, KeyEventKind::Release) => return,
        _ => {}
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
        AppScreen::Lesson => handle_lesson_key(app, key),
        AppScreen::LessonResult => handle_result_key(app, key),
        AppScreen::StatsDashboard => handle_stats_key(app, key),
        AppScreen::Settings => handle_settings_key(app, key),
    }
}

fn handle_menu_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('1') => {
            app.lesson_mode = LessonMode::Adaptive;
            app.start_lesson();
        }
        KeyCode::Char('2') => {
            app.lesson_mode = LessonMode::Code;
            app.start_lesson();
        }
        KeyCode::Char('3') => {
            app.lesson_mode = LessonMode::Passage;
            app.start_lesson();
        }
        KeyCode::Char('s') => app.go_to_stats(),
        KeyCode::Char('c') => app.go_to_settings(),
        KeyCode::Up | KeyCode::Char('k') => app.menu.prev(),
        KeyCode::Down | KeyCode::Char('j') => app.menu.next(),
        KeyCode::Enter => match app.menu.selected {
            0 => {
                app.lesson_mode = LessonMode::Adaptive;
                app.start_lesson();
            }
            1 => {
                app.lesson_mode = LessonMode::Code;
                app.start_lesson();
            }
            2 => {
                app.lesson_mode = LessonMode::Passage;
                app.start_lesson();
            }
            3 => app.go_to_stats(),
            4 => app.go_to_settings(),
            _ => {}
        },
        _ => {}
    }
}

fn handle_lesson_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            let has_progress = app.lesson.as_ref().is_some_and(|l| l.cursor > 0);
            if has_progress && app.lesson_mode != LessonMode::Adaptive {
                // Non-adaptive: show result screen for partial lesson
                if let Some(ref lesson) = app.lesson {
                    let result = LessonResult::from_lesson(lesson, &app.lesson_events, app.lesson_mode.as_str());
                    app.last_result = Some(result);
                }
                app.screen = AppScreen::LessonResult;
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
        KeyCode::Char('r') => app.retry_lesson(),
        KeyCode::Char('q') | KeyCode::Esc => app.go_to_menu(),
        KeyCode::Char('s') => app.go_to_stats(),
        _ => {}
    }
}

fn handle_stats_key(app: &mut App, key: KeyEvent) {
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
                if !app.lesson_history.is_empty() {
                    let max_visible = app.lesson_history.len().min(20) - 1;
                    app.history_selected =
                        (app.history_selected + 1).min(max_visible);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.history_selected = app.history_selected.saturating_sub(1);
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if !app.lesson_history.is_empty() {
                    app.history_confirm_delete = true;
                }
            }
            KeyCode::Char('d') | KeyCode::Char('1') => app.stats_tab = 0,
            KeyCode::Char('h') | KeyCode::Char('2') => {} // already on history
            KeyCode::Char('3') => app.stats_tab = 2,
            KeyCode::Tab => app.stats_tab = (app.stats_tab + 1) % 3,
            KeyCode::BackTab => {
                app.stats_tab = if app.stats_tab == 0 { 2 } else { app.stats_tab - 1 }
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        KeyCode::Char('d') | KeyCode::Char('1') => app.stats_tab = 0,
        KeyCode::Char('h') | KeyCode::Char('2') => app.stats_tab = 1,
        KeyCode::Char('k') | KeyCode::Char('3') => app.stats_tab = 2,
        KeyCode::Tab => app.stats_tab = (app.stats_tab + 1) % 3,
        KeyCode::BackTab => app.stats_tab = if app.stats_tab == 0 { 2 } else { app.stats_tab - 1 },
        _ => {}
    }
}

fn handle_settings_key(app: &mut App, key: KeyEvent) {
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
            if app.settings_selected < 3 {
                app.settings_selected += 1;
            }
        }
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
            app.settings_cycle_forward();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.settings_cycle_backward();
        }
        _ => {}
    }
}

fn render(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let bg = Block::default().style(Style::default().bg(colors.bg()));
    frame.render_widget(bg, area);

    match app.screen {
        AppScreen::Menu => render_menu(frame, app),
        AppScreen::Lesson => render_lesson(frame, app),
        AppScreen::LessonResult => render_result(frame, app),
        AppScreen::StatsDashboard => render_stats(frame, app),
        AppScreen::Settings => render_settings(frame, app),
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
    let header_info = format!(
        " Level {} | Score {:.0} | {}/{} letters{}",
        crate::engine::scoring::level_from_score(app.profile.total_score),
        app.profile.total_score,
        app.letter_unlock.unlocked_count(),
        app.letter_unlock.total_letters(),
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
        " [1-3] Start  [s] Stats  [q] Quit ",
        Style::default().fg(colors.text_pending()),
    )]));
    frame.render_widget(footer, layout[2]);
}

fn render_lesson(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    if let Some(ref lesson) = app.lesson {
        let app_layout = AppLayout::new(area);
        let tier = app_layout.tier;

        let mode_name = match app.lesson_mode {
            LessonMode::Adaptive => "Adaptive",
            LessonMode::Code => "Code",
            LessonMode::Passage => "Passage",
        };

        // For medium/narrow: show compact stats in header
        if !tier.show_sidebar() {
            let wpm = lesson.wpm();
            let accuracy = lesson.accuracy();
            let errors = lesson.typo_count();
            let header_text = format!(
                " {mode_name} | WPM: {wpm:.0} | Acc: {accuracy:.1}% | Errors: {errors}"
            );
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
            let header_title = format!(" {mode_name} Practice ");
            let focus_text = if let Some(focused) = app.letter_unlock.focused {
                format!(" | Focus: '{focused}'")
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

        let mut constraints: Vec<Constraint> = vec![Constraint::Min(5)];
        if show_progress {
            constraints.push(Constraint::Length(3));
        }
        if show_kbd {
            constraints.push(Constraint::Length(5));
        }

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(app_layout.main);

        let typing = TypingArea::new(lesson, app.theme);
        frame.render_widget(typing, main_layout[0]);

        let mut idx = 1;
        if show_progress {
            let progress = ProgressBar::new(
                "Letter Progress",
                app.letter_unlock.progress(),
                app.theme,
            );
            frame.render_widget(progress, main_layout[idx]);
            idx += 1;
        }

        if show_kbd {
            let next_char = lesson.target.get(lesson.cursor).copied();
            let kbd = KeyboardDiagram::new(
                app.letter_unlock.focused,
                next_char,
                &app.letter_unlock.included,
                &app.depressed_keys,
                app.theme,
            )
            .compact(tier.compact_keyboard());
            frame.render_widget(kbd, main_layout[idx]);
        }

        if let Some(sidebar_area) = app_layout.sidebar {
            let sidebar = StatsSidebar::new(lesson, app.last_result.as_ref(), app.theme);
            frame.render_widget(sidebar, sidebar_area);
        }

        let footer = Paragraph::new(Line::from(Span::styled(
            " [ESC] End lesson  [Backspace] Delete ",
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
        &app.lesson_history,
        &app.key_stats,
        app.stats_tab,
        app.config.target_wpm,
        app.theme,
        app.history_selected,
        app.history_confirm_delete,
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
    let languages_all = ["rust", "python", "javascript", "go"];
    let current_lang = app
        .config
        .code_languages
        .first()
        .map(|s| s.as_str())
        .unwrap_or("rust");

    let fields: Vec<(String, String)> = vec![
        ("Target WPM".to_string(), format!("{}", app.config.target_wpm)),
        ("Theme".to_string(), app.config.theme.clone()),
        ("Word Count".to_string(), format!("{}", app.config.word_count)),
        ("Code Language".to_string(), current_lang.to_string()),
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
        .constraints(fields.iter().map(|_| Constraint::Length(3)).collect::<Vec<_>>())
        .split(layout[1]);

    for (i, (label, value)) in fields.iter().enumerate() {
        let is_selected = i == app.settings_selected;
        let indicator = if is_selected { " > " } else { "   " };

        let label_text = format!("{indicator}{label}:");
        let value_text = format!("  < {value} >");

        let label_style = Style::default().fg(if is_selected {
            colors.accent()
        } else {
            colors.fg()
        }).add_modifier(if is_selected { Modifier::BOLD } else { Modifier::empty() });

        let value_style = Style::default().fg(if is_selected {
            colors.focused_key()
        } else {
            colors.text_pending()
        });

        let lines = vec![
            Line::from(Span::styled(label_text, label_style)),
            Line::from(Span::styled(value_text, value_style)),
        ];
        Paragraph::new(lines).render(field_layout[i], frame.buffer_mut());
    }

    let _ = (available_themes, languages_all);

    let footer = Paragraph::new(Line::from(Span::styled(
        "  [ESC] Save & back  [Enter/arrows] Change value",
        Style::default().fg(colors.accent()),
    )));
    footer.render(layout[3], frame.buffer_mut());
}
