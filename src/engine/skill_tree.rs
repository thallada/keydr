use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::engine::key_stats::KeyStatsStore;
use crate::keyboard::display::{BACKSPACE, SPACE};

/// Events returned by `SkillTree::update` describing what changed.
pub struct SkillTreeUpdate {
    pub newly_unlocked: Vec<char>,
    pub newly_mastered: Vec<char>,
}

// --- Branch ID ---

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BranchId {
    Lowercase,
    Capitals,
    Numbers,
    ProsePunctuation,
    Whitespace,
    CodeSymbols,
}

impl BranchId {
    pub fn to_key(self) -> &'static str {
        match self {
            BranchId::Lowercase => "lowercase",
            BranchId::Capitals => "capitals",
            BranchId::Numbers => "numbers",
            BranchId::ProsePunctuation => "prose_punctuation",
            BranchId::Whitespace => "whitespace",
            BranchId::CodeSymbols => "code_symbols",
        }
    }

    #[allow(dead_code)]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "lowercase" => Some(BranchId::Lowercase),
            "capitals" => Some(BranchId::Capitals),
            "numbers" => Some(BranchId::Numbers),
            "prose_punctuation" => Some(BranchId::ProsePunctuation),
            "whitespace" => Some(BranchId::Whitespace),
            "code_symbols" => Some(BranchId::CodeSymbols),
            _ => None,
        }
    }

    pub fn all() -> &'static [BranchId] {
        &[
            BranchId::Lowercase,
            BranchId::Capitals,
            BranchId::Numbers,
            BranchId::ProsePunctuation,
            BranchId::Whitespace,
            BranchId::CodeSymbols,
        ]
    }
}

// --- Branch Status ---

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BranchStatus {
    Locked,
    Available,
    InProgress,
    Complete,
}

// --- Static Definitions ---

pub struct LevelDefinition {
    pub name: &'static str,
    pub keys: &'static [char],
}

pub struct BranchDefinition {
    pub id: BranchId,
    pub name: &'static str,
    pub levels: &'static [LevelDefinition],
}

const LOWERCASE_LEVELS: &[LevelDefinition] = &[LevelDefinition {
    name: "Frequency Order",
    keys: &[
        'e', 't', 'a', 'o', 'i', 'n', 's', 'h', 'r', 'd', 'l', 'c', 'u', 'm', 'w', 'f', 'g', 'y',
        'p', 'b', 'v', 'k', 'j', 'x', 'q', 'z',
    ],
}];

const CAPITALS_LEVELS: &[LevelDefinition] = &[
    LevelDefinition {
        name: "Common Sentence Capitals",
        keys: &['T', 'I', 'A', 'S', 'W', 'H', 'B', 'M'],
    },
    LevelDefinition {
        name: "Name Capitals",
        keys: &['J', 'D', 'R', 'C', 'E', 'N', 'P', 'L', 'F', 'G'],
    },
    LevelDefinition {
        name: "Remaining Capitals",
        keys: &['O', 'U', 'K', 'V', 'Y', 'X', 'Q', 'Z'],
    },
];

const NUMBERS_LEVELS: &[LevelDefinition] = &[
    LevelDefinition {
        name: "Common Digits",
        keys: &['1', '2', '3', '4', '5'],
    },
    LevelDefinition {
        name: "All Digits",
        keys: &['0', '6', '7', '8', '9'],
    },
];

const PROSE_PUNCTUATION_LEVELS: &[LevelDefinition] = &[
    LevelDefinition {
        name: "Essential",
        keys: &['.', ',', '\''],
    },
    LevelDefinition {
        name: "Common",
        keys: &[';', ':', '"', '-'],
    },
    LevelDefinition {
        name: "Expressive",
        keys: &['?', '!', '(', ')'],
    },
];

const WHITESPACE_LEVELS: &[LevelDefinition] = &[
    LevelDefinition {
        name: "Enter/Return",
        keys: &['\n'],
    },
    LevelDefinition {
        name: "Tab/Indent",
        keys: &['\t'],
    },
];

const CODE_SYMBOLS_LEVELS: &[LevelDefinition] = &[
    LevelDefinition {
        name: "Arithmetic & Assignment",
        keys: &['=', '+', '*', '/', '-'],
    },
    LevelDefinition {
        name: "Grouping",
        keys: &['{', '}', '[', ']', '<', '>'],
    },
    LevelDefinition {
        name: "Logic & Reference",
        keys: &['&', '|', '^', '~', '!'],
    },
    LevelDefinition {
        name: "Special",
        keys: &['@', '#', '$', '%', '_', '\\', '`'],
    },
];

pub const ALL_BRANCHES: &[BranchDefinition] = &[
    BranchDefinition {
        id: BranchId::Lowercase,
        name: "Lowercase a-z",
        levels: LOWERCASE_LEVELS,
    },
    BranchDefinition {
        id: BranchId::Capitals,
        name: "Capitals A-Z",
        levels: CAPITALS_LEVELS,
    },
    BranchDefinition {
        id: BranchId::Numbers,
        name: "Numbers 0-9",
        levels: NUMBERS_LEVELS,
    },
    BranchDefinition {
        id: BranchId::ProsePunctuation,
        name: "Prose Punctuation",
        levels: PROSE_PUNCTUATION_LEVELS,
    },
    BranchDefinition {
        id: BranchId::Whitespace,
        name: "Whitespace",
        levels: WHITESPACE_LEVELS,
    },
    BranchDefinition {
        id: BranchId::CodeSymbols,
        name: "Code Symbols",
        levels: CODE_SYMBOLS_LEVELS,
    },
];

/// Find which branch and level a key belongs to.
/// Returns (branch_def, level_name, 1-based position in level).
pub fn find_key_branch(ch: char) -> Option<(&'static BranchDefinition, &'static str, usize)> {
    for branch in ALL_BRANCHES {
        for level in branch.levels {
            if let Some(pos) = level.keys.iter().position(|&k| k == ch) {
                return Some((branch, level.name, pos + 1));
            }
        }
    }
    None
}

pub fn get_branch_definition(id: BranchId) -> &'static BranchDefinition {
    ALL_BRANCHES
        .iter()
        .find(|b| b.id == id)
        .expect("branch definition not found")
}

// --- Persisted Progress ---

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchProgress {
    pub status: BranchStatus,
    pub current_level: usize,
}

impl Default for BranchProgress {
    fn default() -> Self {
        Self {
            status: BranchStatus::Locked,
            current_level: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillTreeProgress {
    pub branches: HashMap<String, BranchProgress>,
}

impl Default for SkillTreeProgress {
    fn default() -> Self {
        let mut branches = HashMap::new();
        // Lowercase starts as InProgress; everything else Locked
        branches.insert(
            BranchId::Lowercase.to_key().to_string(),
            BranchProgress {
                status: BranchStatus::InProgress,
                current_level: 0,
            },
        );
        for &id in &[
            BranchId::Capitals,
            BranchId::Numbers,
            BranchId::ProsePunctuation,
            BranchId::Whitespace,
            BranchId::CodeSymbols,
        ] {
            branches.insert(id.to_key().to_string(), BranchProgress::default());
        }
        Self { branches }
    }
}

// --- Skill Tree Engine ---

/// The scope for key collection and focus selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrillScope {
    /// Global adaptive: all InProgress + Complete branches
    Global,
    /// Branch-specific drill: specific branch + a-z background
    Branch(BranchId),
}

pub struct SkillTree {
    pub progress: SkillTreeProgress,
    pub total_unique_keys: usize,
}

/// Number of lowercase letters to start with before unlocking one-at-a-time
const LOWERCASE_MIN_KEYS: usize = 6;
const ALWAYS_UNLOCKED_KEYS: &[char] = &[SPACE, BACKSPACE];

impl SkillTree {
    pub fn new(progress: SkillTreeProgress) -> Self {
        let total_unique_keys = Self::compute_total_unique_keys();
        Self {
            progress,
            total_unique_keys,
        }
    }

    fn compute_total_unique_keys() -> usize {
        let mut all_keys: HashSet<char> = HashSet::new();
        for branch in ALL_BRANCHES {
            for level in branch.levels {
                for &key in level.keys {
                    all_keys.insert(key);
                }
            }
        }
        all_keys.extend(ALWAYS_UNLOCKED_KEYS.iter().copied());
        all_keys.len()
    }

    pub fn branch_status(&self, id: BranchId) -> &BranchStatus {
        self.progress
            .branches
            .get(id.to_key())
            .map(|bp| &bp.status)
            .unwrap_or(&BranchStatus::Locked)
    }

    pub fn branch_progress(&self, id: BranchId) -> &BranchProgress {
        static DEFAULT: BranchProgress = BranchProgress {
            status: BranchStatus::Locked,
            current_level: 0,
        };
        self.progress.branches.get(id.to_key()).unwrap_or(&DEFAULT)
    }

    pub fn branch_progress_mut(&mut self, id: BranchId) -> &mut BranchProgress {
        self.progress
            .branches
            .entry(id.to_key().to_string())
            .or_default()
    }

    /// Start a branch (transition Available -> InProgress).
    pub fn start_branch(&mut self, id: BranchId) {
        let bp = self.branch_progress_mut(id);
        if bp.status == BranchStatus::Available {
            bp.status = BranchStatus::InProgress;
            bp.current_level = 0;
        }
    }

    /// Collect all unlocked keys for the given scope.
    pub fn unlocked_keys(&self, scope: DrillScope) -> Vec<char> {
        match scope {
            DrillScope::Global => self.global_unlocked_keys(),
            DrillScope::Branch(id) => self.branch_unlocked_keys(id),
        }
    }

    fn global_unlocked_keys(&self) -> Vec<char> {
        let mut keys = ALWAYS_UNLOCKED_KEYS.to_vec();
        for branch_def in ALL_BRANCHES {
            let bp = self.branch_progress(branch_def.id);
            match bp.status {
                BranchStatus::InProgress => {
                    // For lowercase, use the progressive unlock system
                    if branch_def.id == BranchId::Lowercase {
                        keys.extend(self.lowercase_unlocked_keys());
                    } else {
                        // Include current level's keys + all prior levels
                        for (i, level) in branch_def.levels.iter().enumerate() {
                            if i <= bp.current_level {
                                keys.extend_from_slice(level.keys);
                            }
                        }
                    }
                }
                BranchStatus::Complete => {
                    for level in branch_def.levels {
                        keys.extend_from_slice(level.keys);
                    }
                }
                _ => {}
            }
        }
        keys
    }

    fn branch_unlocked_keys(&self, id: BranchId) -> Vec<char> {
        let mut keys = ALWAYS_UNLOCKED_KEYS.to_vec();

        // Always include a-z background keys
        if id != BranchId::Lowercase {
            let lowercase_def = get_branch_definition(BranchId::Lowercase);
            let lowercase_bp = self.branch_progress(BranchId::Lowercase);
            match lowercase_bp.status {
                BranchStatus::InProgress => keys.extend(self.lowercase_unlocked_keys()),
                BranchStatus::Complete => {
                    for level in lowercase_def.levels {
                        keys.extend_from_slice(level.keys);
                    }
                }
                _ => {}
            }
        }

        // Include keys from the target branch
        let branch_def = get_branch_definition(id);
        let bp = self.branch_progress(id);
        if id == BranchId::Lowercase {
            keys.extend(self.lowercase_unlocked_keys());
        } else {
            match bp.status {
                BranchStatus::InProgress => {
                    for (i, level) in branch_def.levels.iter().enumerate() {
                        if i <= bp.current_level {
                            keys.extend_from_slice(level.keys);
                        }
                    }
                }
                BranchStatus::Complete => {
                    for level in branch_def.levels {
                        keys.extend_from_slice(level.keys);
                    }
                }
                _ => {}
            }
        }

        keys
    }

    /// Get the progressively-unlocked lowercase keys (mirrors old LetterUnlock logic).
    fn lowercase_unlocked_keys(&self) -> Vec<char> {
        let def = get_branch_definition(BranchId::Lowercase);
        let bp = self.branch_progress(BranchId::Lowercase);
        let all_keys = def.levels[0].keys;

        match bp.status {
            BranchStatus::Complete => all_keys.to_vec(),
            BranchStatus::InProgress => {
                // current_level represents number of keys unlocked beyond LOWERCASE_MIN_KEYS
                let count = (LOWERCASE_MIN_KEYS + bp.current_level).min(all_keys.len());
                all_keys[..count].to_vec()
            }
            _ => Vec::new(),
        }
    }

    /// Number of unlocked lowercase letters (for display).
    pub fn lowercase_unlocked_count(&self) -> usize {
        self.lowercase_unlocked_keys().len()
    }

    /// Find the focused (weakest) key for the given scope.
    pub fn focused_key(&self, scope: DrillScope, stats: &KeyStatsStore) -> Option<char> {
        match scope {
            DrillScope::Global => self.global_focused_key(stats),
            DrillScope::Branch(id) => self.branch_focused_key(id, stats),
        }
    }

    fn global_focused_key(&self, stats: &KeyStatsStore) -> Option<char> {
        // Collect keys from all InProgress branches (current level only) + complete branches
        let mut focus_candidates = Vec::new();
        for branch_def in ALL_BRANCHES {
            let bp = self.branch_progress(branch_def.id);
            match bp.status {
                BranchStatus::InProgress => {
                    if branch_def.id == BranchId::Lowercase {
                        focus_candidates.extend(self.lowercase_unlocked_keys());
                    } else if bp.current_level < branch_def.levels.len() {
                        // Only current level keys are focus candidates
                        focus_candidates
                            .extend_from_slice(branch_def.levels[bp.current_level].keys);
                        // Plus prior level keys for reinforcement
                        for i in 0..bp.current_level {
                            focus_candidates.extend_from_slice(branch_def.levels[i].keys);
                        }
                    }
                }
                BranchStatus::Complete => {
                    for level in branch_def.levels {
                        focus_candidates.extend_from_slice(level.keys);
                    }
                }
                _ => {}
            }
        }

        Self::weakest_key(&focus_candidates, stats)
    }

    fn branch_focused_key(&self, id: BranchId, stats: &KeyStatsStore) -> Option<char> {
        let branch_def = get_branch_definition(id);
        let bp = self.branch_progress(id);

        if id == BranchId::Lowercase {
            return Self::weakest_key(&self.lowercase_unlocked_keys(), stats);
        }

        match bp.status {
            BranchStatus::InProgress if bp.current_level < branch_def.levels.len() => {
                // Focus only within current level's keys
                let current_keys = branch_def.levels[bp.current_level].keys;
                Self::weakest_key(&current_keys.to_vec(), stats)
            }
            _ => None,
        }
    }

    fn weakest_key(keys: &[char], stats: &KeyStatsStore) -> Option<char> {
        keys.iter()
            .filter(|&&ch| stats.get_confidence(ch) < 1.0)
            .min_by(|&&a, &&b| {
                stats
                    .get_confidence(a)
                    .partial_cmp(&stats.get_confidence(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    }

    /// Update skill tree progress based on current key stats.
    /// Call after updating KeyStatsStore.
    ///
    /// `before_stats` is an optional snapshot of key stats *before* this drill's data was added.
    /// When provided, it's used to detect which keys were newly mastered (confidence crossing 1.0).
    /// Returns a `SkillTreeUpdate` describing which keys were newly unlocked or mastered.
    pub fn update(
        &mut self,
        stats: &KeyStatsStore,
        before_stats: Option<&KeyStatsStore>,
    ) -> SkillTreeUpdate {
        // Snapshot unlocked keys before tree structure changes
        let before_unlocked: HashSet<char> =
            self.unlocked_keys(DrillScope::Global).into_iter().collect();

        // Update lowercase branch (progressive unlock)
        self.update_lowercase(stats);

        // Check if lowercase is complete -> unlock other branches
        if *self.branch_status(BranchId::Lowercase) == BranchStatus::Complete {
            for &id in &[
                BranchId::Capitals,
                BranchId::Numbers,
                BranchId::ProsePunctuation,
                BranchId::Whitespace,
                BranchId::CodeSymbols,
            ] {
                let bp = self.branch_progress_mut(id);
                if bp.status == BranchStatus::Locked {
                    bp.status = BranchStatus::Available;
                }
            }
        }

        // Update InProgress branches (non-lowercase)
        for branch_def in ALL_BRANCHES {
            if branch_def.id == BranchId::Lowercase {
                continue;
            }
            let bp = self.branch_progress(branch_def.id).clone();
            if bp.status != BranchStatus::InProgress {
                continue;
            }
            self.update_branch_level(branch_def, stats);
        }

        // Snapshot after
        let after_unlocked: HashSet<char> =
            self.unlocked_keys(DrillScope::Global).into_iter().collect();

        let newly_unlocked: Vec<char> = after_unlocked
            .difference(&before_unlocked)
            .copied()
            .collect();

        // Detect mastery: keys that were unlocked before, had confidence < 1.0 in before_stats,
        // but now have confidence >= 1.0 in current stats
        let newly_mastered: Vec<char> = if let Some(before) = before_stats {
            before_unlocked
                .iter()
                .filter(|&&ch| before.get_confidence(ch) < 1.0 && stats.get_confidence(ch) >= 1.0)
                .copied()
                .collect()
        } else {
            Vec::new()
        };

        SkillTreeUpdate {
            newly_unlocked,
            newly_mastered,
        }
    }

    fn update_lowercase(&mut self, stats: &KeyStatsStore) {
        let bp = self.branch_progress(BranchId::Lowercase).clone();
        if bp.status != BranchStatus::InProgress {
            return;
        }

        let all_keys = get_branch_definition(BranchId::Lowercase).levels[0].keys;
        let current_count = LOWERCASE_MIN_KEYS + bp.current_level;

        if current_count >= all_keys.len() {
            // All 26 keys unlocked, check if all confident
            let all_confident = all_keys.iter().all(|&ch| stats.get_confidence(ch) >= 1.0);
            if all_confident {
                let bp_mut = self.branch_progress_mut(BranchId::Lowercase);
                bp_mut.status = BranchStatus::Complete;
                bp_mut.current_level = all_keys.len() - LOWERCASE_MIN_KEYS;
            }
            return;
        }

        // Check if all current keys are confident -> unlock next
        let current_keys = &all_keys[..current_count];
        let all_confident = current_keys
            .iter()
            .all(|&ch| stats.get_confidence(ch) >= 1.0);

        if all_confident {
            let bp_mut = self.branch_progress_mut(BranchId::Lowercase);
            bp_mut.current_level += 1;
        }
    }

    fn update_branch_level(&mut self, branch_def: &BranchDefinition, stats: &KeyStatsStore) {
        let bp = self.branch_progress(branch_def.id).clone();
        if bp.current_level >= branch_def.levels.len() {
            // Already past last level, mark complete
            let bp_mut = self.branch_progress_mut(branch_def.id);
            bp_mut.status = BranchStatus::Complete;
            return;
        }

        // Check if all keys in current level are confident
        let current_level_keys = branch_def.levels[bp.current_level].keys;
        let all_confident = current_level_keys
            .iter()
            .all(|&ch| stats.get_confidence(ch) >= 1.0);

        if all_confident {
            let bp_mut = self.branch_progress_mut(branch_def.id);
            bp_mut.current_level += 1;
            if bp_mut.current_level >= branch_def.levels.len() {
                bp_mut.status = BranchStatus::Complete;
            }
        }
    }

    /// Total number of unlocked unique keys across all branches.
    pub fn total_unlocked_count(&self) -> usize {
        let mut keys: HashSet<char> = HashSet::new();
        keys.extend(ALWAYS_UNLOCKED_KEYS.iter().copied());
        for branch_def in ALL_BRANCHES {
            let bp = self.branch_progress(branch_def.id);
            match bp.status {
                BranchStatus::InProgress => {
                    if branch_def.id == BranchId::Lowercase {
                        for key in self.lowercase_unlocked_keys() {
                            keys.insert(key);
                        }
                    } else {
                        for (i, level) in branch_def.levels.iter().enumerate() {
                            if i <= bp.current_level {
                                for &key in level.keys {
                                    keys.insert(key);
                                }
                            }
                        }
                    }
                }
                BranchStatus::Complete => {
                    for level in branch_def.levels {
                        for &key in level.keys {
                            keys.insert(key);
                        }
                    }
                }
                _ => {}
            }
        }
        keys.len()
    }

    /// Complexity for scoring: total_unlocked / total_unique
    pub fn complexity(&self) -> f64 {
        (self.total_unlocked_count() as f64 / self.total_unique_keys as f64).max(0.1)
    }

    /// Get all branch definitions with their current progress (for UI).
    #[allow(dead_code)]
    pub fn all_branches_with_progress(&self) -> Vec<(&'static BranchDefinition, &BranchProgress)> {
        ALL_BRANCHES
            .iter()
            .map(|def| (def, self.branch_progress(def.id)))
            .collect()
    }

    /// Number of unlocked keys in a branch.
    pub fn branch_unlocked_count(&self, id: BranchId) -> usize {
        let def = get_branch_definition(id);
        let bp = self.branch_progress(id);
        match bp.status {
            BranchStatus::Complete => def.levels.iter().map(|l| l.keys.len()).sum(),
            BranchStatus::InProgress => {
                if id == BranchId::Lowercase {
                    self.lowercase_unlocked_count()
                } else {
                    def.levels
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i <= bp.current_level)
                        .map(|(_, l)| l.keys.len())
                        .sum()
                }
            }
            _ => 0,
        }
    }

    /// Total keys defined in a branch (across all levels).
    pub fn branch_total_keys(id: BranchId) -> usize {
        let def = get_branch_definition(id);
        def.levels.iter().map(|l| l.keys.len()).sum()
    }

    /// Count of unique confident keys across all branches.
    pub fn total_confident_keys(&self, stats: &KeyStatsStore) -> usize {
        let mut keys: HashSet<char> = HashSet::new();
        for &ch in ALWAYS_UNLOCKED_KEYS {
            if stats.get_confidence(ch) >= 1.0 {
                keys.insert(ch);
            }
        }
        for branch_def in ALL_BRANCHES {
            for level in branch_def.levels {
                for &ch in level.keys {
                    if stats.get_confidence(ch) >= 1.0 {
                        keys.insert(ch);
                    }
                }
            }
        }
        keys.len()
    }

    /// Count of confident keys in a branch.
    pub fn branch_confident_keys(&self, id: BranchId, stats: &KeyStatsStore) -> usize {
        let def = get_branch_definition(id);
        def.levels
            .iter()
            .flat_map(|l| l.keys.iter())
            .filter(|&&ch| stats.get_confidence(ch) >= 1.0)
            .count()
    }
}

impl Default for SkillTree {
    fn default() -> Self {
        Self::new(SkillTreeProgress::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stats_confident(stats: &mut KeyStatsStore, keys: &[char]) {
        for &ch in keys {
            for _ in 0..50 {
                stats.update_key(ch, 200.0);
            }
        }
    }

    #[test]
    fn test_initial_state() {
        let tree = SkillTree::default();
        assert_eq!(
            *tree.branch_status(BranchId::Lowercase),
            BranchStatus::InProgress
        );
        assert_eq!(
            *tree.branch_status(BranchId::Capitals),
            BranchStatus::Locked
        );
        assert_eq!(*tree.branch_status(BranchId::Numbers), BranchStatus::Locked);
    }

    #[test]
    fn test_total_unique_keys() {
        let tree = SkillTree::default();
        assert_eq!(tree.total_unique_keys, 98);
    }

    #[test]
    fn test_initial_lowercase_unlocked() {
        let tree = SkillTree::default();
        let keys = tree.unlocked_keys(DrillScope::Global);
        assert_eq!(keys.len(), LOWERCASE_MIN_KEYS + ALWAYS_UNLOCKED_KEYS.len());
        assert_eq!(&keys[2..8], &['e', 't', 'a', 'o', 'i', 'n']);
        assert!(keys.contains(&SPACE));
        assert!(keys.contains(&BACKSPACE));
    }

    #[test]
    fn test_lowercase_progressive_unlock() {
        let mut tree = SkillTree::default();
        let mut stats = KeyStatsStore::default();

        // Make initial 6 keys confident
        make_stats_confident(&mut stats, &['e', 't', 'a', 'o', 'i', 'n']);
        tree.update(&stats, None);

        // Should unlock 7th key ('s')
        let keys = tree.unlocked_keys(DrillScope::Global);
        assert_eq!(keys.len(), 9);
        assert!(keys.contains(&'s'));
    }

    #[test]
    fn test_lowercase_completion_unlocks_branches() {
        let mut tree = SkillTree::default();
        let mut stats = KeyStatsStore::default();

        // Make all 26 lowercase keys confident
        let all_lowercase = get_branch_definition(BranchId::Lowercase).levels[0].keys;
        make_stats_confident(&mut stats, all_lowercase);

        // Need to repeatedly update as each unlock requires all current keys confident
        for _ in 0..30 {
            tree.update(&stats, None);
        }

        assert_eq!(
            *tree.branch_status(BranchId::Lowercase),
            BranchStatus::Complete
        );
        assert_eq!(
            *tree.branch_status(BranchId::Capitals),
            BranchStatus::Available
        );
        assert_eq!(
            *tree.branch_status(BranchId::Numbers),
            BranchStatus::Available
        );
        assert_eq!(
            *tree.branch_status(BranchId::ProsePunctuation),
            BranchStatus::Available
        );
        assert_eq!(
            *tree.branch_status(BranchId::Whitespace),
            BranchStatus::Available
        );
        assert_eq!(
            *tree.branch_status(BranchId::CodeSymbols),
            BranchStatus::Available
        );
    }

    #[test]
    fn test_start_branch() {
        let mut tree = SkillTree::default();
        // Force capitals to Available
        tree.branch_progress_mut(BranchId::Capitals).status = BranchStatus::Available;

        tree.start_branch(BranchId::Capitals);
        assert_eq!(
            *tree.branch_status(BranchId::Capitals),
            BranchStatus::InProgress
        );
        assert_eq!(tree.branch_progress(BranchId::Capitals).current_level, 0);
    }

    #[test]
    fn test_branch_level_advancement() {
        let mut tree = SkillTree::default();
        let mut stats = KeyStatsStore::default();

        // Set capitals to InProgress at level 0
        let bp = tree.branch_progress_mut(BranchId::Capitals);
        bp.status = BranchStatus::InProgress;
        bp.current_level = 0;

        // Make level 1 capitals confident: T I A S W H B M
        make_stats_confident(&mut stats, &['T', 'I', 'A', 'S', 'W', 'H', 'B', 'M']);
        tree.update(&stats, None);

        assert_eq!(tree.branch_progress(BranchId::Capitals).current_level, 1);
        assert_eq!(
            *tree.branch_status(BranchId::Capitals),
            BranchStatus::InProgress
        );
    }

    #[test]
    fn test_branch_completion() {
        let mut tree = SkillTree::default();
        let mut stats = KeyStatsStore::default();

        let bp = tree.branch_progress_mut(BranchId::Capitals);
        bp.status = BranchStatus::InProgress;
        bp.current_level = 0;

        // Make all capital letter levels confident
        let all_caps: Vec<char> = ('A'..='Z').collect();
        make_stats_confident(&mut stats, &all_caps);

        // Update multiple times for level advancement
        for _ in 0..5 {
            tree.update(&stats, None);
        }

        assert_eq!(
            *tree.branch_status(BranchId::Capitals),
            BranchStatus::Complete
        );
    }

    #[test]
    fn test_shared_key_confidence() {
        let _tree = SkillTree::default();
        let mut stats = KeyStatsStore::default();

        // '-' is shared between ProsePunctuation L2 and CodeSymbols L1
        // Master it once
        make_stats_confident(&mut stats, &['-']);

        // Both branches should see it as confident
        assert!(stats.get_confidence('-') >= 1.0);
    }

    #[test]
    fn test_focused_key_global() {
        let tree = SkillTree::default();
        let stats = KeyStatsStore::default();

        // All keys at 0 confidence, focused should be first in order
        let focused = tree.focused_key(DrillScope::Global, &stats);
        assert!(focused.is_some());
        // Should be one of the initial 6 lowercase keys
        assert!(
            ['e', 't', 'a', 'o', 'i', 'n'].contains(&focused.unwrap()),
            "focused: {:?}",
            focused
        );
    }

    #[test]
    fn test_focused_key_branch() {
        let mut tree = SkillTree::default();
        let stats = KeyStatsStore::default();

        let bp = tree.branch_progress_mut(BranchId::Capitals);
        bp.status = BranchStatus::InProgress;
        bp.current_level = 0;

        let focused = tree.focused_key(DrillScope::Branch(BranchId::Capitals), &stats);
        assert!(focused.is_some());
        // Should be one of level 1 capitals
        assert!(
            ['T', 'I', 'A', 'S', 'W', 'H', 'B', 'M'].contains(&focused.unwrap()),
            "focused: {:?}",
            focused
        );
    }

    #[test]
    fn test_complexity_scales() {
        let tree = SkillTree::default();
        let initial_complexity = tree.complexity();
        assert!(initial_complexity > 0.0);
        assert!(initial_complexity < 1.0);

        // Full unlock should give complexity ~1.0
        let mut full_tree = SkillTree::default();
        for id in BranchId::all() {
            let bp = full_tree.branch_progress_mut(*id);
            bp.status = BranchStatus::Complete;
        }
        let full_complexity = full_tree.complexity();
        assert!((full_complexity - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_branch_keys_for_drill() {
        let mut tree = SkillTree::default();

        // Set lowercase complete, capitals in progress at level 1
        tree.branch_progress_mut(BranchId::Lowercase).status = BranchStatus::Complete;
        let bp = tree.branch_progress_mut(BranchId::Capitals);
        bp.status = BranchStatus::InProgress;
        bp.current_level = 1;

        let keys = tree.unlocked_keys(DrillScope::Branch(BranchId::Capitals));
        // Should include all 26 lowercase + Capitals L1 (8) + Capitals L2 (10)
        assert!(keys.contains(&'e')); // lowercase background
        assert!(keys.contains(&'T')); // Capitals L1
        assert!(keys.contains(&'J')); // Capitals L2 (current level)
        assert!(!keys.contains(&'O')); // Capitals L3 (locked)
    }

    #[test]
    fn test_branch_unlocked_count() {
        let tree = SkillTree::default();
        // Lowercase starts InProgress with LOWERCASE_MIN_KEYS
        assert_eq!(
            tree.branch_unlocked_count(BranchId::Lowercase),
            LOWERCASE_MIN_KEYS
        );

        // Locked branches return 0
        assert_eq!(tree.branch_unlocked_count(BranchId::Capitals), 0);
        assert_eq!(tree.branch_unlocked_count(BranchId::Numbers), 0);

        // InProgress non-lowercase branch
        let mut tree2 = SkillTree::default();
        let bp = tree2.branch_progress_mut(BranchId::Capitals);
        bp.status = BranchStatus::InProgress;
        bp.current_level = 1;
        // Level 0 (8 keys) + Level 1 (10 keys)
        assert_eq!(tree2.branch_unlocked_count(BranchId::Capitals), 18);

        // Complete branch returns all keys
        let mut tree3 = SkillTree::default();
        tree3.branch_progress_mut(BranchId::Numbers).status = BranchStatus::Complete;
        assert_eq!(tree3.branch_unlocked_count(BranchId::Numbers), 10);
    }

    #[test]
    fn test_selectable_branches_bounds() {
        use crate::ui::components::skill_tree::selectable_branches;

        let branches = selectable_branches();
        assert!(!branches.is_empty());
        assert_eq!(branches[0], BranchId::Lowercase);

        let tree = SkillTree::default();
        // Accessing branch_progress for every selectable branch should not panic
        for &branch_id in &branches {
            let _ = tree.branch_progress(branch_id);
            let _ = SkillTree::branch_total_keys(branch_id);
            let _ = tree.branch_unlocked_count(branch_id);
        }

        // Selection at 0 and at max index should be valid
        assert!(0 < branches.len());
        assert!(branches.len() - 1 < branches.len());
    }

    #[test]
    fn test_update_returns_newly_unlocked() {
        let mut tree = SkillTree::default();
        let mut stats = KeyStatsStore::default();

        // Make initial 6 keys confident
        make_stats_confident(&mut stats, &['e', 't', 'a', 'o', 'i', 'n']);
        let result = tree.update(&stats, None);

        // Should unlock 7th key ('s')
        assert!(
            result.newly_unlocked.contains(&'s'),
            "newly_unlocked: {:?}",
            result.newly_unlocked
        );
    }

    #[test]
    fn test_update_returns_newly_mastered() {
        let mut tree = SkillTree::default();
        let mut stats = KeyStatsStore::default();

        // Snapshot before any key stats are added
        let before_stats = stats.clone();

        // Make first 5 keys confident
        make_stats_confident(&mut stats, &['e', 't', 'a', 'o', 'i']);
        let result = tree.update(&stats, Some(&before_stats));

        // The 5 keys that went from <1.0 to >=1.0 should be in newly_mastered
        for &ch in &['e', 't', 'a', 'o', 'i'] {
            assert!(
                result.newly_mastered.contains(&ch),
                "expected {} in newly_mastered: {:?}",
                ch,
                result.newly_mastered
            );
        }
    }

    #[test]
    fn test_find_key_branch_lowercase() {
        let result = find_key_branch('e');
        assert!(result.is_some());
        let (branch, level_name, pos) = result.unwrap();
        assert_eq!(branch.id, BranchId::Lowercase);
        assert_eq!(level_name, "Frequency Order");
        assert_eq!(pos, 1); // 'e' is first in the frequency order
    }

    #[test]
    fn test_find_key_branch_capitals() {
        let result = find_key_branch('T');
        assert!(result.is_some());
        let (branch, level_name, pos) = result.unwrap();
        assert_eq!(branch.id, BranchId::Capitals);
        assert_eq!(level_name, "Common Sentence Capitals");
        assert_eq!(pos, 1); // 'T' is first
    }

    #[test]
    fn test_find_key_branch_unknown() {
        assert!(find_key_branch('\x00').is_none());
    }
}
