use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use crate::config::Config;
use crate::engine::FocusSelection;
use crate::engine::filter::CharFilter;
use crate::engine::key_stats::KeyStatsStore;
use crate::engine::ngram_stats::{
    self, BigramStatsStore, TrigramStatsStore, extract_ngram_events, select_focus,
};
use crate::engine::scoring;
use crate::engine::skill_tree::{BranchId, BranchStatus, DrillScope, SkillTree};
use crate::generator::TextGenerator;
use crate::generator::capitalize;
use crate::generator::code_patterns;
use crate::generator::code_syntax::{
    CodeSyntaxGenerator, build_code_download_queue, code_language_options,
    download_code_repo_to_cache_with_progress, is_language_cached, language_by_key,
    languages_with_content,
};
use crate::generator::dictionary::Dictionary;
use crate::generator::numbers;
use crate::generator::passage::{
    GUTENBERG_BOOKS, PassageGenerator, book_by_key, download_book_to_cache_with_progress,
    is_book_cached, passage_options, uncached_books,
};
use crate::generator::phonetic::PhoneticGenerator;
use crate::generator::punctuate;
use crate::generator::transition_table::TransitionTable;
use crate::keyboard::display::BACKSPACE;
use crate::keyboard::model::KeyboardModel;

use crate::session::drill::DrillState;
use crate::session::input::{self, KeystrokeEvent};
use crate::session::result::{DrillResult, KeyTime};
use crate::store::json_store::JsonStore;
use crate::store::schema::{
    DrillHistoryData, EXPORT_VERSION, ExportData, KeyStatsData, ProfileData,
};
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
    PassageBookSelect,
    PassageIntro,
    PassageDownloadProgress,
    CodeIntro,
    CodeDownloadProgress,
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrillMode {
    Adaptive,
    Code,
    Passage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassageDownloadCompleteAction {
    StartPassageDrill,
    ReturnToSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeDownloadCompleteAction {
    StartCodeDrill,
    ReturnToSettings,
}

pub enum MilestoneKind {
    Unlock,
    Mastery,
}

pub struct KeyMilestonePopup {
    pub kind: MilestoneKind,
    pub keys: Vec<char>,
    pub finger_info: Vec<(char, String)>,
    pub message: &'static str,
}

const UNLOCK_MESSAGES: &[&str] = &[
    "Nice work! Keep building your typing skills.",
    "Another key added to your arsenal!",
    "Your keyboard is growing! Keep it up.",
    "One step closer to full keyboard mastery!",
];

const MASTERY_MESSAGES: &[&str] = &[
    "This key is now at full confidence!",
    "You've got this key down pat!",
    "Muscle memory locked in!",
    "One more key conquered!",
];

const POST_DRILL_INPUT_LOCK_MS: u64 = 800;

struct DownloadJob {
    downloaded_bytes: Arc<AtomicU64>,
    total_bytes: Arc<AtomicU64>,
    done: Arc<AtomicBool>,
    success: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct StatusMessage {
    pub kind: StatusKind,
    pub text: String,
}

/// Given a file path, find the next available path by appending/incrementing
/// a `-N` numeric suffix before the extension. Strips any existing trailing
/// `-N` suffix to normalize before scanning.
pub fn next_available_path(path_str: &str) -> String {
    let path = std::path::Path::new(path_str).to_path_buf();
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("json");
    let full_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export");

    // Strip existing trailing -N suffix to get base stem
    let base_stem = if let Some(pos) = full_stem.rfind('-') {
        let suffix = &full_stem[pos + 1..];
        // Only strip if the suffix is a pure positive integer AND the base before
        // it also exists as a file (i.e., this is our rename suffix, not part of
        // the original name like a date component)
        if suffix.parse::<u32>().is_ok() {
            let candidate_base = &full_stem[..pos];
            let base_file = parent.join(format!("{candidate_base}.{extension}"));
            if base_file.exists() {
                candidate_base
            } else {
                full_stem
            }
        } else {
            full_stem
        }
    } else {
        full_stem
    };

    let mut n = 1u32;
    loop {
        let candidate = parent.join(format!("{base_stem}-{n}.{extension}"));
        if !candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
        n += 1;
    }
}

fn default_export_path() -> String {
    let dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let date = chrono::Utc::now().format("%Y-%m-%d");
    dir.join(format!("keydr-export-{date}.json"))
        .to_string_lossy()
        .to_string()
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
    pub ranked_key_stats: KeyStatsStore,
    pub skill_tree: SkillTree,
    pub profile: ProfileData,
    pub store: Option<JsonStore>,
    pub should_quit: bool,
    pub settings_selected: usize,
    pub settings_editing_download_dir: bool,
    pub stats_tab: usize,
    pub depressed_keys: HashSet<char>,
    pub last_key_time: Option<Instant>,
    pub history_selected: usize,
    pub history_confirm_delete: bool,
    pub skill_tree_selected: usize,
    pub skill_tree_detail_scroll: usize,
    pub drill_source_info: Option<String>,
    pub code_language_selected: usize,
    pub code_language_scroll: usize,
    pub passage_book_selected: usize,
    pub passage_intro_selected: usize,
    pub passage_intro_downloads_enabled: bool,
    pub passage_intro_download_dir: String,
    pub passage_intro_paragraph_limit: usize,
    pub passage_intro_downloading: bool,
    pub passage_intro_download_total: usize,
    pub passage_intro_downloaded: usize,
    pub passage_intro_current_book: String,
    pub passage_intro_download_bytes: u64,
    pub passage_intro_download_bytes_total: u64,
    pub passage_download_queue: Vec<usize>,
    pub passage_drill_selection_override: Option<String>,
    pub last_passage_drill_selection: Option<String>,
    pub passage_download_action: PassageDownloadCompleteAction,
    pub code_intro_selected: usize,
    pub code_intro_downloads_enabled: bool,
    pub code_intro_download_dir: String,
    pub code_intro_snippets_per_repo: usize,
    pub code_intro_downloading: bool,
    pub code_intro_download_total: usize,
    pub code_intro_downloaded: usize,
    pub code_intro_current_repo: String,
    pub code_intro_download_bytes: u64,
    pub code_intro_download_bytes_total: u64,
    pub code_download_queue: Vec<(String, usize)>,
    pub code_drill_language_override: Option<String>,
    pub last_code_drill_language: Option<String>,
    pub code_download_attempted: bool,
    pub code_download_action: CodeDownloadCompleteAction,
    pub shift_held: bool,
    pub caps_lock: bool,
    pub keyboard_model: KeyboardModel,
    pub milestone_queue: VecDeque<KeyMilestonePopup>,
    pub settings_confirm_import: bool,
    pub settings_export_conflict: bool,
    pub settings_status_message: Option<StatusMessage>,
    pub settings_export_path: String,
    pub settings_import_path: String,
    pub settings_editing_export_path: bool,
    pub settings_editing_import_path: bool,
    pub keyboard_explorer_selected: Option<char>,
    pub explorer_accuracy_cache_overall: Option<(char, usize, usize)>,
    pub explorer_accuracy_cache_ranked: Option<(char, usize, usize)>,
    pub bigram_stats: BigramStatsStore,
    pub ranked_bigram_stats: BigramStatsStore,
    pub trigram_stats: TrigramStatsStore,
    pub ranked_trigram_stats: TrigramStatsStore,
    pub user_median_transition_ms: f64,
    pub transition_buffer: Vec<f64>,
    pub trigram_gain_history: Vec<f64>,
    pub current_focus: Option<FocusSelection>,
    pub post_drill_input_lock_until: Option<Instant>,
    adaptive_word_history: VecDeque<HashSet<String>>,
    rng: SmallRng,
    transition_table: TransitionTable,
    #[allow(dead_code)]
    dictionary: Dictionary,
    passage_download_job: Option<DownloadJob>,
    code_download_job: Option<DownloadJob>,
}

impl App {
    pub fn new() -> Self {
        let mut config = Config::load().unwrap_or_default();

        // Normalize code_language: reset to default if not a valid option
        let valid_keys: Vec<&str> = code_language_options().iter().map(|(k, _)| *k).collect();
        config.normalize_code_language(&valid_keys);
        let loaded_theme = Theme::load(&config.theme).unwrap_or_default();
        let theme: &'static Theme = Box::leak(Box::new(loaded_theme));
        let menu = Menu::new(theme);

        let store = JsonStore::new().ok();

        let (key_stats, ranked_key_stats, skill_tree, profile, drill_history) =
            if let Some(ref s) = store {
                // load_profile returns None if file exists but can't parse (schema mismatch)
                let pd = s.load_profile();

                match pd {
                    Some(pd) if !pd.needs_reset() => {
                        let ksd = s.load_key_stats();
                        let rksd = s.load_ranked_key_stats();
                        let lhd = s.load_drill_history();
                        let st = SkillTree::new(pd.skill_tree.clone());
                        (ksd.stats, rksd.stats, st, pd, lhd.drills)
                    }
                    _ => {
                        // Schema mismatch or parse failure: full reset of all stores
                        (
                            KeyStatsStore::default(),
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
                    KeyStatsStore::default(),
                    SkillTree::default(),
                    ProfileData::default(),
                    Vec::new(),
                )
            };

        let mut key_stats_with_target = key_stats;
        key_stats_with_target.target_cpm = config.target_cpm();
        let mut ranked_key_stats_with_target = ranked_key_stats;
        ranked_key_stats_with_target.target_cpm = config.target_cpm();

        let dictionary = Dictionary::load();
        let transition_table = TransitionTable::build_from_words(&dictionary.words_list());
        let keyboard_model = KeyboardModel::from_name(&config.keyboard_layout);
        let intro_downloads_enabled = config.passage_downloads_enabled;
        let intro_download_dir = config.passage_download_dir.clone();
        let intro_paragraph_limit = config.passage_paragraphs_per_book;
        let code_intro_downloads_enabled = config.code_downloads_enabled;
        let code_intro_download_dir = config.code_download_dir.clone();
        let code_intro_snippets_per_repo = config.code_snippets_per_repo;

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
            ranked_key_stats: ranked_key_stats_with_target,
            skill_tree,
            profile,
            store,
            should_quit: false,
            settings_selected: 0,
            settings_editing_download_dir: false,
            stats_tab: 0,
            depressed_keys: HashSet::new(),
            last_key_time: None,
            history_selected: 0,
            history_confirm_delete: false,
            skill_tree_selected: 0,
            skill_tree_detail_scroll: 0,
            drill_source_info: None,
            code_language_selected: 0,
            code_language_scroll: 0,
            passage_book_selected: 0,
            passage_intro_selected: 0,
            passage_intro_downloads_enabled: intro_downloads_enabled,
            passage_intro_download_dir: intro_download_dir,
            passage_intro_paragraph_limit: intro_paragraph_limit,
            passage_intro_downloading: false,
            passage_intro_download_total: 0,
            passage_intro_downloaded: 0,
            passage_intro_current_book: String::new(),
            passage_intro_download_bytes: 0,
            passage_intro_download_bytes_total: 0,
            passage_download_queue: Vec::new(),
            passage_drill_selection_override: None,
            last_passage_drill_selection: None,
            passage_download_action: PassageDownloadCompleteAction::StartPassageDrill,
            code_intro_selected: 0,
            code_intro_downloads_enabled,
            code_intro_download_dir,
            code_intro_snippets_per_repo,
            code_intro_downloading: false,
            code_intro_download_total: 0,
            code_intro_downloaded: 0,
            code_intro_current_repo: String::new(),
            code_intro_download_bytes: 0,
            code_intro_download_bytes_total: 0,
            code_download_queue: Vec::new(),
            code_drill_language_override: None,
            last_code_drill_language: None,
            code_download_attempted: false,
            code_download_action: CodeDownloadCompleteAction::StartCodeDrill,
            shift_held: false,
            caps_lock: false,
            keyboard_model,
            milestone_queue: VecDeque::new(),
            settings_confirm_import: false,
            settings_export_conflict: false,
            settings_status_message: None,
            settings_export_path: default_export_path(),
            settings_import_path: default_export_path(),
            settings_editing_export_path: false,
            settings_editing_import_path: false,
            keyboard_explorer_selected: None,
            explorer_accuracy_cache_overall: None,
            explorer_accuracy_cache_ranked: None,
            bigram_stats: BigramStatsStore::default(),
            ranked_bigram_stats: BigramStatsStore::default(),
            trigram_stats: TrigramStatsStore::default(),
            ranked_trigram_stats: TrigramStatsStore::default(),
            user_median_transition_ms: 0.0,
            transition_buffer: Vec::new(),
            trigram_gain_history: Vec::new(),
            current_focus: None,
            post_drill_input_lock_until: None,
            adaptive_word_history: VecDeque::new(),
            rng: SmallRng::from_entropy(),
            transition_table,
            dictionary,
            passage_download_job: None,
            code_download_job: None,
        };

        // Check for leftover .bak files from interrupted import
        if let Some(ref s) = app.store
            && s.check_interrupted_import()
        {
            app.settings_status_message = Some(StatusMessage {
                kind: StatusKind::Error,
                text: "Recovery files found from interrupted import. Data may be inconsistent — consider re-importing.".to_string(),
            });
        }

        // Rebuild n-gram stats from drill history
        app.rebuild_ngram_stats();

        app.start_drill();
        app
    }

    /// Clear all import/export modal and edit states.
    pub fn clear_settings_modals(&mut self) {
        self.settings_confirm_import = false;
        self.settings_export_conflict = false;
        self.settings_editing_export_path = false;
        self.settings_editing_import_path = false;
        self.settings_editing_download_dir = false;
    }

    pub fn arm_post_drill_input_lock(&mut self) {
        self.post_drill_input_lock_until =
            Some(Instant::now() + Duration::from_millis(POST_DRILL_INPUT_LOCK_MS));
    }

    pub fn clear_post_drill_input_lock(&mut self) {
        self.post_drill_input_lock_until = None;
    }

    pub fn post_drill_input_lock_remaining_ms(&self) -> Option<u64> {
        self.post_drill_input_lock_until.and_then(|until| {
            until
                .checked_duration_since(Instant::now())
                .map(|remaining| remaining.as_millis().max(1) as u64)
        })
    }

    pub fn export_data(&mut self) {
        let path = std::path::Path::new(&self.settings_export_path);

        // Check for existing file
        if path.exists() {
            self.settings_export_conflict = true;
            return;
        }

        self.write_export_to_path();
    }

    pub fn export_data_overwrite(&mut self) {
        self.write_export_to_path();
    }

    pub fn export_data_rename(&mut self) {
        self.settings_export_path = next_available_path(&self.settings_export_path);
        self.write_export_to_path();
    }

    fn write_export_to_path(&mut self) {
        // Check parent directory exists
        let path = std::path::Path::new(&self.settings_export_path);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            self.settings_status_message = Some(StatusMessage {
                kind: StatusKind::Error,
                text: format!("Directory does not exist: {}", parent.display()),
            });
            return;
        }

        let Some(ref store) = self.store else {
            self.settings_status_message = Some(StatusMessage {
                kind: StatusKind::Error,
                text: "No data store available".to_string(),
            });
            return;
        };

        let export = store.export_all(&self.config);
        let json = match serde_json::to_string_pretty(&export) {
            Ok(j) => j,
            Err(e) => {
                self.settings_status_message = Some(StatusMessage {
                    kind: StatusKind::Error,
                    text: format!("Serialization error: {e}"),
                });
                return;
            }
        };

        let path = std::path::Path::new(&self.settings_export_path);
        let tmp_path = path.with_extension("json.tmp");

        let result = (|| -> anyhow::Result<()> {
            let mut file = std::fs::File::create(&tmp_path)?;
            std::io::Write::write_all(&mut file, json.as_bytes())?;
            file.sync_all()?;
            std::fs::rename(&tmp_path, path)?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                self.settings_status_message = Some(StatusMessage {
                    kind: StatusKind::Success,
                    text: format!("Exported to {}", self.settings_export_path),
                });
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_path);
                self.settings_status_message = Some(StatusMessage {
                    kind: StatusKind::Error,
                    text: format!("Export failed: {e}"),
                });
            }
        }
    }

    pub fn import_data(&mut self) {
        let path = std::path::Path::new(&self.settings_import_path);

        // Read and parse
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                self.settings_status_message = Some(StatusMessage {
                    kind: StatusKind::Error,
                    text: format!("Could not read file: {e}"),
                });
                return;
            }
        };

        let export: ExportData = match serde_json::from_str(&content) {
            Ok(d) => d,
            Err(e) => {
                self.settings_status_message = Some(StatusMessage {
                    kind: StatusKind::Error,
                    text: format!("Invalid export file: {e}"),
                });
                return;
            }
        };

        // Version check
        if export.keydr_export_version != EXPORT_VERSION {
            self.settings_status_message = Some(StatusMessage {
                kind: StatusKind::Error,
                text: format!(
                    "Unsupported export version: {} (expected {})",
                    export.keydr_export_version, EXPORT_VERSION
                ),
            });
            return;
        }

        // Write data files transactionally
        let Some(ref store) = self.store else {
            self.settings_status_message = Some(StatusMessage {
                kind: StatusKind::Error,
                text: "No data store available".to_string(),
            });
            return;
        };
        if let Err(e) = store.import_all(&export) {
            self.settings_status_message = Some(StatusMessage {
                kind: StatusKind::Error,
                text: format!("Import failed: {e}"),
            });
            return;
        }

        // Merge config: import everything except machine-local paths
        let preserved_code_dir = self.config.code_download_dir.clone();
        let preserved_passage_dir = self.config.passage_download_dir.clone();
        self.config = export.config.clone();
        self.config.code_download_dir = preserved_code_dir;
        self.config.passage_download_dir = preserved_passage_dir;

        // Validate and save config
        let valid_keys: Vec<&str> = code_language_options().iter().map(|(k, _)| *k).collect();
        self.config.validate(&valid_keys);
        let _ = self.config.save();

        // Reload in-memory state from imported data
        self.profile = export.profile;
        self.key_stats = export.key_stats.stats;
        self.key_stats.target_cpm = self.config.target_cpm();
        self.ranked_key_stats = export.ranked_key_stats.stats;
        self.ranked_key_stats.target_cpm = self.config.target_cpm();
        self.drill_history = export.drill_history.drills;
        self.skill_tree = SkillTree::new(self.profile.skill_tree.clone());
        self.keyboard_model = KeyboardModel::from_name(&self.config.keyboard_layout);

        // Rebuild n-gram stats from imported drill history
        self.rebuild_ngram_stats();

        // Check theme availability
        let theme_name = self.config.theme.clone();
        let loaded_theme = Theme::load(&theme_name).unwrap_or_default();
        let theme_fell_back = loaded_theme.name != theme_name;
        let theme: &'static Theme = Box::leak(Box::new(loaded_theme));
        self.theme = theme;
        self.menu = Menu::new(theme);

        if theme_fell_back {
            self.config.theme = self.theme.name.clone();
            let _ = self.config.save();
            self.settings_status_message = Some(StatusMessage {
                kind: StatusKind::Success,
                text: format!(
                    "Imported successfully (theme '{}' not found, using default)",
                    theme_name
                ),
            });
        } else {
            self.settings_status_message = Some(StatusMessage {
                kind: StatusKind::Success,
                text: "Imported successfully".to_string(),
            });
        }
    }

    pub fn start_drill(&mut self) {
        self.clear_post_drill_input_lock();
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

                // Select focus targets: char and bigram independently
                let selection = select_focus(
                    &self.skill_tree,
                    scope,
                    &self.ranked_key_stats,
                    &self.ranked_bigram_stats,
                );
                self.current_focus = Some(selection.clone());
                let focused_char = selection.char_focus;
                let focused_bigram = selection.bigram_focus.map(|(k, _, _)| k.0);

                // Generate base lowercase text using only lowercase keys from scope
                let lowercase_keys: Vec<char> = all_keys
                    .iter()
                    .copied()
                    .filter(|ch| ch.is_ascii_lowercase() || *ch == ' ')
                    .collect();
                let filter = CharFilter::new(lowercase_keys);
                // Only pass focused to phonetic generator if it's a lowercase letter
                let lowercase_focused = focused_char.filter(|ch| ch.is_ascii_lowercase());
                let table = self.transition_table.clone();
                let dict = Dictionary::load();
                let rng = SmallRng::from_rng(&mut self.rng).unwrap();
                let cross_drill_history: HashSet<String> =
                    self.adaptive_word_history.iter().flatten().cloned().collect();
                let mut generator =
                    PhoneticGenerator::new(table, dict, rng, cross_drill_history);
                let mut text =
                    generator.generate(&filter, lowercase_focused, focused_bigram, word_count);

                // Track words for cross-drill history (before capitalization/punctuation)
                let drill_words: HashSet<String> =
                    text.split_whitespace().map(|w| w.to_string()).collect();
                self.adaptive_word_history.push_back(drill_words);
                if self.adaptive_word_history.len() > 5 {
                    self.adaptive_word_history.pop_front();
                }

                // Apply capitalization if uppercase keys are in scope
                let cap_keys: Vec<char> = all_keys
                    .iter()
                    .copied()
                    .filter(|ch| ch.is_ascii_uppercase())
                    .collect();
                if !cap_keys.is_empty() {
                    let mut rng = SmallRng::from_rng(&mut self.rng).unwrap();
                    text =
                        capitalize::apply_capitalization(&text, &cap_keys, focused_char, &mut rng);
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
                    text = punctuate::apply_punctuation(&text, &punct_keys, focused_char, &mut rng);
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
                    text =
                        numbers::apply_numbers(&text, &digit_keys, has_dot, focused_char, &mut rng);
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
                            focused_char,
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
                let lang = self
                    .code_drill_language_override
                    .clone()
                    .unwrap_or_else(|| self.config.code_language.clone());
                self.last_code_drill_language = Some(lang.clone());
                let rng = SmallRng::from_rng(&mut self.rng).unwrap();
                let mut generator =
                    CodeSyntaxGenerator::new(rng, &lang, &self.config.code_download_dir);
                self.code_drill_language_override = None;
                let text = generator.generate(&filter, None, None, word_count);
                (text, Some(generator.last_source().to_string()))
            }
            DrillMode::Passage => {
                let filter = CharFilter::new(('a'..='z').collect());
                let rng = SmallRng::from_rng(&mut self.rng).unwrap();
                let selection = self
                    .passage_drill_selection_override
                    .clone()
                    .unwrap_or_else(|| self.config.passage_book.clone());
                self.last_passage_drill_selection = Some(selection.clone());
                let mut generator = PassageGenerator::new(
                    rng,
                    &selection,
                    &self.config.passage_download_dir,
                    self.config.passage_paragraphs_per_book,
                    self.config.passage_downloads_enabled,
                );
                self.passage_drill_selection_override = None;
                let text = generator.generate(&filter, None, None, word_count);
                (text, Some(generator.last_source().to_string()))
            }
        }
    }

    pub fn type_char(&mut self, ch: char) {
        if let Some(ref mut drill) = self.drill {
            let event = input::process_char(drill, ch);
            let had_event = event.is_some();
            if let Some(event) = event {
                self.drill_events.push(event);
            }

            if drill.is_complete() {
                let synthetic_reached_end = drill
                    .synthetic_spans
                    .last()
                    .is_some_and(|span| span.end == drill.target.len());
                if synthetic_reached_end && had_event {
                    // Give the user a chance to backspace erroneous Enter/Tab spans at EOF.
                    return;
                }
                self.finish_drill();
            }
        }
    }

    pub fn backspace(&mut self) {
        if let Some(ref mut drill) = self.drill {
            if drill.cursor == 0 {
                return;
            }
            self.drill_events.push(KeystrokeEvent {
                expected: BACKSPACE,
                actual: BACKSPACE,
                timestamp: Instant::now(),
                correct: true,
            });
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
                false,
            );

            // Update timing stats for all drill modes
            let before_stats = if ranked {
                Some(self.ranked_key_stats.clone())
            } else {
                None
            };
            for kt in &result.per_key_times {
                if kt.correct {
                    self.key_stats.update_key(kt.key, kt.time_ms);
                } else {
                    self.key_stats.update_key_error(kt.key);
                }
            }

            // Extract and update n-gram stats for all drill modes
            let drill_index = self.drill_history.len() as u32;
            let hesitation_thresh =
                ngram_stats::hesitation_threshold(self.user_median_transition_ms);
            let (bigram_events, trigram_events) =
                extract_ngram_events(&result.per_key_times, hesitation_thresh);
            // Collect unique bigram keys for per-drill streak updates
            let mut seen_bigrams: std::collections::HashSet<ngram_stats::BigramKey> =
                std::collections::HashSet::new();
            for ev in &bigram_events {
                seen_bigrams.insert(ev.key.clone());
                self.bigram_stats.update(
                    ev.key.clone(),
                    ev.total_time_ms,
                    ev.correct,
                    ev.has_hesitation,
                    drill_index,
                );
            }
            // Update streaks once per drill per unique bigram (not per event)
            for key in &seen_bigrams {
                self.bigram_stats
                    .update_error_anomaly_streak(key, &self.key_stats);
                self.bigram_stats
                    .update_speed_anomaly_streak(key, &self.key_stats);
            }
            for ev in &trigram_events {
                self.trigram_stats.update(
                    ev.key.clone(),
                    ev.total_time_ms,
                    ev.correct,
                    ev.has_hesitation,
                    drill_index,
                );
            }

            if ranked {
                let mut seen_ranked_bigrams: std::collections::HashSet<ngram_stats::BigramKey> =
                    std::collections::HashSet::new();
                for kt in &result.per_key_times {
                    if kt.correct {
                        self.ranked_key_stats.update_key(kt.key, kt.time_ms);
                    } else {
                        self.ranked_key_stats.update_key_error(kt.key);
                    }
                }
                for ev in &bigram_events {
                    seen_ranked_bigrams.insert(ev.key.clone());
                    self.ranked_bigram_stats.update(
                        ev.key.clone(),
                        ev.total_time_ms,
                        ev.correct,
                        ev.has_hesitation,
                        drill_index,
                    );
                }
                for key in &seen_ranked_bigrams {
                    self.ranked_bigram_stats
                        .update_error_anomaly_streak(key, &self.ranked_key_stats);
                    self.ranked_bigram_stats
                        .update_speed_anomaly_streak(key, &self.ranked_key_stats);
                }
                for ev in &trigram_events {
                    self.ranked_trigram_stats.update(
                        ev.key.clone(),
                        ev.total_time_ms,
                        ev.correct,
                        ev.has_hesitation,
                        drill_index,
                    );
                }
                let update = self
                    .skill_tree
                    .update(&self.ranked_key_stats, before_stats.as_ref());

                // Queue milestone overlays for newly unlocked keys
                if !update.newly_unlocked.is_empty() {
                    let finger_info: Vec<(char, String)> = update
                        .newly_unlocked
                        .iter()
                        .map(|&ch| {
                            let desc = self.keyboard_model.finger_for_char(ch).description();
                            (ch, desc.to_string())
                        })
                        .collect();
                    let msg = UNLOCK_MESSAGES[self.rng.gen_range(0..UNLOCK_MESSAGES.len())];
                    self.milestone_queue.push_back(KeyMilestonePopup {
                        kind: MilestoneKind::Unlock,
                        keys: update.newly_unlocked,
                        finger_info,
                        message: msg,
                    });
                }

                // Queue milestone overlays for newly mastered keys
                if !update.newly_mastered.is_empty() {
                    let finger_info: Vec<(char, String)> = update
                        .newly_mastered
                        .iter()
                        .map(|&ch| {
                            let desc = self.keyboard_model.finger_for_char(ch).description();
                            (ch, desc.to_string())
                        })
                        .collect();
                    let msg = MASTERY_MESSAGES[self.rng.gen_range(0..MASTERY_MESSAGES.len())];
                    self.milestone_queue.push_back(KeyMilestonePopup {
                        kind: MilestoneKind::Mastery,
                        keys: update.newly_mastered,
                        finger_info,
                        message: msg,
                    });
                }
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

            // Update transition buffer for hesitation baseline
            self.update_transition_buffer(&result.per_key_times);

            // Periodic trigram marginal gain analysis (every 50 drills)
            if self.profile.total_drills % 50 == 0 && self.profile.total_drills > 0 {
                let gain = ngram_stats::trigram_marginal_gain(
                    &self.ranked_trigram_stats,
                    &self.ranked_bigram_stats,
                    &self.ranked_key_stats,
                );
                self.trigram_gain_history.push(gain);
            }

            self.drill_history.push(result.clone());
            if self.drill_history.len() > 500 {
                self.drill_history.remove(0);
            }

            self.last_result = Some(result);
            if !self.milestone_queue.is_empty() || self.drill_mode != DrillMode::Adaptive {
                self.arm_post_drill_input_lock();
            }

            // Adaptive mode auto-continues unless milestone popups must be shown first.
            if self.drill_mode == DrillMode::Adaptive && self.milestone_queue.is_empty() {
                self.start_drill();
                self.arm_post_drill_input_lock();
            } else {
                self.screen = AppScreen::DrillResult;
            }

            self.save_data();
        }
    }

    pub fn finish_partial_drill(&mut self) {
        if let Some(ref drill) = self.drill {
            let result = DrillResult::from_drill(
                drill,
                &self.drill_events,
                self.drill_mode.as_str(),
                false,
                true,
            );

            // Update timing stats for all completed keystrokes
            for kt in &result.per_key_times {
                if kt.correct {
                    self.key_stats.update_key(kt.key, kt.time_ms);
                } else {
                    self.key_stats.update_key_error(kt.key);
                }
            }

            // Extract and update n-gram stats
            let drill_index = self.drill_history.len() as u32;
            let hesitation_thresh =
                ngram_stats::hesitation_threshold(self.user_median_transition_ms);
            let (bigram_events, trigram_events) =
                extract_ngram_events(&result.per_key_times, hesitation_thresh);
            let mut seen_bigrams: std::collections::HashSet<ngram_stats::BigramKey> =
                std::collections::HashSet::new();
            for ev in &bigram_events {
                seen_bigrams.insert(ev.key.clone());
                self.bigram_stats.update(
                    ev.key.clone(),
                    ev.total_time_ms,
                    ev.correct,
                    ev.has_hesitation,
                    drill_index,
                );
            }
            for key in &seen_bigrams {
                self.bigram_stats
                    .update_error_anomaly_streak(key, &self.key_stats);
                self.bigram_stats
                    .update_speed_anomaly_streak(key, &self.key_stats);
            }
            for ev in &trigram_events {
                self.trigram_stats.update(
                    ev.key.clone(),
                    ev.total_time_ms,
                    ev.correct,
                    ev.has_hesitation,
                    drill_index,
                );
            }

            // Update transition buffer for hesitation baseline
            self.update_transition_buffer(&result.per_key_times);

            self.drill_history.push(result.clone());
            if self.drill_history.len() > 500 {
                self.drill_history.remove(0);
            }

            self.last_result = Some(result);
            self.arm_post_drill_input_lock();
            self.screen = AppScreen::DrillResult;
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
            let _ = store.save_ranked_key_stats(&KeyStatsData {
                schema_version: 2,
                stats: self.ranked_key_stats.clone(),
            });
            let _ = store.save_drill_history(&DrillHistoryData {
                schema_version: 2,
                drills: self.drill_history.clone(),
            });
        }
    }

    /// Replace up to 40% of words with dictionary words containing the target bigram.
    /// No more than 3 consecutive bigram-focused words to prevent repetitive feel.
    /// Update the rolling transition buffer with new inter-keystroke intervals.
    fn update_transition_buffer(&mut self, per_key_times: &[KeyTime]) {
        for kt in per_key_times {
            if kt.key == BACKSPACE {
                continue;
            }
            self.transition_buffer.push(kt.time_ms);
        }
        // Keep only last 200 entries
        if self.transition_buffer.len() > 200 {
            let excess = self.transition_buffer.len() - 200;
            self.transition_buffer.drain(..excess);
        }
        // Recompute median
        let mut buf = self.transition_buffer.clone();
        self.user_median_transition_ms = ngram_stats::compute_median(&mut buf);
    }

    /// Rebuild all n-gram stats and char-level error/total counts from drill history.
    /// This is the sole source of truth for error_count/total_count on KeyStat
    /// and all n-gram stores. Timing EMA on KeyStat is NOT touched here
    /// (it is either loaded from disk or rebuilt by `rebuild_from_history`).
    fn rebuild_ngram_stats(&mut self) {
        // Reset n-gram stores
        self.bigram_stats = BigramStatsStore::default();
        self.ranked_bigram_stats = BigramStatsStore::default();
        self.trigram_stats = TrigramStatsStore::default();
        self.ranked_trigram_stats = TrigramStatsStore::default();
        self.transition_buffer.clear();
        self.user_median_transition_ms = 0.0;

        // Reset char-level error/total counts and EMA (timing fields are untouched)
        for stat in self.key_stats.stats.values_mut() {
            stat.error_count = 0;
            stat.total_count = 0;
            stat.error_rate_ema = 0.5;
        }
        for stat in self.ranked_key_stats.stats.values_mut() {
            stat.error_count = 0;
            stat.total_count = 0;
            stat.error_rate_ema = 0.5;
        }

        // Take drill_history out temporarily to avoid borrow conflict
        let history = std::mem::take(&mut self.drill_history);

        for (drill_index, result) in history.iter().enumerate() {
            let hesitation_thresh =
                ngram_stats::hesitation_threshold(self.user_median_transition_ms);
            let (bigram_events, trigram_events) =
                extract_ngram_events(&result.per_key_times, hesitation_thresh);

            // Rebuild char-level error/total counts and EMA from history
            for kt in &result.per_key_times {
                if kt.correct {
                    let stat = self.key_stats.stats.entry(kt.key).or_default();
                    stat.total_count += 1;
                    // Update error rate EMA for correct stroke
                    if stat.total_count == 1 {
                        stat.error_rate_ema = 0.0;
                    } else {
                        stat.error_rate_ema = 0.1 * 0.0 + 0.9 * stat.error_rate_ema;
                    }
                } else {
                    self.key_stats.update_key_error(kt.key);
                }
            }

            // Collect unique bigram keys seen this drill for per-drill streak updates
            let mut seen_bigrams: std::collections::HashSet<ngram_stats::BigramKey> =
                std::collections::HashSet::new();

            for ev in &bigram_events {
                seen_bigrams.insert(ev.key.clone());
                self.bigram_stats.update(
                    ev.key.clone(),
                    ev.total_time_ms,
                    ev.correct,
                    ev.has_hesitation,
                    drill_index as u32,
                );
            }
            // Update streaks once per drill per unique bigram (not per event)
            for key in &seen_bigrams {
                self.bigram_stats
                    .update_error_anomaly_streak(key, &self.key_stats);
                self.bigram_stats
                    .update_speed_anomaly_streak(key, &self.key_stats);
            }
            for ev in &trigram_events {
                self.trigram_stats.update(
                    ev.key.clone(),
                    ev.total_time_ms,
                    ev.correct,
                    ev.has_hesitation,
                    drill_index as u32,
                );
            }

            if result.ranked {
                let mut seen_ranked_bigrams: std::collections::HashSet<ngram_stats::BigramKey> =
                    std::collections::HashSet::new();

                for kt in &result.per_key_times {
                    if kt.correct {
                        let stat = self.ranked_key_stats.stats.entry(kt.key).or_default();
                        stat.total_count += 1;
                        if stat.total_count == 1 {
                            stat.error_rate_ema = 0.0;
                        } else {
                            stat.error_rate_ema = 0.1 * 0.0 + 0.9 * stat.error_rate_ema;
                        }
                    } else {
                        self.ranked_key_stats.update_key_error(kt.key);
                    }
                }
                for ev in &bigram_events {
                    seen_ranked_bigrams.insert(ev.key.clone());
                    self.ranked_bigram_stats.update(
                        ev.key.clone(),
                        ev.total_time_ms,
                        ev.correct,
                        ev.has_hesitation,
                        drill_index as u32,
                    );
                }
                for key in &seen_ranked_bigrams {
                    self.ranked_bigram_stats
                        .update_error_anomaly_streak(key, &self.ranked_key_stats);
                    self.ranked_bigram_stats
                        .update_speed_anomaly_streak(key, &self.ranked_key_stats);
                }
                for ev in &trigram_events {
                    self.ranked_trigram_stats.update(
                        ev.key.clone(),
                        ev.total_time_ms,
                        ev.correct,
                        ev.has_hesitation,
                        drill_index as u32,
                    );
                }
            }

            // Update transition buffer
            for kt in &result.per_key_times {
                if kt.key != BACKSPACE {
                    self.transition_buffer.push(kt.time_ms);
                }
            }
            if self.transition_buffer.len() > 200 {
                let excess = self.transition_buffer.len() - 200;
                self.transition_buffer.drain(..excess);
            }
            let mut buf = self.transition_buffer.clone();
            self.user_median_transition_ms = ngram_stats::compute_median(&mut buf);
        }

        // Put drill_history back
        self.drill_history = history;

        // Prune trigrams — use drill_history.len() as total, matching the drill_index
        // space used in last_seen_drill_index above (history position, includes partials)
        let total_history_entries = self.drill_history.len() as u32;
        self.trigram_stats.prune(
            ngram_stats::MAX_TRIGRAMS,
            total_history_entries,
            &self.bigram_stats,
            &self.key_stats,
        );
        self.ranked_trigram_stats.prune(
            ngram_stats::MAX_TRIGRAMS,
            total_history_entries,
            &self.ranked_bigram_stats,
            &self.ranked_key_stats,
        );
    }

    pub fn retry_drill(&mut self) {
        if let Some(ref drill) = self.drill {
            let text: String = drill.target.iter().collect();
            self.drill = Some(DrillState::new(&text));
            self.drill_events.clear();
            self.last_result = None;
            self.screen = AppScreen::Drill;
        } else {
            self.start_drill();
        }
    }

    pub fn continue_drill(&mut self) {
        self.history_confirm_delete = false;
        match self.drill_mode {
            DrillMode::Adaptive => self.start_drill(),
            DrillMode::Code => {
                if let Some(lang) = self.last_code_drill_language.clone() {
                    self.code_drill_language_override = Some(lang);
                }
                self.start_code_drill();
            }
            DrillMode::Passage => {
                if let Some(selection) = self.last_passage_drill_selection.clone() {
                    self.passage_drill_selection_override = Some(selection);
                }
                self.start_passage_drill();
            }
        }
    }

    pub fn go_to_menu(&mut self) {
        self.clear_post_drill_input_lock();
        self.screen = AppScreen::Menu;
        self.drill = None;
        self.drill_source_info = None;
        self.drill_events.clear();
    }

    pub fn go_to_stats(&mut self) {
        self.clear_post_drill_input_lock();
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
        self.ranked_key_stats = KeyStatsStore::default();
        self.ranked_key_stats.target_cpm = self.config.target_cpm();
        self.skill_tree = SkillTree::default();
        self.profile.total_score = 0.0;
        self.profile.total_drills = 0;
        self.profile.streak_days = 0;
        self.profile.best_streak = 0;
        self.profile.last_practice_date = None;

        // Replay each remaining session oldest->newest
        for result in &self.drill_history {
            // Update timing stats for all sessions
            for kt in &result.per_key_times {
                if kt.correct {
                    self.key_stats.update_key(kt.key, kt.time_ms);
                }
            }
            // Only update skill tree for ranked sessions
            if result.ranked {
                for kt in &result.per_key_times {
                    if kt.correct {
                        self.ranked_key_stats.update_key(kt.key, kt.time_ms);
                    }
                }
                self.skill_tree.update(&self.ranked_key_stats, None);
            }

            // Partial sessions are visible in history but do not affect profile/streak activity.
            if result.partial {
                continue;
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

        // Rebuild n-gram stats from the replayed history
        self.rebuild_ngram_stats();
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
        let old_mode = self.drill_mode;
        let old_scope = self.drill_scope;
        self.drill_mode = DrillMode::Adaptive;
        self.drill_scope = DrillScope::Branch(branch_id);
        if old_mode != DrillMode::Adaptive || old_scope != self.drill_scope {
            self.adaptive_word_history.clear();
        }
        self.start_drill();
    }

    pub fn go_to_keyboard(&mut self) {
        self.keyboard_explorer_selected = None;
        self.explorer_accuracy_cache_overall = None;
        self.explorer_accuracy_cache_ranked = None;
        self.screen = AppScreen::Keyboard;
    }

    pub fn key_accuracy(&mut self, ch: char, ranked_only: bool) -> (usize, usize) {
        let cache = if ranked_only {
            self.explorer_accuracy_cache_ranked
        } else {
            self.explorer_accuracy_cache_overall
        };
        if let Some((cached_key, correct, total)) = cache {
            if cached_key == ch {
                return (correct, total);
            }
        }
        let mut correct = 0usize;
        let mut total = 0usize;
        for result in &self.drill_history {
            if ranked_only && !result.ranked {
                continue;
            }
            for kt in &result.per_key_times {
                if kt.key == ch {
                    total += 1;
                    if kt.correct {
                        correct += 1;
                    }
                }
            }
        }
        if ranked_only {
            self.explorer_accuracy_cache_ranked = Some((ch, correct, total));
        } else {
            self.explorer_accuracy_cache_overall = Some((ch, correct, total));
        }
        (correct, total)
    }

    pub fn go_to_code_language_select(&mut self) {
        let options = code_language_options();
        self.code_language_selected = options
            .iter()
            .position(|(k, _)| *k == self.config.code_language)
            .unwrap_or(0);
        // Center the selected item in the viewport (rough estimate of 15 visible rows)
        self.code_language_scroll = self.code_language_selected.saturating_sub(7);
        self.screen = AppScreen::CodeLanguageSelect;
    }

    pub fn go_to_settings(&mut self) {
        self.settings_selected = 0;
        self.settings_editing_download_dir = false;
        self.screen = AppScreen::Settings;
    }

    pub fn go_to_passage_book_select(&mut self) {
        let options = passage_options();
        let selected = options
            .iter()
            .position(|(key, _)| *key == self.config.passage_book)
            .unwrap_or(0);
        self.passage_book_selected = selected;
        self.screen = AppScreen::PassageBookSelect;
    }

    pub fn go_to_passage_intro(&mut self) {
        self.passage_intro_selected = 0;
        self.passage_intro_downloads_enabled = self.config.passage_downloads_enabled;
        self.passage_intro_download_dir = self.config.passage_download_dir.clone();
        self.passage_intro_paragraph_limit = self.config.passage_paragraphs_per_book;
        self.passage_intro_downloading = false;
        self.passage_intro_download_total = 0;
        self.passage_intro_downloaded = 0;
        self.passage_intro_current_book.clear();
        self.passage_intro_download_bytes = 0;
        self.passage_intro_download_bytes_total = 0;
        self.passage_download_queue.clear();
        self.passage_download_job = None;
        self.passage_download_action = PassageDownloadCompleteAction::StartPassageDrill;
        self.screen = AppScreen::PassageIntro;
    }

    pub fn go_to_code_intro(&mut self) {
        self.code_intro_selected = 0;
        self.code_intro_downloads_enabled = self.config.code_downloads_enabled;
        self.code_intro_download_dir = self.config.code_download_dir.clone();
        self.code_intro_snippets_per_repo = self.config.code_snippets_per_repo;
        self.code_intro_downloading = false;
        self.code_intro_download_total = 0;
        self.code_intro_downloaded = 0;
        self.code_intro_current_repo.clear();
        self.code_intro_download_bytes = 0;
        self.code_intro_download_bytes_total = 0;
        self.code_download_queue.clear();
        self.code_download_job = None;
        self.code_download_action = CodeDownloadCompleteAction::StartCodeDrill;
        self.code_download_attempted = false;
        self.screen = AppScreen::CodeIntro;
    }

    pub fn start_code_drill(&mut self) {
        // Step 1: Resolve concrete language (never download with "all" selected)
        if self.code_drill_language_override.is_none() {
            let chosen = if self.config.code_language == "all" {
                let available = languages_with_content(&self.config.code_download_dir);
                if available.is_empty() {
                    "rust".to_string()
                } else {
                    let idx = self.rng.gen_range(0..available.len());
                    available[idx].to_string()
                }
            } else {
                self.config.code_language.clone()
            };
            self.code_drill_language_override = Some(chosen);
        }

        let chosen = self.code_drill_language_override.clone().unwrap();

        // Step 2: Check if we need to download (only if not already attempted)
        if self.config.code_downloads_enabled
            && !self.code_download_attempted
            && !is_language_cached(&self.config.code_download_dir, &chosen)
        {
            if let Some(lang) = language_by_key(&chosen) {
                if !lang.repos.is_empty() {
                    let repo_idx = self.rng.gen_range(0..lang.repos.len());
                    self.code_download_queue = vec![(chosen.clone(), repo_idx)];
                    self.code_intro_download_total = 1;
                    self.code_intro_downloaded = 0;
                    self.code_intro_downloading = true;
                    self.code_intro_current_repo = lang.repos[repo_idx].key.to_string();
                    self.code_download_action = CodeDownloadCompleteAction::StartCodeDrill;
                    self.code_download_job = None;
                    self.code_download_attempted = true;
                    self.screen = AppScreen::CodeDownloadProgress;
                    return;
                }
            }
        }

        // Step 3: If language has no built-in AND no cache → fallback
        if !is_language_cached(&self.config.code_download_dir, &chosen) {
            if let Some(lang) = language_by_key(&chosen) {
                if !lang.has_builtin {
                    self.code_drill_language_override = Some("rust".to_string());
                }
            }
        }

        // Step 4: Start the drill
        self.code_download_attempted = false;
        self.adaptive_word_history.clear();
        self.drill_mode = DrillMode::Code;
        self.drill_scope = DrillScope::Global;
        self.start_drill();
    }

    pub fn start_code_downloads(&mut self) {
        let queue =
            build_code_download_queue(&self.config.code_language, &self.code_intro_download_dir);

        self.code_intro_download_total = queue.len();
        self.code_download_queue = queue;
        self.code_intro_downloaded = 0;
        self.code_intro_downloading = self.code_intro_download_total > 0;
        self.code_intro_download_bytes = 0;
        self.code_intro_download_bytes_total = 0;
        self.code_download_job = None;
    }

    pub fn start_code_downloads_from_settings(&mut self) {
        self.go_to_code_intro();
        self.code_download_action = CodeDownloadCompleteAction::ReturnToSettings;
        self.start_code_downloads();
        if !self.code_intro_downloading {
            self.go_to_settings();
        }
    }

    pub fn process_code_download_tick(&mut self) {
        if !self.code_intro_downloading {
            return;
        }

        if self.code_download_job.is_none() {
            let Some((lang_key, repo_idx)) = self.code_download_queue.pop() else {
                self.code_intro_downloading = false;
                self.code_intro_current_repo.clear();
                match self.code_download_action {
                    CodeDownloadCompleteAction::StartCodeDrill => self.start_code_drill(),
                    CodeDownloadCompleteAction::ReturnToSettings => self.go_to_settings(),
                }
                return;
            };

            self.spawn_code_download_job(&lang_key, repo_idx);
            return;
        }

        let mut finished = false;
        if let Some(job) = self.code_download_job.as_mut() {
            self.code_intro_download_bytes = job.downloaded_bytes.load(Ordering::Relaxed);
            self.code_intro_download_bytes_total = job.total_bytes.load(Ordering::Relaxed);
            finished = job.done.load(Ordering::Relaxed);
        }

        if !finished {
            return;
        }

        if let Some(mut job) = self.code_download_job.take() {
            if let Some(handle) = job.handle.take() {
                let _ = handle.join();
            }
            self.code_intro_downloaded = self.code_intro_downloaded.saturating_add(1);
        }

        if self.code_intro_downloaded >= self.code_intro_download_total {
            self.code_intro_downloading = false;
            self.code_intro_current_repo.clear();
            self.code_intro_download_bytes = 0;
            self.code_intro_download_bytes_total = 0;
            match self.code_download_action {
                CodeDownloadCompleteAction::StartCodeDrill => self.start_code_drill(),
                CodeDownloadCompleteAction::ReturnToSettings => self.go_to_settings(),
            }
        }
    }

    fn spawn_code_download_job(&mut self, language_key: &str, repo_idx: usize) {
        let Some(lang) = language_by_key(language_key) else {
            return;
        };
        let Some(repo) = lang.repos.get(repo_idx) else {
            return;
        };

        self.code_intro_current_repo = repo.key.to_string();
        self.code_intro_download_bytes = 0;
        self.code_intro_download_bytes_total = 0;

        let downloaded_bytes = Arc::new(AtomicU64::new(0));
        let total_bytes = Arc::new(AtomicU64::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let success = Arc::new(AtomicBool::new(false));

        let dl_clone = Arc::clone(&downloaded_bytes);
        let total_clone = Arc::clone(&total_bytes);
        let done_clone = Arc::clone(&done);
        let success_clone = Arc::clone(&success);

        let cache_dir = self.code_intro_download_dir.clone();
        let lang_key = language_key.to_string();
        let snippets_limit = self.code_intro_snippets_per_repo;

        // Get static references for thread
        let repo_ref: &'static crate::generator::code_syntax::CodeRepo = &lang.repos[repo_idx];
        let block_style_ref: &'static crate::generator::code_syntax::BlockStyle = &lang.block_style;

        let handle = thread::spawn(move || {
            let ok = download_code_repo_to_cache_with_progress(
                &cache_dir,
                &lang_key,
                repo_ref,
                block_style_ref,
                snippets_limit,
                |downloaded, total| {
                    dl_clone.store(downloaded, Ordering::Relaxed);
                    if let Some(total) = total {
                        total_clone.store(total, Ordering::Relaxed);
                    }
                },
            );

            success_clone.store(ok, Ordering::Relaxed);
            done_clone.store(true, Ordering::Relaxed);
        });

        self.code_download_job = Some(DownloadJob {
            downloaded_bytes,
            total_bytes,
            done,
            success,
            handle: Some(handle),
        });
    }

    pub fn start_passage_drill(&mut self) {
        // Lazy source selection: choose a specific source for this drill and
        // download exactly one missing book when needed.
        if self.passage_drill_selection_override.is_none() && self.config.passage_downloads_enabled
        {
            let chosen = if self.config.passage_book == "all" {
                let count = GUTENBERG_BOOKS.len() + 1; // + built-in
                let idx = self.rng.gen_range(0..count);
                if idx == 0 {
                    "builtin".to_string()
                } else {
                    GUTENBERG_BOOKS[idx - 1].key.to_string()
                }
            } else {
                self.config.passage_book.clone()
            };

            if chosen != "builtin"
                && !is_book_cached(&self.config.passage_download_dir, &chosen)
                && book_by_key(&chosen).is_some()
            {
                self.passage_drill_selection_override = Some(chosen.clone());
                self.passage_intro_downloading = true;
                self.passage_intro_download_total = 1;
                self.passage_intro_downloaded = 0;
                self.passage_intro_download_bytes = 0;
                self.passage_intro_download_bytes_total = 0;
                self.passage_intro_current_book = book_by_key(&chosen)
                    .map(|b| b.title.to_string())
                    .unwrap_or_default();
                self.passage_download_queue = GUTENBERG_BOOKS
                    .iter()
                    .enumerate()
                    .filter_map(|(i, b)| (b.key == chosen).then_some(i))
                    .collect();
                self.passage_download_action = PassageDownloadCompleteAction::StartPassageDrill;
                self.passage_download_job = None;
                self.screen = AppScreen::PassageDownloadProgress;
                return;
            }

            self.passage_drill_selection_override = Some(chosen);
        } else if self.passage_drill_selection_override.is_none()
            && self.config.passage_book != "all"
            && self.config.passage_book != "builtin"
        {
            // Downloads disabled: gracefully fall back to built-in if selected book is unavailable.
            if !is_book_cached(&self.config.passage_download_dir, &self.config.passage_book) {
                self.passage_drill_selection_override = Some("builtin".to_string());
            } else {
                self.passage_drill_selection_override = Some(self.config.passage_book.clone());
            }
        }

        self.adaptive_word_history.clear();
        self.drill_mode = DrillMode::Passage;
        self.drill_scope = DrillScope::Global;
        self.start_drill();
    }

    pub fn start_passage_downloads(&mut self) {
        let uncached = uncached_books(&self.passage_intro_download_dir);
        let uncached_keys: std::collections::HashSet<&str> =
            uncached.iter().map(|b| b.key).collect();
        self.passage_download_queue = GUTENBERG_BOOKS
            .iter()
            .enumerate()
            .filter_map(|(i, book)| uncached_keys.contains(book.key).then_some(i))
            .collect();
        self.passage_intro_download_total = self.passage_download_queue.len();
        self.passage_intro_downloaded = 0;
        self.passage_intro_downloading = self.passage_intro_download_total > 0;
        self.passage_intro_download_bytes = 0;
        self.passage_intro_download_bytes_total = 0;
        self.passage_download_job = None;
    }

    pub fn cancel_code_download(&mut self) {
        self.code_download_queue.clear();
        self.code_intro_downloading = false;
        self.code_download_job = None;
        self.code_drill_language_override = None;
        self.code_download_attempted = false;
    }

    pub fn start_passage_downloads_from_settings(&mut self) {
        self.go_to_passage_intro();
        self.passage_download_action = PassageDownloadCompleteAction::ReturnToSettings;
        self.start_passage_downloads();
        if !self.passage_intro_downloading {
            self.go_to_settings();
        }
    }

    pub fn process_passage_download_tick(&mut self) {
        if !self.passage_intro_downloading {
            return;
        }

        if self.passage_download_job.is_none() {
            let Some(book_index) = self.passage_download_queue.pop() else {
                self.passage_intro_downloading = false;
                self.passage_intro_current_book.clear();
                match self.passage_download_action {
                    PassageDownloadCompleteAction::StartPassageDrill => self.start_passage_drill(),
                    PassageDownloadCompleteAction::ReturnToSettings => self.go_to_settings(),
                }
                return;
            };
            self.spawn_passage_download_job(book_index);
            return;
        }

        let mut finished = false;
        if let Some(job) = self.passage_download_job.as_mut() {
            self.passage_intro_download_bytes = job.downloaded_bytes.load(Ordering::Relaxed);
            self.passage_intro_download_bytes_total = job.total_bytes.load(Ordering::Relaxed);
            finished = job.done.load(Ordering::Relaxed);
        }

        if !finished {
            return;
        }

        if let Some(mut job) = self.passage_download_job.take() {
            if let Some(handle) = job.handle.take() {
                let _ = handle.join();
            }
            if job.success.load(Ordering::Relaxed) {
                self.passage_intro_downloaded = self.passage_intro_downloaded.saturating_add(1);
            } else {
                // Skip failed book and continue queue without hanging.
                self.passage_intro_downloaded = self.passage_intro_downloaded.saturating_add(1);
            }
        }

        if self.passage_intro_downloaded >= self.passage_intro_download_total {
            self.passage_intro_downloading = false;
            self.passage_intro_current_book.clear();
            self.passage_intro_download_bytes = 0;
            self.passage_intro_download_bytes_total = 0;
            match self.passage_download_action {
                PassageDownloadCompleteAction::StartPassageDrill => self.start_passage_drill(),
                PassageDownloadCompleteAction::ReturnToSettings => self.go_to_settings(),
            }
        }
    }

    fn spawn_passage_download_job(&mut self, book_index: usize) {
        let Some(book) = GUTENBERG_BOOKS.get(book_index) else {
            return;
        };

        self.passage_intro_current_book = book.title.to_string();
        self.passage_intro_download_bytes = 0;
        self.passage_intro_download_bytes_total = 0;

        let downloaded_bytes = Arc::new(AtomicU64::new(0));
        let total_bytes = Arc::new(AtomicU64::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let success = Arc::new(AtomicBool::new(false));

        let dl_clone = Arc::clone(&downloaded_bytes);
        let total_clone = Arc::clone(&total_bytes);
        let done_clone = Arc::clone(&done);
        let success_clone = Arc::clone(&success);

        let cache_dir = self.passage_intro_download_dir.clone();
        let book_ref: &'static crate::generator::passage::GutenbergBook =
            &GUTENBERG_BOOKS[book_index];

        let handle = thread::spawn(move || {
            let ok = download_book_to_cache_with_progress(
                cache_dir.as_str(),
                book_ref,
                |downloaded, total| {
                    dl_clone.store(downloaded, Ordering::Relaxed);
                    if let Some(total) = total {
                        total_clone.store(total, Ordering::Relaxed);
                    }
                },
            );

            success_clone.store(ok, Ordering::Relaxed);
            done_clone.store(true, Ordering::Relaxed);
        });

        self.passage_download_job = Some(DownloadJob {
            downloaded_bytes,
            total_bytes,
            done,
            success,
            handle: Some(handle),
        });
    }

    pub fn settings_cycle_forward(&mut self) {
        match self.settings_selected {
            0 => {
                self.config.target_wpm = (self.config.target_wpm + 5).min(200);
                self.key_stats.target_cpm = self.config.target_cpm();
                self.ranked_key_stats.target_cpm = self.config.target_cpm();
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
                let options = code_language_options();
                let keys: Vec<&str> = options.iter().map(|(k, _)| *k).collect();
                let idx = keys
                    .iter()
                    .position(|&l| l == self.config.code_language)
                    .unwrap_or(0);
                let next = (idx + 1) % keys.len();
                self.config.code_language = keys[next].to_string();
            }
            4 => {
                self.config.code_downloads_enabled = !self.config.code_downloads_enabled;
            }
            5 => {
                // Editable text field handled directly in key handler.
            }
            6 => {
                self.config.code_snippets_per_repo = match self.config.code_snippets_per_repo {
                    0 => 1,
                    n if n >= 200 => 0,
                    n => n + 10,
                };
            }
            // 7 = Download Code Now (action button)
            8 => {
                self.config.passage_downloads_enabled = !self.config.passage_downloads_enabled;
            }
            9 => {
                // Passage download dir - editable text field handled directly in key handler.
            }
            10 => {
                self.config.passage_paragraphs_per_book =
                    match self.config.passage_paragraphs_per_book {
                        0 => 1,
                        n if n >= 500 => 0,
                        n => n + 25,
                    };
            }
            _ => {}
        }
    }

    pub fn settings_cycle_backward(&mut self) {
        match self.settings_selected {
            0 => {
                self.config.target_wpm = self.config.target_wpm.saturating_sub(5).max(10);
                self.key_stats.target_cpm = self.config.target_cpm();
                self.ranked_key_stats.target_cpm = self.config.target_cpm();
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
                let options = code_language_options();
                let keys: Vec<&str> = options.iter().map(|(k, _)| *k).collect();
                let idx = keys
                    .iter()
                    .position(|&l| l == self.config.code_language)
                    .unwrap_or(0);
                let next = if idx == 0 { keys.len() - 1 } else { idx - 1 };
                self.config.code_language = keys[next].to_string();
            }
            4 => {
                self.config.code_downloads_enabled = !self.config.code_downloads_enabled;
            }
            5 => {
                // Editable text field handled directly in key handler.
            }
            6 => {
                self.config.code_snippets_per_repo = match self.config.code_snippets_per_repo {
                    0 => 200,
                    1 => 0,
                    n => n.saturating_sub(10).max(1),
                };
            }
            // 7 = Download Code Now (action button)
            8 => {
                self.config.passage_downloads_enabled = !self.config.passage_downloads_enabled;
            }
            9 => {
                // Passage download dir - editable text field handled directly in key handler.
            }
            10 => {
                self.config.passage_paragraphs_per_book =
                    match self.config.passage_paragraphs_per_book {
                        0 => 500,
                        1 => 0,
                        n => n.saturating_sub(25).max(1),
                    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::skill_tree::BranchId;

    #[test]
    fn adaptive_word_history_clears_on_code_mode_switch() {
        let mut app = App::new();

        // App starts in Adaptive/Global; new() calls start_drill() which populates history
        assert_eq!(app.drill_mode, DrillMode::Adaptive);
        assert!(
            !app.adaptive_word_history.is_empty(),
            "History should be populated after initial adaptive drill"
        );

        // Use the real start_code_drill path. Pre-set language override to skip
        // download logic and ensure it reaches the drill-start code path.
        app.code_drill_language_override = Some("rust".to_string());
        app.start_code_drill();
        assert_eq!(app.drill_mode, DrillMode::Code);
        assert!(
            app.adaptive_word_history.is_empty(),
            "History should clear when switching to Code mode via start_code_drill"
        );
    }

    #[test]
    fn adaptive_word_history_clears_on_passage_mode_switch() {
        let mut app = App::new();

        assert_eq!(app.drill_mode, DrillMode::Adaptive);
        assert!(!app.adaptive_word_history.is_empty());

        // Use the real start_passage_drill path. Pre-set selection override to
        // skip download logic and use built-in passages.
        app.config.passage_downloads_enabled = false;
        app.passage_drill_selection_override = Some("builtin".to_string());
        app.start_passage_drill();
        assert_eq!(app.drill_mode, DrillMode::Passage);
        assert!(
            app.adaptive_word_history.is_empty(),
            "History should clear when switching to Passage mode via start_passage_drill"
        );
    }

    #[test]
    fn adaptive_word_history_clears_on_scope_change() {
        let mut app = App::new();

        // Start in Adaptive/Global — drill already started in new()
        assert_eq!(app.drill_scope, DrillScope::Global);
        assert!(!app.adaptive_word_history.is_empty());

        // Use start_branch_drill to switch from Global to Branch scope.
        // This is the real production path for scope changes.
        app.start_branch_drill(BranchId::Lowercase);
        assert_eq!(app.drill_scope, DrillScope::Branch(BranchId::Lowercase));
        assert_eq!(app.drill_mode, DrillMode::Adaptive);
        // History was cleared by the Global->Branch scope change, then repopulated
        // by the single start_drill call inside start_branch_drill.
        assert_eq!(
            app.adaptive_word_history.len(),
            1,
            "History should have exactly 1 entry after Global->Branch clear + new drill"
        );

        // Record history state, then switch to a different branch
        let history_before = app.adaptive_word_history.clone();
        app.start_branch_drill(BranchId::Capitals);
        assert_eq!(app.drill_scope, DrillScope::Branch(BranchId::Capitals));
        // History was cleared by scope change and repopulated with new drill words.
        // New history should not contain the old drill's words.
        let old_words: HashSet<String> = history_before.into_iter().flatten().collect();
        let new_words: HashSet<String> = app
            .adaptive_word_history
            .iter()
            .flatten()
            .cloned()
            .collect();
        // After clearing, the new history has exactly 1 drill entry (the one just generated).
        assert_eq!(
            app.adaptive_word_history.len(),
            1,
            "History should have exactly 1 entry after scope-clearing branch switch"
        );
        // The new words should mostly differ from old (not a superset or continuation)
        assert!(
            !new_words.is_subset(&old_words) || new_words.is_empty(),
            "New history should not be a subset of old history"
        );
    }

    #[test]
    fn adaptive_word_history_persists_within_same_context() {
        let mut app = App::new();

        // Adaptive/Global: run multiple drills, history should accumulate
        let history_after_first = app.adaptive_word_history.len();
        app.start_drill();
        let history_after_second = app.adaptive_word_history.len();

        assert!(
            history_after_second > history_after_first,
            "History should accumulate across drills: {} -> {}",
            history_after_first,
            history_after_second
        );
        assert!(
            app.adaptive_word_history.len() <= 5,
            "History should be capped at 5 drills"
        );
    }

    #[test]
    fn adaptive_word_history_not_cleared_on_same_branch_redrill() {
        let mut app = App::new();

        // Start a branch drill
        app.start_branch_drill(BranchId::Lowercase);
        let history_after_first = app.adaptive_word_history.len();
        assert_eq!(history_after_first, 1);

        // Re-drill the same branch via start_branch_drill — scope doesn't change,
        // so history should NOT clear; it should accumulate.
        app.start_branch_drill(BranchId::Lowercase);
        assert!(
            app.adaptive_word_history.len() > history_after_first,
            "History should accumulate when re-drilling same branch: {} -> {}",
            history_after_first,
            app.adaptive_word_history.len()
        );
    }

    /// Helper: make the current drill look "completed" so finish_drill() processes it.
    fn complete_current_drill(app: &mut App) {
        if let Some(ref mut drill) = app.drill {
            let now = Instant::now();
            drill.started_at = Some(now - Duration::from_millis(500));
            drill.finished_at = Some(now);
            drill.cursor = drill.target.len();
            // Fill input so DrillResult::from_drill doesn't panic on length mismatches
            drill.input = vec![crate::session::input::CharStatus::Correct; drill.target.len()];
        }
    }

    #[test]
    fn adaptive_auto_continue_arms_input_lock() {
        let mut app = App::new();
        assert_eq!(app.drill_mode, DrillMode::Adaptive);
        assert_eq!(app.screen, AppScreen::Drill);
        assert!(app.drill.is_some());

        // Make sure no milestones are queued
        app.milestone_queue.clear();

        complete_current_drill(&mut app);
        app.finish_drill();

        // Auto-continue should have started a new drill and armed the lock
        assert_eq!(app.screen, AppScreen::Drill);
        assert!(
            app.post_drill_input_lock_remaining_ms().is_some(),
            "Input lock should be armed after adaptive auto-continue"
        );
    }

    #[test]
    fn adaptive_does_not_auto_continue_with_milestones() {
        let mut app = App::new();
        assert_eq!(app.drill_mode, DrillMode::Adaptive);

        // Push a milestone before finishing the drill
        app.milestone_queue.push_back(KeyMilestonePopup {
            kind: MilestoneKind::Unlock,
            keys: vec!['a'],
            finger_info: vec![('a', "left pinky".to_string())],
            message: "Test milestone",
        });

        complete_current_drill(&mut app);
        app.finish_drill();

        // Should go to DrillResult (not auto-continue) since milestones are queued
        assert_eq!(app.screen, AppScreen::DrillResult);
        // Lock IS armed via the existing milestone path
        assert!(
            app.post_drill_input_lock_remaining_ms().is_some(),
            "Input lock should be armed for milestone path"
        );
    }
}
