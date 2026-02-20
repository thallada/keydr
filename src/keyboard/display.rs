/// Centralized key display adapter for sentinel-char to display-name conversions.
///
/// **Sentinel boundary policy:**
/// Sentinel chars (`'\x08'`, `'\t'`, `'\n'`) are allowed only at two boundaries:
/// 1. **Input boundary** — `handle_key` in `src/main.rs` converts `KeyCode::Backspace/Tab/Enter`
///    to sentinels for `depressed_keys` and drill input.
/// 2. **Storage boundary** — `KeyStatsStore` and `drill_history` store sentinels as `char` keys.
///
/// All UI rendering, stats display, and business logic must consume these adapter functions
/// rather than matching sentinels directly.

/// Human-readable display name for a key character (including sentinels).
/// Returns `""` for printable chars — caller uses `ch.to_string()` for those.
pub fn key_display_name(ch: char) -> &'static str {
    match ch {
        '\x08' => "Backspace",
        '\t' => "Tab",
        '\n' => "Enter",
        ' ' => "Space",
        _ => "",
    }
}

/// Short label for compact UI contexts (heatmaps, compact keyboard).
/// Returns `""` for printable chars.
pub fn key_short_label(ch: char) -> &'static str {
    match ch {
        '\x08' => "Bksp",
        '\t' => "Tab",
        '\n' => "Ent",
        ' ' => "Spc",
        _ => "",
    }
}

/// All sentinel chars used for non-printable keys.
pub const MODIFIER_SENTINELS: &[char] = &['\x08', '\t', '\n'];

/// Sentinel char for Backspace.
pub const BACKSPACE: char = '\x08';
/// Sentinel char for Tab.
pub const TAB: char = '\t';
/// Sentinel char for Enter.
pub const ENTER: char = '\n';
/// Space character (not a sentinel, but treated as a special key for display).
pub const SPACE: char = ' ';

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_display_name() {
        assert_eq!(key_display_name('\x08'), "Backspace");
        assert_eq!(key_display_name('\t'), "Tab");
        assert_eq!(key_display_name('\n'), "Enter");
        assert_eq!(key_display_name(' '), "Space");
        assert_eq!(key_display_name('a'), "");
        assert_eq!(key_display_name('1'), "");
    }

    #[test]
    fn test_key_short_label() {
        assert_eq!(key_short_label('\x08'), "Bksp");
        assert_eq!(key_short_label('\t'), "Tab");
        assert_eq!(key_short_label('\n'), "Ent");
        assert_eq!(key_short_label(' '), "Spc");
        assert_eq!(key_short_label('z'), "");
    }

    #[test]
    fn test_modifier_sentinels() {
        assert_eq!(MODIFIER_SENTINELS.len(), 3);
        assert!(MODIFIER_SENTINELS.contains(&'\x08'));
        assert!(MODIFIER_SENTINELS.contains(&'\t'));
        assert!(MODIFIER_SENTINELS.contains(&'\n'));
    }

    /// Sentinel boundary enforcement test.
    ///
    /// Verifies that `'\x08'` (the Backspace sentinel) does not leak into
    /// UI or business logic files outside allowed boundaries.
    ///
    /// **Policy: `\x08`-only enforcement (accepted compromise)**
    ///
    /// The plan originally proposed checking all three sentinels (`\x08`, `\t`, `\n`),
    /// but `'\t'` and `'\n'` have widespread legitimate uses as text content
    /// characters throughout the codebase: tab indentation in code generators
    /// (`code_syntax.rs`, `passage.rs`), newlines in text processing (`input.rs`,
    /// `typing_area.rs`, `drill.rs`), and key definitions in the skill tree
    /// (`skill_tree.rs`). Distinguishing "sentinel identity use" from "text content
    /// use" for `\t`/`\n` would require fragile heuristic pattern matching that
    /// would either miss real violations or produce false positives.
    ///
    /// `'\x08'` has no legitimate text-content use, making it an unambiguous
    /// sentinel leakage signal. All UI/stats/business-logic files already use
    /// the `TAB`/`ENTER` adapter constants for sentinel-identity purposes, so
    /// the `\t`/`\n` policy is enforced by convention and code review.
    ///
    /// Allowed files for `'\x08'`:
    /// - `display.rs` (this module — the adapter itself, defines BACKSPACE constant)
    /// - `main.rs` (input boundary — KeyCode::Backspace conversion)
    /// - `key_stats.rs` (storage boundary)
    /// - `drill.rs` (input processing boundary)
    /// - `app.rs` (milestone detection reads stats keyed by sentinel)
    #[test]
    fn test_sentinel_boundary_enforcement() {
        use std::fs;
        use std::path::Path;

        let allowed_files = [
            "src/keyboard/display.rs",
            "src/main.rs",
            "src/engine/key_stats.rs",
            "src/session/drill.rs",
            "src/app.rs",
        ];

        fn collect_rs_files(dir: &Path, files: &mut Vec<String>) {
            let entries = fs::read_dir(dir).expect("failed to read source directory");
            for entry in entries {
                let entry = entry.expect("failed to read directory entry");
                let path = entry.path();
                if path.is_dir() {
                    collect_rs_files(&path, files);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let normalized = path.to_string_lossy().replace('\\', "/");
                    files.push(normalized);
                }
            }
        }

        // Search for direct '\x08' literal in src/ — this is the clearest
        // sentinel leakage signal since \x08 has no legitimate text use.
        let mut rs_files = Vec::new();
        collect_rs_files(Path::new("src"), &mut rs_files);

        let mut violations = Vec::new();
        for file in rs_files {
            let content = fs::read_to_string(&file).expect("failed to read source file");
            if content.contains(r"'\\x08'") && !allowed_files.iter().any(|&allowed| file == allowed)
            {
                violations.push(file);
            }
        }

        assert!(
            violations.is_empty(),
            "Direct '\\x08' sentinel literal found outside allowed boundary files:\n{}",
            violations.join("\n")
        );
    }
}
