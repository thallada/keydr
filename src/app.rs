use std::collections::HashSet;
use std::time::Instant;

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use crate::config::Config;
use crate::engine::filter::CharFilter;
use crate::engine::key_stats::KeyStatsStore;
use crate::engine::scoring;
use crate::engine::skill_tree::{BranchId, BranchStatus, DrillScope, SkillTree};
use crate::generator::TextGenerator;
use crate::generator::capitalize;
use crate::generator::code_patterns;
use crate::generator::code_syntax::CodeSyntaxGenerator;
use crate::generator::dictionary::Dictionary;
use crate::generator::numbers;
use crate::generator::passage::PassageGenerator;
use crate::generator::phonetic::PhoneticGenerator;
use crate::generator::punctuate;
use crate::generator::transition_table::TransitionTable;
use crate::keyboard::model::KeyboardModel;

use crate::session::drill::DrillState;
use crate::session::input::{self, KeystrokeEvent};
use crate::session::result::DrillResult;
use crate::store::json_store::JsonStore;
use crate::store::schema::{DrillHistoryData, KeyStatsData, ProfileData};
use crate::ui::components::menu::Menu;
use crate::ui::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppScreen {
    Menu,
    Drill,
    DrillResult,
    StatsDashboard,
    Settings,
    SkillTree,
    CodeLanguageSelect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrillMode {
    Adaptive,
    Code,
    Passage,
}

impl DrillMode {
    pub fn as_str(self) -> &'static str {
        match self {
            DrillMode::Adaptive => "adaptive",
            DrillMode::Code => "code",
            DrillMode::Passage => "passage",
        }
    }

    pub fn is_ranked(self) -> bool {
        matches!(self, DrillMode::Adaptive)
    }
}

pub struct App {
    pub screen: AppScreen,
    pub drill_mode: DrillMode,
    pub drill_scope: DrillScope,
    pub drill: Option<DrillState>,
    pub drill_events: Vec<KeystrokeEvent>,
    pub last_result: Option<DrillResult>,
    pub drill_history: Vec<DrillResult>,
    pub menu: Menu<'static>,
    pub theme: &'static Theme,
    pub config: Config,
    pub key_stats: KeyStatsStore,
    pub skill_tree: SkillTree,
    pub profile: ProfileData,
    pub store: Option<JsonStore>,
    pub should_quit: bool,
    pub settings_selected: usize,
    pub stats_tab: usize,
    pub depressed_keys: HashSet<char>,
    pub last_key_time: Option<Instant>,
    pub history_selected: usize,
    pub history_confirm_delete: bool,
    pub skill_tree_selected: usize,
    pub skill_tree_detail_scroll: usize,
    pub drill_source_info: Option<String>,
    pub code_language_selected: usize,
    pub shift_held: bool,
    pub keyboard_model: KeyboardModel,
    rng: SmallRng,
    transition_table: TransitionTable,
    #[allow(dead_code)]
    dictionary: Dictionary,
}

impl App {
    pub fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        let loaded_theme = Theme::load(&config.theme).unwrap_or_default();
        let theme: &'static Theme = Box::leak(Box::new(loaded_theme));
        let menu = Menu::new(theme);

        let store = JsonStore::new().ok();

        let (key_stats, skill_tree, profile, drill_history) = if let Some(ref s) = store {
            // load_profile returns None if file exists but can't parse (schema mismatch)
            let pd = s.load_profile();

            match pd {
                Some(pd) if !pd.needs_reset() => {
                    let ksd = s.load_key_stats();
                    let lhd = s.load_drill_history();
                    let st = SkillTree::new(pd.skill_tree.clone());
                    (ksd.stats, st, pd, lhd.drills)
                }
                _ => {
                    // Schema mismatch or parse failure: full reset of all stores
                    (
                        KeyStatsStore::default(),
                        SkillTree::default(),
                        ProfileData::default(),
                        Vec::new(),
                    )
                }
            }
        } else {
            (
                KeyStatsStore::default(),
                SkillTree::default(),
                ProfileData::default(),
                Vec::new(),
            )
        };

        let mut key_stats_with_target = key_stats;
        key_stats_with_target.target_cpm = config.target_cpm();

        let dictionary = Dictionary::load();
        let transition_table = TransitionTable::build_from_words(&dictionary.words_list());
        let keyboard_model = KeyboardModel::from_name(&config.keyboard_layout);

        let mut app = Self {
            screen: AppScreen::Menu,
            drill_mode: DrillMode::Adaptive,
            drill_scope: DrillScope::Global,
            drill: None,
            drill_events: Vec::new(),
            last_result: None,
            drill_history,
            menu,
            theme,
            config,
            key_stats: key_stats_with_target,
            skill_tree,
            profile,
            store,
            should_quit: false,
            settings_selected: 0,
            stats_tab: 0,
            depressed_keys: HashSet::new(),
            last_key_time: None,
            history_selected: 0,
            history_confirm_delete: false,
            skill_tree_selected: 0,
            skill_tree_detail_scroll: 0,
            drill_source_info: None,
            code_language_selected: 0,
            shift_held: false,
            keyboard_model,
            rng: SmallRng::from_entropy(),
            transition_table,
            dictionary,
        };
        app.start_drill();
        app
    }

    pub fn start_drill(&mut self) {
        let (text, source_info) = self.generate_text();
        self.drill = Some(DrillState::new(&text));
        self.drill_source_info = source_info;
        self.drill_events.clear();
        self.screen = AppScreen::Drill;
    }

    fn generate_text(&mut self) -> (String, Option<String>) {
        let word_count = self.config.word_count;
        let mode = self.drill_mode;

        match mode {
            DrillMode::Adaptive => {
                let scope = self.drill_scope;
                let all_keys = self.skill_tree.unlocked_keys(scope);
                let focused = self.skill_tree.focused_key(scope, &self.key_stats);

                // Generate base lowercase text using only lowercase keys from scope
                let lowercase_keys: Vec<char> = all_keys
                    .iter()
                    .copied()
                    .filter(|ch| ch.is_ascii_lowercase() || *ch == ' ')
                    .collect();
                let filter = CharFilter::new(lowercase_keys);
                // Only pass focused to phonetic generator if it's a lowercase letter
                let lowercase_focused = focused.filter(|ch| ch.is_ascii_lowercase());
                let table = self.transition_table.clone();
                let dict = Dictionary::load();
                let rng = SmallRng::from_rng(&mut self.rng).unwrap();
                let mut generator = PhoneticGenerator::new(table, dict, rng);
                let mut text = generator.generate(&filter, lowercase_focused, word_count);

                // Apply capitalization if uppercase keys are in scope
                let cap_keys: Vec<char> = all_keys
                    .iter()
                    .copied()
                    .filter(|ch| ch.is_ascii_uppercase())
                    .collect();
                if !cap_keys.is_empty() {
                    let mut rng = SmallRng::from_rng(&mut self.rng).unwrap();
                    text = capitalize::apply_capitalization(&text, &cap_keys, focused, &mut rng);
                }

                // Apply punctuation if punctuation keys are in scope
                let punct_keys: Vec<char> = all_keys
                    .iter()
                    .copied()
                    .filter(|ch| {
                        matches!(
                            ch,
                            '.' | ',' | '\'' | ';' | ':' | '"' | '-' | '?' | '!' | '(' | ')'
                        )
                    })
                    .collect();
                if !punct_keys.is_empty() {
                    let mut rng = SmallRng::from_rng(&mut self.rng).unwrap();
                    text = punctuate::apply_punctuation(&text, &punct_keys, focused, &mut rng);
                }

                // Apply numbers if digit keys are in scope
                let digit_keys: Vec<char> = all_keys
                    .iter()
                    .copied()
                    .filter(|ch| ch.is_ascii_digit())
                    .collect();
                if !digit_keys.is_empty() {
                    let has_dot = all_keys.contains(&'.');
                    let mut rng = SmallRng::from_rng(&mut self.rng).unwrap();
                    text = numbers::apply_numbers(&text, &digit_keys, has_dot, focused, &mut rng);
                }

                // Apply code symbols only if this drill is for the CodeSymbols branch,
                // or if it's a global drill and CodeSymbols is active
                let code_active = match scope {
                    DrillScope::Branch(id) => id == BranchId::CodeSymbols,
                    DrillScope::Global => matches!(
                        self.skill_tree.branch_status(BranchId::CodeSymbols),
                        BranchStatus::InProgress | BranchStatus::Complete
                    ),
                };
                if code_active {
                    let symbol_keys: Vec<char> = all_keys
                        .iter()
                        .copied()
                        .filter(|ch| {
                            matches!(
                                ch,
                                '=' | '+'
                                    | '*'
                                    | '/'
                                    | '-'
                                    | '{'
                                    | '}'
                                    | '['
                                    | ']'
                                    | '<'
                                    | '>'
                                    | '&'
                                    | '|'
                                    | '^'
                                    | '~'
                                    | '@'
                                    | '#'
                                    | '$'
                                    | '%'
                                    | '_'
                                    | '\\'
                                    | '`'
                            )
                        })
                        .collect();
                    if !symbol_keys.is_empty() {
                        let mut rng = SmallRng::from_rng(&mut self.rng).unwrap();
                        text = code_patterns::apply_code_symbols(
                            &text,
                            &symbol_keys,
                            focused,
                            &mut rng,
                        );
                    }
                }

                // Apply whitespace line breaks if newline is in scope
                if all_keys.contains(&'\n') {
                    text = insert_line_breaks(&text);
                }

                (text, None)
            }
            DrillMode::Code => {
                let filter = CharFilter::new(('a'..='z').collect());
                let lang = if self.config.code_language == "all" {
                    let langs = ["rust", "python", "javascript", "go"];
                    let idx = self.rng.gen_range(0..langs.len());
                    langs[idx].to_string()
                } else {
                    self.config.code_language.clone()
                };
                let rng = SmallRng::from_rng(&mut self.rng).unwrap();
                let mut generator = CodeSyntaxGenerator::new(rng, &lang);
                let text = generator.generate(&filter, None, word_count);
                (text, Some(generator.last_source().to_string()))
            }
            DrillMode::Passage => {
                let filter = CharFilter::new(('a'..='z').collect());
                let rng = SmallRng::from_rng(&mut self.rng).unwrap();
                let mut generator = PassageGenerator::new(rng);
                let text = generator.generate(&filter, None, word_count);
                (text, Some(generator.last_source().to_string()))
            }
        }
    }

    pub fn type_char(&mut self, ch: char) {
        if let Some(ref mut drill) = self.drill {
            if let Some(event) = input::process_char(drill, ch) {
                self.drill_events.push(event);
            }

            if drill.is_complete() {
                self.finish_drill();
            }
        }
    }

    pub fn backspace(&mut self) {
        if let Some(ref mut drill) = self.drill {
            input::process_backspace(drill);
        }
    }

    fn finish_drill(&mut self) {
        if let Some(ref drill) = self.drill {
            let ranked = self.drill_mode.is_ranked();
            let result = DrillResult::from_drill(
                drill,
                &self.drill_events,
                self.drill_mode.as_str(),
                ranked,
            );

            if ranked {
                for kt in &result.per_key_times {
                    if kt.correct {
                        self.key_stats.update_key(kt.key, kt.time_ms);
                    }
                }
                self.skill_tree.update(&self.key_stats);
            }

            let complexity = self.skill_tree.complexity();
            let score = scoring::compute_score(&result, complexity);
            self.profile.total_score += score;
            self.profile.total_drills += 1;
            self.profile.skill_tree = self.skill_tree.progress.clone();

            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
            if self.profile.last_practice_date.as_deref() != Some(&today) {
                if let Some(ref last) = self.profile.last_practice_date {
                    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
                        .format("%Y-%m-%d")
                        .to_string();
                    if last == &yesterday {
                        self.profile.streak_days += 1;
                    } else {
                        self.profile.streak_days = 1;
                    }
                } else {
                    self.profile.streak_days = 1;
                }
                self.profile.best_streak = self.profile.best_streak.max(self.profile.streak_days);
                self.profile.last_practice_date = Some(today);
            }

            self.drill_history.push(result.clone());
            if self.drill_history.len() > 500 {
                self.drill_history.remove(0);
            }

            self.last_result = Some(result);

            // Adaptive mode auto-continues to next drill (like keybr.com)
            if self.drill_mode == DrillMode::Adaptive {
                self.start_drill();
            } else {
                self.screen = AppScreen::DrillResult;
            }

            self.save_data();
        }
    }

    fn save_data(&self) {
        if let Some(ref store) = self.store {
            let _ = store.save_profile(&self.profile);
            let _ = store.save_key_stats(&KeyStatsData {
                schema_version: 2,
                stats: self.key_stats.clone(),
            });
            let _ = store.save_drill_history(&DrillHistoryData {
                schema_version: 2,
                drills: self.drill_history.clone(),
            });
        }
    }

    pub fn retry_drill(&mut self) {
        self.start_drill();
    }

    pub fn go_to_menu(&mut self) {
        self.screen = AppScreen::Menu;
        self.drill = None;
        self.drill_source_info = None;
        self.drill_events.clear();
    }

    pub fn go_to_stats(&mut self) {
        self.stats_tab = 0;
        self.history_selected = 0;
        self.history_confirm_delete = false;
        self.screen = AppScreen::StatsDashboard;
    }

    pub fn delete_session(&mut self) {
        if self.drill_history.is_empty() {
            return;
        }
        // History tab shows reverse order, so convert display index to actual index
        let actual_idx = self.drill_history.len() - 1 - self.history_selected;
        self.drill_history.remove(actual_idx);
        self.rebuild_from_history();
        self.save_data();

        // Clamp selection to visible range (max 20 visible rows)
        if !self.drill_history.is_empty() {
            let max_visible = self.drill_history.len().min(20) - 1;
            self.history_selected = self.history_selected.min(max_visible);
        } else {
            self.history_selected = 0;
        }
    }

    pub fn rebuild_from_history(&mut self) {
        // Reset all derived state
        self.key_stats = KeyStatsStore::default();
        self.key_stats.target_cpm = self.config.target_cpm();
        self.skill_tree = SkillTree::default();
        self.profile.total_score = 0.0;
        self.profile.total_drills = 0;
        self.profile.streak_days = 0;
        self.profile.best_streak = 0;
        self.profile.last_practice_date = None;

        // Replay each remaining session oldest->newest
        for result in &self.drill_history {
            // Only update skill tree for ranked sessions
            if result.ranked {
                for kt in &result.per_key_times {
                    if kt.correct {
                        self.key_stats.update_key(kt.key, kt.time_ms);
                    }
                }
                self.skill_tree.update(&self.key_stats);
            }

            // Compute score
            let complexity = self.skill_tree.complexity();
            let score = scoring::compute_score(result, complexity);
            self.profile.total_score += score;
            self.profile.total_drills += 1;

            // Rebuild streak tracking
            let day = result.timestamp.format("%Y-%m-%d").to_string();
            if self.profile.last_practice_date.as_deref() != Some(&day) {
                if let Some(ref last) = self.profile.last_practice_date {
                    let result_date = result.timestamp.date_naive();
                    let last_date =
                        chrono::NaiveDate::parse_from_str(last, "%Y-%m-%d").unwrap_or(result_date);
                    let diff = result_date.signed_duration_since(last_date).num_days();
                    if diff == 1 {
                        self.profile.streak_days += 1;
                    } else {
                        self.profile.streak_days = 1;
                    }
                } else {
                    self.profile.streak_days = 1;
                }
                self.profile.best_streak = self.profile.best_streak.max(self.profile.streak_days);
                self.profile.last_practice_date = Some(day);
            }
        }

        self.profile.skill_tree = self.skill_tree.progress.clone();
    }

    pub fn go_to_skill_tree(&mut self) {
        self.skill_tree_selected = 0;
        self.skill_tree_detail_scroll = 0;
        self.screen = AppScreen::SkillTree;
    }

    pub fn start_branch_drill(&mut self, branch_id: BranchId) {
        // Start the branch if it's Available
        self.skill_tree.start_branch(branch_id);
        self.profile.skill_tree = self.skill_tree.progress.clone();
        self.save_data();

        // Use adaptive mode with branch-specific scope
        self.drill_mode = DrillMode::Adaptive;
        self.drill_scope = DrillScope::Branch(branch_id);
        self.start_drill();
    }

    pub fn go_to_code_language_select(&mut self) {
        let langs = ["rust", "python", "javascript", "go", "all"];
        self.code_language_selected = langs
            .iter()
            .position(|&l| l == self.config.code_language)
            .unwrap_or(0);
        self.screen = AppScreen::CodeLanguageSelect;
    }

    pub fn go_to_settings(&mut self) {
        self.settings_selected = 0;
        self.screen = AppScreen::Settings;
    }

    pub fn settings_cycle_forward(&mut self) {
        match self.settings_selected {
            0 => {
                self.config.target_wpm = (self.config.target_wpm + 5).min(200);
                self.key_stats.target_cpm = self.config.target_cpm();
            }
            1 => {
                let themes = Theme::available_themes();
                if let Some(idx) = themes.iter().position(|t| *t == self.config.theme) {
                    let next = (idx + 1) % themes.len();
                    self.config.theme = themes[next].clone();
                } else if let Some(first) = themes.first() {
                    self.config.theme = first.clone();
                }
                if let Some(new_theme) = Theme::load(&self.config.theme) {
                    let theme: &'static Theme = Box::leak(Box::new(new_theme));
                    self.theme = theme;
                    self.menu.theme = theme;
                }
            }
            2 => {
                self.config.word_count = (self.config.word_count + 5).min(100);
            }
            3 => {
                let langs = ["rust", "python", "javascript", "go", "all"];
                let idx = langs
                    .iter()
                    .position(|&l| l == self.config.code_language)
                    .unwrap_or(0);
                let next = (idx + 1) % langs.len();
                self.config.code_language = langs[next].to_string();
            }
            _ => {}
        }
    }

    pub fn settings_cycle_backward(&mut self) {
        match self.settings_selected {
            0 => {
                self.config.target_wpm = self.config.target_wpm.saturating_sub(5).max(10);
                self.key_stats.target_cpm = self.config.target_cpm();
            }
            1 => {
                let themes = Theme::available_themes();
                if let Some(idx) = themes.iter().position(|t| *t == self.config.theme) {
                    let next = if idx == 0 { themes.len() - 1 } else { idx - 1 };
                    self.config.theme = themes[next].clone();
                } else if let Some(first) = themes.first() {
                    self.config.theme = first.clone();
                }
                if let Some(new_theme) = Theme::load(&self.config.theme) {
                    let theme: &'static Theme = Box::leak(Box::new(new_theme));
                    self.theme = theme;
                    self.menu.theme = theme;
                }
            }
            2 => {
                self.config.word_count = self.config.word_count.saturating_sub(5).max(5);
            }
            3 => {
                let langs = ["rust", "python", "javascript", "go", "all"];
                let idx = langs
                    .iter()
                    .position(|&l| l == self.config.code_language)
                    .unwrap_or(0);
                let next = if idx == 0 { langs.len() - 1 } else { idx - 1 };
                self.config.code_language = langs[next].to_string();
            }
            _ => {}
        }
    }
}

/// Insert newlines at sentence boundaries (~60-80 chars per line).
fn insert_line_breaks(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut col = 0;

    for (i, ch) in text.chars().enumerate() {
        result.push(ch);
        col += 1;

        // After sentence-ending punctuation + space, insert newline if past 60 chars
        if col >= 60 && (ch == '.' || ch == '?' || ch == '!') {
            // Check if next char is a space
            let next = text.chars().nth(i + 1);
            if next == Some(' ') {
                result.push('\n');
                col = 0;
                // Skip the space (will be consumed by next iteration but we already broke the line)
            }
        } else if col >= 80 && ch == ' ' {
            // Hard wrap at spaces if no sentence boundary found
            // Replace the space we just pushed with a newline
            result.pop();
            result.push('\n');
            col = 0;
        }
    }

    result
}
