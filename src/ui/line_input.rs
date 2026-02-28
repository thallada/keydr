use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Which settings path field is being edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathField {
    CodeDownloadDir,
    PassageDownloadDir,
    ExportPath,
    ImportPath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputResult {
    Continue,
    Submit,
    Cancel,
}

pub struct LineInput {
    text: String,
    /// Cursor position as a char index (0 = before first char).
    cursor: usize,
    completions: Vec<String>,
    completion_index: Option<usize>,
    /// Text snapshot when Tab was first pressed.
    completion_seed: String,
    /// True if last read_dir call failed.
    pub completion_error: bool,
}

impl LineInput {
    pub fn new(text: &str) -> Self {
        let cursor = text.chars().count();
        Self {
            text: text.to_string(),
            cursor,
            completions: Vec::new(),
            completion_index: None,
            completion_seed: String::new(),
            completion_error: false,
        }
    }

    pub fn value(&self) -> &str {
        &self.text
    }

    /// Returns (before_cursor, cursor_char, after_cursor) for styled rendering.
    /// When cursor is at end of text, cursor_char is None.
    pub fn render_parts(&self) -> (&str, Option<char>, &str) {
        let byte_offset = self.char_to_byte(self.cursor);
        if self.cursor >= self.text.chars().count() {
            (&self.text, None, "")
        } else {
            let ch = self.text[byte_offset..].chars().next().unwrap();
            let next_byte = byte_offset + ch.len_utf8();
            (&self.text[..byte_offset], Some(ch), &self.text[next_byte..])
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Esc => return InputResult::Cancel,
            KeyCode::Enter => return InputResult::Submit,

            KeyCode::Left => {
                self.reset_completion();
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            KeyCode::Right => {
                self.reset_completion();
                let len = self.text.chars().count();
                if self.cursor < len {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => {
                self.reset_completion();
                self.cursor = 0;
            }
            KeyCode::End => {
                self.reset_completion();
                self.cursor = self.text.chars().count();
            }
            KeyCode::Backspace => {
                self.reset_completion();
                if self.cursor > 0 {
                    let byte_offset = self.char_to_byte(self.cursor - 1);
                    let ch = self.text[byte_offset..].chars().next().unwrap();
                    self.text
                        .replace_range(byte_offset..byte_offset + ch.len_utf8(), "");
                    self.cursor -= 1;
                }
            }
            KeyCode::Delete => {
                self.reset_completion();
                let len = self.text.chars().count();
                if self.cursor < len {
                    let byte_offset = self.char_to_byte(self.cursor);
                    let ch = self.text[byte_offset..].chars().next().unwrap();
                    self.text
                        .replace_range(byte_offset..byte_offset + ch.len_utf8(), "");
                }
            }
            KeyCode::Tab => {
                self.tab_complete(true);
            }
            KeyCode::BackTab => {
                self.tab_complete(false);
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.reset_completion();
                self.cursor = 0;
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.reset_completion();
                self.cursor = self.text.chars().count();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.reset_completion();
                self.text.clear();
                self.cursor = 0;
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.reset_completion();
                self.delete_word_back();
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.reset_completion();
                let byte_offset = self.char_to_byte(self.cursor);
                self.text.insert(byte_offset, ch);
                self.cursor += 1;
            }
            _ => {}
        }
        InputResult::Continue
    }

    /// Convert char index to byte offset.
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }

    /// Delete word before cursor (unix-word-rubout: skip whitespace, then non-whitespace).
    fn delete_word_back(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let chars: Vec<char> = self.text.chars().collect();
        let mut pos = self.cursor;

        // Skip trailing whitespace
        while pos > 0 && chars[pos - 1].is_whitespace() {
            pos -= 1;
        }
        // Skip non-whitespace
        while pos > 0 && !chars[pos - 1].is_whitespace() {
            pos -= 1;
        }

        let start_byte = self.char_to_byte(pos);
        let end_byte = self.char_to_byte(self.cursor);
        self.text.replace_range(start_byte..end_byte, "");
        self.cursor = pos;
    }

    fn reset_completion(&mut self) {
        self.completions.clear();
        self.completion_index = None;
        self.completion_seed.clear();
        self.completion_error = false;
    }

    fn tab_complete(&mut self, forward: bool) {
        // Only activate when cursor is at end of line
        let len = self.text.chars().count();
        if self.cursor < len {
            return;
        }

        if self.completion_index.is_none() {
            // First tab press: build completions
            self.completion_seed = self.text.clone();
            self.completion_error = false;
            self.completions = self.build_completions();
            if self.completions.is_empty() {
                return;
            }
            self.completion_index = Some(0);
            self.apply_completion(0);
        } else if !self.completions.is_empty() {
            // Cycle
            let idx = self.completion_index.unwrap();
            let count = self.completions.len();
            let next = if forward {
                (idx + 1) % count
            } else {
                (idx + count - 1) % count
            };
            self.completion_index = Some(next);
            self.apply_completion(next);
        }
    }

    fn apply_completion(&mut self, idx: usize) {
        self.text = self.completions[idx].clone();
        self.cursor = self.text.chars().count();
    }

    fn build_completions(&mut self) -> Vec<String> {
        let seed = self.completion_seed.clone();

        // Split seed into (dir_part, partial_filename) by last path separator.
        // Accept both '/' and '\\' so user-typed alternate separators work on any platform.
        let last_sep_pos = seed.rfind('/').into_iter().chain(seed.rfind('\\')).max();
        let (dir_str, partial) = if let Some(pos) = last_sep_pos {
            (&seed[..=pos], &seed[pos + 1..])
        } else {
            ("", seed.as_str())
        };

        // Expand ~ for read_dir, but keep ~ in output
        let expanded_dir = if dir_str.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                let home_str = home.to_string_lossy().to_string();
                format!("{}{}", home_str, &dir_str[1..])
            } else {
                dir_str.to_string()
            }
        } else if dir_str.is_empty() {
            ".".to_string()
        } else {
            dir_str.to_string()
        };

        let read_result = std::fs::read_dir(&expanded_dir);
        let entries = match read_result {
            Ok(rd) => rd,
            Err(_) => {
                self.completion_error = true;
                return Vec::new();
            }
        };

        let entry_iter = entries.map(|result| {
            result.map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                (name, is_dir)
            })
        });

        self.collect_completions(entry_iter, dir_str, partial)
    }

    /// Scan an iterator of (name, is_dir) results, filter/sort, and return completions.
    /// Extracted so tests can inject synthetic iterators with errors.
    fn collect_completions(
        &mut self,
        entries: impl Iterator<Item = std::io::Result<(String, bool)>>,
        dir_str: &str,
        partial: &str,
    ) -> Vec<String> {
        let sep = std::path::MAIN_SEPARATOR;
        let include_hidden = partial.starts_with('.');

        let mut candidates: Vec<(bool, String)> = Vec::new(); // (is_dir, full_text)
        let mut scanned = 0usize;
        for entry_result in entries {
            if scanned >= 1000 {
                break;
            }
            scanned += 1;

            let (name_str, is_dir) = match entry_result {
                Ok(pair) => pair,
                Err(_) => {
                    self.completion_error = true;
                    return Vec::new();
                }
            };

            // Skip hidden files unless partial starts with '.'
            if !include_hidden && name_str.starts_with('.') {
                continue;
            }

            // Filter by prefix
            if !name_str.starts_with(partial) {
                continue;
            }

            let full = if is_dir {
                format!("{}{}{}", dir_str, name_str, sep)
            } else {
                format!("{}{}", dir_str, name_str)
            };

            candidates.push((is_dir, full));
        }

        // Sort: directories first, then files, alphabetical within each group
        candidates.sort_by(|a, b| {
            b.0.cmp(&a.0) // true (dir) before false (file)
                .then_with(|| a.1.cmp(&b.1))
        });

        // Cap at 100
        candidates.truncate(100);

        candidates.into_iter().map(|(_, path)| path).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL)
    }

    #[test]
    fn insert_at_start_middle_end() {
        let mut input = LineInput::new("ac");
        // Cursor at end (2), insert 'd' -> "acd"
        input.handle(key(KeyCode::Char('d')));
        assert_eq!(input.value(), "acd");

        // Move to start, insert 'z' -> "zacd"
        input.handle(key(KeyCode::Home));
        input.handle(key(KeyCode::Char('z')));
        assert_eq!(input.value(), "zacd");
        assert_eq!(input.cursor, 1);

        // Move right once (past 'a'), insert 'b' -> "zabcd"
        input.handle(key(KeyCode::Right));
        input.handle(key(KeyCode::Char('b')));
        assert_eq!(input.value(), "zabcd");
        assert_eq!(input.cursor, 3);
    }

    #[test]
    fn backspace_at_boundaries() {
        let mut input = LineInput::new("ab");
        // Backspace at end -> "a"
        input.handle(key(KeyCode::Backspace));
        assert_eq!(input.value(), "a");

        // Backspace again -> ""
        input.handle(key(KeyCode::Backspace));
        assert_eq!(input.value(), "");

        // Backspace on empty -> no panic
        input.handle(key(KeyCode::Backspace));
        assert_eq!(input.value(), "");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn delete_at_boundaries() {
        let mut input = LineInput::new("ab");
        // Move to start, delete -> "b"
        input.handle(key(KeyCode::Home));
        input.handle(key(KeyCode::Delete));
        assert_eq!(input.value(), "b");
        assert_eq!(input.cursor, 0);

        // Delete at end -> no change
        input.handle(key(KeyCode::End));
        input.handle(key(KeyCode::Delete));
        assert_eq!(input.value(), "b");

        // Empty string delete -> no panic
        let mut empty = LineInput::new("");
        empty.handle(key(KeyCode::Delete));
        assert_eq!(empty.value(), "");
    }

    #[test]
    fn ctrl_w_word_delete() {
        // "foo bar  " -> "foo "
        let mut input = LineInput::new("foo bar  ");
        input.handle(ctrl('w'));
        assert_eq!(input.value(), "foo ");

        // "  foo" cursor at end -> "  "
        let mut input2 = LineInput::new("  foo");
        input2.handle(ctrl('w'));
        assert_eq!(input2.value(), "  ");

        // empty -> empty
        let mut input3 = LineInput::new("");
        input3.handle(ctrl('w'));
        assert_eq!(input3.value(), "");
    }

    #[test]
    fn cursor_left_at_zero_stays() {
        let mut input = LineInput::new("a");
        input.handle(key(KeyCode::Home));
        assert_eq!(input.cursor, 0);
        input.handle(key(KeyCode::Left));
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn cursor_right_at_end_stays() {
        let mut input = LineInput::new("a");
        assert_eq!(input.cursor, 1);
        input.handle(key(KeyCode::Right));
        assert_eq!(input.cursor, 1);
    }

    #[test]
    fn home_end_position() {
        let mut input = LineInput::new("hello");
        input.handle(key(KeyCode::Home));
        assert_eq!(input.cursor, 0);
        input.handle(key(KeyCode::End));
        assert_eq!(input.cursor, 5);
    }

    #[test]
    fn ctrl_a_and_ctrl_e() {
        let mut input = LineInput::new("test");
        input.handle(ctrl('a'));
        assert_eq!(input.cursor, 0);
        input.handle(ctrl('e'));
        assert_eq!(input.cursor, 4);
    }

    #[test]
    fn ctrl_u_clears() {
        let mut input = LineInput::new("hello world");
        input.handle(ctrl('u'));
        assert_eq!(input.value(), "");
        assert_eq!(input.cursor, 0);
    }

    #[test]
    fn tab_at_midline_is_noop() {
        let mut input = LineInput::new("hello");
        input.handle(key(KeyCode::Home));
        input.handle(key(KeyCode::Right)); // cursor at 1
        let result = input.handle(key(KeyCode::Tab));
        assert_eq!(result, InputResult::Continue);
        assert_eq!(input.value(), "hello");
        assert_eq!(input.cursor, 1);
    }

    #[test]
    fn tab_no_match_sets_error_state() {
        let mut input = LineInput::new("/nonexistent_path_zzzzz/");
        let result = input.handle(key(KeyCode::Tab));
        assert_eq!(result, InputResult::Continue);
        assert!(input.completions.is_empty());
        assert!(input.completion_index.is_none());
        assert!(input.completion_error);
    }

    #[test]
    fn non_tab_key_resets_completion() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("aaa.txt"), "").unwrap();
        let path = format!("{}/", dir.path().display());
        let mut input = LineInput::new(&path);

        // Tab to start completion
        input.handle(key(KeyCode::Tab));
        assert!(!input.completions.is_empty());
        assert!(input.completion_index.is_some());

        // Any non-tab key resets
        input.handle(key(KeyCode::Char('x')));
        assert!(input.completions.is_empty());
        assert!(input.completion_index.is_none());
        assert!(input.value().ends_with('x'));
    }

    #[test]
    fn render_parts_at_start() {
        let input = LineInput::new("abc");
        let mut input = input;
        input.cursor = 0;
        let (before, ch, after) = input.render_parts();
        assert_eq!(before, "");
        assert_eq!(ch, Some('a'));
        assert_eq!(after, "bc");
    }

    #[test]
    fn render_parts_at_middle() {
        let mut input = LineInput::new("abc");
        input.cursor = 1;
        let (before, ch, after) = input.render_parts();
        assert_eq!(before, "a");
        assert_eq!(ch, Some('b'));
        assert_eq!(after, "c");
    }

    #[test]
    fn render_parts_at_end() {
        let input = LineInput::new("abc");
        let (before, ch, after) = input.render_parts();
        assert_eq!(before, "abc");
        assert_eq!(ch, None);
        assert_eq!(after, "");
    }

    #[test]
    fn submit_and_cancel() {
        let mut input = LineInput::new("test");
        assert_eq!(input.handle(key(KeyCode::Enter)), InputResult::Submit);

        let mut input2 = LineInput::new("test");
        assert_eq!(input2.handle(key(KeyCode::Esc)), InputResult::Cancel);
    }

    #[test]
    fn tab_completion_cycles_and_backtab_reverses() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("alpha.txt"), "").unwrap();
        std::fs::write(dir.path().join("beta.txt"), "").unwrap();
        std::fs::create_dir(dir.path().join("gamma_dir")).unwrap();
        let path = format!("{}/", dir.path().display());

        let mut input = LineInput::new(&path);
        input.handle(key(KeyCode::Tab));

        // Should have 3 completions: gamma_dir/ first (dirs first), then alpha.txt, beta.txt
        assert_eq!(input.completions.len(), 3);
        assert!(input.completions[0].ends_with("gamma_dir/"));
        assert!(input.completions[1].ends_with("alpha.txt"));
        assert!(input.completions[2].ends_with("beta.txt"));

        // First tab selects gamma_dir/
        assert!(input.value().ends_with("gamma_dir/"));

        // Second tab cycles to alpha.txt
        input.handle(key(KeyCode::Tab));
        assert!(input.value().ends_with("alpha.txt"));

        // Third tab cycles to beta.txt
        input.handle(key(KeyCode::Tab));
        assert!(input.value().ends_with("beta.txt"));

        // Fourth tab wraps to gamma_dir/
        input.handle(key(KeyCode::Tab));
        assert!(input.value().ends_with("gamma_dir/"));

        // BackTab reverses to beta.txt
        input.handle(key(KeyCode::BackTab));
        assert!(input.value().ends_with("beta.txt"));
    }

    #[test]
    fn completion_error_on_bad_dir() {
        let mut input = LineInput::new("/nonexistent_zzz_dir/");
        input.handle(key(KeyCode::Tab));
        assert!(input.completion_error);
        assert!(input.completions.is_empty());
        assert!(input.completion_index.is_none());

        // Any key clears the error
        input.handle(key(KeyCode::Char('x')));
        assert!(!input.completion_error);
    }

    #[test]
    fn completion_hidden_file_filtering() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".hidden"), "").unwrap();
        std::fs::write(dir.path().join("visible"), "").unwrap();

        // Without dot prefix: hidden files excluded
        let path = format!("{}/", dir.path().display());
        let mut input = LineInput::new(&path);
        input.handle(key(KeyCode::Tab));
        assert_eq!(input.completions.len(), 1);
        assert!(input.completions[0].ends_with("visible"));

        // With dot prefix: hidden files included
        let path_dot = format!("{}/.h", dir.path().display());
        let mut input2 = LineInput::new(&path_dot);
        input2.handle(key(KeyCode::Tab));
        assert_eq!(input2.completions.len(), 1);
        assert!(input2.completions[0].ends_with(".hidden"));
    }

    #[test]
    fn completion_prefix_filtering() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("foo_bar"), "").unwrap();
        std::fs::write(dir.path().join("foo_baz"), "").unwrap();
        std::fs::write(dir.path().join("other"), "").unwrap();

        let path = format!("{}/foo_", dir.path().display());
        let mut input = LineInput::new(&path);
        input.handle(key(KeyCode::Tab));
        assert_eq!(input.completions.len(), 2);
        assert!(input.completions[0].ends_with("foo_bar"));
        assert!(input.completions[1].ends_with("foo_baz"));
    }

    #[test]
    fn completion_directories_get_trailing_separator() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        std::fs::write(dir.path().join("file.txt"), "").unwrap();

        let path = format!("{}/", dir.path().display());
        let mut input = LineInput::new(&path);
        input.handle(key(KeyCode::Tab));

        // First completion is the directory (sorted first)
        let sep = std::path::MAIN_SEPARATOR;
        assert!(input.completions[0].ends_with(&format!("subdir{sep}")));
        // File does not have trailing separator
        assert!(input.completions[1].ends_with("file.txt"));
        assert!(!input.completions[1].ends_with(&sep.to_string()));
    }

    #[test]
    fn collect_completions_entry_error_sets_error_and_returns_empty() {
        let mut input = LineInput::new("");
        let entries: Vec<std::io::Result<(String, bool)>> = vec![
            Ok(("alpha.txt".to_string(), false)),
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "mock",
            )),
            Ok(("beta.txt".to_string(), false)),
        ];

        let result = input.collect_completions(entries.into_iter(), "/some/dir/", "");
        assert!(result.is_empty());
        assert!(input.completion_error);
    }

    #[test]
    fn collect_completions_ok_entries_no_error() {
        let mut input = LineInput::new("");
        let entries: Vec<std::io::Result<(String, bool)>> = vec![
            Ok(("zeta".to_string(), false)),
            Ok(("alpha_dir".to_string(), true)),
            Ok(("beta".to_string(), false)),
        ];

        let result = input.collect_completions(entries.into_iter(), "pfx/", "");
        assert!(!input.completion_error);
        // dirs first, then files, alphabetical within each group
        assert_eq!(result.len(), 3);
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(result[0], format!("pfx/alpha_dir{sep}"));
        assert_eq!(result[1], "pfx/beta");
        assert_eq!(result[2], "pfx/zeta");
    }

    #[test]
    fn collect_completions_scan_budget_caps_at_1000() {
        let mut input = LineInput::new("");
        // Create 1200 entries; only first 1000 should be scanned
        let entries: Vec<std::io::Result<(String, bool)>> = (0..1200)
            .map(|i| Ok((format!("file_{i:04}"), false)))
            .collect();

        let result = input.collect_completions(entries.into_iter(), "", "");
        // Should have at most 100 (candidate cap) from the first 1000 scanned
        assert!(result.len() <= 100);
    }

    #[test]
    fn collect_completions_candidate_cap_at_100() {
        let mut input = LineInput::new("");
        // Create 200 matching entries
        let entries: Vec<std::io::Result<(String, bool)>> = (0..200)
            .map(|i| Ok((format!("item_{i:03}"), false)))
            .collect();

        let result = input.collect_completions(entries.into_iter(), "", "");
        assert_eq!(result.len(), 100);
    }

    #[test]
    fn completion_error_clears_on_non_tab_key() {
        // Trigger a completion error
        let mut input = LineInput::new("/nonexistent_zzz_dir/");
        input.handle(key(KeyCode::Tab));
        assert!(input.completion_error);

        // Non-tab key clears it
        input.handle(key(KeyCode::Left));
        assert!(!input.completion_error);
    }
}
