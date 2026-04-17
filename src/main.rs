rust_i18n::i18n!("locales", fallback = "en");

mod app;
mod config;
mod engine;
mod event;
mod generator;
mod i18n;
mod keyboard;
mod l10n;
mod session;
mod store;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyEventKind, KeyEventState,
    KeyModifiers, KeyboardEnhancementFlags, ModifierKeyCode, MouseButton, MouseEvent,
    MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
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

use app::{App, AppScreen, DrillMode, MilestoneKind, SettingItem, StatusKind};
use i18n::t;
use engine::skill_tree::{BranchStatus, DrillScope, find_key_branch, get_branch_definition};
use event::{AppEvent, EventHandler};
use generator::code_syntax::{code_language_options, is_language_cached, language_by_key};
use generator::passage::{is_book_cached, passage_options};
use keyboard::display::key_display_name;
use keyboard::finger::Hand;
use l10n::language_pack::{
    CapabilityState, default_keyboard_layout_for_language, dictionary_languages_for_layout,
    find_language_pack, language_packs, validate_language_layout_pair,
};
use ui::components::dashboard::Dashboard;
use ui::components::keyboard_diagram::KeyboardDiagram;
use ui::components::menu::Menu;
use ui::components::skill_tree::{
    SkillTreeWidget, branch_list_spacing_flags, detail_line_count_with_level_spacing_for_tree,
    selectable_branches, use_expanded_level_spacing_for_tree, use_side_by_side_layout,
};
use ui::components::stats_dashboard::{
    AnomalyBigramRow, NgramTabData, StatsDashboard, history_page_size_for_terminal,
};
use ui::components::stats_sidebar::StatsSidebar;
use ui::components::typing_area::TypingArea;
use ui::layout::AppLayout;
use ui::layout::{pack_hint_lines, wrapped_line_count};
use ui::line_input::{InputResult, LineInput, PathField};

#[derive(Parser)]
#[command(
    name = "keydr",
    version,
    about = "Terminal typing tutor with adaptive learning"
)]
struct Cli {
    #[arg(short, long, help = "Theme name")]
    theme: Option<String>,

    #[arg(
        short,
        long,
        help = "Keyboard layout (qwerty, dvorak, colemak, de_qwertz, fr_azerty)"
    )]
    layout: Option<String>,

    #[arg(short, long, help = "Number of words per drill")]
    words: Option<usize>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut app = App::new();
    i18n::set_ui_locale(&app.config.ui_language);

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
    if let Some(layout_key) = cli.layout {
        match app.set_keyboard_layout(&layout_key) {
            Ok(_capability) => {
                let _ = app.config.save();
            }
            Err(err) => {
                eprintln!("Ignoring --layout {layout_key:?}: {err}");
            }
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    // Request kitty keyboard protocol enhancements from the terminal.
    // - DISAMBIGUATE_ESCAPE_CODES: CSI u sequences for unambiguous key IDs,
    //   enables CAPS_LOCK/NUM_LOCK state detection.
    // - REPORT_EVENT_TYPES: key release and repeat events (for depressed-key
    //   tracking in the keyboard diagram).
    // - REPORT_ALL_KEYS_AS_ESCAPE_CODES: standalone modifier key events
    //   (LeftShift, RightShift, etc.) so we can track shift state independently.
    // Falls back gracefully in terminals that don't support the protocol (tmux,
    // mosh, SSH, older emulators) — the execute! returns Err and we use
    // timer-based fallbacks instead.
    let keyboard_enhanced = execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES,
        )
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
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
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
            AppEvent::Mouse(mouse) => handle_mouse(app, mouse),
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
                // Fallback: clear depressed keys and shift state on a timer.
                // Needed because not all terminals send Release events (e.g.
                // WezTerm doesn't implement REPORT_EVENT_TYPES). Terminals that
                // DO send Release events handle cleanup in handle_key instead,
                // and the repeated Press events they send while a key is held
                // keep resetting last_key_time so this fallback never fires.
                // This causes a brief flicker (key clears, then re-appears when
                // OS key repeat kicks in after ~300-500ms), but that's an
                // acceptable tradeoff for responsive key press visualization.
                if let Some(last) = app.last_key_time {
                    if last.elapsed() > Duration::from_millis(150) {
                        if !app.depressed_keys.is_empty() {
                            app.depressed_keys.clear();
                        }
                        if app.shift_held {
                            app.shift_held = false;
                        }
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
    // Track caps lock state via Kitty protocol metadata (KeyEventState::CAPS_LOCK).
    // Only Modifier key events reliably report lock state in WezTerm; regular
    // character events have empty state (0x0). So we only set caps_lock=true
    // when CAPS_LOCK appears, and only clear it from Modifier events that
    // reliably report state.
    if key.state.contains(KeyEventState::CAPS_LOCK) {
        app.caps_lock = true;
    } else if matches!(key.code, KeyCode::Modifier(_) | KeyCode::CapsLock) {
        // Modifier events and CapsLock key events reliably report lock state.
        // If CAPS_LOCK isn't in state, caps lock was toggled off.
        app.caps_lock = false;
    }

    // Determine whether the physical Shift key is held. When caps lock is on,
    // crossterm infers SHIFT from uppercase chars, so we need a heuristic:
    // caps lock + shift inverts case, producing lowercase. So if caps lock is
    // on and the char is uppercase, that's caps lock alone, not shift.
    let infer_shift = |ch: char, mods: KeyModifiers, caps: bool| -> bool {
        let has_shift = mods.contains(KeyModifiers::SHIFT);
        if caps && ch.is_alphabetic() {
            // Caps lock on: shift would invert to lowercase
            has_shift && ch.is_lowercase()
        } else {
            has_shift
        }
    };

    // Track depressed keys and shift state for keyboard diagram
    match (&key.code, key.kind) {
        (
            KeyCode::Modifier(ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift),
            KeyEventKind::Press | KeyEventKind::Repeat,
        ) => {
            app.shift_held = true;
            app.last_key_time = Some(Instant::now());
            return; // Don't dispatch bare shift presses to screen handlers
        }
        (
            KeyCode::Modifier(ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift),
            KeyEventKind::Release,
        ) => {
            app.shift_held = false;
            return;
        }
        (KeyCode::Char(ch), KeyEventKind::Press) => {
            let normalized = app.keyboard_model.shifted_to_base(*ch).unwrap_or(*ch);
            app.depressed_keys.insert(normalized);
            app.last_key_time = Some(Instant::now());
            app.shift_held = infer_shift(*ch, key.modifiers, app.caps_lock);
        }
        (KeyCode::Char(ch), KeyEventKind::Release) => {
            let normalized = app.keyboard_model.shifted_to_base(*ch).unwrap_or(*ch);
            app.depressed_keys.remove(&normalized);
            return; // Don't process Release events as input
        }
        (KeyCode::Backspace, KeyEventKind::Press) => {
            app.depressed_keys.insert('\x08');
            app.last_key_time = Some(Instant::now());
            app.shift_held = key.modifiers.contains(KeyModifiers::SHIFT);
        }
        (KeyCode::Backspace, KeyEventKind::Release) => {
            app.depressed_keys.remove(&'\x08');
            return;
        }
        (KeyCode::Tab, KeyEventKind::Press) => {
            app.depressed_keys.insert('\t');
            app.last_key_time = Some(Instant::now());
            app.shift_held = key.modifiers.contains(KeyModifiers::SHIFT);
        }
        (KeyCode::Tab, KeyEventKind::Release) => {
            app.depressed_keys.remove(&'\t');
            return;
        }
        (KeyCode::Enter, KeyEventKind::Press) => {
            app.depressed_keys.insert('\n');
            app.last_key_time = Some(Instant::now());
            app.shift_held = key.modifiers.contains(KeyModifiers::SHIFT);
        }
        (KeyCode::Enter, KeyEventKind::Release) => {
            app.depressed_keys.remove(&'\n');
            return;
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

    // Ctrl+C always quits, even during input lock.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Briefly block all input right after a drill completes to avoid accidental
    // popup dismissal or continuation from trailing keystrokes.
    if app.post_drill_input_lock_remaining_ms().is_some()
        && (!app.milestone_queue.is_empty()
            || app.screen == AppScreen::DrillResult
            || app.screen == AppScreen::Drill)
    {
        return;
    }

    // Adaptive intro overlay intercepts input before milestone handling.
    if app.show_adaptive_intro {
        match key.code {
            KeyCode::Left => {
                app.config.target_wpm = app.config.target_wpm.saturating_sub(5).max(10);
                app.key_stats.target_cpm = app.config.target_cpm();
                app.ranked_key_stats.target_cpm = app.config.target_cpm();
            }
            KeyCode::Right => {
                app.config.target_wpm = (app.config.target_wpm + 5).min(200);
                app.key_stats.target_cpm = app.config.target_cpm();
                app.ranked_key_stats.target_cpm = app.config.target_cpm();
            }
            _ => {
                app.show_adaptive_intro = false;
                app.config.adaptive_intro_done = true;
                let _ = app.config.save();
                app.start_global_adaptive_drill();
            }
        }
        return;
    }

    // Milestone overlays are modal: one key action applies and is consumed.
    if let Some(milestone) = app.milestone_queue.front() {
        let open_skill_tree = milestone_supports_skill_tree_shortcut(milestone)
            && matches!(key.code, KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'t'));
        app.milestone_queue.pop_front();
        if open_skill_tree {
            app.go_to_skill_tree();
        }
        return;
    }

    match app.screen {
        AppScreen::Menu => handle_menu_key(app, key),
        AppScreen::Drill => handle_drill_key(app, key),
        AppScreen::DrillResult => handle_result_key(app, key),
        AppScreen::StatsDashboard => handle_stats_key(app, key),
        AppScreen::Settings => handle_settings_key(app, key),
        AppScreen::DictionaryLanguageSelect => handle_dictionary_language_key(app, key),
        AppScreen::KeyboardLayoutSelect => handle_keyboard_layout_key(app, key),
        AppScreen::SkillTree => handle_skill_tree_key(app, key),
        AppScreen::CodeLanguageSelect => handle_code_language_key(app, key),
        AppScreen::PassageBookSelect => handle_passage_book_key(app, key),
        AppScreen::PassageIntro => handle_passage_intro_key(app, key),
        AppScreen::PassageDownloadProgress => handle_passage_download_progress_key(app, key),
        AppScreen::CodeIntro => handle_code_intro_key(app, key),
        AppScreen::CodeDownloadProgress => handle_code_download_progress_key(app, key),
        AppScreen::Keyboard => handle_keyboard_explorer_key(app, key),
        AppScreen::UiLanguageSelect => handle_ui_language_key(app, key),
    }
}

fn milestone_supports_skill_tree_shortcut(milestone: &app::KeyMilestonePopup) -> bool {
    matches!(
        milestone.kind,
        MilestoneKind::BranchesAvailable | MilestoneKind::BranchComplete
    )
}

fn terminal_area() -> Rect {
    let (w, h) = crossterm::terminal::size().unwrap_or((120, 40));
    Rect::new(0, 0, w, h)
}

fn point_in_rect(x: u16, y: u16, rect: Rect) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}

fn hint_token_at(area: Rect, hints: &[&str], x: u16, y: u16) -> Option<String> {
    if !point_in_rect(x, y, area) {
        return None;
    }

    let prefix = "  ";
    let separator = "  ";
    let width = area.width as usize;
    if width == 0 || hints.is_empty() {
        return None;
    }

    let row = y.saturating_sub(area.y) as usize;
    let col = x.saturating_sub(area.x) as usize;
    let prefix_len = prefix.chars().count();
    let sep_len = separator.chars().count();

    let mut current_line = 0usize;
    let mut line_len = prefix_len;
    let mut has_hint_on_line = false;

    for hint in hints {
        if hint.is_empty() {
            continue;
        }
        let hint_len = hint.chars().count();
        let candidate_len = if has_hint_on_line {
            line_len + sep_len + hint_len
        } else {
            line_len + hint_len
        };

        if candidate_len > width && has_hint_on_line {
            current_line += 1;
            line_len = prefix_len;
            has_hint_on_line = false;
        }

        let start_col = if has_hint_on_line {
            line_len + sep_len
        } else {
            line_len
        };
        let end_col = start_col + hint_len;
        if current_line == row && (start_col..end_col).contains(&col) {
            if let (Some(lb), Some(rb)) = (hint.find('['), hint.find(']'))
                && rb > lb + 1
            {
                return Some(hint[lb + 1..rb].to_string());
            }
            return None;
        }

        line_len = end_col;
        has_hint_on_line = true;
    }

    // Fallback for unexpected layout drift: use packed lines and bracket search.
    let lines = pack_hint_lines(hints, area.width as usize);
    if row >= lines.len() {
        return None;
    }
    let chars: Vec<char> = lines[row].chars().collect();
    if col >= chars.len() {
        return None;
    }
    for hint in hints {
        if hint.is_empty() {
            continue;
        }
        let line = &lines[row];
        if let Some(start) = line.find(hint) {
            let start_col = line[..start].chars().count();
            let end_col = start_col + hint.chars().count();
            if (start_col..end_col).contains(&col) {
                if let (Some(lb), Some(rb)) = (hint.find('['), hint.find(']'))
                    && rb > lb + 1
                {
                    return Some(hint[lb + 1..rb].to_string());
                }
            }
        }
    }
    None
}

fn milestone_footer_hint_token_at(
    milestone: &app::KeyMilestonePopup,
    x: u16,
    y: u16,
) -> Option<String> {
    if !milestone_supports_skill_tree_shortcut(milestone) {
        return None;
    }
    let area = terminal_area();
    let is_key_milestone = matches!(
        milestone.kind,
        MilestoneKind::Unlock | MilestoneKind::Mastery
    );
    let kbd_mode = if is_key_milestone {
        overlay_keyboard_mode(area.height)
    } else {
        0
    };
    let overlay_height = match &milestone.kind {
        MilestoneKind::BranchesAvailable => 18u16.min(area.height.saturating_sub(2)),
        MilestoneKind::BranchComplete
        | MilestoneKind::AllKeysUnlocked
        | MilestoneKind::AllKeysMastered => 12u16.min(area.height.saturating_sub(2)),
        _ => match kbd_mode {
            2 => 18u16.min(area.height.saturating_sub(2)),
            1 => 14u16.min(area.height.saturating_sub(2)),
            _ => 10u16.min(area.height.saturating_sub(2)),
        },
    };
    let overlay_width = 60u16.min(area.width.saturating_sub(4));
    let left = area.x + (area.width.saturating_sub(overlay_width)) / 2;
    let top = area.y + (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(left, top, overlay_width, overlay_height);
    let inner = Block::bordered().inner(overlay_area);
    let footer_y = inner.y + inner.height.saturating_sub(1);
    let footer_area = Rect::new(inner.x, footer_y, inner.width, 1);
    let hint_skill_tree = ui::hint::hint(ui::hint::K_T, t!("milestones.hint_skill_tree_continue").as_ref());
    let hint_any_key = t!("milestones.hint_any_key");
    let hints: Vec<&str> = vec![hint_skill_tree.as_str(), hint_any_key.as_ref()];
    hint_token_at(
        footer_area,
        &hints,
        x,
        y,
    )
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    if app.post_drill_input_lock_remaining_ms().is_some()
        && (!app.milestone_queue.is_empty()
            || app.screen == AppScreen::DrillResult
            || app.screen == AppScreen::Drill)
    {
        return;
    }

    if !app.milestone_queue.is_empty() {
        if matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
        ) {
            if let Some(milestone) = app.milestone_queue.front()
                && milestone_footer_hint_token_at(milestone, mouse.column, mouse.row)
                    .is_some_and(|t| t == "t")
            {
                app.milestone_queue.pop_front();
                app.go_to_skill_tree();
                return;
            }
            app.milestone_queue.pop_front();
        }
        return;
    }

    match app.screen {
        AppScreen::Menu => handle_menu_mouse(app, mouse),
        AppScreen::Drill => handle_drill_mouse(app, mouse),
        AppScreen::DrillResult => handle_result_mouse(app, mouse),
        AppScreen::StatsDashboard => handle_stats_mouse(app, mouse),
        AppScreen::Settings => handle_settings_mouse(app, mouse),
        AppScreen::DictionaryLanguageSelect => handle_dictionary_language_mouse(app, mouse),
        AppScreen::KeyboardLayoutSelect => handle_keyboard_layout_mouse(app, mouse),
        AppScreen::SkillTree => handle_skill_tree_mouse(app, mouse),
        AppScreen::CodeLanguageSelect => handle_code_language_mouse(app, mouse),
        AppScreen::PassageBookSelect => handle_passage_book_mouse(app, mouse),
        AppScreen::PassageIntro => handle_passage_intro_mouse(app, mouse),
        AppScreen::PassageDownloadProgress => handle_passage_download_progress_mouse(app, mouse),
        AppScreen::CodeIntro => handle_code_intro_mouse(app, mouse),
        AppScreen::CodeDownloadProgress => handle_code_download_progress_mouse(app, mouse),
        AppScreen::Keyboard => handle_keyboard_explorer_mouse(app, mouse),
        AppScreen::UiLanguageSelect => handle_ui_language_mouse(app, mouse),
    }
}

fn activate_menu_selected(app: &mut App) {
    match app.menu.selected {
        0 => {
            if !app.config.adaptive_intro_done {
                app.show_adaptive_intro = true;
            } else {
                app.start_global_adaptive_drill();
            }
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
        4 => app.go_to_keyboard(),
        5 => app.go_to_stats(),
        6 => app.go_to_settings(),
        _ => {}
    }
}

fn handle_menu_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollUp => app.menu.prev(),
        MouseEventKind::ScrollDown => app.menu.next(),
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
            let is_secondary = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right));
            let area = terminal_area();
            let mh_start = ui::hint::hint(ui::hint::K_1_3, t!("menu.hint_start").as_ref());
            let mh_tree = ui::hint::hint(ui::hint::K_T, t!("menu.hint_skill_tree").as_ref());
            let mh_kbd = ui::hint::hint(ui::hint::K_B, t!("menu.hint_keyboard").as_ref());
            let mh_stats = ui::hint::hint(ui::hint::K_S, t!("menu.hint_stats").as_ref());
            let mh_settings = ui::hint::hint(ui::hint::K_C, t!("menu.hint_settings").as_ref());
            let mh_quit = ui::hint::hint(ui::hint::K_Q, t!("menu.hint_quit").as_ref());
            let menu_hints: Vec<&str> = vec![
                mh_start.as_str(),
                mh_tree.as_str(),
                mh_kbd.as_str(),
                mh_stats.as_str(),
                mh_settings.as_str(),
                mh_quit.as_str(),
            ];
            let footer_line_count = pack_hint_lines(&menu_hints, area.width as usize)
                .len()
                .max(1) as u16;
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(footer_line_count),
                ])
                .split(area);
            if let Some(token) = hint_token_at(layout[2], &menu_hints, mouse.column, mouse.row) {
                match token.as_str() {
                    "1-3" => {
                        let mut selected = app.menu.selected.min(2);
                        selected = if is_secondary {
                            if selected == 0 { 2 } else { selected - 1 }
                        } else {
                            (selected + 1) % 3
                        };
                        app.menu.selected = selected;
                        activate_menu_selected(app);
                    }
                    "t" => {
                        app.menu.selected = 3;
                        activate_menu_selected(app);
                    }
                    "b" => {
                        app.menu.selected = 4;
                        activate_menu_selected(app);
                    }
                    "s" => {
                        app.menu.selected = 5;
                        activate_menu_selected(app);
                    }
                    "c" => {
                        app.menu.selected = 6;
                        activate_menu_selected(app);
                    }
                    "q" => app.should_quit = true,
                    _ => {}
                }
                return;
            }
            let menu_area = ui::layout::centered_rect(50, 80, layout[1]);
            let inner = Block::bordered().inner(menu_area);
            let sections = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(inner);
            let list_area = sections[2];
            if point_in_rect(mouse.column, mouse.row, list_area) {
                let row = ((mouse.row - list_area.y) / 3) as usize;
                if row < Menu::item_count() {
                    app.menu.selected = row;
                    activate_menu_selected(app);
                }
            }
        }
        _ => {}
    }
}

fn handle_drill_mouse(app: &mut App, mouse: MouseEvent) {
    if !matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
    ) {
        return;
    }
    let layout = AppLayout::new(terminal_area());
    if point_in_rect(mouse.column, mouse.row, layout.footer) {
        let hint_end = ui::hint::hint(ui::hint::K_ESC, t!("drill.hint_end").as_ref());
        let hint_bs = ui::hint::hint(ui::hint::K_BACKSPACE, t!("drill.hint_backspace").as_ref());
        let hints: Vec<&str> = vec![hint_end.as_str(), hint_bs.as_str()];
        if let Some(token) = hint_token_at(layout.footer, &hints, mouse.column, mouse.row) {
            match token.as_str() {
                "ESC" => {
                    let has_progress = app.drill.as_ref().is_some_and(|d| d.cursor > 0);
                    if has_progress {
                        app.finish_partial_drill();
                    } else {
                        app.go_to_menu();
                    }
                }
                "Backspace" => app.backspace(),
                _ => {}
            }
            return;
        }
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            let has_progress = app.drill.as_ref().is_some_and(|d| d.cursor > 0);
            if has_progress {
                app.finish_partial_drill();
            } else {
                app.go_to_menu();
            }
        }
    }
}

fn delete_confirm_dialog_area() -> Rect {
    let area = terminal_area();
    let dialog_width = 34u16;
    let dialog_height = 5u16;
    let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
    let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
    Rect::new(dialog_x, dialog_y, dialog_width, dialog_height)
}

fn handle_result_mouse(app: &mut App, mouse: MouseEvent) {
    if !matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
    ) {
        return;
    }
    if app.history_confirm_delete && !app.drill_history.is_empty() {
        let dialog = delete_confirm_dialog_area();
        if point_in_rect(mouse.column, mouse.row, dialog) {
            if mouse.column < dialog.x + dialog.width / 2 {
                app.delete_session();
                app.history_confirm_delete = false;
                app.continue_drill();
            } else {
                app.history_confirm_delete = false;
            }
        }
        return;
    }
    if app.last_result.is_some() {
        let area = terminal_area();
        let centered = ui::layout::centered_rect(60, 70, area);
        let inner = Block::bordered().inner(centered);
        let h_cont = ui::hint::hint(ui::hint::K_C_ENTER_SPACE, t!("dashboard.hint_continue").as_ref());
        let h_retry = ui::hint::hint(ui::hint::K_R, t!("dashboard.hint_retry").as_ref());
        let h_menu = ui::hint::hint(ui::hint::K_Q, t!("dashboard.hint_menu").as_ref());
        let h_stats = ui::hint::hint(ui::hint::K_S, t!("dashboard.hint_stats").as_ref());
        let h_del = ui::hint::hint(ui::hint::K_X, t!("dashboard.hint_delete").as_ref());
        let hints: Vec<&str> = vec![
            h_cont.as_str(),
            h_retry.as_str(),
            h_menu.as_str(),
            h_stats.as_str(),
            h_del.as_str(),
        ];
        let footer_line_count = pack_hint_lines(&hints, inner.width as usize).len().max(1) as u16;
        let footer_y = inner
            .y
            .saturating_add(inner.height.saturating_sub(footer_line_count));
        let footer_area = Rect::new(inner.x, footer_y, inner.width, footer_line_count);
        if let Some(token) = hint_token_at(footer_area, &hints, mouse.column, mouse.row) {
            match token.as_str() {
                "c/Enter/Space" => app.continue_drill(),
                "r" => app.retry_drill(),
                "q" => app.go_to_menu(),
                "s" => app.go_to_stats(),
                "x" => {
                    if !app.drill_history.is_empty() {
                        app.history_selected = 0;
                        app.history_confirm_delete = true;
                    }
                }
                _ => {}
            }
            return;
        }
    }
    if app.last_result.is_some() {
        app.continue_drill();
    }
}

fn stats_tab_labels() -> [String; 6] {
    [
        t!("stats.tab_dashboard").to_string(),
        t!("stats.tab_history").to_string(),
        t!("stats.tab_activity").to_string(),
        t!("stats.tab_accuracy").to_string(),
        t!("stats.tab_timing").to_string(),
        t!("stats.tab_ngrams").to_string(),
    ]
}

fn wrapped_stats_tab_line_count(width: usize) -> usize {
    let labels = stats_tab_labels();
    let mut lines = 1usize;
    let mut current_width = 0usize;
    for label in &labels {
        let item_width = format!(" {label} ").chars().count() + 2;
        if current_width > 0 && current_width + item_width > width {
            lines += 1;
            current_width = 0;
        }
        current_width += item_width;
    }
    lines.max(1)
}

fn stats_tab_at_point(tab_area: Rect, width: usize, x: u16, y: u16) -> Option<usize> {
    let labels = stats_tab_labels();
    let mut row = tab_area.y;
    let mut col = tab_area.x;
    let max_col = tab_area.x + width as u16;

    for (idx, label) in labels.iter().enumerate() {
        let text = format!(" {label} ");
        let text_width = text.chars().count() as u16;
        let item_width = text_width + 2; // separator
        if col > tab_area.x && col + item_width > max_col {
            row = row.saturating_add(1);
            col = tab_area.x;
        }
        if y == row && x >= col && x < col + text_width {
            return Some(idx);
        }
        col = col.saturating_add(item_width);
    }
    None
}

fn handle_stats_mouse(app: &mut App, mouse: MouseEvent) {
    const STATS_TAB_COUNT: usize = 6;

    if app.history_confirm_delete {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            let dialog = delete_confirm_dialog_area();
            if point_in_rect(mouse.column, mouse.row, dialog) {
                if mouse.column < dialog.x + dialog.width / 2 {
                    app.delete_session();
                    app.history_confirm_delete = false;
                } else {
                    app.history_confirm_delete = false;
                }
            }
        }
        return;
    }

    if app.drill_history.is_empty() {
        return;
    }

    let area = terminal_area();
    let inner = Block::bordered().inner(area);
    let width = inner.width as usize;
    let tab_line_count = wrapped_stats_tab_line_count(width) as u16;
    let sh_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("stats.hint_back").as_ref());
    let sh_next = ui::hint::hint(ui::hint::K_TAB, t!("stats.hint_next_tab").as_ref());
    let sh_switch = ui::hint::hint(ui::hint::K_1_6, t!("stats.hint_switch_tab").as_ref());
    let sh_nav = ui::hint::hint(ui::hint::K_J_K, t!("stats.hint_navigate").as_ref());
    let sh_page = ui::hint::hint(ui::hint::K_PGUP_PGDN, t!("stats.hint_page").as_ref());
    let sh_del = ui::hint::hint(ui::hint::K_X, t!("stats.hint_delete").as_ref());
    let footer_hints: Vec<&str> = if app.stats_tab == 1 {
        vec![
            sh_back.as_str(),
            sh_next.as_str(),
            sh_switch.as_str(),
            sh_nav.as_str(),
            sh_page.as_str(),
            sh_del.as_str(),
        ]
    } else {
        vec![sh_back.as_str(), sh_next.as_str(), sh_switch.as_str()]
    };
    let footer_line_count = pack_hint_lines(&footer_hints, width).len().max(1) as u16;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(tab_line_count),
            Constraint::Min(10),
            Constraint::Length(footer_line_count),
        ])
        .split(inner);

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
            let is_secondary = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right));
            if let Some(token) = hint_token_at(layout[2], &footer_hints, mouse.column, mouse.row) {
                match token.as_str() {
                    "q/ESC" => app.go_to_menu(),
                    "Tab" => {
                        app.stats_tab = if is_secondary {
                            if app.stats_tab == 0 {
                                STATS_TAB_COUNT - 1
                            } else {
                                app.stats_tab - 1
                            }
                        } else {
                            (app.stats_tab + 1) % STATS_TAB_COUNT
                        };
                    }
                    "1-6" => {
                        app.stats_tab = if is_secondary {
                            if app.stats_tab == 0 {
                                STATS_TAB_COUNT - 1
                            } else {
                                app.stats_tab - 1
                            }
                        } else {
                            (app.stats_tab + 1) % STATS_TAB_COUNT
                        };
                    }
                    "j/k" => {
                        if app.stats_tab == 1 && !app.drill_history.is_empty() {
                            if is_secondary {
                                app.history_selected = app.history_selected.saturating_sub(1);
                            } else {
                                app.history_selected =
                                    (app.history_selected + 1).min(app.drill_history.len() - 1);
                            }
                            keep_history_selection_visible(app, current_history_page_size());
                        }
                    }
                    "PgUp/PgDn" => {
                        if app.stats_tab == 1 && !app.drill_history.is_empty() {
                            let page_size = current_history_page_size();
                            if is_secondary {
                                let max_idx = app.drill_history.len() - 1;
                                app.history_selected =
                                    (app.history_selected + page_size).min(max_idx);
                            } else {
                                app.history_selected =
                                    app.history_selected.saturating_sub(page_size);
                            }
                            keep_history_selection_visible(app, page_size);
                        }
                    }
                    "x" => {
                        if app.stats_tab == 1 && !app.drill_history.is_empty() {
                            app.history_confirm_delete = true;
                        }
                    }
                    _ => {}
                }
                return;
            }
            if point_in_rect(mouse.column, mouse.row, layout[0])
                && let Some(tab) = stats_tab_at_point(layout[0], width, mouse.column, mouse.row)
            {
                app.stats_tab = tab;
                return;
            }

            if app.stats_tab == 1 {
                let table_inner = Block::bordered().inner(layout[1]);
                if point_in_rect(mouse.column, mouse.row, table_inner)
                    && mouse.row >= table_inner.y.saturating_add(2)
                {
                    let row = (mouse.row - table_inner.y.saturating_add(2)) as usize;
                    let idx = app.history_scroll + row;
                    if idx < app.drill_history.len() {
                        app.history_selected = idx;
                        keep_history_selection_visible(app, current_history_page_size());
                    }
                }
            }
        }
        MouseEventKind::ScrollUp => {
            if app.stats_tab == 1 {
                app.history_selected = app.history_selected.saturating_sub(1);
                keep_history_selection_visible(app, current_history_page_size());
            } else {
                app.stats_tab = app.stats_tab.saturating_sub(1);
            }
        }
        MouseEventKind::ScrollDown => {
            if app.stats_tab == 1 {
                if !app.drill_history.is_empty() {
                    app.history_selected =
                        (app.history_selected + 1).min(app.drill_history.len() - 1);
                    keep_history_selection_visible(app, current_history_page_size());
                }
            } else {
                app.stats_tab = (app.stats_tab + 1).min(STATS_TAB_COUNT - 1);
            }
        }
        _ => {}
    }
}

fn settings_fields(app: &App) -> Vec<(SettingItem, String, String)> {
    let dictionary_language_label = find_language_pack(&app.config.dictionary_language)
        .map(|pack| pack.autonym.to_string())
        .unwrap_or_else(|| app.config.dictionary_language.clone());
    let keyboard_layout_label = app.config.keyboard_layout.clone();

    vec![
        (
            SettingItem::TargetWpm,
            t!("settings.target_wpm").to_string(),
            format!("{}", app.config.target_wpm),
        ),
        (
            SettingItem::Theme,
            t!("settings.theme").to_string(),
            app.config.theme.clone(),
        ),
        (
            SettingItem::WordCount,
            t!("settings.word_count").to_string(),
            format!("{}", app.config.word_count),
        ),
        (
            SettingItem::UiLanguage,
            t!("settings.ui_language").to_string(),
            find_language_pack(&app.config.ui_language)
                .map(|pack| pack.autonym.to_string())
                .unwrap_or_else(|| app.config.ui_language.clone()),
        ),
        (
            SettingItem::DictionaryLanguage,
            t!("settings.dictionary_language").to_string(),
            dictionary_language_label,
        ),
        (
            SettingItem::KeyboardLayout,
            t!("settings.keyboard_layout").to_string(),
            keyboard_layout_label,
        ),
        (
            SettingItem::CodeLanguage,
            t!("settings.code_language").to_string(),
            app.config.code_language.clone(),
        ),
        (
            SettingItem::CodeDownloads,
            t!("settings.code_downloads").to_string(),
            if app.config.code_downloads_enabled {
                t!("settings.on").to_string()
            } else {
                t!("settings.off").to_string()
            },
        ),
        (
            SettingItem::CodeDownloadDir,
            t!("settings.code_download_dir").to_string(),
            app.config.code_download_dir.clone(),
        ),
        (
            SettingItem::SnippetsPerRepo,
            t!("settings.snippets_per_repo").to_string(),
            if app.config.code_snippets_per_repo == 0 {
                t!("settings.unlimited").to_string()
            } else {
                format!("{}", app.config.code_snippets_per_repo)
            },
        ),
        (
            SettingItem::DownloadCodeNow,
            t!("settings.download_code_now").to_string(),
            t!("settings.run_downloader").to_string(),
        ),
        (
            SettingItem::PassageDownloads,
            t!("settings.passage_downloads").to_string(),
            if app.config.passage_downloads_enabled {
                t!("settings.on").to_string()
            } else {
                t!("settings.off").to_string()
            },
        ),
        (
            SettingItem::PassageDownloadDir,
            t!("settings.passage_download_dir").to_string(),
            app.config.passage_download_dir.clone(),
        ),
        (
            SettingItem::ParagraphsPerBook,
            t!("settings.paragraphs_per_book").to_string(),
            if app.config.passage_paragraphs_per_book == 0 {
                t!("settings.whole_book").to_string()
            } else {
                format!("{}", app.config.passage_paragraphs_per_book)
            },
        ),
        (
            SettingItem::DownloadPassagesNow,
            t!("settings.download_passages_now").to_string(),
            t!("settings.run_downloader").to_string(),
        ),
        (
            SettingItem::ExportPath,
            t!("settings.export_path").to_string(),
            app.settings_export_path.clone(),
        ),
        (
            SettingItem::ExportData,
            t!("settings.export_data").to_string(),
            t!("settings.export_now").to_string(),
        ),
        (
            SettingItem::ImportPath,
            t!("settings.import_path").to_string(),
            app.settings_import_path.clone(),
        ),
        (
            SettingItem::ImportData,
            t!("settings.import_data").to_string(),
            t!("settings.import_now").to_string(),
        ),
    ]
}

fn handle_settings_mouse(app: &mut App, mouse: MouseEvent) {
    let is_click = matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
    );
    let is_secondary = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right));

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.settings_selected = app.settings_selected.saturating_sub(1);
            return;
        }
        MouseEventKind::ScrollDown => {
            let max_settings = SettingItem::ALL.len().saturating_sub(1);
            app.settings_selected = (app.settings_selected + 1).min(max_settings);
            return;
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {}
        _ => return,
    }

    if app.settings_status_message.is_some() {
        app.settings_status_message = None;
        return;
    }

    if app.settings_export_conflict {
        let area = terminal_area();
        let dialog_width = 52u16.min(area.width.saturating_sub(4));
        let dialog_height = 6u16;
        let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
        let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
        let dialog = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);
        if point_in_rect(mouse.column, mouse.row, dialog) {
            let third = dialog.width / 3;
            if mouse.column < dialog.x + third {
                app.settings_export_conflict = false;
                app.export_data_overwrite();
            } else if mouse.column < dialog.x + 2 * third {
                app.settings_export_conflict = false;
                app.export_data_rename();
            } else {
                app.settings_export_conflict = false;
            }
        }
        return;
    }

    if app.settings_confirm_import {
        let area = terminal_area();
        let dialog_width = 52u16.min(area.width.saturating_sub(4));
        let dialog_height = 7u16;
        let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
        let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
        let dialog = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);
        if point_in_rect(mouse.column, mouse.row, dialog) {
            if mouse.column < dialog.x + dialog.width / 2 {
                app.settings_confirm_import = false;
                app.import_data();
                i18n::set_ui_locale(&app.config.ui_language);
            } else {
                app.settings_confirm_import = false;
            }
        }
        return;
    }

    let area = terminal_area();
    let centered = ui::layout::centered_rect(60, 80, area);
    let inner = Block::bordered().inner(centered);
    let fields = settings_fields(app);
    let header_height = if inner.height > 0 { 1 } else { 0 };
    let sfh_save = ui::hint::hint(ui::hint::K_ESC, t!("settings.hint_save_back").as_ref());
    let sfh_change = ui::hint::hint(ui::hint::K_ENTER_ARROWS, t!("settings.hint_change_value").as_ref());
    let sfh_edit = ui::hint::hint(ui::hint::K_ENTER_ON_PATH, t!("settings.hint_edit_path").as_ref());
    let footer_hints: Vec<&str> = vec![
        sfh_save.as_str(),
        sfh_change.as_str(),
        sfh_edit.as_str(),
    ];
    let footer_height = if inner.height > header_height {
        pack_hint_lines(&footer_hints, inner.width as usize)
            .len()
            .max(1) as u16
    } else {
        0
    };
    let field_height = inner.height.saturating_sub(header_height + footer_height);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(field_height),
            Constraint::Length(footer_height),
        ])
        .split(inner);

    if is_click && layout[2].height > 0 {
        let efh_move = ui::hint::hint(ui::hint::K_ARROW_LR, t!("settings.hint_move").as_ref());
        let efh_tab = ui::hint::hint(ui::hint::K_TAB, t!("settings.hint_tab_complete").as_ref());
        let efh_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("settings.hint_confirm").as_ref());
        let efh_cancel = ui::hint::hint(ui::hint::K_ESC, t!("settings.hint_cancel").as_ref());
        let footer_hints: Vec<&str> = if app.settings_editing_path.is_some() {
            vec![
                efh_move.as_str(),
                efh_tab.as_str(),
                efh_confirm.as_str(),
                efh_cancel.as_str(),
            ]
        } else {
            vec![
                sfh_save.as_str(),
                sfh_change.as_str(),
                sfh_edit.as_str(),
            ]
        };
        if let Some(token) = hint_token_at(layout[2], &footer_hints, mouse.column, mouse.row) {
            if app.settings_editing_path.is_some() {
                match token.as_str() {
                    "←→" => {
                        let code = if is_secondary {
                            KeyCode::Left
                        } else {
                            KeyCode::Right
                        };
                        handle_settings_key(app, KeyEvent::new(code, KeyModifiers::NONE));
                    }
                    "Tab" => handle_settings_key(
                        app,
                        KeyEvent::new(
                            if is_secondary {
                                KeyCode::BackTab
                            } else {
                                KeyCode::Tab
                            },
                            KeyModifiers::NONE,
                        ),
                    ),
                    "Enter" => {
                        handle_settings_key(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                    }
                    "ESC" => {
                        handle_settings_key(app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
                    }
                    _ => {}
                }
            } else {
                match token.as_str() {
                    "ESC" => {
                        handle_settings_key(app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
                    }
                    "Enter/arrows" => {
                        let code = if is_secondary {
                            KeyCode::Right
                        } else {
                            KeyCode::Enter
                        };
                        handle_settings_key(app, KeyEvent::new(code, KeyModifiers::NONE));
                    }
                    "Enter on path" => {
                        handle_settings_key(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                    }
                    _ => {}
                }
            }
            return;
        }
    }

    if app.settings_editing_path.is_some() {
        return;
    }

    let row_height = 2u16;
    let visible_rows = (layout[1].height / row_height).max(1) as usize;
    let max_start = fields.len().saturating_sub(visible_rows);
    let start = app
        .settings_selected
        .saturating_sub(visible_rows.saturating_sub(1))
        .min(max_start);
    let end = (start + visible_rows).min(fields.len());
    let visible_fields = &fields[start..end];
    let field_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            visible_fields
                .iter()
                .map(|_| Constraint::Length(row_height))
                .collect::<Vec<_>>(),
        )
        .split(layout[1]);

    for (row, _) in visible_fields.iter().enumerate() {
        let rect = field_layout[row];
        if point_in_rect(mouse.column, mouse.row, rect) {
            let idx = start + row;
            app.settings_selected = idx;
            let setting = SettingItem::from_index(idx);
            let is_button = setting.is_action_button();
            let is_path = setting.is_path_field();
            let value_row = mouse.row > rect.y;
            if is_button || is_path || value_row {
                activate_settings_selected(app);
            }
            break;
        }
    }
}

fn handle_menu_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('1') => {
            app.menu.selected = 0;
            app.start_global_adaptive_drill();
        }
        KeyCode::Char('2') => {
            app.menu.selected = 1;
            activate_menu_selected(app);
        }
        KeyCode::Char('3') => {
            app.menu.selected = 2;
            activate_menu_selected(app);
        }
        KeyCode::Char('t') => {
            app.menu.selected = 3;
            activate_menu_selected(app);
        }
        KeyCode::Char('b') => {
            app.menu.selected = 4;
            activate_menu_selected(app);
        }
        KeyCode::Char('s') => {
            app.menu.selected = 5;
            activate_menu_selected(app);
        }
        KeyCode::Char('c') => {
            app.menu.selected = 6;
            activate_menu_selected(app);
        }
        KeyCode::Up | KeyCode::Char('k') => app.menu.prev(),
        KeyCode::Down | KeyCode::Char('j') => app.menu.next(),
        KeyCode::Enter => activate_menu_selected(app),
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
    if app.history_confirm_delete {
        match key.code {
            KeyCode::Char('y') => {
                app.delete_session();
                app.history_confirm_delete = false;
                app.continue_drill();
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.history_confirm_delete = false;
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('c') | KeyCode::Enter | KeyCode::Char(' ') => app.continue_drill(),
        KeyCode::Char('r') => app.retry_drill(),
        KeyCode::Char('q') | KeyCode::Esc => app.go_to_menu(),
        KeyCode::Char('s') => app.go_to_stats(),
        KeyCode::Char('x') => {
            if !app.drill_history.is_empty() {
                // On result screen, delete always targets the just-completed (most recent) session.
                app.history_selected = 0;
                app.history_confirm_delete = true;
            }
        }
        _ => {}
    }
}

fn handle_stats_key(app: &mut App, key: KeyEvent) {
    const STATS_TAB_COUNT: usize = 6;

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
        let page_size = current_history_page_size();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.go_to_menu(),
            KeyCode::Char('j') | KeyCode::Down => {
                if !app.drill_history.is_empty() {
                    let max_idx = app.drill_history.len() - 1;
                    app.history_selected = (app.history_selected + 1).min(max_idx);
                    keep_history_selection_visible(app, page_size);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                app.history_selected = app.history_selected.saturating_sub(1);
                keep_history_selection_visible(app, page_size);
            }
            KeyCode::PageDown => {
                if !app.drill_history.is_empty() {
                    let max_idx = app.drill_history.len() - 1;
                    app.history_selected = (app.history_selected + page_size).min(max_idx);
                    keep_history_selection_visible(app, page_size);
                }
            }
            KeyCode::PageUp => {
                app.history_selected = app.history_selected.saturating_sub(page_size);
                keep_history_selection_visible(app, page_size);
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
            KeyCode::Char('6') => app.stats_tab = 5,
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
        KeyCode::Char('6') => app.stats_tab = 5,
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

fn activate_settings_selected(app: &mut App) {
    match SettingItem::from_index(app.settings_selected) {
        SettingItem::UiLanguage => app.go_to_ui_language_select(),
        SettingItem::DictionaryLanguage => app.go_to_dictionary_language_select(),
        SettingItem::KeyboardLayout => app.go_to_keyboard_layout_select(),
        SettingItem::CodeDownloadDir => {
            app.clear_settings_modals();
            app.settings_editing_path = Some((
                PathField::CodeDownloadDir,
                LineInput::new(&app.config.code_download_dir),
            ));
        }
        SettingItem::PassageDownloadDir => {
            app.clear_settings_modals();
            app.settings_editing_path = Some((
                PathField::PassageDownloadDir,
                LineInput::new(&app.config.passage_download_dir),
            ));
        }
        SettingItem::DownloadCodeNow => app.start_code_downloads_from_settings(),
        SettingItem::DownloadPassagesNow => app.start_passage_downloads_from_settings(),
        SettingItem::ExportPath => {
            app.clear_settings_modals();
            app.settings_editing_path = Some((
                PathField::ExportPath,
                LineInput::new(&app.settings_export_path),
            ));
        }
        SettingItem::ExportData => {
            app.export_data();
        }
        SettingItem::ImportPath => {
            app.clear_settings_modals();
            app.settings_editing_path = Some((
                PathField::ImportPath,
                LineInput::new(&app.settings_import_path),
            ));
        }
        SettingItem::ImportData => {
            app.clear_settings_modals();
            app.settings_confirm_import = true;
        }
        _ => app.settings_cycle_forward(),
    }
}

fn handle_settings_key(app: &mut App, key: KeyEvent) {
    let max_settings: usize = SettingItem::ALL.len().saturating_sub(1);

    // Priority 1: dismiss status message
    if app.settings_status_message.is_some() {
        app.settings_status_message = None;
        return;
    }

    // Priority 2: export conflict dialog
    if app.settings_export_conflict {
        match key.code {
            KeyCode::Char('d') => {
                app.settings_export_conflict = false;
                app.export_data_overwrite();
            }
            KeyCode::Char('r') => {
                app.settings_export_conflict = false;
                app.export_data_rename();
            }
            KeyCode::Esc => {
                app.settings_export_conflict = false;
            }
            _ => {}
        }
        return;
    }

    // Priority 3: import confirmation dialog
    if app.settings_confirm_import {
        match key.code {
            KeyCode::Char('y') => {
                app.settings_confirm_import = false;
                app.import_data();
                i18n::set_ui_locale(&app.config.ui_language);
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.settings_confirm_import = false;
            }
            _ => {}
        }
        return;
    }

    // Priority 4: editing a path field
    if let Some((field, ref mut input)) = app.settings_editing_path {
        match input.handle(key) {
            InputResult::Submit => {
                let value = input.value().to_string();
                match field {
                    PathField::CodeDownloadDir => app.config.code_download_dir = value,
                    PathField::PassageDownloadDir => app.config.passage_download_dir = value,
                    PathField::ExportPath => app.settings_export_path = value,
                    PathField::ImportPath => app.settings_import_path = value,
                }
                app.settings_editing_path = None;
            }
            InputResult::Cancel => {
                app.settings_editing_path = None;
            }
            InputResult::Continue => {}
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
            if app.settings_selected < max_settings {
                app.settings_selected += 1;
            }
        }
        KeyCode::Enter => activate_settings_selected(app),
        KeyCode::Right | KeyCode::Char('l') => {
            if SettingItem::from_index(app.settings_selected).supports_left_right_cycle() {
                app.settings_cycle_forward();
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if SettingItem::from_index(app.settings_selected).supports_left_right_cycle() {
                app.settings_cycle_backward();
            }
        }
        _ => {}
    }
}

fn is_dictionary_language_disabled(_app: &App, language_key: &str) -> bool {
    find_language_pack(language_key).is_none()
}

fn dictionary_language_list_area(area: Rect) -> Rect {
    let centered = ui::layout::centered_rect(60, 70, area);
    let inner = Block::bordered().inner(centered);
    let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN_PGUP_PGDN, t!("select.hint_navigate").as_ref());
    let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
    let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
    let hint_lines = pack_hint_lines(
        &[h_nav.as_str(), h_confirm.as_str(), h_back.as_str()],
        inner.width as usize,
    );
    let footer_height = (hint_lines.len() as u16).max(1);
    if inner.height > footer_height {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner)[0]
    } else {
        inner
    }
}

fn confirm_dictionary_language_selection(app: &mut App) {
    let options = language_packs();
    if app.dictionary_language_selected >= options.len() {
        return;
    }
    let selected = options[app.dictionary_language_selected];
    if is_dictionary_language_disabled(app, selected.language_key) {
        return;
    }
    match app.set_dictionary_language(selected.language_key) {
        Ok(CapabilityState::Enabled | CapabilityState::Disabled) => {
            app.settings_status_message = Some(app::StatusMessage {
                kind: StatusKind::Success,
                text: format!(
                    "Switched to {}. Keyboard layout reset to {}",
                    selected.display_name, app.config.keyboard_layout
                ),
            });
        }
        Err(err) => {
            app.settings_status_message = Some(app::StatusMessage {
                kind: StatusKind::Error,
                text: err.to_string(),
            });
        }
    }
    app.go_to_settings();
    app.settings_selected = SettingItem::DictionaryLanguage.index();
}

fn handle_dictionary_language_key(app: &mut App, key: KeyEvent) {
    let options = language_packs();
    let len = options.len();
    if len == 0 {
        return;
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.go_to_settings();
            app.settings_selected = SettingItem::DictionaryLanguage.index();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.dictionary_language_selected = app.dictionary_language_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.dictionary_language_selected + 1 < len {
                app.dictionary_language_selected += 1;
            }
        }
        KeyCode::PageUp => {
            app.dictionary_language_selected = app.dictionary_language_selected.saturating_sub(10);
        }
        KeyCode::PageDown => {
            app.dictionary_language_selected = (app.dictionary_language_selected + 10).min(len - 1);
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.dictionary_language_selected = 0;
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.dictionary_language_selected = len - 1;
        }
        KeyCode::Enter => confirm_dictionary_language_selection(app),
        KeyCode::Char(ch) if ('1'..='9').contains(&ch) => {
            let idx = (ch as usize) - ('1' as usize);
            if idx < len {
                app.dictionary_language_selected = idx;
                confirm_dictionary_language_selection(app);
                return;
            }
        }
        _ => {}
    }

    let viewport = 15usize;
    if app.dictionary_language_selected < app.dictionary_language_scroll {
        app.dictionary_language_scroll = app.dictionary_language_selected;
    } else if app.dictionary_language_selected >= app.dictionary_language_scroll + viewport {
        app.dictionary_language_scroll = app.dictionary_language_selected + 1 - viewport;
    }
}

fn handle_dictionary_language_mouse(app: &mut App, mouse: MouseEvent) {
    let options = language_packs();
    if options.is_empty() {
        return;
    }
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.dictionary_language_selected = app.dictionary_language_selected.saturating_sub(1);
        }
        MouseEventKind::ScrollDown => {
            if app.dictionary_language_selected + 1 < options.len() {
                app.dictionary_language_selected += 1;
            }
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
            let area = terminal_area();
            let centered = ui::layout::centered_rect(60, 70, area);
            let inner = Block::bordered().inner(centered);
            let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN_PGUP_PGDN, t!("select.hint_navigate").as_ref());
            let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
            let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
            let hints: Vec<&str> = vec![h_nav.as_str(), h_confirm.as_str(), h_back.as_str()];
            let footer_h = pack_hint_lines(&hints, inner.width as usize).len().max(1) as u16;
            let chunks = if inner.height > footer_h {
                Some(
                    Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(footer_h)])
                        .split(inner),
                )
            } else {
                None
            };
            if let Some(chunks) = &chunks
                && let Some(token) = hint_token_at(chunks[1], &hints, mouse.column, mouse.row)
            {
                match token.as_str() {
                    "Enter" => confirm_dictionary_language_selection(app),
                    "q/ESC" => {
                        app.go_to_settings();
                        app.settings_selected = SettingItem::DictionaryLanguage.index();
                    }
                    _ => {}
                }
                return;
            }
            let list_area = dictionary_language_list_area(area);
            if !point_in_rect(mouse.column, mouse.row, list_area) {
                return;
            }
            let viewport_height = (list_area.height as usize).saturating_sub(2).max(1);
            let scroll = app.dictionary_language_scroll;
            let visible_end = (scroll + viewport_height).min(options.len());
            let line_offset = (mouse.row - list_area.y) as usize;
            if line_offset == 0 {
                return;
            }
            let idx = scroll + line_offset - 1;
            if idx < visible_end {
                let was_selected = idx == app.dictionary_language_selected;
                app.dictionary_language_selected = idx;
                if was_selected {
                    confirm_dictionary_language_selection(app);
                    return;
                }
            }
        }
        _ => {}
    }
    let viewport = 15usize;
    if app.dictionary_language_selected < app.dictionary_language_scroll {
        app.dictionary_language_scroll = app.dictionary_language_selected;
    } else if app.dictionary_language_selected >= app.dictionary_language_scroll + viewport {
        app.dictionary_language_scroll = app.dictionary_language_selected + 1 - viewport;
    }
}

// --- UI Language Select ---

fn confirm_ui_language_selection(app: &mut App) {
    let locales = i18n::SUPPORTED_UI_LOCALES;
    if app.ui_language_selected >= locales.len() {
        return;
    }
    let selected = locales[app.ui_language_selected];
    app.config.ui_language = selected.to_string();
    i18n::set_ui_locale(selected);
    let _ = app.config.save();
    app.go_to_settings();
    app.settings_selected = SettingItem::UiLanguage.index();
}

fn handle_ui_language_key(app: &mut App, key: KeyEvent) {
    let locales = i18n::SUPPORTED_UI_LOCALES;
    let len = locales.len();
    if len == 0 {
        return;
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.go_to_settings();
            app.settings_selected = SettingItem::UiLanguage.index();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.ui_language_selected = app.ui_language_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.ui_language_selected + 1 < len {
                app.ui_language_selected += 1;
            }
        }
        KeyCode::Enter => confirm_ui_language_selection(app),
        _ => {}
    }
}

fn handle_ui_language_mouse(app: &mut App, mouse: MouseEvent) {
    let locales = i18n::SUPPORTED_UI_LOCALES;
    if locales.is_empty() {
        return;
    }
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.ui_language_selected = app.ui_language_selected.saturating_sub(1);
        }
        MouseEventKind::ScrollDown => {
            if app.ui_language_selected + 1 < locales.len() {
                app.ui_language_selected += 1;
            }
        }
        _ => {}
    }
}

// --- Keyboard Layout ---

fn is_keyboard_layout_disabled(layout_key: &str) -> bool {
    dictionary_languages_for_layout(layout_key).is_empty()
}

fn keyboard_layout_list_area(area: Rect) -> Rect {
    let centered = ui::layout::centered_rect(60, 70, area);
    let inner = Block::bordered().inner(centered);
    let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN_PGUP_PGDN, t!("select.hint_navigate").as_ref());
    let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
    let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
    let hint_lines = pack_hint_lines(
        &[h_nav.as_str(), h_confirm.as_str(), h_back.as_str()],
        inner.width as usize,
    );
    let footer_height = (hint_lines.len() as u16).max(1);
    if inner.height > footer_height {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner)[0]
    } else {
        inner
    }
}

fn confirm_keyboard_layout_selection(app: &mut App) {
    let options = keyboard::model::KeyboardModel::supported_layout_keys();
    if app.keyboard_layout_selected >= options.len() {
        return;
    }
    let selected = options[app.keyboard_layout_selected];
    if is_keyboard_layout_disabled(selected) {
        return;
    }
    match app.set_keyboard_layout(selected) {
        Ok(CapabilityState::Enabled | CapabilityState::Disabled) => {}
        Err(err) => {
            app.settings_status_message = Some(app::StatusMessage {
                kind: StatusKind::Error,
                text: err.to_string(),
            });
        }
    }
    app.go_to_settings();
    app.settings_selected = SettingItem::KeyboardLayout.index();
}

fn handle_keyboard_layout_key(app: &mut App, key: KeyEvent) {
    let options = keyboard::model::KeyboardModel::supported_layout_keys();
    let len = options.len();
    if len == 0 {
        return;
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.go_to_settings();
            app.settings_selected = SettingItem::KeyboardLayout.index();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.keyboard_layout_selected = app.keyboard_layout_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.keyboard_layout_selected + 1 < len {
                app.keyboard_layout_selected += 1;
            }
        }
        KeyCode::PageUp => {
            app.keyboard_layout_selected = app.keyboard_layout_selected.saturating_sub(10);
        }
        KeyCode::PageDown => {
            app.keyboard_layout_selected = (app.keyboard_layout_selected + 10).min(len - 1);
        }
        KeyCode::Home | KeyCode::Char('g') => app.keyboard_layout_selected = 0,
        KeyCode::End | KeyCode::Char('G') => app.keyboard_layout_selected = len - 1,
        KeyCode::Enter => confirm_keyboard_layout_selection(app),
        KeyCode::Char(ch) if ('1'..='9').contains(&ch) => {
            let idx = (ch as usize) - ('1' as usize);
            if idx < len {
                app.keyboard_layout_selected = idx;
                confirm_keyboard_layout_selection(app);
                return;
            }
        }
        _ => {}
    }
    let viewport = 15usize;
    if app.keyboard_layout_selected < app.keyboard_layout_scroll {
        app.keyboard_layout_scroll = app.keyboard_layout_selected;
    } else if app.keyboard_layout_selected >= app.keyboard_layout_scroll + viewport {
        app.keyboard_layout_scroll = app.keyboard_layout_selected + 1 - viewport;
    }
}

fn handle_keyboard_layout_mouse(app: &mut App, mouse: MouseEvent) {
    let options = keyboard::model::KeyboardModel::supported_layout_keys();
    if options.is_empty() {
        return;
    }
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.keyboard_layout_selected = app.keyboard_layout_selected.saturating_sub(1);
        }
        MouseEventKind::ScrollDown => {
            if app.keyboard_layout_selected + 1 < options.len() {
                app.keyboard_layout_selected += 1;
            }
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
            let area = terminal_area();
            let centered = ui::layout::centered_rect(60, 70, area);
            let inner = Block::bordered().inner(centered);
            let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN_PGUP_PGDN, t!("select.hint_navigate").as_ref());
            let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
            let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
            let hints: Vec<&str> = vec![h_nav.as_str(), h_confirm.as_str(), h_back.as_str()];
            let footer_h = pack_hint_lines(&hints, inner.width as usize).len().max(1) as u16;
            let chunks = if inner.height > footer_h {
                Some(
                    Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(footer_h)])
                        .split(inner),
                )
            } else {
                None
            };
            if let Some(chunks) = &chunks
                && let Some(token) = hint_token_at(chunks[1], &hints, mouse.column, mouse.row)
            {
                match token.as_str() {
                    "Enter" => confirm_keyboard_layout_selection(app),
                    "q/ESC" => {
                        app.go_to_settings();
                        app.settings_selected = SettingItem::KeyboardLayout.index();
                    }
                    _ => {}
                }
                return;
            }
            let list_area = keyboard_layout_list_area(area);
            if !point_in_rect(mouse.column, mouse.row, list_area) {
                return;
            }
            let viewport_height = (list_area.height as usize).saturating_sub(2).max(1);
            let scroll = app.keyboard_layout_scroll;
            let visible_end = (scroll + viewport_height).min(options.len());
            let line_offset = (mouse.row - list_area.y) as usize;
            if line_offset == 0 {
                return;
            }
            let idx = scroll + line_offset - 1;
            if idx < visible_end {
                let was_selected = idx == app.keyboard_layout_selected;
                app.keyboard_layout_selected = idx;
                if was_selected {
                    confirm_keyboard_layout_selection(app);
                    return;
                }
            }
        }
        _ => {}
    }
    let viewport = 15usize;
    if app.keyboard_layout_selected < app.keyboard_layout_scroll {
        app.keyboard_layout_scroll = app.keyboard_layout_selected;
    } else if app.keyboard_layout_selected >= app.keyboard_layout_scroll + viewport {
        app.keyboard_layout_scroll = app.keyboard_layout_selected + 1 - viewport;
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

fn code_language_list_area(app: &App, area: Rect) -> Rect {
    let centered = ui::layout::centered_rect(50, 70, area);
    let inner = Block::bordered().inner(centered);
    let options = code_language_options();
    let width = inner.width as usize;
    let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN_PGUP_PGDN, t!("select.hint_navigate").as_ref());
    let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
    let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
    let hint_lines = pack_hint_lines(
        &[h_nav.as_str(), h_confirm.as_str(), h_back.as_str()],
        width,
    );
    let disabled_notice_t = t!("select.disabled_network_notice");
    let disabled_notice = disabled_notice_t.as_ref();
    let has_disabled = !app.config.code_downloads_enabled
        && options
            .iter()
            .any(|(key, _)| is_code_language_disabled(app, key));
    let notice_lines = wrapped_line_count(disabled_notice, width);
    let total_height = inner.height as usize;
    let show_notice = has_disabled && total_height >= hint_lines.len() + notice_lines + 3;
    let desired_footer_height = hint_lines.len() + if show_notice { notice_lines } else { 0 };
    let footer_height = desired_footer_height.min(total_height.saturating_sub(1)) as u16;
    if footer_height > 0 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner)[0]
    } else {
        inner
    }
}

fn handle_code_language_mouse(app: &mut App, mouse: MouseEvent) {
    let options = code_language_options();
    let len = options.len();
    if len == 0 {
        return;
    }
    let is_secondary = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right));
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.code_language_selected = app.code_language_selected.saturating_sub(1);
        }
        MouseEventKind::ScrollDown => {
            if app.code_language_selected + 1 < len {
                app.code_language_selected += 1;
            }
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
            let centered = ui::layout::centered_rect(50, 70, terminal_area());
            let inner = Block::bordered().inner(centered);
            let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN_PGUP_PGDN, t!("select.hint_navigate").as_ref());
            let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
            let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
            let hints: Vec<&str> = vec![h_nav.as_str(), h_confirm.as_str(), h_back.as_str()];
            let hint_lines = pack_hint_lines(&hints, inner.width as usize);
            let disabled_notice_t = t!("select.disabled_network_notice");
            let disabled_notice = disabled_notice_t.as_ref();
            let has_disabled = !app.config.code_downloads_enabled
                && options
                    .iter()
                    .any(|(key, _)| is_code_language_disabled(app, key));
            let notice_lines = wrapped_line_count(disabled_notice, inner.width as usize);
            let total_height = inner.height as usize;
            let show_notice = has_disabled && total_height >= hint_lines.len() + notice_lines + 3;
            let desired_footer_height =
                hint_lines.len() + if show_notice { notice_lines } else { 0 };
            let footer_height = desired_footer_height.min(total_height.saturating_sub(1)) as u16;
            if footer_height > 0 {
                let footer_area = Rect::new(
                    inner.x,
                    inner.y + inner.height.saturating_sub(footer_height),
                    inner.width,
                    footer_height,
                );
                if let Some(token) = hint_token_at(footer_area, &hints, mouse.column, mouse.row) {
                    match token.as_str() {
                        "Up/Down/PgUp/PgDn" => {
                            if is_secondary {
                                app.code_language_selected =
                                    app.code_language_selected.saturating_sub(1);
                            } else if app.code_language_selected + 1 < len {
                                app.code_language_selected += 1;
                            }
                        }
                        "Enter" => {
                            handle_code_language_key(
                                app,
                                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                            );
                        }
                        "q/ESC" => app.go_to_menu(),
                        _ => {}
                    }
                    return;
                }
            }
            let list_area = code_language_list_area(app, terminal_area());
            if !point_in_rect(mouse.column, mouse.row, list_area) {
                return;
            }
            let viewport_height = (list_area.height as usize).saturating_sub(2).max(1);
            let scroll = app.code_language_scroll;
            let visible_end = (scroll + viewport_height).min(len);
            let line_offset = (mouse.row - list_area.y) as usize;
            if line_offset == 0 {
                return;
            }
            let idx = scroll + line_offset - 1;
            if idx < visible_end {
                let selected_before = app.code_language_selected;
                app.code_language_selected = idx;
                let key = options[idx].0;
                if selected_before == idx && !is_code_language_disabled(app, key) {
                    confirm_code_language_and_continue(app, &options);
                    return;
                }
            }
        }
        _ => {}
    }

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

fn passage_book_list_area(app: &App, area: Rect) -> Rect {
    let centered = ui::layout::centered_rect(60, 70, area);
    let inner = Block::bordered().inner(centered);
    let options = passage_options();
    let width = inner.width as usize;
    let h_nav_t = ui::hint::hint(ui::hint::K_UP_DOWN, t!("select.hint_navigate").as_ref());
    let h_confirm_t = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
    let h_back_t = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
    let hint_lines = pack_hint_lines(
        &[h_nav_t.as_str(), h_confirm_t.as_str(), h_back_t.as_str()],
        width,
    );
    let disabled_notice_t = t!("select.disabled_sources_notice");
    let disabled_notice = disabled_notice_t.as_ref();
    let has_disabled = !app.config.passage_downloads_enabled
        && options
            .iter()
            .any(|(key, _)| is_passage_option_disabled(app, key));
    let notice_lines = wrapped_line_count(disabled_notice, width);
    let total_height = inner.height as usize;
    let show_notice = has_disabled && total_height >= hint_lines.len() + notice_lines + 3;
    let desired_footer_height = hint_lines.len() + if show_notice { notice_lines } else { 0 };
    let footer_height = desired_footer_height.min(total_height.saturating_sub(1)) as u16;
    if footer_height > 0 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner)[0]
    } else {
        inner
    }
}

fn handle_passage_book_mouse(app: &mut App, mouse: MouseEvent) {
    let options = passage_options();
    if options.is_empty() {
        return;
    }
    let is_secondary = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right));
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.passage_book_selected = app.passage_book_selected.saturating_sub(1);
        }
        MouseEventKind::ScrollDown => {
            if app.passage_book_selected + 1 < options.len() {
                app.passage_book_selected += 1;
            }
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
            let centered = ui::layout::centered_rect(60, 70, terminal_area());
            let inner = Block::bordered().inner(centered);
            let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN, t!("select.hint_navigate").as_ref());
            let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
            let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
            let hints: Vec<&str> = vec![h_nav.as_str(), h_confirm.as_str(), h_back.as_str()];
            let hint_lines = pack_hint_lines(&hints, inner.width as usize);
            let disabled_notice_t = t!("select.disabled_sources_notice");
            let disabled_notice = disabled_notice_t.as_ref();
            let has_disabled = !app.config.passage_downloads_enabled
                && options
                    .iter()
                    .any(|(key, _)| is_passage_option_disabled(app, key));
            let notice_lines = wrapped_line_count(disabled_notice, inner.width as usize);
            let total_height = inner.height as usize;
            let show_notice = has_disabled && total_height >= hint_lines.len() + notice_lines + 3;
            let desired_footer_height =
                hint_lines.len() + if show_notice { notice_lines } else { 0 };
            let footer_height = desired_footer_height.min(total_height.saturating_sub(1)) as u16;
            if footer_height > 0 {
                let footer_area = Rect::new(
                    inner.x,
                    inner.y + inner.height.saturating_sub(footer_height),
                    inner.width,
                    footer_height,
                );
                if let Some(token) = hint_token_at(footer_area, &hints, mouse.column, mouse.row) {
                    match token.as_str() {
                        "Up/Down" => {
                            if is_secondary {
                                app.passage_book_selected =
                                    app.passage_book_selected.saturating_sub(1);
                            } else if app.passage_book_selected + 1 < options.len() {
                                app.passage_book_selected += 1;
                            }
                        }
                        "Enter" => handle_passage_book_key(
                            app,
                            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                        ),
                        "q/ESC" => app.go_to_menu(),
                        _ => {}
                    }
                    return;
                }
            }
            let list_area = passage_book_list_area(app, terminal_area());
            if !point_in_rect(mouse.column, mouse.row, list_area) {
                return;
            }
            let viewport_height = list_area.height as usize;
            let start = app
                .passage_book_selected
                .saturating_sub(viewport_height.saturating_sub(1));
            let row = (mouse.row - list_area.y) as usize;
            let idx = start + row;
            if idx < options.len() {
                let selected_before = app.passage_book_selected;
                app.passage_book_selected = idx;
                let key = options[idx].0;
                if selected_before == idx && !is_passage_option_disabled(app, key) {
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

fn intro_field_at_row(base_y: u16, y: u16) -> Option<(usize, bool)> {
    if y < base_y {
        return None;
    }
    let rel = y - base_y;
    let field = (rel / 3) as usize;
    if field >= 4 {
        return None;
    }
    let value_row = rel % 3 == 1;
    Some((field, value_row))
}

fn passage_intro_content_area(area: Rect) -> Rect {
    let centered = ui::layout::centered_rect(75, 80, area);
    let inner = Block::bordered().inner(centered);
    let ih_nav = ui::hint::hint(ui::hint::K_UP_DOWN, t!("intro.hint_navigate").as_ref());
    let ih_adj = ui::hint::hint(ui::hint::K_LEFT_RIGHT, t!("intro.hint_adjust").as_ref());
    let ih_edit = ui::hint::hint(ui::hint::K_TYPE_BACKSPACE, t!("intro.hint_edit").as_ref());
    let ih_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("intro.hint_confirm").as_ref());
    let ih_cancel = ui::hint::hint(ui::hint::K_Q_ESC, t!("intro.hint_cancel").as_ref());
    let hint_lines = pack_hint_lines(
        &[ih_nav.as_str(), ih_adj.as_str(), ih_edit.as_str(), ih_confirm.as_str(), ih_cancel.as_str()],
        inner.width as usize,
    );
    let footer_height = (hint_lines.len() + 1) as u16;
    if footer_height > 0 && footer_height < inner.height {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner)[0]
    } else {
        inner
    }
}

fn handle_passage_intro_mouse(app: &mut App, mouse: MouseEvent) {
    if app.passage_intro_downloading {
        return;
    }
    let is_secondary = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right));
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.passage_intro_selected = app.passage_intro_selected.saturating_sub(1);
        }
        MouseEventKind::ScrollDown => {
            app.passage_intro_selected = (app.passage_intro_selected + 1).min(3);
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
            let centered = ui::layout::centered_rect(75, 80, terminal_area());
            let inner = Block::bordered().inner(centered);
            let ih_nav = ui::hint::hint(ui::hint::K_UP_DOWN, t!("intro.hint_navigate").as_ref());
            let ih_adj = ui::hint::hint(ui::hint::K_LEFT_RIGHT, t!("intro.hint_adjust").as_ref());
            let ih_edit = ui::hint::hint(ui::hint::K_TYPE_BACKSPACE, t!("intro.hint_edit").as_ref());
            let ih_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("intro.hint_confirm").as_ref());
            let ih_cancel = ui::hint::hint(ui::hint::K_Q_ESC, t!("intro.hint_cancel").as_ref());
            let hints: Vec<&str> = vec![ih_nav.as_str(), ih_adj.as_str(), ih_edit.as_str(), ih_confirm.as_str(), ih_cancel.as_str()];
            let hint_lines = pack_hint_lines(&hints, inner.width as usize);
            let footer_height = (hint_lines.len() + 1) as u16;
            if footer_height > 0 && footer_height < inner.height {
                let footer_area = Rect::new(
                    inner.x,
                    inner.y + inner.height.saturating_sub(footer_height),
                    inner.width,
                    footer_height,
                );
                if let Some(token) = hint_token_at(footer_area, &hints, mouse.column, mouse.row) {
                    match token.as_str() {
                        "Up/Down" => {
                            if is_secondary {
                                app.passage_intro_selected =
                                    app.passage_intro_selected.saturating_sub(1);
                            } else {
                                app.passage_intro_selected =
                                    (app.passage_intro_selected + 1).min(3);
                            }
                        }
                        "Left/Right" => {
                            handle_passage_intro_key(
                                app,
                                KeyEvent::new(
                                    if is_secondary {
                                        KeyCode::Right
                                    } else {
                                        KeyCode::Left
                                    },
                                    KeyModifiers::NONE,
                                ),
                            );
                        }
                        "Type/Backspace" => {
                            if is_secondary {
                                handle_passage_intro_key(
                                    app,
                                    KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
                                );
                            }
                        }
                        "Enter" => handle_passage_intro_key(
                            app,
                            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                        ),
                        "q/ESC" => app.go_to_menu(),
                        _ => {}
                    }
                    return;
                }
            }
            let content = passage_intro_content_area(terminal_area());
            if !point_in_rect(mouse.column, mouse.row, content) {
                return;
            }
            let base_y = content.y.saturating_add(4);
            if let Some((field, value_row)) = intro_field_at_row(base_y, mouse.row) {
                let was_selected = app.passage_intro_selected == field;
                app.passage_intro_selected = field;
                if field == 0 && (value_row || was_selected) {
                    app.passage_intro_downloads_enabled = !app.passage_intro_downloads_enabled;
                } else if field == 3 && (value_row || was_selected) {
                    handle_passage_intro_key(
                        app,
                        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                    );
                }
            }
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

fn handle_passage_download_progress_mouse(app: &mut App, mouse: MouseEvent) {
    if matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
    ) {
        app.go_to_menu();
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

fn code_intro_content_area(area: Rect) -> Rect {
    let centered = ui::layout::centered_rect(75, 80, area);
    let inner = Block::bordered().inner(centered);
    let ih_nav = ui::hint::hint(ui::hint::K_UP_DOWN, t!("intro.hint_navigate").as_ref());
    let ih_adj = ui::hint::hint(ui::hint::K_LEFT_RIGHT, t!("intro.hint_adjust").as_ref());
    let ih_edit = ui::hint::hint(ui::hint::K_TYPE_BACKSPACE, t!("intro.hint_edit").as_ref());
    let ih_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("intro.hint_confirm").as_ref());
    let ih_cancel = ui::hint::hint(ui::hint::K_Q_ESC, t!("intro.hint_cancel").as_ref());
    let hint_lines = pack_hint_lines(
        &[ih_nav.as_str(), ih_adj.as_str(), ih_edit.as_str(), ih_confirm.as_str(), ih_cancel.as_str()],
        inner.width as usize,
    );
    let footer_height = (hint_lines.len() + 1) as u16;
    if footer_height > 0 && footer_height < inner.height {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(footer_height)])
            .split(inner)[0]
    } else {
        inner
    }
}

fn handle_code_intro_mouse(app: &mut App, mouse: MouseEvent) {
    if app.code_intro_downloading {
        return;
    }
    let is_secondary = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right));
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.code_intro_selected = app.code_intro_selected.saturating_sub(1);
        }
        MouseEventKind::ScrollDown => {
            app.code_intro_selected = (app.code_intro_selected + 1).min(3);
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
            let centered = ui::layout::centered_rect(75, 80, terminal_area());
            let inner = Block::bordered().inner(centered);
            let ih_nav = ui::hint::hint(ui::hint::K_UP_DOWN, t!("intro.hint_navigate").as_ref());
            let ih_adj = ui::hint::hint(ui::hint::K_LEFT_RIGHT, t!("intro.hint_adjust").as_ref());
            let ih_edit = ui::hint::hint(ui::hint::K_TYPE_BACKSPACE, t!("intro.hint_edit").as_ref());
            let ih_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("intro.hint_confirm").as_ref());
            let ih_cancel = ui::hint::hint(ui::hint::K_Q_ESC, t!("intro.hint_cancel").as_ref());
            let hints: Vec<&str> = vec![ih_nav.as_str(), ih_adj.as_str(), ih_edit.as_str(), ih_confirm.as_str(), ih_cancel.as_str()];
            let hint_lines = pack_hint_lines(&hints, inner.width as usize);
            let footer_height = (hint_lines.len() + 1) as u16;
            if footer_height > 0 && footer_height < inner.height {
                let footer_area = Rect::new(
                    inner.x,
                    inner.y + inner.height.saturating_sub(footer_height),
                    inner.width,
                    footer_height,
                );
                if let Some(token) = hint_token_at(footer_area, &hints, mouse.column, mouse.row) {
                    match token.as_str() {
                        "Up/Down" => {
                            if is_secondary {
                                app.code_intro_selected = app.code_intro_selected.saturating_sub(1);
                            } else {
                                app.code_intro_selected = (app.code_intro_selected + 1).min(3);
                            }
                        }
                        "Left/Right" => {
                            handle_code_intro_key(
                                app,
                                KeyEvent::new(
                                    if is_secondary {
                                        KeyCode::Right
                                    } else {
                                        KeyCode::Left
                                    },
                                    KeyModifiers::NONE,
                                ),
                            );
                        }
                        "Type/Backspace" => {
                            if is_secondary {
                                handle_code_intro_key(
                                    app,
                                    KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
                                );
                            }
                        }
                        "Enter" => handle_code_intro_key(
                            app,
                            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                        ),
                        "q/ESC" => app.go_to_menu(),
                        _ => {}
                    }
                    return;
                }
            }
            let content = code_intro_content_area(terminal_area());
            if !point_in_rect(mouse.column, mouse.row, content) {
                return;
            }
            let base_y = content.y.saturating_add(4);
            if let Some((field, value_row)) = intro_field_at_row(base_y, mouse.row) {
                let was_selected = app.code_intro_selected == field;
                app.code_intro_selected = field;
                if field == 0 && (value_row || was_selected) {
                    app.code_intro_downloads_enabled = !app.code_intro_downloads_enabled;
                } else if field == 3 && (value_row || was_selected) {
                    handle_code_intro_key(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
            }
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

fn handle_code_download_progress_mouse(app: &mut App, mouse: MouseEvent) {
    if matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
    ) {
        app.cancel_code_download();
        app.go_to_menu();
    }
}

fn handle_skill_tree_key(app: &mut App, key: KeyEvent) {
    const DETAIL_SCROLL_STEP: usize = 10;
    if let Some(branch_id) = app.skill_tree_confirm_unlock {
        match key.code {
            KeyCode::Char('y') => {
                app.unlock_branch(branch_id);
                app.skill_tree_confirm_unlock = None;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                app.skill_tree_confirm_unlock = None;
            }
            _ => {}
        }
        return;
    }

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
                if status == engine::skill_tree::BranchStatus::Available {
                    app.skill_tree_confirm_unlock = Some(branch_id);
                } else if status == engine::skill_tree::BranchStatus::InProgress {
                    app.start_branch_drill(branch_id);
                }
            }
        }
        _ => {}
    }
}

struct SkillTreeMouseLayout {
    branch_area: Rect,
    detail_area: Rect,
    inter_branch_spacing: bool,
    separator_padding: bool,
}

fn locked_branch_notice(app: &App) -> String {
    t!("skill_tree.locked_notice", count = app.skill_tree.primary_letters().len()).to_string()
}

fn skill_tree_interactive_areas(app: &App, area: Rect) -> SkillTreeMouseLayout {
    let centered = skill_tree_popup_rect(area);
    let inner = Block::bordered().inner(centered);
    let branches = selectable_branches();
    let selected = app
        .skill_tree_selected
        .min(branches.len().saturating_sub(1));
    let bp = branches
        .get(selected)
        .map(|id| app.skill_tree.branch_progress(*id));
    let st_nav = ui::hint::hint(ui::hint::K_UD_JK, t!("skill_tree.hint_navigate").as_ref());
    let st_scroll = ui::hint::hint(ui::hint::K_SCROLL_KEYS, t!("skill_tree.hint_scroll").as_ref());
    let st_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("skill_tree.hint_back").as_ref());
    let st_unlock = ui::hint::hint(ui::hint::K_ENTER, t!("skill_tree.hint_unlock").as_ref());
    let st_drill = ui::hint::hint(ui::hint::K_ENTER, t!("skill_tree.hint_start_drill").as_ref());
    let (footer_hints, footer_notice): (Vec<&str>, Option<String>) =
        match bp.map(|b| b.status.clone()) {
            Some(BranchStatus::Locked) => (
                vec![
                    st_nav.as_str(),
                    st_scroll.as_str(),
                    st_back.as_str(),
                ],
                Some(locked_branch_notice(app)),
            ),
            Some(BranchStatus::Available) => (
                vec![
                    st_unlock.as_str(),
                    st_nav.as_str(),
                    st_scroll.as_str(),
                    st_back.as_str(),
                ],
                None,
            ),
            Some(BranchStatus::InProgress) => (
                vec![
                    st_drill.as_str(),
                    st_nav.as_str(),
                    st_scroll.as_str(),
                    st_back.as_str(),
                ],
                None,
            ),
            _ => (
                vec![
                    st_nav.as_str(),
                    st_scroll.as_str(),
                    st_back.as_str(),
                ],
                None,
            ),
        };
    let hint_lines = pack_hint_lines(&footer_hints, inner.width as usize);
    let notice_lines = footer_notice
        .as_deref()
        .map(|text| wrapped_line_count(text, inner.width as usize))
        .unwrap_or(0);
    let show_notice =
        footer_notice.is_some() && (inner.height as usize >= hint_lines.len() + notice_lines + 8);
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
        let (inter_branch_spacing, separator_padding) =
            branch_list_spacing_flags(main[0].height, branches.len());
        SkillTreeMouseLayout {
            branch_area: main[0],
            detail_area: main[2],
            inter_branch_spacing,
            separator_padding,
        }
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
        SkillTreeMouseLayout {
            branch_area: main[0],
            detail_area: main[2],
            inter_branch_spacing: false,
            separator_padding: false,
        }
    }
}

fn skill_tree_branch_index_from_y(
    branch_area: Rect,
    y: u16,
    branch_count: usize,
    inter_branch_spacing: bool,
    separator_padding: bool,
) -> Option<usize> {
    if y < branch_area.y || y >= branch_area.y + branch_area.height {
        return None;
    }
    let rel_y = (y - branch_area.y) as usize;
    let mut line = 0usize;
    for idx in 0..branch_count {
        if idx > 0 && inter_branch_spacing {
            line += 1;
        }
        let title_line = line;
        let progress_line = line + 1;
        if rel_y == title_line || rel_y == progress_line {
            return Some(idx);
        }
        line += 2;

        if idx == 0 {
            if separator_padding {
                line += 1;
            }
            line += 1;
            if separator_padding && !inter_branch_spacing {
                line += 1;
            }
        }
    }
    None
}

fn handle_skill_tree_mouse(app: &mut App, mouse: MouseEvent) {
    const DETAIL_SCROLL_STEP: usize = 3;
    if let Some(branch_id) = app.skill_tree_confirm_unlock {
        if matches!(
            mouse.kind,
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
        ) {
            let area = terminal_area();
            let dialog_width = 72u16.min(area.width.saturating_sub(4));
            let sentence_one = "Once unlocked, the default adaptive drill will mix in keys in this branch that are unlocked.";
            let sentence_two = "If you want to focus only on this branch, launch a drill directly from this branch in the Skill Tree.";
            let content_width = dialog_width.saturating_sub(6).max(1) as usize;
            let body_required = 5
                + wrapped_line_count(sentence_one, content_width)
                + wrapped_line_count(sentence_two, content_width);
            let min_dialog_height = (body_required + 1 + 2) as u16;
            let preferred_dialog_height = (body_required + 2 + 2) as u16;
            let max_dialog_height = area.height.saturating_sub(1).max(7);
            let dialog_height = preferred_dialog_height
                .min(max_dialog_height)
                .max(min_dialog_height.min(max_dialog_height));
            let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
            let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
            let dialog = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);
            if point_in_rect(mouse.column, mouse.row, dialog) {
                if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right))
                    || mouse.column >= dialog.x + dialog.width / 2
                {
                    // right-click (or right half) maps to "No"
                } else {
                    app.unlock_branch(branch_id);
                }
                app.skill_tree_confirm_unlock = None;
            }
        }
        return;
    }

    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.skill_tree_detail_scroll = app
                .skill_tree_detail_scroll
                .saturating_sub(DETAIL_SCROLL_STEP);
        }
        MouseEventKind::ScrollDown => {
            let max_scroll = skill_tree_detail_max_scroll(app);
            app.skill_tree_detail_scroll = app
                .skill_tree_detail_scroll
                .saturating_add(DETAIL_SCROLL_STEP)
                .min(max_scroll);
        }
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right) => {
            let is_secondary = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Right));
            let screen = terminal_area();
            let centered = skill_tree_popup_rect(screen);
            let inner = Block::bordered().inner(centered);
            let branches = selectable_branches();
            let selected = app
                .skill_tree_selected
                .min(branches.len().saturating_sub(1));
            let bp = app.skill_tree.branch_progress(branches[selected]);
            let st_nav = ui::hint::hint(ui::hint::K_UD_JK, t!("skill_tree.hint_navigate").as_ref());
            let st_scroll = ui::hint::hint(ui::hint::K_SCROLL_KEYS, t!("skill_tree.hint_scroll").as_ref());
            let st_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("skill_tree.hint_back").as_ref());
            let st_unlock = ui::hint::hint(ui::hint::K_ENTER, t!("skill_tree.hint_unlock").as_ref());
            let st_drill = ui::hint::hint(ui::hint::K_ENTER, t!("skill_tree.hint_start_drill").as_ref());
            let (footer_hints, footer_notice): (Vec<&str>, Option<String>) =
                if *app.skill_tree.branch_status(branches[selected])
                    == engine::skill_tree::BranchStatus::Locked
                {
                    (
                        vec![
                            st_nav.as_str(),
                            st_scroll.as_str(),
                            st_back.as_str(),
                        ],
                        Some(locked_branch_notice(app)),
                    )
                } else if bp.status == engine::skill_tree::BranchStatus::Available {
                    (
                        vec![
                            st_unlock.as_str(),
                            st_nav.as_str(),
                            st_scroll.as_str(),
                            st_back.as_str(),
                        ],
                        None,
                    )
                } else if bp.status == engine::skill_tree::BranchStatus::InProgress {
                    (
                        vec![
                            st_drill.as_str(),
                            st_nav.as_str(),
                            st_scroll.as_str(),
                            st_back.as_str(),
                        ],
                        None,
                    )
                } else {
                    (
                        vec![
                            st_nav.as_str(),
                            st_scroll.as_str(),
                            st_back.as_str(),
                        ],
                        None,
                    )
                };
            let hint_lines = pack_hint_lines(&footer_hints, inner.width as usize);
            let notice_lines = footer_notice
                .as_deref()
                .map(|text| wrapped_line_count(text, inner.width as usize))
                .unwrap_or(0);
            let show_notice = footer_notice.is_some()
                && (inner.height as usize >= hint_lines.len() + notice_lines + 8);
            let footer_needed = hint_lines.len() + if show_notice { notice_lines } else { 0 } + 1;
            let footer_height = footer_needed
                .min(inner.height.saturating_sub(5) as usize)
                .max(1) as u16;
            let footer_area = Rect::new(
                inner.x,
                inner.y + inner.height.saturating_sub(footer_height),
                inner.width,
                footer_height,
            );
            if let Some(token) = hint_token_at(footer_area, &footer_hints, mouse.column, mouse.row) {
                match token.as_str() {
                    "q/ESC" => app.go_to_menu(),
                    "Enter" => {
                        let branch_id = branches[selected];
                        let status = app.skill_tree.branch_status(branch_id).clone();
                        if status == BranchStatus::Available {
                            app.skill_tree_confirm_unlock = Some(branch_id);
                        } else if status == BranchStatus::InProgress {
                            app.start_branch_drill(branch_id);
                        }
                    }
                    "↑↓/jk" => {
                        if is_secondary {
                            if app.skill_tree_selected + 1 < branches.len() {
                                app.skill_tree_selected += 1;
                            }
                        } else {
                            app.skill_tree_selected = app.skill_tree_selected.saturating_sub(1);
                        }
                        app.skill_tree_detail_scroll = 0;
                    }
                    "PgUp/PgDn or Ctrl+U/Ctrl+D" => {
                        let max_scroll = skill_tree_detail_max_scroll(app);
                        app.skill_tree_detail_scroll = if is_secondary {
                            app.skill_tree_detail_scroll
                                .saturating_add(DETAIL_SCROLL_STEP)
                                .min(max_scroll)
                        } else {
                            app.skill_tree_detail_scroll
                                .saturating_sub(DETAIL_SCROLL_STEP)
                        };
                    }
                    _ => {}
                }
                return;
            }
            let branches = selectable_branches();
            let layout = skill_tree_interactive_areas(app, terminal_area());
            if point_in_rect(mouse.column, mouse.row, layout.branch_area) {
                if let Some(idx) = skill_tree_branch_index_from_y(
                    layout.branch_area,
                    mouse.row,
                    branches.len(),
                    layout.inter_branch_spacing,
                    layout.separator_padding,
                ) {
                    let already_selected = idx == app.skill_tree_selected;
                    app.skill_tree_selected = idx;
                    app.skill_tree_detail_scroll = 0;
                    if already_selected {
                        let branch_id = branches[idx];
                        let status = app.skill_tree.branch_status(branch_id).clone();
                        if status == BranchStatus::Available {
                            app.skill_tree_confirm_unlock = Some(branch_id);
                        } else if status == BranchStatus::InProgress {
                            app.start_branch_drill(branch_id);
                        }
                    }
                }
            } else if point_in_rect(mouse.column, mouse.row, layout.detail_area) {
                // Click in detail pane focuses selected branch; scroll wheel handles movement.
                let _ = layout.detail_area;
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
    let selected = app
        .skill_tree_selected
        .min(branches.len().saturating_sub(1));
    let bp = app.skill_tree.branch_progress(branches[selected]);
    let st_nav = ui::hint::hint(ui::hint::K_UD_JK, t!("skill_tree.hint_navigate").as_ref());
    let st_scroll = ui::hint::hint(ui::hint::K_SCROLL_KEYS, t!("skill_tree.hint_scroll").as_ref());
    let st_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("skill_tree.hint_back").as_ref());
    let st_unlock = ui::hint::hint(ui::hint::K_ENTER, t!("skill_tree.hint_unlock").as_ref());
    let st_drill = ui::hint::hint(ui::hint::K_ENTER, t!("skill_tree.hint_start_drill").as_ref());
    let (footer_hints, footer_notice): (Vec<&str>, Option<String>) =
        if *app.skill_tree.branch_status(branches[selected])
            == engine::skill_tree::BranchStatus::Locked
        {
            (
                vec![
                    st_nav.as_str(),
                    st_scroll.as_str(),
                    st_back.as_str(),
                ],
                Some(locked_branch_notice(app)),
            )
        } else if bp.status == engine::skill_tree::BranchStatus::Available {
            (
                vec![
                    st_unlock.as_str(),
                    st_nav.as_str(),
                    st_scroll.as_str(),
                    st_back.as_str(),
                ],
                None,
            )
        } else if bp.status == engine::skill_tree::BranchStatus::InProgress {
            (
                vec![
                    st_drill.as_str(),
                    st_nav.as_str(),
                    st_scroll.as_str(),
                    st_back.as_str(),
                ],
                None,
            )
        } else {
            (
                vec![
                    st_nav.as_str(),
                    st_scroll.as_str(),
                    st_back.as_str(),
                ],
                None,
            )
        };
    let hint_lines = pack_hint_lines(&footer_hints, inner.width as usize);
    let notice_lines = footer_notice
        .as_deref()
        .map(|text| wrapped_line_count(text, inner.width as usize))
        .unwrap_or(0);
    let show_notice =
        footer_notice.is_some() && (inner.height as usize >= hint_lines.len() + notice_lines + 8);
    let footer_needed = hint_lines.len() + if show_notice { notice_lines } else { 0 } + 1;
    let footer_height = footer_needed
        .min(inner.height.saturating_sub(5) as usize)
        .max(1) as u16;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(footer_height)])
        .split(inner);
    let side_by_side = use_side_by_side_layout(inner.width);
    let detail_height = if side_by_side {
        layout.first().map(|r| r.height as usize).unwrap_or(0)
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
        main.get(2).map(|r| r.height as usize).unwrap_or(0)
    };
    let expanded = use_expanded_level_spacing_for_tree(
        &app.skill_tree,
        detail_height as u16,
        branches[selected],
    );
    let total_lines = detail_line_count_with_level_spacing_for_tree(
        &app.skill_tree,
        branches[selected],
        expanded,
    );
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

    // Adaptive intro overlay for first-time adaptive drill users.
    if app.show_adaptive_intro {
        render_adaptive_intro_overlay(frame, app);
        return;
    }

    // Milestone overlays are modal and shown before the underlying screen.
    if let Some(milestone) = app.milestone_queue.front() {
        render_milestone_overlay(frame, app, milestone);
        return;
    }

    match app.screen {
        AppScreen::Menu => render_menu(frame, app),
        AppScreen::Drill => render_drill(frame, app),
        AppScreen::DrillResult => render_result(frame, app),
        AppScreen::StatsDashboard => render_stats(frame, app),
        AppScreen::Settings => render_settings(frame, app),
        AppScreen::DictionaryLanguageSelect => render_dictionary_language_select(frame, app),
        AppScreen::KeyboardLayoutSelect => render_keyboard_layout_select(frame, app),
        AppScreen::SkillTree => render_skill_tree(frame, app),
        AppScreen::CodeLanguageSelect => render_code_language_select(frame, app),
        AppScreen::PassageBookSelect => render_passage_book_select(frame, app),
        AppScreen::PassageIntro => render_passage_intro(frame, app),
        AppScreen::PassageDownloadProgress => render_passage_download_progress(frame, app),
        AppScreen::CodeIntro => render_code_intro(frame, app),
        AppScreen::CodeDownloadProgress => render_code_download_progress(frame, app),
        AppScreen::Keyboard => render_keyboard_explorer(frame, app),
        AppScreen::UiLanguageSelect => render_ui_language_select(frame, app),
    }
}

fn render_menu(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let mh_start = ui::hint::hint(ui::hint::K_1_3, t!("menu.hint_start").as_ref());
    let mh_tree = ui::hint::hint(ui::hint::K_T, t!("menu.hint_skill_tree").as_ref());
    let mh_kbd = ui::hint::hint(ui::hint::K_B, t!("menu.hint_keyboard").as_ref());
    let mh_stats = ui::hint::hint(ui::hint::K_S, t!("menu.hint_stats").as_ref());
    let mh_settings = ui::hint::hint(ui::hint::K_C, t!("menu.hint_settings").as_ref());
    let mh_quit = ui::hint::hint(ui::hint::K_Q, t!("menu.hint_quit").as_ref());
    let menu_hints: Vec<&str> = vec![
        mh_start.as_str(),
        mh_tree.as_str(),
        mh_kbd.as_str(),
        mh_stats.as_str(),
        mh_settings.as_str(),
        mh_quit.as_str(),
    ];
    let footer_lines_vec = pack_hint_lines(&menu_hints, area.width as usize);
    let footer_line_count = footer_lines_vec.len().max(1) as u16;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(footer_line_count),
        ])
        .split(area);

    let streak_text = if app.profile.streak_days > 0 {
        t!("menu.day_streak", days = app.profile.streak_days).to_string()
    } else {
        String::new()
    };
    let total_keys = app.skill_tree.total_unique_keys;
    let unlocked = app.skill_tree.total_unlocked_count();
    let mastered = app.skill_tree.total_confident_keys(&app.ranked_key_stats);
    let header_info = t!(
        "menu.key_progress",
        unlocked = unlocked,
        total = total_keys,
        mastered = mastered,
        target = app.config.target_wpm,
        streak = streak_text,
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
            header_info.as_ref(),
            Style::default()
                .fg(colors.text_pending())
                .bg(colors.header_bg()),
        ),
    ]))
    .style(Style::default().bg(colors.header_bg()));
    frame.render_widget(header, layout[0]);

    let menu_area = ui::layout::centered_rect(50, 80, layout[1]);
    frame.render_widget(&app.menu, menu_area);

    let footer_lines: Vec<Line> = footer_lines_vec
        .into_iter()
        .map(|line| {
            Line::from(Span::styled(
                line,
                Style::default().fg(colors.text_pending()),
            ))
        })
        .collect();
    let footer = Paragraph::new(footer_lines);
    frame.render_widget(footer, layout[2]);
}

fn render_drill(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    if let Some(ref drill) = app.drill {
        let app_layout = AppLayout::new(area);
        let tier = app_layout.tier;

        let mode_name_t = match app.drill_mode {
            DrillMode::Adaptive => t!("drill.mode_adaptive"),
            DrillMode::Code => t!("drill.mode_code"),
            DrillMode::Passage => t!("drill.mode_passage"),
        };
        let mode_name = mode_name_t.as_ref();

        // Compute focus text from stored selection (what generated this drill's text)
        let focus_text = if let Some(ref focus) = app.current_focus {
            match (&focus.char_focus, &focus.bigram_focus) {
                (Some(ch), Some((key, _, _))) => {
                    let bigram = format!("{}{}", key.0[0], key.0[1]);
                    format!(" | {}", t!("drill.focus_both", ch = ch, bigram = bigram))
                }
                (Some(ch), None) => format!(" | {}", t!("drill.focus_char", ch = ch)),
                (None, Some((key, _, _))) => {
                    let bigram = format!("{}{}", key.0[0], key.0[1]);
                    format!(" | {}", t!("drill.focus_bigram", bigram = bigram))
                }
                (None, None) => String::new(),
            }
        } else {
            String::new()
        };

        // For medium/narrow: show compact stats in header
        if !tier.show_sidebar() {
            let wpm = drill.wpm();
            let accuracy = drill.accuracy();
            let errors = drill.typo_count();
            let wpm_label = t!("drill.header_wpm");
            let acc_label = t!("drill.header_acc");
            let err_label = t!("drill.header_err");
            let header_text = format!(
                " {mode_name} | {wpm_label}: {wpm:.0} | {acc_label}: {accuracy:.1}% | {err_label}: {errors}{focus_text}"
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
            let header_title = format!(" {mode_name}{}", t!("drill.title"));
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

        let kbd_height = if show_kbd {
            if tier.compact_keyboard() {
                6 // 3 rows + 2 border + 1 modifier space
            } else {
                8 // 5 rows (4 + space bar) + 2 border + 1 spacing
            }
        } else {
            0
        };

        let progress_height = if show_progress {
            // Adaptive progress can use: branch rows + optional separator + overall line.
            // Prefer the separator when space allows, but degrade if constrained.
            let branch_rows = if area.height >= 25 {
                ui::components::branch_progress_list::wrapped_branch_rows(
                    app_layout.main.width,
                    active_branches.len(),
                )
            } else if !active_branches.is_empty() {
                1
            } else {
                0
            };
            let desired = if app.drill_mode == DrillMode::Adaptive {
                (branch_rows + 2).max(2)
            } else {
                1
            };
            // Keep at least 5 lines for typing area.
            let max_budget = app_layout
                .main
                .height
                .saturating_sub(kbd_height)
                .saturating_sub(5);
            desired.min(max_budget)
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
                    key_stats: &app.ranked_key_stats,
                    drill_scope: app.drill_scope,
                    active_branches: &active_branches,
                    theme: app.theme,
                    height: progress_height,
                };
                frame.render_widget(progress_widget, main_layout[idx]);
            } else {
                let source = app.drill_source_info.as_deref().unwrap_or("unknown source");
                let label_t = if app.drill_mode == DrillMode::Code {
                    t!("drill.code_source")
                } else {
                    t!("drill.passage_source")
                };
                let label = label_t.as_ref();
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
            let kbd = KeyboardDiagram::new(
                next_char,
                &unlocked_keys,
                &app.depressed_keys,
                app.theme,
                &app.keyboard_model,
            )
            .compact(tier.compact_keyboard())
            .shift_held(app.shift_held)
            .caps_lock(app.caps_lock);
            frame.render_widget(kbd, main_layout[idx]);
        }

        if let Some(sidebar_area) = app_layout.sidebar {
            let sidebar = StatsSidebar::new(
                drill,
                app.last_result.as_ref(),
                &app.drill_history,
                app.config.target_wpm,
                app.theme,
            );
            frame.render_widget(sidebar, sidebar_area);
        }

        let drill_footer_text = t!("drill.footer");
        let footer = Paragraph::new(Line::from(Span::styled(
            format!(" {} ", drill_footer_text),
            Style::default().fg(colors.text_pending()),
        )));
        frame.render_widget(footer, app_layout.footer);

        // Show a brief countdown overlay while the post-drill input lock is active.
        if let Some(ms) = app.post_drill_input_lock_remaining_ms() {
            let msg = t!("drill.keys_reenabled", ms = ms).to_string();
            let width = msg.len() as u16 + 4; // border + padding
            let height = 3;
            let x = area.x + area.width.saturating_sub(width) / 2;
            let y = area.y + area.height.saturating_sub(height) / 2;
            let overlay_area = Rect::new(x, y, width.min(area.width), height.min(area.height));

            frame.render_widget(ratatui::widgets::Clear, overlay_area);
            let block = Block::bordered()
                .border_style(Style::default().fg(colors.accent()))
                .style(Style::default().bg(colors.bg()));
            let inner = block.inner(overlay_area);
            frame.render_widget(block, overlay_area);
            frame.render_widget(
                Paragraph::new(msg).style(Style::default().fg(colors.text_pending())),
                inner,
            );
        }
    }
}

fn render_adaptive_intro_overlay(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let overlay_height = 20u16.min(area.height.saturating_sub(2));
    let overlay_width = 62u16.min(area.width.saturating_sub(4));

    let left = area.x + (area.width.saturating_sub(overlay_width)) / 2;
    let top = area.y + (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(left, top, overlay_width, overlay_height);

    frame.render_widget(ratatui::widgets::Clear, overlay_area);

    let title_t = t!("adaptive_intro.title");
    let block = Block::bordered()
        .title(title_t.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(overlay_area);
    block.render(overlay_area, frame.buffer_mut());

    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  {}", t!("adaptive_intro.how_it_works")),
        Style::default()
            .fg(colors.accent())
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    for text in [
        t!("adaptive_intro.desc_start").to_string(),
        t!("adaptive_intro.desc_progress").to_string(),
        t!("adaptive_intro.desc_expand").to_string(),
    ] {
        lines.push(Line::from(Span::styled(
            format!("  {text}"),
            Style::default().fg(colors.fg()),
        )));
    }
    lines.push(Line::from(""));

    // Target WPM adjuster
    let wpm_label = t!("adaptive_intro.target_wpm_label");
    let wpm_value = format!("\u{2190} {} \u{2192}", app.config.target_wpm);
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {wpm_label} "),
            Style::default().fg(colors.fg()).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            wpm_value,
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(""));
    for text in [
        t!("adaptive_intro.target_wpm_desc").to_string(),
        t!("adaptive_intro.target_wpm_default").to_string(),
    ] {
        lines.push(Line::from(Span::styled(
            format!("  {text}"),
            Style::default().fg(colors.fg()),
        )));
    }
    lines.push(Line::from(""));

    // Footer hint
    lines.push(Line::from(Span::styled(
        format!("  {}", t!("adaptive_intro.hint")),
        Style::default().fg(colors.text_pending()),
    )));

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_milestone_overlay(
    frame: &mut ratatui::Frame,
    app: &App,
    milestone: &app::KeyMilestonePopup,
) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let is_key_milestone = matches!(
        milestone.kind,
        MilestoneKind::Unlock | MilestoneKind::Mastery
    );

    // Determine overlay size based on terminal height:
    // Key milestones get keyboard diagrams; other milestones are text-only
    let kbd_mode = if is_key_milestone {
        overlay_keyboard_mode(area.height)
    } else {
        0
    };
    let overlay_height = match &milestone.kind {
        MilestoneKind::BranchesAvailable => 18u16.min(area.height.saturating_sub(2)),
        MilestoneKind::BranchComplete
        | MilestoneKind::AllKeysUnlocked
        | MilestoneKind::AllKeysMastered => 12u16.min(area.height.saturating_sub(2)),
        _ => match kbd_mode {
            2 => 18u16.min(area.height.saturating_sub(2)),
            1 => 14u16.min(area.height.saturating_sub(2)),
            _ => 10u16.min(area.height.saturating_sub(2)),
        },
    };
    let overlay_width = 60u16.min(area.width.saturating_sub(4));

    let left = area.x + (area.width.saturating_sub(overlay_width)) / 2;
    let top = area.y + (area.height.saturating_sub(overlay_height)) / 2;
    let overlay_area = Rect::new(left, top, overlay_width, overlay_height);

    // Clear the area behind the overlay
    frame.render_widget(ratatui::widgets::Clear, overlay_area);

    let title_t = match milestone.kind {
        MilestoneKind::Unlock => t!("milestones.unlock_title"),
        MilestoneKind::Mastery => t!("milestones.mastery_title"),
        MilestoneKind::BranchesAvailable => t!("milestones.branches_title"),
        MilestoneKind::BranchComplete => t!("milestones.branch_complete_title"),
        MilestoneKind::AllKeysUnlocked => t!("milestones.all_unlocked_title"),
        MilestoneKind::AllKeysMastered => t!("milestones.all_mastered_title"),
    };

    let block = Block::bordered()
        .title(title_t.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(overlay_area);
    block.render(overlay_area, frame.buffer_mut());

    let mut lines: Vec<Line> = Vec::new();

    match milestone.kind {
        MilestoneKind::Unlock | MilestoneKind::Mastery => {
            let key_action_t = match milestone.kind {
                MilestoneKind::Unlock => t!("milestones.unlocked"),
                _ => t!("milestones.mastered"),
            };
            let key_action = key_action_t.as_ref();

            let key_names: Vec<String> = milestone
                .keys
                .iter()
                .map(|&ch| {
                    let name = keyboard::display::key_display_name(ch);
                    if name.is_empty() {
                        format!("'{ch}'")
                    } else {
                        name.to_string()
                    }
                })
                .collect();
            let keys_str = key_names.join(", ");

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("  You {key_action}: {keys_str}"),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )));

            // Finger info (for unlocks)
            if matches!(milestone.kind, MilestoneKind::Unlock) {
                for (ch, finger_desc) in &milestone.finger_info {
                    let key_label = {
                        let name = keyboard::display::key_display_name(*ch);
                        if name.is_empty() {
                            format!("'{ch}'")
                        } else {
                            name.to_string()
                        }
                    };
                    let use_finger_msg = t!("milestones.use_finger", finger = finger_desc.to_lowercase().as_str());
                    lines.push(Line::from(Span::styled(
                        format!("  {key_label}: {use_finger_msg}"),
                        Style::default().fg(colors.fg()),
                    )));

                    // Shift key guidance for shifted characters
                    let fa = app.keyboard_model.finger_for_char(*ch);
                    if ch.is_uppercase()
                        || (!ch.is_lowercase()
                            // Digits are intentionally ASCII-scoped in current drills/keyboard progression.
                            && !ch.is_ascii_digit()
                            && !ch.is_whitespace()
                            && *ch != ' ')
                    {
                        let shift_hint = if fa.hand == keyboard::finger::Hand::Left {
                            t!("milestones.hold_right_shift")
                        } else {
                            t!("milestones.hold_left_shift")
                        };
                        lines.push(Line::from(Span::styled(
                            format!("  {shift_hint}"),
                            Style::default().fg(colors.text_pending()),
                        )));
                    }
                }
            }

            // Encouraging message
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("  {}", milestone.message),
                Style::default().fg(colors.focused_key()),
            )));
        }

        MilestoneKind::BranchesAvailable => {
            lines.push(Line::from(""));
            let primary_count = app.skill_tree.primary_letters().len();
            let congrats_msg = t!("milestones.congratulations_all_letters", count = primary_count);
            lines.push(Line::from(Span::styled(
                format!("  {congrats_msg}"),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            let new_branches = t!("milestones.new_branches_available");
            lines.push(Line::from(Span::styled(
                format!("  {new_branches}"),
                Style::default().fg(colors.fg()),
            )));
            for &branch_id in &milestone.branch_ids {
                let name = get_branch_definition(branch_id).display_name();
                lines.push(Line::from(Span::styled(
                    format!("    \u{2022} {name}"),
                    Style::default().fg(colors.focused_key()),
                )));
            }
            lines.push(Line::from(""));
            let visit_msg = t!("milestones.visit_skill_tree");
            lines.push(Line::from(Span::styled(
                format!("  {visit_msg}"),
                Style::default().fg(colors.fg()),
            )));
            let and_start = t!("milestones.and_start_training");
            lines.push(Line::from(Span::styled(
                format!("  {and_start}"),
                Style::default().fg(colors.fg()),
            )));
            lines.push(Line::from(""));
            let open_tree = t!("milestones.open_skill_tree");
            lines.push(Line::from(Span::styled(
                format!("  {open_tree}"),
                Style::default().fg(colors.text_pending()),
            )));
        }

        MilestoneKind::BranchComplete => {
            lines.push(Line::from(""));
            let branch_names: Vec<String> = milestone
                .branch_ids
                .iter()
                .map(|&id| get_branch_definition(id).display_name())
                .collect();
            let branch_text = if branch_names.len() == 1 {
                branch_names[0].to_string()
            } else {
                let all_but_last = &branch_names[..branch_names.len() - 1];
                let last = &branch_names[branch_names.len() - 1];
                format!("{} and {}", all_but_last.join(", "), last)
            };
            let complete_msg = t!("milestones.branch_complete_msg", branch = branch_text);
            lines.push(Line::from(Span::styled(
                format!("  {complete_msg}"),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            let visit_msg = t!("milestones.visit_skill_tree");
            lines.push(Line::from(Span::styled(
                format!("  {visit_msg}"),
                Style::default().fg(colors.fg()),
            )));
            let and_start = t!("milestones.and_start_training");
            lines.push(Line::from(Span::styled(
                format!("  {and_start}"),
                Style::default().fg(colors.fg()),
            )));
            lines.push(Line::from(""));
            let open_tree = t!("milestones.open_skill_tree");
            lines.push(Line::from(Span::styled(
                format!("  {open_tree}"),
                Style::default().fg(colors.text_pending()),
            )));
        }

        MilestoneKind::AllKeysUnlocked => {
            lines.push(Line::from(""));
            let unlocked_msg = t!("milestones.all_unlocked_msg");
            lines.push(Line::from(Span::styled(
                format!("  {unlocked_msg}"),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            let desc = t!("milestones.all_unlocked_desc");
            lines.push(Line::from(Span::styled(
                format!("  {desc}"),
                Style::default().fg(colors.fg()),
            )));
            let keep_practicing = t!("milestones.keep_practicing_mastery");
            lines.push(Line::from(Span::styled(
                format!("  {keep_practicing}"),
                Style::default().fg(colors.fg()),
            )));
            let confidence = t!("milestones.confidence_complete");
            lines.push(Line::from(Span::styled(
                format!("  {confidence}"),
                Style::default().fg(colors.fg()),
            )));
        }

        MilestoneKind::AllKeysMastered => {
            lines.push(Line::from(""));
            let mastered_msg = t!("milestones.all_mastered_msg");
            lines.push(Line::from(Span::styled(
                format!("  {mastered_msg}"),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )));
            let mastered_desc = t!("milestones.all_mastered_desc");
            lines.push(Line::from(Span::styled(
                format!("  {mastered_desc}"),
                Style::default()
                    .fg(colors.accent())
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            let takes_practice = t!("milestones.mastery_takes_practice");
            lines.push(Line::from(Span::styled(
                format!("  {takes_practice}"),
                Style::default().fg(colors.fg()),
            )));
            let keep_drilling = t!("milestones.keep_drilling");
            lines.push(Line::from(Span::styled(
                format!("  {keep_drilling}"),
                Style::default().fg(colors.fg()),
            )));
        }
    }

    // Keyboard diagram (only for key milestones, if space permits)
    if kbd_mode > 0 && is_key_milestone {
        let min_kbd_height: u16 = if kbd_mode == 2 { 6 } else { 4 };
        let remaining = inner.height.saturating_sub(lines.len() as u16 + 2);
        if remaining >= min_kbd_height {
            let kbd_y_start = inner.y + lines.len() as u16 + 1;
            let kbd_height = remaining.min(if kbd_mode == 2 { 8 } else { 6 });
            let kbd_area = Rect::new(inner.x, kbd_y_start, inner.width, kbd_height);
            let milestone_key = milestone.keys.first().copied();
            let unlocked_keys = app.skill_tree.unlocked_keys(app.drill_scope);
            let is_shifted = milestone_key.is_some_and(|ch| {
                ch.is_uppercase() || app.keyboard_model.shifted_to_base(ch).is_some()
            });
            let kbd = KeyboardDiagram::new(
                None,
                &unlocked_keys,
                &app.depressed_keys,
                app.theme,
                &app.keyboard_model,
            )
            .selected_key(milestone_key)
            .compact(kbd_mode == 1)
            .shift_held(is_shifted)
            .caps_lock(app.caps_lock);
            frame.render_widget(kbd, kbd_area);
        }
    }

    // Render the text content
    let text_area = Rect::new(
        inner.x,
        inner.y,
        inner.width,
        inner.height.saturating_sub(1),
    );
    Paragraph::new(lines).render(text_area, frame.buffer_mut());

    // Footer
    let footer_y = inner.y + inner.height.saturating_sub(1);
    if footer_y < inner.y + inner.height {
        let footer_area = Rect::new(inner.x, footer_y, inner.width, 1);
        let footer_text = if let Some(ms) = app.post_drill_input_lock_remaining_ms() {
            format!("  {}", t!("milestones.input_blocked", ms = ms))
        } else if milestone_supports_skill_tree_shortcut(milestone) {
            format!("  {}", ui::hint::hint(ui::hint::K_T, t!("milestones.hint_skill_tree_continue").as_ref()))
        } else {
            format!("  {}", t!("milestones.hint_any_key"))
        };
        let footer = Paragraph::new(Line::from(Span::styled(
            footer_text,
            Style::default().fg(colors.text_pending()),
        )));
        frame.render_widget(footer, footer_area);
    }
}

fn overlay_keyboard_mode(height: u16) -> u8 {
    if height >= 25 {
        2 // full
    } else if height >= 15 {
        1 // compact
    } else {
        0 // text only
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    use crate::engine::skill_tree::SkillTreeProgress;
    use crate::session::result::DrillResult;
    use chrono::{TimeDelta, Utc};

    /// Create an App for testing with the store disabled so tests never
    /// read or write the user's real data files.
    fn test_app() -> App {
        App::new_test()
    }

    fn test_result(ts_offset_secs: i64) -> DrillResult {
        DrillResult {
            wpm: 60.0,
            cpm: 300.0,
            accuracy: 98.0,
            correct: 49,
            incorrect: 1,
            total_chars: 50,
            elapsed_secs: 10.0,
            timestamp: Utc::now() + TimeDelta::seconds(ts_offset_secs),
            per_key_times: vec![],
            drill_mode: "adaptive".to_string(),
            ranked: true,
            partial: false,
            completion_percent: 100.0,
        }
    }

    #[test]
    fn locked_branch_notice_uses_primary_letter_count() {
        let mut app = test_app();
        app.skill_tree = crate::engine::skill_tree::SkillTree::new_with_primary_sequence(
            SkillTreeProgress::default(),
            "abcde",
        );
        assert_eq!(
            locked_branch_notice(&app),
            "Complete 5 primary letters to unlock branches"
        );
    }

    #[test]
    fn milestone_overlay_blocks_underlying_input() {
        let mut app = test_app();
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("abc"));
        app.milestone_queue
            .push_back(crate::app::KeyMilestonePopup {
                kind: crate::app::MilestoneKind::Unlock,
                keys: vec!['a'],
                finger_info: vec![('a', "left pinky".to_string())],
                message: "msg".to_string(),
                branch_ids: vec![],
            });

        let before_cursor = app.drill.as_ref().map(|d| d.cursor).unwrap_or(0);
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        let after_cursor = app.drill.as_ref().map(|d| d.cursor).unwrap_or(0);

        assert_eq!(before_cursor, after_cursor);
        assert!(app.milestone_queue.is_empty());
    }

    #[test]
    fn milestone_queue_chains_before_result_actions() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.milestone_queue
            .push_back(crate::app::KeyMilestonePopup {
                kind: crate::app::MilestoneKind::Unlock,
                keys: vec!['a'],
                finger_info: vec![('a', "left pinky".to_string())],
                message: "msg1".to_string(),
                branch_ids: vec![],
            });
        app.milestone_queue
            .push_back(crate::app::KeyMilestonePopup {
                kind: crate::app::MilestoneKind::Mastery,
                keys: vec!['a'],
                finger_info: vec![('a', "left pinky".to_string())],
                message: "msg2".to_string(),
                branch_ids: vec![],
            });

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        );
        assert_eq!(app.screen, AppScreen::DrillResult);
        assert_eq!(app.milestone_queue.len(), 1);

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        );
        assert_eq!(app.screen, AppScreen::DrillResult);
        assert!(app.milestone_queue.is_empty());

        // Now normal result action should apply.
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        );
        assert_eq!(app.screen, AppScreen::Menu);
    }

    #[test]
    fn post_drill_lock_blocks_result_shortcuts_temporarily() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.last_result = Some(test_result(1));
        app.post_drill_input_lock_until =
            Some(Instant::now() + std::time::Duration::from_millis(500));

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        );

        assert_eq!(app.screen, AppScreen::DrillResult);
    }

    #[test]
    fn post_drill_lock_blocks_milestone_dismissal_temporarily() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.milestone_queue
            .push_back(crate::app::KeyMilestonePopup {
                kind: crate::app::MilestoneKind::Unlock,
                keys: vec!['a'],
                finger_info: vec![('a', "left pinky".to_string())],
                message: "msg".to_string(),
                branch_ids: vec![],
            });
        app.post_drill_input_lock_until =
            Some(Instant::now() + std::time::Duration::from_millis(500));

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );

        assert_eq!(app.milestone_queue.len(), 1);
    }

    #[test]
    fn milestone_t_shortcut_opens_skill_tree_for_congrats_popup() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.milestone_queue
            .push_back(crate::app::KeyMilestonePopup {
                kind: crate::app::MilestoneKind::BranchesAvailable,
                keys: vec![],
                finger_info: vec![],
                message: "msg".to_string(),
                branch_ids: vec![engine::skill_tree::BranchId::Capitals],
            });

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
        );

        assert!(app.milestone_queue.is_empty());
        assert_eq!(app.screen, AppScreen::SkillTree);
    }

    #[test]
    fn overlay_mode_height_boundaries() {
        assert_eq!(overlay_keyboard_mode(14), 0);
        assert_eq!(overlay_keyboard_mode(15), 1);
        assert_eq!(overlay_keyboard_mode(24), 1);
        assert_eq!(overlay_keyboard_mode(25), 2);
    }

    #[test]
    fn result_delete_shortcut_opens_confirmation_for_latest() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.last_result = Some(test_result(2));
        app.drill_history = vec![test_result(1), test_result(2)];
        app.history_selected = 1;

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );

        assert!(app.history_confirm_delete);
        assert_eq!(app.history_selected, 0);
    }

    #[test]
    fn result_delete_confirmation_yes_deletes_latest() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.last_result = Some(test_result(3));
        let older = test_result(1);
        let newer = test_result(2);
        app.drill_history = vec![older.clone(), newer.clone()];

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        );

        assert!(!app.history_confirm_delete);
        assert_eq!(app.drill_history.len(), 1);
        assert_eq!(app.drill_history[0].timestamp, older.timestamp);
        assert_eq!(app.screen, AppScreen::Drill);
        assert!(app.drill.is_some());
    }

    #[test]
    fn result_delete_confirmation_cancel_keeps_history() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.last_result = Some(test_result(2));
        app.drill_history = vec![test_result(1), test_result(2)];

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        );
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        );

        assert!(!app.history_confirm_delete);
        assert_eq!(app.drill_history.len(), 2);
        assert_eq!(app.screen, AppScreen::DrillResult);
    }

    #[test]
    fn result_continue_shortcuts_start_next_drill() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.last_result = Some(test_result(2));

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        );
        assert_eq!(app.screen, AppScreen::Drill);

        app.screen = AppScreen::DrillResult;
        handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.screen, AppScreen::Drill);

        app.screen = AppScreen::DrillResult;
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        );
        assert_eq!(app.screen, AppScreen::Drill);
    }

    #[test]
    fn result_continue_code_uses_last_language_params() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.last_result = Some(test_result(2));
        app.drill_mode = DrillMode::Code;
        app.config.code_downloads_enabled = false;
        app.config.code_language = "python".to_string();
        app.last_code_drill_language = Some("rust".to_string());

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        );

        assert_eq!(app.screen, AppScreen::Drill);
        assert_eq!(app.drill_mode, DrillMode::Code);
        assert_eq!(app.last_code_drill_language.as_deref(), Some("rust"));
    }

    /// Helper: count how many settings modal/edit flags are active
    fn modal_edit_count(app: &App) -> usize {
        let mut count = 0;
        if app.settings_confirm_import {
            count += 1;
        }
        if app.settings_export_conflict {
            count += 1;
        }
        if app.is_editing_path() {
            count += 1;
        }
        count
    }

    #[test]
    fn settings_modal_invariant_enter_export_path_clears_others() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;

        // First, activate import confirmation
        app.settings_selected = SettingItem::ImportData.index();
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.settings_confirm_import);
        assert!(modal_edit_count(&app) <= 1);

        // Cancel it
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.settings_confirm_import);

        // Enter export path editing
        app.settings_selected = SettingItem::ExportPath.index();
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.is_editing_field(SettingItem::ExportPath.index()));
        assert!(modal_edit_count(&app) <= 1);

        // Esc out
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.is_editing_path());
    }

    #[test]
    fn settings_modal_invariant_enter_import_path_clears_others() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;

        // Activate export path editing first
        app.settings_selected = SettingItem::ExportPath.index();
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.is_editing_field(SettingItem::ExportPath.index()));

        // Esc out, then enter import path editing
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        app.settings_selected = SettingItem::ImportPath.index();
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.is_editing_field(SettingItem::ImportPath.index()));
        assert!(!app.is_editing_field(SettingItem::ExportPath.index()));
        assert!(modal_edit_count(&app) <= 1);
    }

    #[test]
    fn settings_confirm_import_dialog_y_n_esc() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;

        // Trigger import confirmation
        app.settings_selected = SettingItem::ImportData.index();
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.settings_confirm_import);

        // 'n' cancels
        handle_settings_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        );
        assert!(!app.settings_confirm_import);

        // Trigger again
        app.settings_selected = SettingItem::ImportData.index();
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.settings_confirm_import);

        // Esc cancels
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.settings_confirm_import);
    }

    #[test]
    fn settings_status_message_dismissed_on_keypress() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;

        // Set a status message
        app.settings_status_message = Some(crate::app::StatusMessage {
            kind: StatusKind::Success,
            text: "test".to_string(),
        });

        // Any keypress should dismiss it
        handle_settings_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        );
        assert!(app.settings_status_message.is_none());
    }

    #[test]
    fn smart_rename_canonical_filename() {
        use crate::app::next_available_path;
        let dir = tempfile::TempDir::new().unwrap();
        let base = dir.path();

        // Create base file
        let base_path = base.join("keydr-export-2026-01-01.json");
        std::fs::write(&base_path, "{}").unwrap();

        // First rename: picks -1
        let result = next_available_path(base_path.to_str().unwrap());
        assert!(result.ends_with("keydr-export-2026-01-01-1.json"));

        // Create -1
        std::fs::write(base.join("keydr-export-2026-01-01-1.json"), "{}").unwrap();

        // From base: picks -2
        let result = next_available_path(base_path.to_str().unwrap());
        assert!(result.ends_with("keydr-export-2026-01-01-2.json"));

        // From -1 path: normalizes to base stem and picks -2
        let path_1 = base.join("keydr-export-2026-01-01-1.json");
        let result = next_available_path(path_1.to_str().unwrap());
        assert!(result.ends_with("keydr-export-2026-01-01-2.json"));
    }

    #[test]
    fn smart_rename_custom_filename() {
        use crate::app::next_available_path;
        let dir = tempfile::TempDir::new().unwrap();
        let base = dir.path();

        let custom_path = base.join("my-backup.json");
        std::fs::write(&custom_path, "{}").unwrap();

        let result = next_available_path(custom_path.to_str().unwrap());
        assert!(result.ends_with("my-backup-1.json"));

        std::fs::write(base.join("my-backup-1.json"), "{}").unwrap();
        let result = next_available_path(custom_path.to_str().unwrap());
        assert!(result.ends_with("my-backup-2.json"));
    }

    // --- Keyboard state tracking tests ---

    /// Helper to build a KeyEvent with specific state flags.
    fn key_event_with_state(
        code: KeyCode,
        modifiers: KeyModifiers,
        kind: KeyEventKind,
        state: KeyEventState,
    ) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind,
            state,
        }
    }

    #[test]
    fn caps_lock_set_from_state_flag() {
        let mut app = test_app();
        assert!(!app.caps_lock);

        // Modifier event with CAPS_LOCK in state turns it on
        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Modifier(ModifierKeyCode::LeftShift),
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
                KeyEventState::CAPS_LOCK,
            ),
        );
        assert!(app.caps_lock);
    }

    #[test]
    fn caps_lock_not_cleared_by_char_event_with_empty_state() {
        let mut app = test_app();
        app.caps_lock = true;
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("abc"));

        // Character event with empty state should NOT clear caps_lock
        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Char('A'),
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(
            app.caps_lock,
            "char event with empty state must not clear caps_lock"
        );
    }

    #[test]
    fn caps_lock_cleared_by_modifier_event_without_caps_flag() {
        let mut app = test_app();
        app.caps_lock = true;

        // Modifier event WITHOUT CAPS_LOCK in state clears it
        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Modifier(ModifierKeyCode::LeftShift),
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(
            !app.caps_lock,
            "modifier event without CAPS_LOCK flag should clear caps_lock"
        );
    }

    #[test]
    fn caps_lock_on_uppercase_char_does_not_set_shift() {
        let mut app = test_app();
        app.caps_lock = true;
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("ABC"));

        // Caps lock on, typing 'A' — crossterm may report SHIFT modifier,
        // but this is caps lock, not physical shift
        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Char('A'),
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(
            !app.shift_held,
            "uppercase char with caps lock should not set shift_held"
        );
    }

    #[test]
    fn caps_lock_on_lowercase_char_with_shift_sets_shift() {
        let mut app = test_app();
        app.caps_lock = true;
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("abc"));

        // Caps lock on + shift held produces lowercase: shift IS held
        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Char('a'),
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(
            app.shift_held,
            "lowercase char with caps+shift should set shift_held"
        );
    }

    #[test]
    fn caps_lock_off_uppercase_char_with_shift_sets_shift() {
        let mut app = test_app();
        assert!(!app.caps_lock);
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("ABC"));

        // Normal shift+a = 'A', caps lock off
        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Char('A'),
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(
            app.shift_held,
            "uppercase char without caps lock should set shift_held"
        );
    }

    #[test]
    fn caps_lock_off_lowercase_char_without_shift_clears_shift() {
        let mut app = test_app();
        app.shift_held = true;
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("abc"));

        // Normal lowercase typing, no shift
        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Char('a'),
                KeyModifiers::NONE,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(
            !app.shift_held,
            "lowercase char without shift should clear shift_held"
        );
    }

    #[test]
    fn shift_modifier_press_sets_shift_held() {
        let mut app = test_app();

        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Modifier(ModifierKeyCode::LeftShift),
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(app.shift_held);
    }

    #[test]
    fn shift_modifier_release_clears_shift_held() {
        let mut app = test_app();
        app.shift_held = true;

        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Modifier(ModifierKeyCode::RightShift),
                KeyModifiers::SHIFT,
                KeyEventKind::Release,
                KeyEventState::NONE,
            ),
        );
        assert!(!app.shift_held);
    }

    #[test]
    fn depressed_keys_tracks_char_press() {
        let mut app = test_app();
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("abc"));

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        );
        assert!(app.depressed_keys.contains(&'a'));
    }

    #[test]
    fn depressed_keys_release_removes_char() {
        let mut app = test_app();
        app.depressed_keys.insert('a');

        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Char('a'),
                KeyModifiers::NONE,
                KeyEventKind::Release,
                KeyEventState::NONE,
            ),
        );
        assert!(!app.depressed_keys.contains(&'a'));
    }

    #[test]
    fn depressed_keys_normalizes_shifted_chars_to_keyboard_base_key() {
        let mut app = test_app();
        app.set_keyboard_layout("fr_azerty")
            .expect("fr_azerty should be available in tests");
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("2"));

        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Char('2'),
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(
            app.depressed_keys.contains(&'é'),
            "AZERTY shifted digit should map to its base physical key"
        );

        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Char('2'),
                KeyModifiers::SHIFT,
                KeyEventKind::Release,
                KeyEventState::NONE,
            ),
        );
        assert!(
            !app.depressed_keys.contains(&'é'),
            "Release should clear normalized depressed key entry"
        );
    }

    #[test]
    fn caps_lock_cleared_by_capslock_key_without_caps_flag() {
        let mut app = test_app();
        app.caps_lock = true;

        // Pressing CapsLock key to toggle off: event has KeyCode::CapsLock
        // but state no longer contains CAPS_LOCK
        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::CapsLock,
                KeyModifiers::NONE,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(
            !app.caps_lock,
            "CapsLock key event without CAPS_LOCK state should clear caps_lock"
        );
    }

    #[test]
    fn caps_lock_non_alpha_char_with_shift_still_sets_shift() {
        let mut app = test_app();
        app.caps_lock = true;
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("!@#"));

        // Caps lock doesn't affect non-alpha chars like '!', so SHIFT
        // modifier should be trusted as-is
        handle_key(
            &mut app,
            key_event_with_state(
                KeyCode::Char('!'),
                KeyModifiers::SHIFT,
                KeyEventKind::Press,
                KeyEventState::NONE,
            ),
        );
        assert!(
            app.shift_held,
            "non-alpha char with shift should set shift_held regardless of caps"
        );
    }

    #[test]
    fn build_ngram_tab_data_maps_fields_correctly() {
        use crate::engine::ngram_stats::{ANOMALY_STREAK_REQUIRED, BigramKey};

        let mut app = test_app();

        // Set up char stats with known EMA error rates
        for &ch in &['e', 't', 'a', 'o', 'n', 'i'] {
            let stat = app.ranked_key_stats.stats.entry(ch).or_default();
            stat.confidence = 0.95;
            stat.filtered_time_ms = 360.0;
            stat.sample_count = 50;
            stat.total_count = 50;
            stat.error_rate_ema = 0.03;
        }
        // Make 'n' weak so we get a focused char
        app.ranked_key_stats.stats.get_mut(&'n').unwrap().confidence = 0.5;
        app.ranked_key_stats
            .stats
            .get_mut(&'n')
            .unwrap()
            .filtered_time_ms = 686.0;

        // Add a confirmed error anomaly bigram
        let et_key = BigramKey(['e', 't']);
        let stat = app
            .ranked_bigram_stats
            .stats
            .entry(et_key.clone())
            .or_default();
        stat.sample_count = 30;
        stat.error_rate_ema = 0.80;
        stat.error_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        // Add an unconfirmed anomaly bigram (low samples)
        let ao_key = BigramKey(['a', 'o']);
        let stat = app
            .ranked_bigram_stats
            .stats
            .entry(ao_key.clone())
            .or_default();
        stat.sample_count = 10;
        stat.error_rate_ema = 0.60;
        stat.error_anomaly_streak = 1;

        // Set drill scope
        app.drill_scope = DrillScope::Global;
        app.stats_tab = 5;

        let data = build_ngram_tab_data(&app);

        // Verify scope label
        assert_eq!(data.scope_label, "Global");

        // Verify bigram count
        assert_eq!(data.total_bigrams, app.ranked_bigram_stats.stats.len());

        // Verify hesitation threshold
        assert!(data.hesitation_threshold_ms >= 800.0);

        // Verify FocusSelection has both char and bigram
        assert!(data.focus.char_focus.is_some(), "should have char focus");
        assert!(
            data.focus.bigram_focus.is_some(),
            "should have bigram focus"
        );

        // Verify error anomaly rows have correct fields populated
        if !data.error_anomalies.is_empty() {
            let row = &data.error_anomalies[0];
            assert!(row.anomaly_pct > 0.0, "anomaly_pct should be positive");
            assert!(row.sample_count > 0, "sample_count should be positive");
        }

        // Verify 'ao' appears in error anomalies (high error rate, above min samples)
        let ao_row = data.error_anomalies.iter().find(|r| r.bigram == "ao");
        if let Some(ao) = ao_row {
            assert_eq!(ao.sample_count, 10);
            assert!(!ao.confirmed, "ao should not be confirmed (low samples)");
        }

        // Add a speed anomaly bigram and verify speed_anomalies mapping
        let ni_key = BigramKey(['n', 'i']);
        let stat = app
            .ranked_bigram_stats
            .stats
            .entry(ni_key.clone())
            .or_default();
        stat.sample_count = 25;
        stat.error_rate_ema = 0.02;
        stat.filtered_time_ms = 600.0; // much slower than char 'i' baseline
        stat.speed_anomaly_streak = ANOMALY_STREAK_REQUIRED;

        // Make char 'i' baseline fast enough that 600ms is a big anomaly
        app.ranked_key_stats
            .stats
            .get_mut(&'i')
            .unwrap()
            .filtered_time_ms = 200.0;

        let data2 = build_ngram_tab_data(&app);

        // Verify speed anomalies contain our bigram with correct field mapping
        let ni_row = data2.speed_anomalies.iter().find(|r| r.bigram == "ni");
        assert!(ni_row.is_some(), "ni should appear in speed_anomalies");
        let ni = ni_row.unwrap();
        assert_eq!(ni.sample_count, 25);
        assert!(ni.anomaly_pct > 100.0, "600ms / 200ms => 200% anomaly");
        assert!(
            (ni.expected_baseline - 200.0).abs() < 1.0,
            "expected baseline should be char 'i' speed (200ms), got {}",
            ni.expected_baseline
        );
        assert!(
            ni.confirmed,
            "ni should be confirmed (samples >= 20, streak >= required)"
        );
    }

    #[test]
    fn drill_screen_input_lock_blocks_normal_keys() {
        let mut app = test_app();
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("abc"));
        app.post_drill_input_lock_until =
            Some(Instant::now() + std::time::Duration::from_millis(500));

        let before_cursor = app.drill.as_ref().unwrap().cursor;
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        );
        let after_cursor = app.drill.as_ref().unwrap().cursor;

        assert_eq!(
            before_cursor, after_cursor,
            "Key should be blocked during input lock on Drill screen"
        );
        assert_eq!(app.screen, AppScreen::Drill);
    }

    #[test]
    fn ctrl_c_passes_through_input_lock() {
        let mut app = test_app();
        app.screen = AppScreen::Drill;
        app.drill = Some(crate::session::drill::DrillState::new("abc"));
        app.post_drill_input_lock_until =
            Some(Instant::now() + std::time::Duration::from_millis(500));

        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );

        assert!(
            app.should_quit,
            "Ctrl+C should set should_quit even during input lock"
        );
    }

    /// Helper: render settings to a test buffer and return its text content.
    fn render_settings_to_string(app: &App) -> String {
        let backend = ratatui::backend::TestBackend::new(80, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_settings(frame, app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        text
    }

    /// Helper: render skill tree to a test buffer and return its text content.
    fn render_skill_tree_to_string_with_size(app: &App, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_skill_tree(frame, app))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        text
    }

    fn render_skill_tree_to_string(app: &App) -> String {
        render_skill_tree_to_string_with_size(app, 120, 40)
    }

    fn render_app_to_string_with_size(app: &App, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn footer_shows_completion_error_and_clears_on_keystroke() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;
        app.settings_selected = SettingItem::ExportPath.index();

        // Enter editing mode
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.is_editing_field(SettingItem::ExportPath.index()));

        // Set path to nonexistent dir and trigger tab completion error
        if let Some((_, ref mut input)) = app.settings_editing_path {
            input.handle(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)); // clear
            for ch in "/nonexistent_zzz_dir/".chars() {
                input.handle(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
            }
            input.handle(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            assert!(input.completion_error);
        }

        // Render and check footer contains the error hint
        let output = render_settings_to_string(&app);
        assert!(
            output.contains("(cannot read directory)"),
            "Footer should show completion error hint"
        );

        // Press a non-tab key to clear the error
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));

        // Render again — error hint should be gone
        let output_after = render_settings_to_string(&app);
        assert!(
            !output_after.contains("(cannot read directory)"),
            "Footer error hint should clear after non-Tab keystroke"
        );
    }

    #[test]
    fn footer_shows_editing_hints_when_path_editing() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;
        app.settings_selected = SettingItem::ExportPath.index();

        // Before editing: shows default hints
        let output_before = render_settings_to_string(&app);
        assert!(output_before.contains("[ESC] Save & back"));

        // Enter editing mode
        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // After editing: shows editing hints
        let output_during = render_settings_to_string(&app);
        assert!(output_during.contains("[Enter] Confirm"));
        assert!(output_during.contains("[ESC] Cancel"));
        assert!(output_during.contains("[Tab] Complete"));
    }

    #[test]
    fn settings_show_dictionary_language_display_name() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;
        app.config.dictionary_language = "en".to_string();

        let output = render_settings_to_string(&app);
        assert!(output.contains("Dictionary Language"));
        assert!(output.contains("English"));
    }

    #[test]
    fn settings_show_keyboard_layout_value() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;
        app.config.keyboard_layout = "qwerty".to_string();

        let output = render_settings_to_string(&app);
        assert!(output.contains("Keyboard Layout"));
        assert!(output.contains("qwerty"));
    }

    #[test]
    fn settings_show_language_without_preview_label() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;
        app.set_dictionary_language("de")
            .expect("de should be selectable");

        let output = render_settings_to_string(&app);
        assert!(output.contains("Deutsch"));
        assert!(!output.contains("Deutsch (preview)"));
        assert!(output.contains("de_qwertz"));
        assert!(!output.contains("qwerty (preview)"));
    }

    #[test]
    fn settings_enter_on_dictionary_language_opens_selector() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;
        app.settings_selected = SettingItem::DictionaryLanguage.index();

        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.screen, AppScreen::DictionaryLanguageSelect);
        assert_eq!(app.dictionary_language_selected, 0);
    }

    #[test]
    fn settings_enter_on_keyboard_layout_opens_selector() {
        let mut app = test_app();
        app.screen = AppScreen::Settings;
        app.settings_selected = SettingItem::KeyboardLayout.index();

        handle_settings_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.screen, AppScreen::KeyboardLayoutSelect);
        assert_eq!(app.keyboard_layout_selected, 0);
    }

    #[test]
    fn dictionary_language_selector_confirm_applies_selection_and_returns_to_settings() {
        let mut app = test_app();
        app.go_to_dictionary_language_select();
        app.dictionary_language_selected = crate::l10n::language_pack::language_packs()
            .iter()
            .position(|pack| pack.language_key == "de")
            .expect("de language pack should exist");

        handle_dictionary_language_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.screen, AppScreen::Settings);
        assert_eq!(app.config.dictionary_language, "de");
        assert_eq!(
            app.settings_selected,
            SettingItem::DictionaryLanguage.index()
        );
        let status = app
            .settings_status_message
            .as_ref()
            .expect("language switch should show layout reset message");
        assert!(status.text.contains("Keyboard layout reset"));
        assert!(status.text.contains("de_qwertz"));
    }

    #[test]
    fn keyboard_layout_selector_confirm_keeps_selected_language() {
        let mut app = test_app();
        app.set_dictionary_language("es")
            .expect("es should be selectable");
        app.go_to_keyboard_layout_select();
        app.keyboard_layout_selected =
            crate::keyboard::model::KeyboardModel::supported_layout_keys()
                .iter()
                .position(|&key| key == "de_qwertz")
                .expect("de_qwertz layout should exist");

        handle_keyboard_layout_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.screen, AppScreen::Settings);
        assert_eq!(app.config.keyboard_layout, "de_qwertz");
        assert_eq!(app.config.dictionary_language, "es");
        assert_eq!(app.settings_selected, SettingItem::KeyboardLayout.index());
        assert!(app.settings_status_message.is_none());
    }

    #[test]
    fn dictionary_language_selector_zero_shortcut_is_ignored() {
        let mut app = test_app();
        app.go_to_dictionary_language_select();
        app.dictionary_language_selected = 3;

        handle_dictionary_language_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
        );

        assert_eq!(app.screen, AppScreen::DictionaryLanguageSelect);
        assert_eq!(app.dictionary_language_selected, 3);
    }

    #[test]
    fn keyboard_layout_selector_zero_shortcut_is_ignored() {
        let mut app = test_app();
        app.go_to_keyboard_layout_select();
        app.keyboard_layout_selected = 2;

        handle_keyboard_layout_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
        );

        assert_eq!(app.screen, AppScreen::KeyboardLayoutSelect);
        assert_eq!(app.keyboard_layout_selected, 2);
    }

    #[test]
    fn skill_tree_available_branch_enter_opens_unlock_confirm() {
        let mut app = test_app();
        app.screen = AppScreen::SkillTree;
        app.skill_tree_confirm_unlock = None;
        app.skill_tree
            .branch_progress_mut(engine::skill_tree::BranchId::Capitals)
            .status = engine::skill_tree::BranchStatus::Available;
        app.skill_tree_selected = selectable_branches()
            .iter()
            .position(|id| *id == engine::skill_tree::BranchId::Capitals)
            .unwrap();

        handle_skill_tree_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(
            app.skill_tree_confirm_unlock,
            Some(engine::skill_tree::BranchId::Capitals)
        );
        assert_eq!(
            *app.skill_tree
                .branch_status(engine::skill_tree::BranchId::Capitals),
            engine::skill_tree::BranchStatus::Available
        );
        assert_eq!(app.screen, AppScreen::SkillTree);
    }

    #[test]
    fn skill_tree_unlock_confirm_yes_unlocks_without_starting_drill() {
        let mut app = test_app();
        app.screen = AppScreen::SkillTree;
        app.skill_tree_confirm_unlock = Some(engine::skill_tree::BranchId::Capitals);
        app.skill_tree
            .branch_progress_mut(engine::skill_tree::BranchId::Capitals)
            .status = engine::skill_tree::BranchStatus::Available;

        handle_skill_tree_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        );

        assert_eq!(app.skill_tree_confirm_unlock, None);
        assert_eq!(
            *app.skill_tree
                .branch_status(engine::skill_tree::BranchId::Capitals),
            engine::skill_tree::BranchStatus::InProgress
        );
        assert_eq!(app.screen, AppScreen::SkillTree);
    }

    #[test]
    fn skill_tree_in_progress_branch_enter_starts_branch_drill() {
        let mut app = test_app();
        app.screen = AppScreen::SkillTree;
        app.skill_tree
            .branch_progress_mut(engine::skill_tree::BranchId::Capitals)
            .status = engine::skill_tree::BranchStatus::InProgress;
        app.skill_tree_selected = selectable_branches()
            .iter()
            .position(|id| *id == engine::skill_tree::BranchId::Capitals)
            .unwrap();

        handle_skill_tree_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.skill_tree_confirm_unlock, None);
        assert_eq!(app.screen, AppScreen::Drill);
        assert_eq!(
            app.drill_scope,
            DrillScope::Branch(engine::skill_tree::BranchId::Capitals)
        );
    }

    #[test]
    fn skill_tree_available_branch_footer_shows_unlock_hint() {
        let mut app = test_app();
        app.screen = AppScreen::SkillTree;
        app.skill_tree
            .branch_progress_mut(engine::skill_tree::BranchId::Capitals)
            .status = engine::skill_tree::BranchStatus::Available;
        app.skill_tree_selected = selectable_branches()
            .iter()
            .position(|id| *id == engine::skill_tree::BranchId::Capitals)
            .unwrap();

        let output = render_skill_tree_to_string(&app);
        assert!(output.contains("[Enter] Unlock"));
    }

    #[test]
    fn skill_tree_unlock_modal_shows_body_and_prompt_text() {
        let mut app = test_app();
        app.screen = AppScreen::SkillTree;
        app.skill_tree_confirm_unlock = Some(engine::skill_tree::BranchId::Capitals);

        let output = render_skill_tree_to_string(&app);
        assert!(output.contains("default adaptive drill will mix in keys"));
        assert!(output.contains("focus only on this branch"));
        assert!(output.contains("from this branch in the Skill Tree."));
        assert!(output.contains("[y] Unlock"));
    }

    #[test]
    fn skill_tree_unlock_modal_keeps_full_second_sentence_on_smaller_terminal() {
        let mut app = test_app();
        app.screen = AppScreen::SkillTree;
        app.skill_tree_confirm_unlock = Some(engine::skill_tree::BranchId::Capitals);

        let output = render_skill_tree_to_string_with_size(&app, 90, 24);
        assert!(output.contains("focus only on this branch"));
        assert!(output.contains("from this branch in the Skill Tree."));
        assert!(output.contains("[y] Unlock"));
    }

    #[test]
    fn milestone_popup_footer_shows_skill_tree_hint() {
        let mut app = test_app();
        app.screen = AppScreen::DrillResult;
        app.milestone_queue
            .push_back(crate::app::KeyMilestonePopup {
                kind: crate::app::MilestoneKind::BranchesAvailable,
                keys: vec![],
                finger_info: vec![],
                message: "msg".to_string(),
                branch_ids: vec![engine::skill_tree::BranchId::Capitals],
            });

        let output = render_app_to_string_with_size(&app, 100, 28);
        assert!(output.contains("[t] Open Skill Tree"));
    }

    #[test]
    fn keyboard_explorer_non_shifted_selection_clears_latched_shift() {
        let mut app = test_app();
        app.screen = AppScreen::Keyboard;
        app.shift_held = true;

        keyboard_explorer_select_key(&mut app, 'a');

        assert_eq!(app.keyboard_explorer_selected, Some('a'));
        assert!(!app.shift_held);
    }

    #[test]
    fn keyboard_explorer_shifted_selection_keeps_latched_shift() {
        let mut app = test_app();
        app.screen = AppScreen::Keyboard;
        app.shift_held = true;

        keyboard_explorer_select_key(&mut app, 'A');

        assert_eq!(app.keyboard_explorer_selected, Some('A'));
        assert!(app.shift_held);
    }

    #[test]
    fn skill_tree_layout_switches_with_width() {
        assert!(!use_side_by_side_layout(99));
        assert!(use_side_by_side_layout(100));
    }

    #[test]
    fn skill_tree_expanded_branch_spacing_threshold() {
        // 6 branches => base=13 lines, inter-branch spacing needs +5, separator padding needs +2.
        assert_eq!(
            crate::ui::components::skill_tree::branch_list_spacing_flags(17, 6),
            (false, false)
        );
        assert_eq!(
            crate::ui::components::skill_tree::branch_list_spacing_flags(18, 6),
            (true, false)
        );
        assert_eq!(
            crate::ui::components::skill_tree::branch_list_spacing_flags(20, 6),
            (true, true)
        );
    }

    #[test]
    fn skill_tree_expanded_level_spacing_threshold() {
        use crate::engine::skill_tree::BranchId;
        let id = BranchId::Capitals;
        let base = crate::ui::components::skill_tree::detail_line_count(id) as u16;
        // Capitals has 3 levels, so expanded spacing needs +2 lines.
        assert!(!crate::ui::components::skill_tree::use_expanded_level_spacing(base + 1, id));
        assert!(crate::ui::components::skill_tree::use_expanded_level_spacing(base + 2, id));
    }
}

fn render_result(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();

    if let Some(ref result) = app.last_result {
        let centered = ui::layout::centered_rect(60, 70, area);
        let dashboard = Dashboard::new(result, app.theme, app.post_drill_input_lock_remaining_ms());
        frame.render_widget(dashboard, centered);

        if app.history_confirm_delete && !app.drill_history.is_empty() {
            let colors = &app.theme.colors;
            let dialog_width = 34u16;
            let dialog_height = 5u16;
            let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
            let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
            let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

            let idx = app.drill_history.len().saturating_sub(app.history_selected);
            let dialog_text = t!("stats.delete_confirm", idx = idx);

            frame.render_widget(ratatui::widgets::Clear, dialog_area);
            let confirm_title = t!("stats.confirm_title");
            let dialog = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  {}  ", dialog_text),
                    Style::default().fg(colors.fg()),
                )),
            ])
            .style(Style::default().bg(colors.bg()))
            .block(
                Block::bordered()
                    .title(confirm_title.as_ref())
                    .border_style(Style::default().fg(colors.error()))
                    .style(Style::default().bg(colors.bg())),
            );
            frame.render_widget(dialog, dialog_area);
        }
    }
}

fn render_stats(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let ngram_data = if app.stats_tab == 5 {
        Some(build_ngram_tab_data(app))
    } else {
        None
    };
    let dashboard = StatsDashboard::new(
        &app.drill_history,
        &app.key_stats,
        app.stats_tab,
        app.config.target_wpm,
        app.skill_tree.total_unlocked_count(),
        app.skill_tree.total_confident_keys(&app.ranked_key_stats),
        app.skill_tree.total_unique_keys,
        app.theme,
        app.history_selected,
        app.history_scroll,
        app.history_confirm_delete,
        &app.keyboard_model,
        ngram_data.as_ref(),
    );
    frame.render_widget(dashboard, area);
}

fn keep_history_selection_visible(app: &mut App, page_size: usize) {
    let viewport = page_size.max(1);
    if app.history_selected < app.history_scroll {
        app.history_scroll = app.history_selected;
    } else if app.history_selected >= app.history_scroll + viewport {
        app.history_scroll = app.history_selected + 1 - viewport;
    }
}

fn current_history_page_size() -> usize {
    match crossterm::terminal::size() {
        Ok((w, h)) => history_page_size_for_terminal(w, h),
        Err(_) => 10,
    }
}

fn build_ngram_tab_data(app: &App) -> NgramTabData {
    use engine::ngram_stats::{self, select_focus};

    let focus = select_focus(
        &app.skill_tree,
        app.drill_scope,
        &app.ranked_key_stats,
        &app.ranked_bigram_stats,
    );

    let unlocked = app.skill_tree.unlocked_keys(app.drill_scope);

    let error_anomalies_raw = app
        .ranked_bigram_stats
        .error_anomaly_bigrams(&app.ranked_key_stats, &unlocked);
    let speed_anomalies_raw = app
        .ranked_bigram_stats
        .speed_anomaly_bigrams(&app.ranked_key_stats, &unlocked);

    let error_anomalies: Vec<AnomalyBigramRow> = error_anomalies_raw
        .iter()
        .map(|a| AnomalyBigramRow {
            bigram: format!("{}{}", a.key.0[0], a.key.0[1]),
            anomaly_pct: a.anomaly_pct,
            sample_count: a.sample_count,
            error_count: a.error_count,
            error_rate_ema: a.error_rate_ema,
            speed_ms: a.speed_ms,
            expected_baseline: a.expected_baseline,
            confirmed: a.confirmed,
        })
        .collect();

    let speed_anomalies: Vec<AnomalyBigramRow> = speed_anomalies_raw
        .iter()
        .map(|a| AnomalyBigramRow {
            bigram: format!("{}{}", a.key.0[0], a.key.0[1]),
            anomaly_pct: a.anomaly_pct,
            sample_count: a.sample_count,
            error_count: a.error_count,
            error_rate_ema: a.error_rate_ema,
            speed_ms: a.speed_ms,
            expected_baseline: a.expected_baseline,
            confirmed: a.confirmed,
        })
        .collect();

    let scope_label = match app.drill_scope {
        DrillScope::Global => "Global".to_string(),
        DrillScope::Branch(id) => format!("Branch: {}", id.to_key()),
    };

    let hesitation_threshold_ms = ngram_stats::hesitation_threshold(app.user_median_transition_ms);

    NgramTabData {
        focus,
        error_anomalies,
        speed_anomalies,
        total_bigrams: app.ranked_bigram_stats.stats.len(),
        hesitation_threshold_ms,
        scope_label,
    }
}

fn render_settings(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let centered = ui::layout::centered_rect(60, 80, area);

    let settings_title = t!("settings.title");
    let block = Block::bordered()
        .title(settings_title.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let fields = settings_fields(app);

    let header_height = if inner.height > 0 { 1 } else { 0 };

    // Compute footer hints early so we know how many lines they need.
    let completion_error = app
        .settings_editing_path
        .as_ref()
        .map(|(_, input)| input.completion_error)
        .unwrap_or(false);
    let fh_move = ui::hint::hint(ui::hint::K_ARROW_LR, t!("settings.hint_move").as_ref());
    let fh_tab = ui::hint::hint(ui::hint::K_TAB, t!("settings.hint_tab_complete").as_ref());
    let fh_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("settings.hint_confirm").as_ref());
    let fh_cancel = ui::hint::hint(ui::hint::K_ESC, t!("settings.hint_cancel").as_ref());
    let fh_save = ui::hint::hint(ui::hint::K_ESC, t!("settings.hint_save_back").as_ref());
    let fh_change = ui::hint::hint(ui::hint::K_ENTER_ARROWS, t!("settings.hint_change_value").as_ref());
    let fh_edit = ui::hint::hint(ui::hint::K_ENTER_ON_PATH, t!("settings.hint_edit_path").as_ref());
    let footer_hints: Vec<&str> = if app.is_editing_path() {
        let mut hints = vec![
            fh_move.as_str(),
            fh_tab.as_str(),
            fh_confirm.as_str(),
            fh_cancel.as_str(),
        ];
        if completion_error {
            hints.push("(cannot read directory)");
        }
        hints
    } else {
        vec![
            fh_save.as_str(),
            fh_change.as_str(),
            fh_edit.as_str(),
        ]
    };
    let footer_packed = pack_hint_lines(&footer_hints, inner.width as usize);
    let footer_height = if inner.height > header_height {
        (footer_packed.len() as u16).max(1)
    } else {
        0
    };

    let field_height = inner.height.saturating_sub(header_height + footer_height);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(field_height),
            Constraint::Length(footer_height),
        ])
        .split(inner);

    let subtitle = t!("settings.subtitle");
    let header = Paragraph::new(Line::from(Span::styled(
        format!("  {subtitle}"),
        Style::default().fg(colors.text_pending()),
    )));
    header.render(layout[0], frame.buffer_mut());

    let row_height = 2u16;
    let visible_rows = (layout[1].height / row_height).max(1) as usize;
    let max_start = fields.len().saturating_sub(visible_rows);
    let start = app
        .settings_selected
        .saturating_sub(visible_rows.saturating_sub(1))
        .min(max_start);
    let end = (start + visible_rows).min(fields.len());
    let visible_fields = &fields[start..end];

    let field_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            visible_fields
                .iter()
                .map(|_| Constraint::Length(row_height))
                .collect::<Vec<_>>(),
        )
        .split(layout[1]);

    for (row, (item, label, value)) in visible_fields.iter().enumerate() {
        let i = start + row;
        let is_selected = i == app.settings_selected;
        let indicator = if is_selected { " > " } else { "   " };

        let label_text = format!("{indicator}{label}:");
        let is_button = item.is_action_button();
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

        let is_editing_this_path = is_selected && item.is_path_field() && app.is_editing_field(i);
        let lines = if item.is_path_field() {
            if is_editing_this_path {
                if let Some((_, ref input)) = app.settings_editing_path {
                    let (before, cursor_ch, after) = input.render_parts();
                    let cursor_style = Style::default().fg(colors.bg()).bg(colors.focused_key());
                    let path_spans = match cursor_ch {
                        Some(ch) => vec![
                            Span::styled(format!("  {before}"), value_style),
                            Span::styled(ch.to_string(), cursor_style),
                            Span::styled(after.to_string(), value_style),
                        ],
                        None => vec![
                            Span::styled(format!("  {before}"), value_style),
                            Span::styled(" ", cursor_style),
                        ],
                    };
                    vec![
                        Line::from(Span::styled(
                            format!("{indicator}{label}: (editing)"),
                            label_style,
                        )),
                        Line::from(path_spans),
                    ]
                } else {
                    vec![
                        Line::from(Span::styled(label_text, label_style)),
                        Line::from(Span::styled(format!("  {value}"), value_style)),
                    ]
                }
            } else {
                vec![
                    Line::from(Span::styled(label_text, label_style)),
                    Line::from(Span::styled(format!("  {value}"), value_style)),
                ]
            }
        } else {
            vec![
                Line::from(Span::styled(label_text, label_style)),
                Line::from(Span::styled(value_text, value_style)),
            ]
        };
        Paragraph::new(lines).render(field_layout[row], frame.buffer_mut());
    }

    let footer_lines: Vec<Line> = footer_packed
        .into_iter()
        .map(|line| Line::from(Span::styled(line, Style::default().fg(colors.accent()))))
        .collect();
    Paragraph::new(footer_lines)
        .wrap(Wrap { trim: false })
        .render(layout[2], frame.buffer_mut());

    // --- Overlay dialogs (rendered on top of settings) ---

    // Status message takes highest priority
    if let Some(ref msg) = app.settings_status_message {
        let border_color = match msg.kind {
            StatusKind::Success => colors.accent(),
            StatusKind::Error => colors.error(),
        };
        let title_t = match msg.kind {
            StatusKind::Success => t!("settings.success_title"),
            StatusKind::Error => t!("settings.error_title"),
        };
        let title = title_t.as_ref();
        let dialog_width = 56u16.min(area.width.saturating_sub(4));
        let dialog_height = 6u16;
        let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
        let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        frame.render_widget(ratatui::widgets::Clear, dialog_area);
        let dialog = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {}  ", msg.text),
                Style::default().fg(colors.fg()),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  {}", t!("settings.press_any_key")),
                Style::default().fg(colors.text_pending()),
            )),
        ])
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(colors.bg()))
        .block(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(colors.bg())),
        );
        frame.render_widget(dialog, dialog_area);
    } else if app.settings_export_conflict {
        let dialog_width = 52u16.min(area.width.saturating_sub(4));
        let dialog_height = 6u16;
        let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
        let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        frame.render_widget(ratatui::widgets::Clear, dialog_area);
        let file_exists_msg = t!("settings.file_exists");
        let overwrite_rename = t!("settings.overwrite_rename");
        let dialog = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {file_exists_msg}"),
                Style::default().fg(colors.fg()),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  {overwrite_rename}"),
                Style::default().fg(colors.text_pending()),
            )),
        ])
        .style(Style::default().bg(colors.bg()))
        .block({
            let fe_title = t!("settings.file_exists_title");
            Block::bordered()
                .title(fe_title.to_string())
                .border_style(Style::default().fg(colors.error()))
                .style(Style::default().bg(colors.bg()))
        });
        frame.render_widget(dialog, dialog_area);
    } else if app.settings_confirm_import {
        let dialog_width = 52u16.min(area.width.saturating_sub(4));
        let dialog_height = 7u16;
        let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
        let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        frame.render_widget(ratatui::widgets::Clear, dialog_area);
        let erase_warning = t!("settings.erase_warning");
        let export_first = t!("settings.export_first");
        let proceed_yn = t!("settings.proceed_yn");
        let dialog = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {erase_warning}"),
                Style::default().fg(colors.fg()),
            )),
            Line::from(Span::styled(
                format!("  {export_first}"),
                Style::default().fg(colors.text_pending()),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  {proceed_yn}"),
                Style::default().fg(colors.fg()),
            )),
        ])
        .style(Style::default().bg(colors.bg()))
        .block({
            let ci_title = t!("settings.confirm_import_title");
            Block::bordered()
                .title(ci_title.to_string())
                .border_style(Style::default().fg(colors.error()))
                .style(Style::default().bg(colors.bg()))
        });
        frame.render_widget(dialog, dialog_area);
    }
}

fn render_dictionary_language_select(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(60, 70, area);

    let sel_title = t!("select.dictionary_language_title");
    let block = Block::bordered()
        .title(sel_title.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let options = language_packs();
    let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN_PGUP_PGDN, t!("select.hint_navigate").as_ref());
    let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
    let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
    let footer_hints: Vec<&str> = vec![h_nav.as_str(), h_confirm.as_str(), h_back.as_str()];
    let support_notice_t = t!("select.language_resets_layout");
    let support_notice = support_notice_t.as_ref();
    let width = inner.width as usize;
    let hint_lines_vec = pack_hint_lines(&footer_hints, width);
    let hint_lines = hint_lines_vec.len();
    let notice_lines = wrapped_line_count(support_notice, width);
    let total_height = inner.height as usize;
    let show_notice = total_height >= hint_lines + notice_lines + 3;
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
    let scroll = app.dictionary_language_scroll;
    let visible_end = (scroll + viewport_height).min(options.len());

    let mut lines: Vec<Line> = Vec::new();
    if scroll > 0 {
        let more_above = t!("select.more_above", count = scroll);
        lines.push(Line::from(Span::styled(
            format!("   {more_above}"),
            Style::default().fg(colors.text_pending()),
        )));
    } else {
        lines.push(Line::from(""));
    }

    for i in scroll..visible_end {
        let pack = options[i];
        let is_selected = i == app.dictionary_language_selected;
        let is_current = pack.language_key == app.config.dictionary_language;
        let indicator = if is_selected { " > " } else { "   " };
        let current_marker_t = if is_current { t!("select.current") } else { std::borrow::Cow::Borrowed("") };
        let current_marker = current_marker_t.as_ref();
        let is_disabled = is_dictionary_language_disabled(app, pack.language_key);
        let default_layout =
            default_keyboard_layout_for_language(pack.language_key).unwrap_or("unknown");
        let availability = if is_disabled {
            t!("select.disabled").to_string()
        } else {
            t!("select.enabled_default", layout = default_layout).to_string()
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
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "{indicator}{} ({}){current_marker}",
                    pack.display_name, pack.language_key
                ),
                name_style,
            ),
            Span::styled(availability, status_style),
        ]));
    }

    if visible_end < options.len() {
        let more_below = t!("select.more_below", count = options.len() - visible_end);
        lines.push(Line::from(Span::styled(
            format!("   {more_below}"),
            Style::default().fg(colors.text_pending()),
        )));
    } else {
        lines.push(Line::from(""));
    }

    Paragraph::new(lines).render(list_area, frame.buffer_mut());

    if let Some(footer) = footer_area {
        let mut footer_lines: Vec<Line> = hint_lines_vec
            .iter()
            .map(|line| {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(colors.text_pending()),
                ))
            })
            .collect();
        if show_notice {
            footer_lines.push(Line::from(Span::styled(
                support_notice,
                Style::default().fg(colors.text_pending()),
            )));
        }
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer, frame.buffer_mut());
    }
}

fn render_ui_language_select(frame: &mut ratatui::Frame, app: &App) {
    use crate::i18n::t;
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(50, 50, area);

    let title = t!("select.ui_language_title");
    let block = Block::bordered()
        .title(title.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let locales = i18n::SUPPORTED_UI_LOCALES;
    let hint_back = t!("select.hint_back");
    let hint_confirm = t!("select.hint_confirm");
    let footer_hints = [hint_back.as_ref(), hint_confirm.as_ref()];
    let footer_lines = pack_hint_lines(&footer_hints, inner.width as usize);
    let footer_h = footer_lines.len().max(1) as u16;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(footer_h)])
        .split(inner);

    let mut lines: Vec<Line> = Vec::new();
    for (i, &locale) in locales.iter().enumerate() {
        let is_selected = i == app.ui_language_selected;
        let is_current = locale == app.config.ui_language;
        let autonym = find_language_pack(locale)
            .map(|p| p.autonym)
            .unwrap_or(locale);
        let suffix = if is_current {
            t!("select.current").to_string()
        } else {
            String::new()
        };
        let indicator = if is_selected { "> " } else { "  " };
        let label = format!("{indicator}{autonym}{suffix}");
        let style = if is_selected {
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.fg())
        };
        lines.push(Line::from(Span::styled(label, style)));
    }

    let list = Paragraph::new(lines);
    list.render(chunks[0], frame.buffer_mut());

    let footer: Vec<Line> = footer_lines
        .into_iter()
        .map(|l| Line::from(Span::styled(l, Style::default().fg(colors.text_pending()))))
        .collect();
    Paragraph::new(footer).render(chunks[1], frame.buffer_mut());
}

fn render_keyboard_layout_select(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(60, 70, area);

    let sel_title = t!("select.keyboard_layout_title");
    let block = Block::bordered()
        .title(sel_title.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let options = keyboard::model::KeyboardModel::supported_layout_keys();
    let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN_PGUP_PGDN, t!("select.hint_navigate").as_ref());
    let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
    let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
    let footer_hints: Vec<&str> = vec![h_nav.as_str(), h_confirm.as_str(), h_back.as_str()];
    let support_notice_t = t!("select.layout_no_language_change");
    let support_notice = support_notice_t.as_ref();
    let width = inner.width as usize;
    let hint_lines_vec = pack_hint_lines(&footer_hints, width);
    let hint_lines = hint_lines_vec.len();
    let notice_lines = wrapped_line_count(support_notice, width);
    let total_height = inner.height as usize;
    let show_notice = total_height >= hint_lines + notice_lines + 3;
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
    let scroll = app.keyboard_layout_scroll;
    let visible_end = (scroll + viewport_height).min(options.len());

    let mut lines: Vec<Line> = Vec::new();
    if scroll > 0 {
        let more_above = t!("select.more_above", count = scroll);
        lines.push(Line::from(Span::styled(
            format!("   {more_above}"),
            Style::default().fg(colors.text_pending()),
        )));
    } else {
        lines.push(Line::from(""));
    }

    for i in scroll..visible_end {
        let key = options[i];
        let is_selected = i == app.keyboard_layout_selected;
        let is_current = key == app.config.keyboard_layout;
        let indicator = if is_selected { " > " } else { "   " };
        let current_marker_t = if is_current { t!("select.current") } else { std::borrow::Cow::Borrowed("") };
        let current_marker = current_marker_t.as_ref();
        let validation = validate_language_layout_pair(&app.config.dictionary_language, key);

        let (availability, is_disabled) = match validation {
            Ok(CapabilityState::Enabled) => (t!("select.enabled").to_string(), false),
            Ok(CapabilityState::Disabled) => (t!("select.disabled").to_string(), true),
            Err(crate::l10n::language_pack::LanguageLayoutValidationError::UnsupportedLanguageLayoutPair { .. }) => {
                (t!("select.disabled").to_string(), true)
            }
            Err(crate::l10n::language_pack::LanguageLayoutValidationError::LanguageBlockedBySupportLevel(_)) => {
                (t!("select.disabled_blocked").to_string(), true)
            }
            Err(crate::l10n::language_pack::LanguageLayoutValidationError::UnknownLanguage(_))
            | Err(crate::l10n::language_pack::LanguageLayoutValidationError::UnknownLayout(_)) => {
                (t!("select.disabled").to_string(), true)
            }
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
        lines.push(Line::from(vec![
            Span::styled(format!("{indicator}{key}{current_marker}"), name_style),
            Span::styled(availability, status_style),
        ]));
    }

    if visible_end < options.len() {
        let more_below = t!("select.more_below", count = options.len() - visible_end);
        lines.push(Line::from(Span::styled(
            format!("   {more_below}"),
            Style::default().fg(colors.text_pending()),
        )));
    } else {
        lines.push(Line::from(""));
    }

    Paragraph::new(lines).render(list_area, frame.buffer_mut());

    if let Some(footer) = footer_area {
        let mut footer_lines: Vec<Line> = hint_lines_vec
            .iter()
            .map(|line| {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(colors.text_pending()),
                ))
            })
            .collect();
        if show_notice {
            footer_lines.push(Line::from(Span::styled(
                support_notice,
                Style::default().fg(colors.text_pending()),
            )));
        }
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer, frame.buffer_mut());
    }
}

fn render_code_language_select(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(50, 70, area);

    let sel_title = t!("select.code_language_title");
    let block = Block::bordered()
        .title(sel_title.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let options = code_language_options();
    let cache_dir = &app.config.code_download_dir;
    let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN_PGUP_PGDN, t!("select.hint_navigate").as_ref());
    let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
    let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
    let footer_hints: Vec<&str> = vec![h_nav.as_str(), h_confirm.as_str(), h_back.as_str()];
    let disabled_notice_t = t!("select.disabled_sources_notice");
    let disabled_notice = disabled_notice_t.as_ref();
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
        let more_above = t!("select.more_above", count = scroll);
        lines.push(Line::from(Span::styled(
            format!("   {more_above}"),
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
        let current_marker_t = if is_current { t!("select.current") } else { std::borrow::Cow::Borrowed("") };
        let current_marker = current_marker_t.as_ref();

        // Determine availability label
        let availability = if *key == "all" {
            String::new()
        } else if let Some(lang) = language_by_key(key) {
            if lang.has_builtin {
                t!("select.built_in").to_string()
            } else if is_language_cached(cache_dir, key) {
                t!("select.cached").to_string()
            } else if is_disabled {
                t!("select.disabled_download").to_string()
            } else {
                t!("select.download_required").to_string()
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
        let more_below = t!("select.more_below", count = options.len() - visible_end);
        lines.push(Line::from(Span::styled(
            format!("   {more_below}"),
            Style::default().fg(colors.text_pending()),
        )));
    } else {
        lines.push(Line::from(""));
    }

    Paragraph::new(lines).render(list_area, frame.buffer_mut());

    if let Some(footer) = footer_area {
        let mut footer_lines: Vec<Line> = hint_lines_vec
            .iter()
            .map(|line| {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(colors.text_pending()),
                ))
            })
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

    let sel_title = t!("select.passage_source_title");
    let block = Block::bordered()
        .title(sel_title.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let options = passage_options();
    let h_nav = ui::hint::hint(ui::hint::K_UP_DOWN, t!("select.hint_navigate").as_ref());
    let h_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("select.hint_confirm").as_ref());
    let h_back = ui::hint::hint(ui::hint::K_Q_ESC, t!("select.hint_back").as_ref());
    let footer_hints: Vec<&str> = vec![h_nav.as_str(), h_confirm.as_str(), h_back.as_str()];
    let disabled_notice_t = t!("select.disabled_sources_notice");
    let disabled_notice = disabled_notice_t.as_ref();
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
            t!("select.built_in").to_string()
        } else if is_book_cached(&app.config.passage_download_dir, key) {
            t!("select.cached").to_string()
        } else if is_disabled {
            t!("select.disabled_download").to_string()
        } else {
            t!("select.download_required").to_string()
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
            .map(|line| {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(colors.text_pending()),
                ))
            })
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

    let intro_title = t!("intro.passage_title");
    let block = Block::bordered()
        .title(intro_title.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let paragraphs_value = if app.passage_intro_paragraph_limit == 0 {
        t!("intro.whole_book").to_string()
    } else {
        app.passage_intro_paragraph_limit.to_string()
    };

    let enable_label = t!("intro.enable_downloads");
    let dir_label = t!("intro.download_dir");
    let para_label = t!("intro.paragraphs_per_book");
    let start_label = t!("intro.start_passage_drill");
    let confirm_label = t!("intro.confirm");
    let fields = vec![
        (
            enable_label.as_ref(),
            if app.passage_intro_downloads_enabled {
                t!("settings.on").to_string()
            } else {
                t!("settings.off").to_string()
            },
        ),
        (dir_label.as_ref(), app.passage_intro_download_dir.clone()),
        (para_label.as_ref(), paragraphs_value),
        (start_label.as_ref(), confirm_label.to_string()),
    ];

    let instr1 = t!("intro.passage_instructions_1");
    let instr2 = t!("intro.passage_instructions_2");
    let instr3 = t!("intro.passage_instructions_3");
    let mut lines = vec![
        Line::from(Span::styled(
            instr1.as_ref(),
            Style::default()
                .fg(colors.fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            instr2.as_ref(),
            Style::default().fg(colors.text_pending()),
        )),
        Line::from(Span::styled(
            instr3.as_ref(),
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
            format!(" {}", t!("intro.downloading_book_progress", bar = bar, downloaded = done_bytes, total = total_bytes))
        } else {
            format!(" {}", t!("intro.downloading_book_bytes", bytes = done_bytes))
        };
        lines.push(Line::from(Span::styled(
            progress_text,
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        )));
        if !app.passage_intro_current_book.is_empty() {
            let current_text = t!("intro.current_book",
                name = app.passage_intro_current_book.as_str(),
                done = done_books.saturating_add(1).min(total_books),
                total = total_books
            );
            lines.push(Line::from(Span::styled(
                format!(" {current_text}"),
                Style::default().fg(colors.text_pending()),
            )));
        }
    }
    let hint_lines = if app.passage_intro_downloading {
        Vec::new()
    } else {
        let ih_nav = ui::hint::hint(ui::hint::K_UP_DOWN, t!("intro.hint_navigate").as_ref());
        let ih_adj = ui::hint::hint(ui::hint::K_LEFT_RIGHT, t!("intro.hint_adjust").as_ref());
        let ih_edit = ui::hint::hint(ui::hint::K_TYPE_BACKSPACE, t!("intro.hint_edit").as_ref());
        let ih_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("intro.hint_confirm").as_ref());
        let ih_cancel = ui::hint::hint(ui::hint::K_Q_ESC, t!("intro.hint_cancel").as_ref());
        let hints: Vec<&str> = vec![ih_nav.as_str(), ih_adj.as_str(), ih_edit.as_str(), ih_confirm.as_str(), ih_cancel.as_str()];
        pack_hint_lines(
            &hints,
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
        footer_lines.extend(hint_lines.into_iter().map(|hint| {
            Line::from(Span::styled(
                hint,
                Style::default().fg(colors.text_pending()),
            ))
        }));
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer, frame.buffer_mut());
    }
}

fn render_passage_download_progress(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(60, 35, area);

    let dl_title = t!("intro.download_passage_title");
    let block = Block::bordered()
        .title(dl_title.as_ref())
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
        t!("intro.preparing_download").to_string()
    } else {
        app.passage_intro_current_book.clone()
    };

    let book_label = t!("intro.book_label", name = book_name);
    let lines = vec![
        Line::from(Span::styled(
            book_label.as_ref(),
            Style::default()
                .fg(colors.fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            if total_bytes > 0 {
                t!("intro.progress_bytes", name = bar, downloaded = done_bytes, total = total_bytes).to_string()
            } else {
                t!("intro.downloaded_bytes", bytes = done_bytes).to_string()
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

    let intro_title = t!("intro.code_title");
    let block = Block::bordered()
        .title(intro_title.as_ref())
        .border_style(Style::default().fg(colors.accent()))
        .style(Style::default().bg(colors.bg()));
    let inner = block.inner(centered);
    block.render(centered, frame.buffer_mut());

    let snippets_value = if app.code_intro_snippets_per_repo == 0 {
        t!("intro.unlimited").to_string()
    } else {
        app.code_intro_snippets_per_repo.to_string()
    };

    let enable_label = t!("intro.enable_downloads");
    let dir_label = t!("intro.download_dir");
    let snippets_label = t!("intro.snippets_per_repo");
    let start_label = t!("intro.start_code_drill");
    let confirm_label = t!("intro.confirm");
    let fields = vec![
        (
            enable_label.as_ref(),
            if app.code_intro_downloads_enabled {
                t!("settings.on").to_string()
            } else {
                t!("settings.off").to_string()
            },
        ),
        (dir_label.as_ref(), app.code_intro_download_dir.clone()),
        (snippets_label.as_ref(), snippets_value),
        (start_label.as_ref(), confirm_label.to_string()),
    ];

    let instr1 = t!("intro.code_instructions_1");
    let instr2 = t!("intro.code_instructions_2");
    let instr3 = t!("intro.code_instructions_3");
    let mut lines = vec![
        Line::from(Span::styled(
            instr1.as_ref(),
            Style::default()
                .fg(colors.fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            instr2.as_ref(),
            Style::default().fg(colors.text_pending()),
        )),
        Line::from(Span::styled(
            instr3.as_ref(),
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
            format!(" {}", t!("intro.downloading_code_progress", bar = bar, downloaded = done_bytes, total = total_bytes))
        } else {
            format!(" {}", t!("intro.downloading_code_bytes", bytes = done_bytes))
        };
        lines.push(Line::from(Span::styled(
            progress_text,
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        )));
        if !app.code_intro_current_repo.is_empty() {
            let current_text = t!("intro.current_repo",
                name = app.code_intro_current_repo.as_str(),
                done = done_repos.saturating_add(1).min(total_repos),
                total = total_repos
            );
            lines.push(Line::from(Span::styled(
                format!(" {current_text}"),
                Style::default().fg(colors.text_pending()),
            )));
        }
    }
    let hint_lines = if app.code_intro_downloading {
        Vec::new()
    } else {
        let ih_nav = ui::hint::hint(ui::hint::K_UP_DOWN, t!("intro.hint_navigate").as_ref());
        let ih_adj = ui::hint::hint(ui::hint::K_LEFT_RIGHT, t!("intro.hint_adjust").as_ref());
        let ih_edit = ui::hint::hint(ui::hint::K_TYPE_BACKSPACE, t!("intro.hint_edit").as_ref());
        let ih_confirm = ui::hint::hint(ui::hint::K_ENTER, t!("intro.hint_confirm").as_ref());
        let ih_cancel = ui::hint::hint(ui::hint::K_Q_ESC, t!("intro.hint_cancel").as_ref());
        let hints: Vec<&str> = vec![ih_nav.as_str(), ih_adj.as_str(), ih_edit.as_str(), ih_confirm.as_str(), ih_cancel.as_str()];
        pack_hint_lines(
            &hints,
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
        footer_lines.extend(hint_lines.into_iter().map(|hint| {
            Line::from(Span::styled(
                hint,
                Style::default().fg(colors.text_pending()),
            ))
        }));
        Paragraph::new(footer_lines)
            .wrap(Wrap { trim: false })
            .render(footer, frame.buffer_mut());
    }
}

fn render_code_download_progress(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = ui::layout::centered_rect(60, 35, area);

    let dl_title = t!("intro.download_code_title");
    let block = Block::bordered()
        .title(dl_title.as_ref())
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
        t!("intro.preparing_download").to_string()
    } else {
        app.code_intro_current_repo.clone()
    };

    let repo_label = t!("intro.repo_label", name = repo_name);
    let cancel_hint = t!("intro.hint_cancel");
    let lines = vec![
        Line::from(Span::styled(
            repo_label.as_ref(),
            Style::default()
                .fg(colors.fg())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            if total_bytes > 0 {
                t!("intro.progress_bytes", name = bar, downloaded = done_bytes, total = total_bytes).to_string()
            } else {
                t!("intro.downloaded_bytes", bytes = done_bytes).to_string()
            },
            Style::default().fg(colors.accent()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!(" {}", cancel_hint.as_ref()),
            Style::default().fg(colors.text_pending()),
        )),
    ];

    Paragraph::new(lines).render(inner, frame.buffer_mut());
}

fn render_skill_tree(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;
    let centered = skill_tree_popup_rect(area);
    let widget = SkillTreeWidget::new(
        &app.skill_tree,
        &app.ranked_key_stats,
        app.skill_tree_selected,
        app.skill_tree_detail_scroll,
        app.theme,
    );
    frame.render_widget(widget, centered);

    if let Some(branch_id) = app.skill_tree_confirm_unlock {
        let sentence_one_t = t!("skill_tree.unlock_msg_1");
        let sentence_one = sentence_one_t.as_ref();
        let sentence_two_t = t!("skill_tree.unlock_msg_2");
        let sentence_two = sentence_two_t.as_ref();
        let branch_name = engine::skill_tree::get_branch_definition(branch_id).display_name();
        let dialog_width = 72u16.min(area.width.saturating_sub(4));
        let content_width = dialog_width.saturating_sub(6).max(1) as usize; // border + side margins
        let body_required = 4 // blank + title + blank + blank-between-sentences
            + wrapped_line_count(sentence_one, content_width)
            + wrapped_line_count(sentence_two, content_width);
        // Add one safety line because `wrapped_line_count` is a cheap estimator.
        let body_required = body_required + 1;
        let min_dialog_height = (body_required + 1 + 2) as u16; // body + prompt + border
        let preferred_dialog_height = (body_required + 2 + 2) as u16; // + blank before prompt
        let max_dialog_height = area.height.saturating_sub(1).max(7);
        let dialog_height = preferred_dialog_height
            .min(max_dialog_height)
            .max(min_dialog_height.min(max_dialog_height));
        let dialog_x = area.x + area.width.saturating_sub(dialog_width) / 2;
        let dialog_y = area.y + area.height.saturating_sub(dialog_height) / 2;
        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        frame.render_widget(ratatui::widgets::Clear, dialog_area);
        let confirm_title = t!("stats.confirm_title");
        let block = Block::bordered()
            .title(confirm_title.as_ref())
            .border_style(Style::default().fg(colors.error()))
            .style(Style::default().bg(colors.bg()));
        let inner = block.inner(dialog_area);
        frame.render_widget(block, dialog_area);
        let content = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(1),
                Constraint::Length(2),
            ])
            .split(inner)[1];
        let prompt_block_height = if content.height as usize > body_required + 1 {
            2
        } else {
            1
        };
        let content_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(prompt_block_height)])
            .split(content);
        let unlock_prompt = t!("skill_tree.confirm_unlock", branch = branch_name);
        let body = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                unlock_prompt.to_string(),
                Style::default().fg(colors.fg()),
            )),
            Line::from(""),
            Line::from(Span::styled(
                sentence_one,
                Style::default().fg(colors.text_pending()),
            )),
            Line::from(""),
            Line::from(Span::styled(
                sentence_two,
                Style::default().fg(colors.text_pending()),
            )),
        ])
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(colors.bg()));
        frame.render_widget(body, content_layout[0]);
        let confirm_yn = t!("skill_tree.confirm_yn");
        let confirm_lines = if prompt_block_height > 1 {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    confirm_yn.as_ref(),
                    Style::default().fg(colors.fg()),
                )),
            ]
        } else {
            vec![Line::from(Span::styled(
                confirm_yn.as_ref(),
                Style::default().fg(colors.fg()),
            ))]
        };
        let confirm = Paragraph::new(confirm_lines).style(Style::default().bg(colors.bg()));
        frame.render_widget(confirm, content_layout[1]);
    }
}

fn handle_keyboard_explorer_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.go_to_menu(),
        KeyCode::Char('q') if app.keyboard_explorer_selected.is_none() => app.go_to_menu(),
        KeyCode::Char(ch) => {
            keyboard_explorer_select_key(app, ch);
        }
        KeyCode::Tab => {
            keyboard_explorer_select_key(app, '\t');
        }
        KeyCode::Enter => {
            keyboard_explorer_select_key(app, '\n');
        }
        KeyCode::Backspace => {
            keyboard_explorer_select_key(app, '\x08');
        }
        _ => {}
    }
}

fn keyboard_explorer_select_key(app: &mut App, ch: char) {
    app.keyboard_explorer_selected = Some(ch);
    app.key_accuracy(ch, false);
    app.key_accuracy(ch, true);
    if app.shift_held && app.keyboard_model.shifted_to_base(ch).is_none() {
        app.shift_held = false;
    }
}

fn handle_keyboard_explorer_mouse(app: &mut App, mouse: MouseEvent) {
    if !matches!(
        mouse.kind,
        MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Down(MouseButton::Right)
    ) {
        return;
    }
    let area = terminal_area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);
    let h_back = ui::hint::hint(ui::hint::K_ESC, t!("keyboard.hint_back").as_ref());
    let footer_hints: Vec<&str> = vec![h_back.as_str()];
    if hint_token_at(layout[3], &footer_hints, mouse.column, mouse.row).is_some()
        || point_in_rect(mouse.column, mouse.row, layout[3])
    {
        app.go_to_menu();
        return;
    }

    if point_in_rect(mouse.column, mouse.row, layout[1]) {
        if KeyboardDiagram::shift_at_position(
            layout[1],
            &app.keyboard_model,
            false,
            mouse.column,
            mouse.row,
        ) {
            app.shift_held = !app.shift_held;
            return;
        }

        if let Some(mut ch) = KeyboardDiagram::key_at_position(
            layout[1],
            &app.keyboard_model,
            false,
            mouse.column,
            mouse.row,
        ) {
            if app.shift_held
                && let Some(shifted) = app.keyboard_model.base_to_shifted(ch)
            {
                ch = shifted;
            }
            keyboard_explorer_select_key(app, ch);
        }
    }
}

fn render_keyboard_explorer(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let colors = &app.theme.colors;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(8), // keyboard diagram
            Constraint::Min(3),    // detail panel
            Constraint::Length(1), // footer
        ])
        .split(area);

    // Header
    let kbd_title = t!("keyboard.title");
    let kbd_hint_nav = t!("keyboard.subtitle");
    let header_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            kbd_title.as_ref(),
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            kbd_hint_nav.as_ref(),
            Style::default().fg(colors.text_pending()),
        )),
    ];
    let header = Paragraph::new(header_lines).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(header, layout[0]);

    // Keyboard diagram
    let unlocked = app.skill_tree.unlocked_keys(DrillScope::Global);
    let kbd = KeyboardDiagram::new(
        None,
        &unlocked,
        &app.depressed_keys,
        app.theme,
        &app.keyboard_model,
    )
    .selected_key(app.keyboard_explorer_selected)
    .shift_held(app.shift_held)
    .caps_lock(app.caps_lock);
    frame.render_widget(kbd, layout[1]);

    // Detail panel
    render_keyboard_detail_panel(frame, app, layout[2]);

    // Footer
    let kbd_back = ui::hint::hint(ui::hint::K_ESC, t!("keyboard.hint_back").as_ref());
    let footer = Paragraph::new(Line::from(vec![Span::styled(
        format!(" {} ", kbd_back.as_str()),
        Style::default().fg(colors.text_pending()),
    )]));
    frame.render_widget(footer, layout[3]);
}

fn render_keyboard_detail_panel(frame: &mut ratatui::Frame, app: &App, area: Rect) {
    let colors = &app.theme.colors;

    let selected = match app.keyboard_explorer_selected {
        Some(ch) => ch,
        None => {
            let press_hint = t!("keyboard.press_key_hint");
            let key_details_title = t!("keyboard.key_details");
            let hint = Paragraph::new(Line::from(Span::styled(
                press_hint.as_ref(),
                Style::default().fg(colors.text_pending()),
            )))
            .alignment(ratatui::layout::Alignment::Center)
            .block(
                Block::bordered()
                    .border_style(Style::default().fg(colors.border()))
                    .title(key_details_title.to_string()),
            );
            frame.render_widget(hint, area);
            return;
        }
    };

    // Build display name for title
    let display_name = key_display_name(selected);
    let title = if display_name.is_empty() {
        t!("keyboard.key_details_char", ch = selected).to_string()
    } else {
        t!("keyboard.key_details_name", name = display_name).to_string()
    };

    let block = Block::bordered()
        .border_style(Style::default().fg(colors.border()))
        .title(Span::styled(
            title,
            Style::default()
                .fg(colors.accent())
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let finger = app.keyboard_model.finger_for_char(selected);
    let is_shifted =
        selected.is_uppercase() || app.keyboard_model.shifted_to_base(selected).is_some();
    let shift_guidance = if is_shifted {
        if finger.hand == Hand::Left {
            t!("milestones.hold_right_shift").to_string()
        } else {
            t!("milestones.hold_left_shift").to_string()
        }
    } else {
        t!("keyboard.shift_no").to_string()
    };

    let unlocked_keys = app.skill_tree.unlocked_keys(DrillScope::Global);
    let is_unlocked = unlocked_keys.contains(&selected);
    let focus_key = app
        .skill_tree
        .focused_key(DrillScope::Global, &app.ranked_key_stats);
    let in_focus = focus_key == Some(selected);

    let overall_stat = app.key_stats.get_stat(selected);
    let ranked_stat = app.ranked_key_stats.get_stat(selected);
    let overall_acc = app
        .explorer_accuracy_cache_overall
        .filter(|(key, _, _)| *key == selected);
    let ranked_acc = app
        .explorer_accuracy_cache_ranked
        .filter(|(key, _, _)| *key == selected);

    let fmt_avg_time = |stat: Option<&crate::engine::key_stats::KeyStat>| -> String {
        if let Some(stat) = stat {
            if stat.sample_count > 0 {
                return format!("{:.0}ms", stat.filtered_time_ms);
            }
        }
        t!("keyboard.no_data_short").to_string()
    };
    let fmt_best_time = |stat: Option<&crate::engine::key_stats::KeyStat>| -> String {
        if let Some(stat) = stat {
            if stat.sample_count > 0 {
                let best = if stat.best_time_ms < f64::MAX {
                    stat.best_time_ms
                } else {
                    stat.filtered_time_ms
                };
                return format!("{best:.0}ms");
            }
        }
        t!("keyboard.no_data_short").to_string()
    };
    let fmt_samples = |stat: Option<&crate::engine::key_stats::KeyStat>| -> String {
        stat.map(|s| s.sample_count.to_string())
            .unwrap_or_else(|| "0".to_string())
    };
    let fmt_acc = |entry: Option<(char, usize, usize)>| -> String {
        if let Some((_, correct, total)) = entry {
            if total > 0 {
                let pct = (correct as f64 / total as f64) * 100.0;
                return format!("{:.1}% ({}/{})", pct, correct, total);
            }
        }
        t!("keyboard.no_data_short").to_string()
    };

    let branch_info = find_key_branch(selected)
        .map(|(branch, level, pos)| (branch.display_name(), format!("{} (key #{pos})", level.display_name())));

    // Ranked-only mastery display (same semantics as skill tree per-key progress)
    let ranked_conf = app.ranked_key_stats.get_confidence(selected).min(1.0);
    let mastery_bar_width = 10usize;
    let filled = (ranked_conf * mastery_bar_width as f64).round() as usize;
    let mastery_bar = format!(
        "{}{}",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(mastery_bar_width.saturating_sub(filled))
    );
    let mastery_text = format!("{mastery_bar} {:>3.0}%", ranked_conf * 100.0);

    let mut left_col: Vec<String> = vec![
        format!("{}{}", t!("keyboard.finger_label"), finger.localized_description()),
        format!("{}{shift_guidance}", t!("keyboard.shift_label")),
        format!("{}{}", t!("keyboard.overall_avg_time"), fmt_avg_time(overall_stat)),
        format!("{}{}", t!("keyboard.overall_best_time"), fmt_best_time(overall_stat)),
        format!("{}{}", t!("keyboard.overall_samples"), fmt_samples(overall_stat)),
        format!("{}{}", t!("keyboard.overall_accuracy_label"), fmt_acc(overall_acc)),
    ];

    let mut right_col: Vec<String> = Vec::new();
    if let Some((branch_name, level_name)) = branch_info {
        right_col.push(format!("{}{branch_name}", t!("keyboard.branch_label")));
        right_col.push(format!("{}{level_name}", t!("keyboard.level_label")));
    } else {
        right_col.push(t!("keyboard.built_in_key").to_string());
    }
    let yes_t = t!("keyboard.yes");
    let no_t = t!("keyboard.no");
    right_col.push(format!(
        "{}{}",
        t!("keyboard.unlocked_label"),
        if is_unlocked { yes_t.as_ref() } else { no_t.as_ref() }
    ));
    let yes_t2 = t!("keyboard.yes");
    let no_t2 = t!("keyboard.no");
    right_col.push(format!(
        "{}{}",
        t!("keyboard.in_focus_label"),
        if in_focus { yes_t2.as_ref() } else { no_t2.as_ref() }
    ));
    if is_unlocked {
        right_col.push(format!("{}{mastery_text}", t!("keyboard.mastery_label")));
    } else {
        right_col.push(format!("{}{}", t!("keyboard.mastery_label"), t!("keyboard.mastery_locked")));
    }
    right_col.push(format!("{}{}", t!("keyboard.ranked_avg_time"), fmt_avg_time(ranked_stat)));
    right_col.push(format!("{}{}", t!("keyboard.ranked_best_time"), fmt_best_time(ranked_stat)));
    right_col.push(format!("{}{}", t!("keyboard.ranked_samples"), fmt_samples(ranked_stat)));
    right_col.push(format!("{}{}", t!("keyboard.ranked_accuracy_label"), fmt_acc(ranked_acc)));

    if left_col.is_empty() {
        left_col.push(t!("keyboard.no_data").to_string());
    }
    if right_col.is_empty() {
        right_col.push(t!("keyboard.no_data").to_string());
    }

    let mut lines: Vec<Line> = Vec::new();
    let split_gap = 3usize;
    let left_width = inner.width.saturating_sub(split_gap as u16) as usize / 2;
    let right_width = inner.width as usize - left_width.saturating_sub(0) - split_gap;
    let row_count = left_col.len().max(right_col.len());
    for i in 0..row_count {
        let left = left_col.get(i).map(String::as_str).unwrap_or("");
        let right = right_col.get(i).map(String::as_str).unwrap_or("");
        let mut left_fit: String = left.chars().take(left_width).collect();
        if left_fit.len() < left_width {
            left_fit.push_str(&" ".repeat(left_width - left_fit.len()));
        }
        let right_fit: String = right.chars().take(right_width).collect();
        lines.push(Line::from(vec![
            Span::styled(" ", Style::default().fg(colors.fg())),
            Span::styled(left_fit, Style::default().fg(colors.fg())),
            Span::styled(" | ", Style::default().fg(colors.border())),
            Span::styled(right_fit, Style::default().fg(colors.fg())),
        ]));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}
