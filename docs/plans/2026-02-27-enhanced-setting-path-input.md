# Plan: Enhanced Path Input with Cursor Navigation and Tab Completion

## Context

Settings page path fields (code download dir, passage download dir, export path, import path) currently only support appending characters and backspace — no cursor movement, no arrow keys, no tab completion. Users can't easily correct a typo in the middle of a path or navigate to an existing file for import.

## Approach: Custom `LineInput` struct + filesystem tab completion

Neither `tui-input` nor `tui-textarea` provide tab/path completion, and both have crossterm version mismatches with our deps (ratatui 0.30 + crossterm 0.28). A custom struct avoids dependency churn and gives us exactly the features we need.

## New file: `src/ui/line_input.rs`

### Struct

```rust
/// Which settings path field is being edited.
pub enum PathField {
    CodeDownloadDir,
    PassageDownloadDir,
    ExportPath,
    ImportPath,
}

pub enum InputResult {
    Continue,
    Submit,
    Cancel,
}

pub struct LineInput {
    text: String,
    cursor: usize,                // char index (NOT byte offset)
    completions: Vec<String>,
    completion_index: Option<usize>,
    completion_seed: String,      // text snapshot when Tab first pressed
    completion_error: bool,       // true if last read_dir failed
}
```

Cursor is stored as a **char index** (0 = before first char, `text.chars().count()` = after last char). Conversion to byte offset happens only at mutation boundaries via `text.char_indices()`.

### Keyboard handling

| Key | Action | Resets completion? |
|-----|--------|--------------------|
| Left | Move cursor one char left | Yes |
| Right | Move cursor one char right | Yes |
| Home / Ctrl+A | Move cursor to start | Yes |
| End / Ctrl+E | Move cursor to end | Yes |
| Backspace | Delete char before cursor | Yes |
| Delete | Delete char at cursor | Yes |
| Ctrl+U | Clear entire line | Yes |
| Ctrl+W | Delete word before cursor (see semantics below) | Yes |
| Tab | Cycle completions forward (end-of-line only) | No |
| BackTab | Cycle completions backward | No |
| Printable char | Insert at cursor | Yes |
| Esc | Return `InputResult::Cancel` | — |
| Enter | Return `InputResult::Submit` | — |

**Only Tab/BackTab preserve** the completion session. All other keys reset it.

**Ctrl+W semantics**: From cursor position, first skip any consecutive whitespace to the left, then delete the contiguous non-whitespace run. This matches standard readline/bash `unix-word-rubout` behavior. Example: `"foo bar  |"` → Ctrl+W → `"foo |"`.

### Tab completion

Tab completion **only activates when cursor is at end-of-line**. If cursor is in the middle, Tab is a no-op.

1. **First Tab** (`completion_index` is `None`): Snapshot `text` as `completion_seed`. Split into (directory, partial_filename) using the last path separator. Expand leading `~` to `dirs::home_dir()` for the `read_dir` call only — preserve `~` in output text. Call `std::fs::read_dir` with a **scan budget of 1000 entries** (iterate at most 1000 `DirEntry` results). From the scanned entries, filter those whose name starts with `partial_filename` (always case-sensitive — this is an intentional simplification; case-insensitive matching on macOS HFS+/Windows NTFS is not in scope). Hidden files (names starting with `.`) only included when `partial_filename` starts with `.`. Sort matching candidates: **directories first, then files, alphabetical within each group**. Cap the final candidate list at 100. On any `read_dir` or entry I/O error, produce zero completions and set `completion_error = true` (renders `"(cannot read directory)"` in footer).
2. **Cycling**: Increment/decrement `completion_index`, wrapping. Replace `text` with selected completion. Directories get a trailing `std::path::MAIN_SEPARATOR`. Cursor moves to end.
3. **Reset**: Any non-Tab/BackTab key clears completion state **and** clears `completion_error`. This means the error hint disappears on the next keystroke (text mutation, cursor move, submit, or cancel).
4. **Mixed paths** like `~/../tmp` are not normalized — they're passed through as-is.
5. **Hidden-file filtering** (`.-prefix only` rule) applies identically on all platforms.

### Rendering

```rust
impl LineInput {
    /// Returns (before_cursor, cursor_char, after_cursor) for styled rendering.
    pub fn render_parts(&self) -> (&str, Option<char>, &str);
    pub fn value(&self) -> &str;
}
```

When cursor is at end of text, `cursor_char` is `None` and a **space with inverted background** is rendered as the cursor (avoids font/glyph compatibility issues with block characters across terminals). When cursor is in the middle, the character at cursor position is rendered with inverted colors (swapped fg/bg).

## Changes to existing files

### `src/ui/mod.rs`
- Add `pub mod line_input;`

### `src/app.rs`
- Replace three booleans (`settings_editing_download_dir`, `settings_editing_export_path`, `settings_editing_import_path`) with:
  ```rust
  pub settings_editing_path: Option<(PathField, LineInput)>,
  ```
- `settings_export_path` and `settings_import_path` remain as `String`. On editing start, `LineInput` is initialized from current value. On `Submit`, value is written back to the field identified by `PathField`.
- `clear_settings_modals()` sets `settings_editing_path` to `None`.
- Add `is_editing_path(&self) -> bool` and `is_editing_field(&self, index: usize) -> bool` helpers.

**Migration checklist** — all sites referencing the old booleans must be updated. Verify by grepping for the removed field names (`settings_editing_download_dir`, `settings_editing_export_path`, `settings_editing_import_path`) — zero hits after migration:
- `src/main.rs` handle_settings_key priority 4 block (~line 550)
- `src/main.rs` Enter handler for fields 5, 9, 12, 14 (~line 605)
- `src/main.rs` render_settings `is_editing_this_path` check (~line 2693)
- `src/main.rs` `any_path_editing` footer check (~line 2724)
- `src/app.rs` field declarations (~line 218)
- `src/app.rs` `clear_settings_modals` (~line 462)
- `src/app.rs` `Default` / `new` initialization

### `src/main.rs` — `handle_settings_key()`
- **Priority 4 block**: Replace with `if let Some((field, ref mut input)) = app.settings_editing_path`. Call `input.handle(key)` and match on result. `Submit` writes value back via `field`, `Cancel` discards.
- **Enter on path fields**: Construct `LineInput::new(current_value)` paired with appropriate `PathField` variant.

### `src/main.rs` — `render_settings()`
- When editing, render via `input.render_parts()` with cursor char in inverted style.
- Footer hints in editing state: `"[←→] Move  [Tab] Complete (at end)  [Enter] Confirm  [Esc] Cancel"`
- If `input.completion_error` is true, append `"(cannot read directory)"` to footer. Clears on next keystroke.

## Key files

- `src/ui/line_input.rs` — new
- `src/ui/mod.rs` — add module
- `src/app.rs` — state fields, `clear_settings_modals()`, helper methods
- `src/main.rs` — key handling, rendering, footer

## Verification

1. `cargo build` — no warnings
2. `cargo test` — all existing + new unit tests pass
3. **Unit tests for `LineInput`** (in `line_input.rs`):
   - Insert char at start, middle, end
   - Delete/backspace at boundaries (start of line, end, empty string)
   - Ctrl+W: `"foo bar  "` → `"foo "`, `"  foo"` → `"  "`, `""` → `""`
   - Cursor: left at 0 stays 0, right at end stays at end
   - Home/End position correctly
   - Ctrl+U clears text and cursor to 0
   - Tab at end-of-line with no match → no completions, no panic
   - Tab at mid-line → no-op
   - Tab cycling wraps around; BackTab cycles reverse
   - Non-Tab key resets completion state
   - `render_parts()` returns correct slices at start, middle, end positions
4. **Grep verification**: `grep -rn 'settings_editing_download_dir\|settings_editing_export_path\|settings_editing_import_path' src/` returns zero hits
5. Manual testing:
   - Navigate to Export Path, press Enter → cursor appears at end
   - Arrow left/right moves cursor, Home/End work
   - Backspace/Delete at cursor position, Ctrl+U/Ctrl+W
   - Type partial path, Tab → completions cycle; Shift+Tab reverses
   - Tab on directory appends separator, allows continued completion
   - Tab on nonexistent path → footer shows "(cannot read directory)"
   - Enter confirms, Esc cancels (value reverts)
   - All four path fields (code dir, passage dir, export, import) work identically
