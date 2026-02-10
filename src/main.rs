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
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
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
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let events = EventHandler::new(Duration::from_millis(100));

    let result = run_app(&mut terminal, &mut app, &events);

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
            AppEvent::Tick => {}
            AppEvent::Resize(_, _) => {}
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
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
            _ => {}
        },
        _ => {}
    }
}

fn handle_lesson_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            let has_progress = app.lesson.as_ref().is_some_and(|l| l.cursor > 0);
            if has_progress {
                if let Some(ref lesson) = app.lesson {
                    let result = LessonResult::from_lesson(lesson, &app.lesson_events);
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
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
        _ => {}
    }
}

fn handle_settings_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.go_to_menu(),
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

        let mode_name = match app.lesson_mode {
            LessonMode::Adaptive => "Adaptive",
            LessonMode::Code => "Code",
            LessonMode::Passage => "Passage",
        };
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

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(3),
                Constraint::Length(4),
            ])
            .split(app_layout.main);

        let typing = TypingArea::new(lesson, app.theme);
        frame.render_widget(typing, main_layout[0]);

        let progress = ProgressBar::new(
            "Letter Progress",
            app.letter_unlock.progress(),
            app.theme,
        );
        frame.render_widget(progress, main_layout[1]);

        let kbd = KeyboardDiagram::new(
            app.letter_unlock.focused,
            &app.letter_unlock.included,
            app.theme,
        );
        frame.render_widget(kbd, main_layout[2]);

        let sidebar = StatsSidebar::new(lesson, app.theme);
        frame.render_widget(sidebar, app_layout.sidebar);

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
    let dashboard = StatsDashboard::new(&app.lesson_history, app.theme);
    frame.render_widget(dashboard, area);
}

fn render_settings(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let block = Block::bordered()
        .title(" Settings ")
        .border_style(Style::default().fg(colors.accent()));

    let target_wpm = format!("  Target WPM: {}", app.config.target_wpm);
    let theme_name = format!("  Theme: {}", app.config.theme);
    let layout_name = format!("  Layout: {}", app.config.keyboard_layout);
    let languages = format!("  Languages: {}", app.config.code_languages.join(", "));

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Settings coming soon...",
            Style::default().fg(colors.text_pending()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            &*target_wpm,
            Style::default().fg(colors.fg()),
        )),
        Line::from(Span::styled(
            &*theme_name,
            Style::default().fg(colors.fg()),
        )),
        Line::from(Span::styled(
            &*layout_name,
            Style::default().fg(colors.fg()),
        )),
        Line::from(Span::styled(
            &*languages,
            Style::default().fg(colors.fg()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  [ESC] Back",
            Style::default().fg(colors.accent()),
        )),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
