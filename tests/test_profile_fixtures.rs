use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

use chrono::Datelike;
use keydr::engine::scoring::level_from_score;
use keydr::engine::skill_tree::{ALL_BRANCHES, BranchId, BranchStatus, DrillScope, SkillTree};
use keydr::store::json_store::JsonStore;
use keydr::store::schema::ExportData;

const ALL_PROFILES: &[&str] = &[
    "01-brand-new.json",
    "02-early-lowercase.json",
    "03-mid-lowercase.json",
    "04-lowercase-complete.json",
    "05-multi-branch.json",
    "06-advanced.json",
    "07-fully-complete.json",
];

static GENERATE: Once = Once::new();

/// Ensure test-profiles/ exists by running the generator binary (once per test run).
fn ensure_profiles_generated() {
    GENERATE.call_once(|| {
        if Path::new("test-profiles/07-fully-complete.json").exists() {
            return;
        }
        let status = Command::new("cargo")
            .args(["run", "--bin", "generate_test_profiles"])
            .status()
            .expect("failed to run generate_test_profiles");
        assert!(
            status.success(),
            "generate_test_profiles exited with {status}"
        );
    });
}

fn load_profile(name: &str) -> ExportData {
    ensure_profiles_generated();
    let path = format!("test-profiles/{name}");
    let json = fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"))
}

/// Get all keys for levels in completed branches.
fn completed_branch_keys(data: &ExportData) -> HashSet<char> {
    let mut keys = HashSet::new();
    for branch_def in ALL_BRANCHES {
        let bp = data.profile.skill_tree.branches.get(branch_def.id.to_key());
        let is_complete = matches!(bp, Some(bp) if bp.status == BranchStatus::Complete);
        if is_complete {
            for level in branch_def.levels {
                for &key in level.keys {
                    keys.insert(key);
                }
            }
        }
    }
    keys
}

/// Get all unlocked keys via SkillTree engine.
fn unlocked_keys_set(data: &ExportData) -> HashSet<char> {
    let tree = SkillTree::new(data.profile.skill_tree.clone());
    tree.unlocked_keys(DrillScope::Global).into_iter().collect()
}

/// Collect keys that are in the current in-progress level (not yet completed).
fn in_progress_level_keys(data: &ExportData) -> HashSet<char> {
    let mut keys = HashSet::new();
    for branch_def in ALL_BRANCHES {
        let bp = match data.profile.skill_tree.branches.get(branch_def.id.to_key()) {
            Some(bp) => bp,
            None => continue,
        };
        if bp.status != BranchStatus::InProgress {
            continue;
        }
        if branch_def.id == BranchId::Lowercase {
            // Lowercase progressive unlock: keys at indices [completed_count..unlocked_count]
            // current_level = number of keys beyond initial 6
            let unlocked_count = 6 + bp.current_level;
            let all_keys = branch_def.levels[0].keys;
            // The "frontier" keys that were most recently unlocked and may be partial
            // For in-progress check, we consider keys that aren't necessarily all mastered yet
            // The last few unlocked keys are the current learning frontier
            if unlocked_count <= all_keys.len() {
                // Keys in the last unlocked batch (the ones most likely < 1.0)
                let frontier_start = unlocked_count.saturating_sub(4).max(6);
                for &k in &all_keys[frontier_start..unlocked_count] {
                    keys.insert(k);
                }
            }
        } else if bp.current_level < branch_def.levels.len() {
            for &k in branch_def.levels[bp.current_level].keys {
                keys.insert(k);
            }
        }
    }
    keys
}

/// Collect keys from ranked drills.
fn ranked_drill_keys(data: &ExportData) -> HashSet<char> {
    let mut keys = HashSet::new();
    for drill in &data.drill_history.drills {
        if drill.ranked {
            for kt in &drill.per_key_times {
                keys.insert(kt.key);
            }
        }
    }
    keys
}

// ── Per-profile structural validation ────────────────────────────────────

fn assert_profile_valid(name: &str) {
    let data = load_profile(name);

    // Invariant #3: total_drills == drills.len()
    assert_eq!(
        data.profile.total_drills as usize,
        data.drill_history.drills.len(),
        "{name}: total_drills mismatch"
    );

    // Invariant #1: all stats keys are subset of unlocked keys
    let unlocked = unlocked_keys_set(&data);
    for &key in data.key_stats.stats.stats.keys() {
        assert!(
            unlocked.contains(&key),
            "{name}: key '{key}' in key_stats is not in the unlocked set"
        );
    }

    // Invariant #1: all keys in stats have sample_count > 0
    for (&key, stat) in &data.key_stats.stats.stats {
        assert!(
            stat.sample_count > 0,
            "{name}: key '{key}' has sample_count 0"
        );
    }

    // Invariant #2 + #8: all keys in completed branches have confidence >= 1.0
    // and completed branches have stats for all their keys
    let completed_keys = completed_branch_keys(&data);
    for &key in &completed_keys {
        assert!(
            data.key_stats.stats.stats.contains_key(&key),
            "{name}: key '{key}' in completed branch has no stats entry"
        );
        let stat = &data.key_stats.stats.stats[&key];
        assert!(
            stat.confidence >= 1.0,
            "{name}: key '{key}' in completed branch has confidence {} < 1.0",
            stat.confidence
        );
    }

    // Invariant #9: timestamps are monotonically increasing
    for i in 1..data.drill_history.drills.len() {
        assert!(
            data.drill_history.drills[i].timestamp >= data.drill_history.drills[i - 1].timestamp,
            "{name}: drill timestamps not monotonic at index {i}"
        );
    }

    // Invariant #6: drill per_key_times only reference keys from the unlocked set
    for (i, drill) in data.drill_history.drills.iter().enumerate() {
        for kt in &drill.per_key_times {
            assert!(
                unlocked.contains(&kt.key),
                "{name}: drill {i} references key '{}' not in unlocked set",
                kt.key
            );
        }
    }
}

#[test]
fn profile_01_brand_new_valid() {
    assert_profile_valid("01-brand-new.json");
}

#[test]
fn profile_02_early_lowercase_valid() {
    assert_profile_valid("02-early-lowercase.json");
}

#[test]
fn profile_03_mid_lowercase_valid() {
    assert_profile_valid("03-mid-lowercase.json");
}

#[test]
fn profile_04_lowercase_complete_valid() {
    assert_profile_valid("04-lowercase-complete.json");
}

#[test]
fn profile_05_multi_branch_valid() {
    assert_profile_valid("05-multi-branch.json");
}

#[test]
fn profile_06_advanced_valid() {
    assert_profile_valid("06-advanced.json");
}

#[test]
fn profile_07_fully_complete_valid() {
    assert_profile_valid("07-fully-complete.json");
}

// ── Invariant #7: ranked stats presence/population ───────────────────────

#[test]
fn profile_01_has_empty_ranked_stats() {
    let data = load_profile("01-brand-new.json");
    assert!(
        data.ranked_key_stats.stats.stats.is_empty(),
        "01-brand-new.json: ranked_key_stats should be empty"
    );
    let ranked_count = data
        .drill_history
        .drills
        .iter()
        .filter(|d| d.ranked)
        .count();
    assert_eq!(
        ranked_count, 0,
        "01-brand-new.json: should have no ranked drills"
    );
}

#[test]
fn profiles_02_to_07_have_ranked_stats_and_ranked_drills() {
    for name in &ALL_PROFILES[1..] {
        let data = load_profile(name);
        assert!(
            !data.ranked_key_stats.stats.stats.is_empty(),
            "{name}: ranked_key_stats should not be empty"
        );
        let ranked_count = data
            .drill_history
            .drills
            .iter()
            .filter(|d| d.ranked)
            .count();
        assert!(
            ranked_count > 0,
            "{name}: expected at least one ranked drill to populate ranked stores"
        );
    }
}

// ── Invariant #7: ranked stats cover ranked drill keys ───────────────────

#[test]
fn ranked_stats_cover_ranked_drill_keys() {
    for name in &ALL_PROFILES[1..] {
        let data = load_profile(name);
        let drill_keys = ranked_drill_keys(&data);
        let ranked_stat_keys: HashSet<char> =
            data.ranked_key_stats.stats.stats.keys().copied().collect();

        for &key in &drill_keys {
            assert!(
                ranked_stat_keys.contains(&key),
                "{name}: key '{key}' appears in ranked drills but not in ranked_key_stats"
            );
        }
    }
}

// ── Invariant #2: in-progress keys have confidence < 1.0 ────────────────

#[test]
fn in_progress_keys_have_partial_confidence() {
    // Profiles 2, 3, 5, 6 have in-progress branches with partial keys
    for name in &[
        "02-early-lowercase.json",
        "03-mid-lowercase.json",
        "05-multi-branch.json",
        "06-advanced.json",
    ] {
        let data = load_profile(name);
        let ip_keys = in_progress_level_keys(&data);

        // At least some in-progress keys should have confidence < 1.0
        let partial_count = ip_keys
            .iter()
            .filter(|&&k| {
                data.key_stats
                    .stats
                    .stats
                    .get(&k)
                    .is_some_and(|s| s.confidence < 1.0)
            })
            .count();
        assert!(
            partial_count > 0,
            "{name}: expected some in-progress keys with confidence < 1.0, \
             but all {} in-progress keys are mastered",
            ip_keys.len()
        );
    }
}

// ── Invariant #4: synthetic score produces reasonable level ──────────────

#[test]
fn synthetic_score_level_in_expected_range() {
    let expected: &[(&str, u32, u32)] = &[
        ("01-brand-new.json", 1, 1),
        ("02-early-lowercase.json", 1, 3),
        ("03-mid-lowercase.json", 2, 4),
        ("04-lowercase-complete.json", 4, 6),
        ("05-multi-branch.json", 6, 8),
        ("06-advanced.json", 10, 14),
        ("07-fully-complete.json", 16, 20),
    ];

    for &(name, min_level, max_level) in expected {
        let data = load_profile(name);
        let level = level_from_score(data.profile.total_score);
        assert!(
            level >= min_level && level <= max_level,
            "{name}: level_from_score({}) = {level}, expected [{min_level}, {max_level}]",
            data.profile.total_score
        );
    }
}

// ── Invariant #5: streak/date consistency ────────────────────────────────

/// Compute trailing consecutive-day streak from drill timestamps.
fn compute_trailing_streak(data: &ExportData) -> u32 {
    let drills = &data.drill_history.drills;
    if drills.is_empty() {
        return 0;
    }

    // Collect unique drill dates (YYYY-MM-DD as ordinal days for easy comparison)
    let unique_dates: BTreeSet<i32> = drills
        .iter()
        .map(|d| d.timestamp.num_days_from_ce())
        .collect();

    let dates_vec: Vec<i32> = unique_dates.into_iter().collect();
    let last_date = *dates_vec.last().unwrap();

    // Count consecutive days backwards from the last date
    let mut streak = 1u32;
    for i in (0..dates_vec.len() - 1).rev() {
        if dates_vec[i] == last_date - streak as i32 {
            streak += 1;
        } else {
            break;
        }
    }
    streak
}

#[test]
fn streak_and_last_practice_date_consistent_with_history() {
    for name in ALL_PROFILES {
        let data = load_profile(name);
        let drills = &data.drill_history.drills;

        if drills.is_empty() {
            assert!(
                data.profile.last_practice_date.is_none(),
                "{name}: empty history should have no last_practice_date"
            );
            assert_eq!(
                data.profile.streak_days, 0,
                "{name}: empty history should have 0 streak"
            );
        } else {
            // last_practice_date should match the last drill's date
            let last_drill_date = drills
                .last()
                .unwrap()
                .timestamp
                .format("%Y-%m-%d")
                .to_string();
            assert_eq!(
                data.profile.last_practice_date.as_deref(),
                Some(last_drill_date.as_str()),
                "{name}: last_practice_date doesn't match last drill timestamp"
            );

            // streak_days should exactly equal trailing consecutive days from history
            let computed_streak = compute_trailing_streak(&data);
            assert_eq!(
                data.profile.streak_days, computed_streak,
                "{name}: streak_days ({}) doesn't match computed trailing streak ({computed_streak})",
                data.profile.streak_days
            );
        }
    }
}

// ── Profile-specific confidence bands ────────────────────────────────────

#[test]
fn profile_specific_confidence_bands() {
    // Profile 02: s,h,r,d should be partial (0.3-0.7); e,t,a,o,i,n should be mastered
    {
        let data = load_profile("02-early-lowercase.json");
        let stats = &data.key_stats.stats.stats;
        for &k in &['e', 't', 'a', 'o', 'i', 'n'] {
            let conf = stats[&k].confidence;
            assert!(conf >= 1.0, "02: key '{k}' should be mastered, got {conf}");
        }
        for &k in &['s', 'h', 'r', 'd'] {
            let conf = stats[&k].confidence;
            assert!(
                (0.2..1.0).contains(&conf),
                "02: key '{k}' should be partial (0.2-1.0), got {conf}"
            );
        }
    }

    // Profile 03: first 14 keys mastered, w,f,g,y partial (0.4-0.8)
    {
        let data = load_profile("03-mid-lowercase.json");
        let stats = &data.key_stats.stats.stats;
        let all_lc: Vec<char> = "etaoinshrdlcum".chars().collect();
        for &k in &all_lc {
            let conf = stats[&k].confidence;
            assert!(conf >= 1.0, "03: key '{k}' should be mastered, got {conf}");
        }
        for &k in &['w', 'f', 'g', 'y'] {
            let conf = stats[&k].confidence;
            assert!(
                (0.3..1.0).contains(&conf),
                "03: key '{k}' should be partial (0.3-1.0), got {conf}"
            );
        }
    }

    // Profile 05: capitals L2 partial (J,D,R,C,E), numbers partial (1,2,3),
    // punctuation partial (.,',')
    {
        let data = load_profile("05-multi-branch.json");
        let stats = &data.key_stats.stats.stats;
        for &k in &['J', 'D', 'R', 'C'] {
            let conf = stats[&k].confidence;
            assert!(
                (0.2..1.0).contains(&conf),
                "05: key '{k}' should be partial, got {conf}"
            );
        }
        for &k in &['1', '2', '3'] {
            let conf = stats[&k].confidence;
            assert!(
                (0.2..1.0).contains(&conf),
                "05: key '{k}' should be partial, got {conf}"
            );
        }
        for &k in &['.', ',', '\''] {
            let conf = stats[&k].confidence;
            assert!(
                (0.2..1.0).contains(&conf),
                "05: key '{k}' should be partial, got {conf}"
            );
        }
    }

    // Profile 06: code symbols L3 partial (&,|,^,~)
    {
        let data = load_profile("06-advanced.json");
        let stats = &data.key_stats.stats.stats;
        for &k in &['&', '|', '^', '~'] {
            let conf = stats[&k].confidence;
            assert!(
                (0.2..1.0).contains(&conf),
                "06: key '{k}' should be partial, got {conf}"
            );
        }
        // '!' is shared with completed ProsePunctuation, must be mastered
        let bang_conf = stats[&'!'].confidence;
        assert!(
            bang_conf >= 1.0,
            "06: key '!' should be mastered (shared with complete branch), got {bang_conf}"
        );
    }
}

// ── Import via JsonStore ─────────────────────────────────────────────────

#[test]
fn imports_all_profiles_into_temp_store() {
    for name in ALL_PROFILES {
        let data = load_profile(name);
        let tmp_dir = tempfile::tempdir().unwrap();
        let store =
            JsonStore::with_base_dir(PathBuf::from(tmp_dir.path())).expect("create temp store");

        store
            .import_all(&data)
            .unwrap_or_else(|e| panic!("{name}: import_all failed: {e}"));

        // Verify we can reload the imported data
        let profile = store.load_profile();
        assert!(profile.is_some(), "{name}: profile not found after import");
        let profile = profile.unwrap();
        assert_eq!(
            profile.total_drills, data.profile.total_drills,
            "{name}: imported profile total_drills mismatch"
        );

        let key_stats = store.load_key_stats();
        assert_eq!(
            key_stats.stats.stats.len(),
            data.key_stats.stats.stats.len(),
            "{name}: imported key_stats entry count mismatch"
        );

        let drill_history = store.load_drill_history();
        assert_eq!(
            drill_history.drills.len(),
            data.drill_history.drills.len(),
            "{name}: imported drill_history count mismatch"
        );
    }
}
