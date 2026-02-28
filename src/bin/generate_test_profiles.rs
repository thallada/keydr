use std::collections::HashMap;
use std::fs;

use chrono::{DateTime, TimeZone, Utc};

use keydr::config::Config;
use keydr::engine::key_stats::{KeyStat, KeyStatsStore};
use keydr::engine::skill_tree::{
    BranchId, BranchProgress, BranchStatus, SkillTreeProgress, ALL_BRANCHES,
};
use keydr::session::result::{DrillResult, KeyTime};
use keydr::store::schema::{
    DrillHistoryData, ExportData, KeyStatsData, ProfileData, EXPORT_VERSION,
};

const SCHEMA_VERSION: u32 = 2;
const TARGET_CPM: f64 = 175.0;

// ── Helpers ──────────────────────────────────────────────────────────────

/// Generate a KeyStat with deterministic values derived from target confidence.
fn make_key_stat(confidence: f64, sample_count: usize) -> KeyStat {
    let target_time_ms = 60000.0 / TARGET_CPM; // ~342.86 ms
    let filtered_time_ms = target_time_ms / confidence;
    let best_time_ms = filtered_time_ms * 0.85;

    // Generate recent_times: up to 30 entries near filtered_time_ms
    let recent_count = sample_count.min(30);
    let recent_times: Vec<f64> = (0..recent_count)
        .map(|i| filtered_time_ms + (i as f64 - recent_count as f64 / 2.0) * 2.0)
        .collect();

    // Error rate scales inversely with confidence
    let error_rate = if confidence >= 1.0 {
        0.02
    } else {
        0.1 + (1.0 - confidence) * 0.3
    };
    let error_count = (sample_count as f64 * error_rate * 0.5) as usize;
    let total_count = sample_count + error_count;

    KeyStat {
        filtered_time_ms,
        best_time_ms,
        confidence,
        sample_count,
        recent_times,
        error_count,
        total_count,
        error_rate_ema: error_rate,
    }
}

/// Generate monotonic timestamps: base_date + day_offset days + drill_offset * 2min.
fn drill_timestamp(base: DateTime<Utc>, day: u32, drill_in_day: u32) -> DateTime<Utc> {
    base + chrono::Duration::days(day as i64)
        + chrono::Duration::seconds(drill_in_day as i64 * 120)
}

/// Generate a DrillResult with deterministic per_key_times.
fn make_drill_result(
    wpm: f64,
    accuracy: f64,
    char_count: usize,
    keys: &[char],
    timestamp: DateTime<Utc>,
    mode: &str,
    ranked: bool,
) -> DrillResult {
    let cpm = wpm * 5.0;
    let incorrect = ((1.0 - accuracy / 100.0) * char_count as f64).round() as usize;
    let correct = char_count - incorrect;
    let elapsed_secs = char_count as f64 / (cpm / 60.0);

    // Generate per_key_times cycling through available keys
    let per_key_times: Vec<KeyTime> = (0..char_count)
        .map(|i| {
            let key = keys[i % keys.len()];
            let is_correct = i >= incorrect; // first N are incorrect, rest correct
            let time_ms = if is_correct {
                60000.0 / cpm + (i as f64 % 7.0) * 3.0
            } else {
                60000.0 / cpm + 150.0 + (i as f64 % 5.0) * 10.0
            };
            KeyTime {
                key,
                time_ms,
                correct: is_correct,
            }
        })
        .collect();

    DrillResult {
        wpm,
        cpm,
        accuracy,
        correct,
        incorrect,
        total_chars: char_count,
        elapsed_secs,
        timestamp,
        per_key_times,
        drill_mode: mode.to_string(),
        ranked,
        partial: false,
        completion_percent: 100.0,
    }
}

fn make_skill_tree_progress(branches: Vec<(BranchId, BranchStatus, usize)>) -> SkillTreeProgress {
    let mut map = HashMap::new();
    for (id, status, level) in branches {
        map.insert(
            id.to_key().to_string(),
            BranchProgress {
                status,
                current_level: level,
            },
        );
    }
    // Fill in any missing branches as Locked
    for id in BranchId::all() {
        map.entry(id.to_key().to_string())
            .or_insert(BranchProgress {
                status: BranchStatus::Locked,
                current_level: 0,
            });
    }
    SkillTreeProgress { branches: map }
}

/// Fixed exported_at timestamp for deterministic output.
fn fixed_export_timestamp() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap()
}

/// Canonical config with fixed paths for deterministic output across environments.
fn canonical_config() -> Config {
    Config {
        passage_download_dir: "/tmp/keydr/passages".to_string(),
        code_download_dir: "/tmp/keydr/code".to_string(),
        ..Config::default()
    }
}

fn make_export(
    profile: ProfileData,
    key_stats: KeyStatsStore,
    ranked_key_stats: KeyStatsStore,
    drill_history: Vec<DrillResult>,
) -> ExportData {
    ExportData {
        keydr_export_version: EXPORT_VERSION,
        exported_at: fixed_export_timestamp(),
        config: canonical_config(),
        profile,
        key_stats: KeyStatsData {
            schema_version: SCHEMA_VERSION,
            stats: key_stats,
        },
        ranked_key_stats: KeyStatsData {
            schema_version: SCHEMA_VERSION,
            stats: ranked_key_stats,
        },
        drill_history: DrillHistoryData {
            schema_version: SCHEMA_VERSION,
            drills: drill_history,
        },
    }
}

/// Get all keys for a branch up to (and including) level_index.
fn branch_keys_up_to(branch_id: BranchId, level_index: usize) -> Vec<char> {
    let def = ALL_BRANCHES
        .iter()
        .find(|b| b.id == branch_id)
        .expect("branch not found");
    let mut keys = Vec::new();
    for (i, level) in def.levels.iter().enumerate() {
        if i <= level_index {
            keys.extend_from_slice(level.keys);
        }
    }
    keys
}

/// Get all keys for all levels of a branch.
fn branch_all_keys(branch_id: BranchId) -> Vec<char> {
    let def = ALL_BRANCHES
        .iter()
        .find(|b| b.id == branch_id)
        .expect("branch not found");
    let mut keys = Vec::new();
    for level in def.levels {
        keys.extend_from_slice(level.keys);
    }
    keys
}

/// Lowercase keys: first `count` from frequency order.
fn lowercase_keys(count: usize) -> Vec<char> {
    let def = ALL_BRANCHES
        .iter()
        .find(|b| b.id == BranchId::Lowercase)
        .unwrap();
    def.levels[0].keys[..count].to_vec()
}

/// Base date for all profiles.
fn base_date() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 1, 1, 8, 0, 0).unwrap()
}

/// Generate drill history spread across `streak_days` days.
fn generate_drills(
    total: usize,
    streak_days: u32,
    keys: &[char],
    mode_distribution: &[(&str, bool, usize)], // (mode, ranked, count)
    base_wpm: f64,
) -> Vec<DrillResult> {
    let base = base_date();
    let mut drills = Vec::new();
    let mut drill_idx = 0usize;

    for &(mode, ranked, count) in mode_distribution {
        for i in 0..count {
            let day = if streak_days > 0 {
                (drill_idx as u32 * streak_days) / total as u32
            } else {
                0
            };
            let drill_in_day = drill_idx as u32 % 15; // max 15 drills per day spacing
            let ts = drill_timestamp(base, day, drill_in_day);

            // Vary WPM slightly by index
            let wpm = base_wpm + (i as f64 % 10.0) - 5.0;
            let accuracy = 92.0 + (i as f64 % 8.0);
            let char_count = 80 + (i % 40);

            drills.push(make_drill_result(wpm, accuracy, char_count, keys, ts, mode, ranked));
            drill_idx += 1;
        }
    }

    // Sort by timestamp to ensure monotonic ordering
    drills.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    drills
}

fn last_practice_date_from_drills(drills: &[DrillResult]) -> Option<String> {
    drills.last().map(|d| d.timestamp.format("%Y-%m-%d").to_string())
}

// ── Profile Builders ─────────────────────────────────────────────────────

fn build_profile_01() -> ExportData {
    let skill_tree = make_skill_tree_progress(vec![
        (BranchId::Lowercase, BranchStatus::InProgress, 0),
    ]);

    make_export(
        ProfileData {
            schema_version: SCHEMA_VERSION,
            skill_tree,
            total_score: 0.0,
            total_drills: 0,
            streak_days: 0,
            best_streak: 0,
            last_practice_date: None,
        },
        KeyStatsStore::default(),
        KeyStatsStore::default(),
        Vec::new(),
    )
}

fn build_profile_02() -> ExportData {
    // Lowercase InProgress level 4 => 6 + 4 = 10 keys: e,t,a,o,i,n,s,h,r,d
    let skill_tree = make_skill_tree_progress(vec![
        (BranchId::Lowercase, BranchStatus::InProgress, 4),
    ]);

    let all_keys = lowercase_keys(10);
    let mastered_keys = &all_keys[..6]; // e,t,a,o,i,n
    let partial_keys = &all_keys[6..];  // s,h,r,d

    let mut stats = KeyStatsStore::default();
    for &k in mastered_keys {
        stats.stats.insert(k, make_key_stat(1.2, 40));
    }
    let partial_confidences = [0.3, 0.5, 0.6, 0.7];
    for (i, &k) in partial_keys.iter().enumerate() {
        stats.stats.insert(k, make_key_stat(partial_confidences[i], 10 + i * 3));
    }

    let drills = generate_drills(
        15, 3, &all_keys,
        &[("adaptive", false, 15)],
        25.0,
    );

    // total_score: level_from_score(x) = (x/100).sqrt() => for level 2: score ~400
    make_export(
        ProfileData {
            schema_version: SCHEMA_VERSION,
            skill_tree,
            total_score: 350.0,
            total_drills: 15,
            streak_days: 3,
            best_streak: 3,
            last_practice_date: last_practice_date_from_drills(&drills),
        },
        stats,
        KeyStatsStore::default(),
        drills,
    )
}

fn build_profile_03() -> ExportData {
    // Lowercase InProgress level 12 => 6 + 12 = 18 keys through 'y'
    let skill_tree = make_skill_tree_progress(vec![
        (BranchId::Lowercase, BranchStatus::InProgress, 12),
    ]);

    let all_keys = lowercase_keys(18);
    let mastered_keys = &all_keys[..14];
    let partial_keys = &all_keys[14..]; // w,f,g,y

    let mut stats = KeyStatsStore::default();
    for &k in mastered_keys {
        stats.stats.insert(k, make_key_stat(1.3, 60));
    }
    let partial_confidences = [0.4, 0.6, 0.7, 0.8];
    for (i, &k) in partial_keys.iter().enumerate() {
        stats.stats.insert(k, make_key_stat(partial_confidences[i], 15 + i * 5));
    }

    let drills = generate_drills(
        50, 7, &all_keys,
        &[("adaptive", false, 50)],
        30.0,
    );

    // level ~3: score ~900
    make_export(
        ProfileData {
            schema_version: SCHEMA_VERSION,
            skill_tree,
            total_score: 900.0,
            total_drills: 50,
            streak_days: 7,
            best_streak: 7,
            last_practice_date: last_practice_date_from_drills(&drills),
        },
        stats,
        KeyStatsStore::default(),
        drills,
    )
}

fn build_profile_04() -> ExportData {
    // Lowercase Complete (level 20), all others Available
    let skill_tree = make_skill_tree_progress(vec![
        (BranchId::Lowercase, BranchStatus::Complete, 20),
        (BranchId::Capitals, BranchStatus::Available, 0),
        (BranchId::Numbers, BranchStatus::Available, 0),
        (BranchId::ProsePunctuation, BranchStatus::Available, 0),
        (BranchId::Whitespace, BranchStatus::Available, 0),
        (BranchId::CodeSymbols, BranchStatus::Available, 0),
    ]);

    let all_keys = lowercase_keys(26);

    let mut stats = KeyStatsStore::default();
    for &k in &all_keys {
        stats.stats.insert(k, make_key_stat(1.4, 80));
    }

    let drills = generate_drills(
        100, 14, &all_keys,
        &[("adaptive", false, 100)],
        35.0,
    );

    // level ~5: score ~2500
    make_export(
        ProfileData {
            schema_version: SCHEMA_VERSION,
            skill_tree,
            total_score: 2500.0,
            total_drills: 100,
            streak_days: 14,
            best_streak: 14,
            last_practice_date: last_practice_date_from_drills(&drills),
        },
        stats,
        KeyStatsStore::default(),
        drills,
    )
}

fn build_profile_05() -> ExportData {
    // Multiple branches in progress
    let skill_tree = make_skill_tree_progress(vec![
        (BranchId::Lowercase, BranchStatus::Complete, 20),
        (BranchId::Capitals, BranchStatus::InProgress, 1),
        (BranchId::Numbers, BranchStatus::InProgress, 0),
        (BranchId::ProsePunctuation, BranchStatus::InProgress, 0),
        (BranchId::Whitespace, BranchStatus::Available, 0),
        (BranchId::CodeSymbols, BranchStatus::Available, 0),
    ]);

    let mut stats = KeyStatsStore::default();

    // All lowercase mastered
    for &k in &lowercase_keys(26) {
        stats.stats.insert(k, make_key_stat(1.5, 100));
    }

    // Capitals L1 mastered: T,I,A,S,W,H,B,M
    for &k in &['T', 'I', 'A', 'S', 'W', 'H', 'B', 'M'] {
        stats.stats.insert(k, make_key_stat(1.2, 50));
    }
    // Capitals L2 partial: J,D,R,C,E
    let cap_partial = [('J', 0.4), ('D', 0.5), ('R', 0.6), ('C', 0.3), ('E', 0.7)];
    for &(k, conf) in &cap_partial {
        stats.stats.insert(k, make_key_stat(conf, 15));
    }

    // Numbers L1 partial: 1,2,3
    let num_partial = [('1', 0.4), ('2', 0.5), ('3', 0.3)];
    for &(k, conf) in &num_partial {
        stats.stats.insert(k, make_key_stat(conf, 12));
    }

    // Prose punctuation L1 partial: . , '
    let punct_partial = [('.', 0.5), (',', 0.4), ('\'', 0.3)];
    for &(k, conf) in &punct_partial {
        stats.stats.insert(k, make_key_stat(conf, 10));
    }

    // Build all unlocked keys for drill history
    let mut all_unlocked: Vec<char> = lowercase_keys(26);
    all_unlocked.extend(branch_keys_up_to(BranchId::Capitals, 1));
    all_unlocked.extend(branch_keys_up_to(BranchId::Numbers, 0));
    all_unlocked.extend(branch_keys_up_to(BranchId::ProsePunctuation, 0));

    let drills = generate_drills(
        200, 21, &all_unlocked,
        &[
            ("adaptive", false, 170),
            ("passage", false, 10),
            ("adaptive", true, 20),
        ],
        40.0,
    );

    // Ranked key stats: cover all keys used in ranked drills (all_unlocked)
    let mut ranked_stats = KeyStatsStore::default();
    for &k in &all_unlocked {
        ranked_stats.stats.insert(k, make_key_stat(1.1, 20));
    }

    // level ~7: score ~5000
    make_export(
        ProfileData {
            schema_version: SCHEMA_VERSION,
            skill_tree,
            total_score: 5000.0,
            total_drills: 200,
            streak_days: 21,
            best_streak: 21,
            last_practice_date: last_practice_date_from_drills(&drills),
        },
        stats,
        ranked_stats,
        drills,
    )
}

fn build_profile_06() -> ExportData {
    // Most branches complete, Code Symbols InProgress level 2
    let skill_tree = make_skill_tree_progress(vec![
        (BranchId::Lowercase, BranchStatus::Complete, 20),
        (BranchId::Capitals, BranchStatus::Complete, 3),
        (BranchId::Numbers, BranchStatus::Complete, 2),
        (BranchId::ProsePunctuation, BranchStatus::Complete, 3),
        (BranchId::Whitespace, BranchStatus::Complete, 2),
        (BranchId::CodeSymbols, BranchStatus::InProgress, 2),
    ]);

    let mut stats = KeyStatsStore::default();

    // All lowercase mastered
    for &k in &lowercase_keys(26) {
        stats.stats.insert(k, make_key_stat(1.6, 200));
    }
    // All capitals mastered
    for &k in &branch_all_keys(BranchId::Capitals) {
        stats.stats.insert(k, make_key_stat(1.4, 120));
    }
    // All numbers mastered
    for &k in &branch_all_keys(BranchId::Numbers) {
        stats.stats.insert(k, make_key_stat(1.3, 100));
    }
    // All prose punctuation mastered
    for &k in &branch_all_keys(BranchId::ProsePunctuation) {
        stats.stats.insert(k, make_key_stat(1.3, 90));
    }
    // All whitespace mastered
    for &k in &branch_all_keys(BranchId::Whitespace) {
        stats.stats.insert(k, make_key_stat(1.2, 80));
    }
    // Code Symbols L1 + L2 mastered
    for &k in &branch_keys_up_to(BranchId::CodeSymbols, 1) {
        stats.stats.insert(k, make_key_stat(1.2, 60));
    }
    // Code Symbols L3 partial: &,|,^,~
    // Note: '!' is shared with ProsePunctuation L3 (Complete), so it must be mastered
    let code_partial = [('&', 0.4), ('|', 0.5), ('^', 0.3), ('~', 0.4)];
    for &(k, conf) in &code_partial {
        stats.stats.insert(k, make_key_stat(conf, 15));
    }
    // '!' is mastered (shared with completed ProsePunctuation)
    stats.stats.insert('!', make_key_stat(1.2, 60));

    // All unlocked keys for drills
    let mut all_unlocked: Vec<char> = lowercase_keys(26);
    all_unlocked.extend(branch_all_keys(BranchId::Capitals));
    all_unlocked.extend(branch_all_keys(BranchId::Numbers));
    all_unlocked.extend(branch_all_keys(BranchId::ProsePunctuation));
    all_unlocked.extend(branch_all_keys(BranchId::Whitespace));
    all_unlocked.extend(branch_keys_up_to(BranchId::CodeSymbols, 2));

    let drills = generate_drills(
        500, 45, &all_unlocked,
        &[
            ("adaptive", false, 350),
            ("passage", false, 50),
            ("code", false, 50),
            ("adaptive", true, 50),
        ],
        50.0,
    );

    // Ranked key stats: cover all keys used in ranked drills (all_unlocked)
    let mut ranked_stats = KeyStatsStore::default();
    for &k in &all_unlocked {
        ranked_stats.stats.insert(k, make_key_stat(1.1, 30));
    }

    // level ~12: score ~15000
    make_export(
        ProfileData {
            schema_version: SCHEMA_VERSION,
            skill_tree,
            total_score: 15000.0,
            total_drills: 500,
            streak_days: 45,
            best_streak: 60,
            last_practice_date: last_practice_date_from_drills(&drills),
        },
        stats,
        ranked_stats,
        drills,
    )
}

fn build_profile_07() -> ExportData {
    // Everything complete
    let skill_tree = make_skill_tree_progress(vec![
        (BranchId::Lowercase, BranchStatus::Complete, 20),
        (BranchId::Capitals, BranchStatus::Complete, 3),
        (BranchId::Numbers, BranchStatus::Complete, 2),
        (BranchId::ProsePunctuation, BranchStatus::Complete, 3),
        (BranchId::Whitespace, BranchStatus::Complete, 2),
        (BranchId::CodeSymbols, BranchStatus::Complete, 4),
    ]);

    let mut stats = KeyStatsStore::default();

    // All keys mastered with high sample counts
    for &k in &lowercase_keys(26) {
        stats.stats.insert(k, make_key_stat(1.8, 400));
    }
    for &k in &branch_all_keys(BranchId::Capitals) {
        stats.stats.insert(k, make_key_stat(1.5, 200));
    }
    for &k in &branch_all_keys(BranchId::Numbers) {
        stats.stats.insert(k, make_key_stat(1.4, 180));
    }
    for &k in &branch_all_keys(BranchId::ProsePunctuation) {
        stats.stats.insert(k, make_key_stat(1.4, 160));
    }
    for &k in &branch_all_keys(BranchId::Whitespace) {
        stats.stats.insert(k, make_key_stat(1.3, 140));
    }
    for &k in &branch_all_keys(BranchId::CodeSymbols) {
        stats.stats.insert(k, make_key_stat(1.3, 120));
    }

    // All keys for drills
    let mut all_keys: Vec<char> = lowercase_keys(26);
    all_keys.extend(branch_all_keys(BranchId::Capitals));
    all_keys.extend(branch_all_keys(BranchId::Numbers));
    all_keys.extend(branch_all_keys(BranchId::ProsePunctuation));
    all_keys.extend(branch_all_keys(BranchId::Whitespace));
    all_keys.extend(branch_all_keys(BranchId::CodeSymbols));

    let drills = generate_drills(
        800, 90, &all_keys,
        &[
            ("adaptive", false, 400),
            ("passage", false, 150),
            ("code", false, 150),
            ("adaptive", true, 100),
        ],
        60.0,
    );

    // Full ranked stats
    let mut ranked_stats = KeyStatsStore::default();
    for &k in &all_keys {
        ranked_stats.stats.insert(k, make_key_stat(1.4, 80));
    }

    // level ~18: score ~35000
    make_export(
        ProfileData {
            schema_version: SCHEMA_VERSION,
            skill_tree,
            total_score: 35000.0,
            total_drills: 800,
            streak_days: 90,
            best_streak: 90,
            last_practice_date: last_practice_date_from_drills(&drills),
        },
        stats,
        ranked_stats,
        drills,
    )
}

// ── Main ─────────────────────────────────────────────────────────────────

fn main() {
    fs::create_dir_all("test-profiles").unwrap();

    let profiles: Vec<(&str, ExportData)> = vec![
        ("01-brand-new", build_profile_01()),
        ("02-early-lowercase", build_profile_02()),
        ("03-mid-lowercase", build_profile_03()),
        ("04-lowercase-complete", build_profile_04()),
        ("05-multi-branch", build_profile_05()),
        ("06-advanced", build_profile_06()),
        ("07-fully-complete", build_profile_07()),
    ];

    for (name, data) in &profiles {
        let json = serde_json::to_string_pretty(data).unwrap();
        let path = format!("test-profiles/{name}.json");
        fs::write(&path, &json).unwrap();
        println!("Wrote {path} ({} bytes)", json.len());
    }

    println!("\nGenerated {} test profiles.", profiles.len());
}
