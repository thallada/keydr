use rand::rngs::SmallRng;
use rand::SeedableRng;

use crate::config::Config;
use crate::engine::filter::CharFilter;
use crate::engine::key_stats::KeyStatsStore;
use crate::engine::letter_unlock::LetterUnlock;
use crate::engine::scoring;
use crate::generator::code_syntax::CodeSyntaxGenerator;
use crate::generator::dictionary::Dictionary;
use crate::generator::passage::PassageGenerator;
use crate::generator::phonetic::PhoneticGenerator;
use crate::generator::TextGenerator;
use crate::generator::transition_table::TransitionTable;

use crate::session::input::{self, KeystrokeEvent};
use crate::session::lesson::LessonState;
use crate::session::result::LessonResult;
use crate::store::json_store::JsonStore;
use crate::store::schema::{KeyStatsData, LessonHistoryData, ProfileData};
use crate::ui::components::menu::Menu;
use crate::ui::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppScreen {
    Menu,
    Lesson,
    LessonResult,
    StatsDashboard,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LessonMode {
    Adaptive,
    Code,
    Passage,
}

pub struct App {
    pub screen: AppScreen,
    pub lesson_mode: LessonMode,
    pub lesson: Option<LessonState>,
    pub lesson_events: Vec<KeystrokeEvent>,
    pub last_result: Option<LessonResult>,
    pub lesson_history: Vec<LessonResult>,
    pub menu: Menu<'static>,
    pub theme: &'static Theme,
    pub config: Config,
    pub key_stats: KeyStatsStore,
    pub letter_unlock: LetterUnlock,
    pub profile: ProfileData,
    pub store: Option<JsonStore>,
    pub should_quit: bool,
    pub settings_selected: usize,
    pub stats_tab: usize,
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

        let (key_stats, letter_unlock, profile, lesson_history) = if let Some(ref s) = store {
            let ksd = s.load_key_stats();
            let pd = s.load_profile();
            let lhd = s.load_lesson_history();

            let lu = if pd.unlocked_letters.is_empty() {
                LetterUnlock::new()
            } else {
                LetterUnlock::from_included(pd.unlocked_letters.clone())
            };

            (ksd.stats, lu, pd, lhd.lessons)
        } else {
            (
                KeyStatsStore::default(),
                LetterUnlock::new(),
                ProfileData::default(),
                Vec::new(),
            )
        };

        let mut key_stats_with_target = key_stats;
        key_stats_with_target.target_cpm = config.target_cpm();

        let dictionary = Dictionary::load();
        let transition_table = TransitionTable::build_from_words(&dictionary.words_list());

        Self {
            screen: AppScreen::Menu,
            lesson_mode: LessonMode::Adaptive,
            lesson: None,
            lesson_events: Vec::new(),
            last_result: None,
            lesson_history,
            menu,
            theme,
            config,
            key_stats: key_stats_with_target,
            letter_unlock,
            profile,
            store,
            should_quit: false,
            settings_selected: 0,
            stats_tab: 0,
            rng: SmallRng::from_entropy(),
            transition_table,
            dictionary,
        }
    }

    pub fn start_lesson(&mut self) {
        let text = self.generate_text();
        self.lesson = Some(LessonState::new(&text));
        self.lesson_events.clear();
        self.screen = AppScreen::Lesson;
    }

    fn generate_text(&mut self) -> String {
        let word_count = self.config.word_count;
        let mode = self.lesson_mode;

        match mode {
            LessonMode::Adaptive => {
                let filter = CharFilter::new(self.letter_unlock.included.clone());
                let focused = self.letter_unlock.focused;
                let table = self.transition_table.clone();
                let dict = Dictionary::load();
                let rng = SmallRng::from_rng(&mut self.rng).unwrap();
                let mut generator = PhoneticGenerator::new(table, dict, rng);
                generator.generate(&filter, focused, word_count)
            }
            LessonMode::Code => {
                let filter = CharFilter::new(('a'..='z').collect());
                let lang = self
                    .config
                    .code_languages
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "rust".to_string());
                let rng = SmallRng::from_rng(&mut self.rng).unwrap();
                let mut generator = CodeSyntaxGenerator::new(rng, &lang);
                generator.generate(&filter, None, word_count)
            }
            LessonMode::Passage => {
                let filter = CharFilter::new(('a'..='z').collect());
                let rng = SmallRng::from_rng(&mut self.rng).unwrap();
                let mut generator = PassageGenerator::new(rng);
                generator.generate(&filter, None, word_count)
            }
        }
    }

    pub fn type_char(&mut self, ch: char) {
        if let Some(ref mut lesson) = self.lesson {
            if let Some(event) = input::process_char(lesson, ch) {
                self.lesson_events.push(event);
            }

            if lesson.is_complete() {
                self.finish_lesson();
            }
        }
    }

    pub fn backspace(&mut self) {
        if let Some(ref mut lesson) = self.lesson {
            input::process_backspace(lesson);
        }
    }

    fn finish_lesson(&mut self) {
        if let Some(ref lesson) = self.lesson {
            let result = LessonResult::from_lesson(lesson, &self.lesson_events);

            if self.lesson_mode == LessonMode::Adaptive {
                for kt in &result.per_key_times {
                    if kt.correct {
                        self.key_stats.update_key(kt.key, kt.time_ms);
                    }
                }
                self.letter_unlock.update(&self.key_stats);
            }

            let complexity = scoring::compute_complexity(self.letter_unlock.unlocked_count());
            let score = scoring::compute_score(&result, complexity);
            self.profile.total_score += score;
            self.profile.total_lessons += 1;
            self.profile.unlocked_letters = self.letter_unlock.included.clone();

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
                self.profile.best_streak =
                    self.profile.best_streak.max(self.profile.streak_days);
                self.profile.last_practice_date = Some(today);
            }

            self.lesson_history.push(result.clone());
            if self.lesson_history.len() > 500 {
                self.lesson_history.remove(0);
            }

            self.last_result = Some(result);
            self.screen = AppScreen::LessonResult;

            self.save_data();
        }
    }

    fn save_data(&self) {
        if let Some(ref store) = self.store {
            let _ = store.save_profile(&self.profile);
            let _ = store.save_key_stats(&KeyStatsData {
                schema_version: 1,
                stats: self.key_stats.clone(),
            });
            let _ = store.save_lesson_history(&LessonHistoryData {
                schema_version: 1,
                lessons: self.lesson_history.clone(),
            });
        }
    }

    pub fn retry_lesson(&mut self) {
        self.start_lesson();
    }

    pub fn go_to_menu(&mut self) {
        self.screen = AppScreen::Menu;
        self.lesson = None;
        self.lesson_events.clear();
    }

    pub fn go_to_stats(&mut self) {
        self.stats_tab = 0;
        self.screen = AppScreen::StatsDashboard;
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
                let langs = ["rust", "python", "javascript", "go"];
                let current = self
                    .config
                    .code_languages
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("rust");
                let idx = langs.iter().position(|&l| l == current).unwrap_or(0);
                let next = (idx + 1) % langs.len();
                self.config.code_languages = vec![langs[next].to_string()];
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
                let langs = ["rust", "python", "javascript", "go"];
                let current = self
                    .config
                    .code_languages
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("rust");
                let idx = langs.iter().position(|&l| l == current).unwrap_or(0);
                let next = if idx == 0 { langs.len() - 1 } else { idx - 1 };
                self.config.code_languages = vec![langs[next].to_string()];
            }
            _ => {}
        }
    }
}
