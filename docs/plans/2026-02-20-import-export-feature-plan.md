# Import/Export Feature Plan

## Context

Users need a way to back up and transfer their keydr data between machines. Currently, data is spread across `~/.config/keydr/config.toml` (config) and `~/.local/share/keydr/*.json` (profile, key stats, drill history). This feature adds Export and Import actions to the Settings page, producing/consuming a single combined JSON file.

## Export Format

Canonical filename: `keydr-export-2026-02-21.json` (date is `Utc::now()`).

```json
{
  "keydr_export_version": 1,
  "exported_at": "2026-02-21T12:00:00Z",
  "config": { ... },
  "profile": { ... },
  "key_stats": { ... },
  "ranked_key_stats": { ... },
  "drill_history": { ... }
}
```

- `exported_at` uses `DateTime<Utc>` (chrono, serialized as RFC3339).
- On import, `keydr_export_version` is checked: if it does not equal the current supported version (1), import is rejected with the error `"Unsupported export version: {v} (expected 1)"`. Future versions can add migration functions as needed.

## Import Scope

Import applies **everything except machine-local path fields**:
- **Imported**: target_wpm, theme, keyboard_layout, word_count, code_language, passage_book, download toggle booleans, snippets_per_repo, paragraphs_per_book, onboarding flags, and all progress data (profile, key stats, drill history).
- **Preserved from current config**: `code_download_dir`, `passage_download_dir` (machine-local paths stay as-is).
- Theme and keyboard_layout are imported as-is. If the imported theme is unavailable on the target machine, `Theme::load()` falls back to `terminal-default` and the success message includes a note: `"Imported successfully (theme '{name}' not found, using default)"`.

## Changes

### 1. Add export data struct (`src/store/schema.rs`)

Add an `ExportData` struct with all the fields above, deriving `Serialize`/`Deserialize`. Include `keydr_export_version: u32` and `exported_at: DateTime<Utc>` metadata fields.

### 2. Add export/import methods to `JsonStore` (`src/store/json_store.rs`)

- `export_all(&self, config: &Config) -> Result<ExportData>` — loads all data files and bundles with config into `ExportData`.
- `import_all(&self, data: &ExportData) -> Result<()>` — **transactional two-phase write** with best-effort rollback:
  1. **Stage phase**: write each data file to a `.tmp` sibling (profile.json.tmp, key_stats.json.tmp, etc.). If any `.tmp` write fails, delete all `.tmp` files created so far and return an error. Originals are untouched.
  2. **Commit phase**: for each file, rename the existing original to `.bak`, then rename `.tmp` to final. If any rename fails mid-sequence, **rollback**: restore all `.bak` files back to their original names and clean up remaining `.tmp` files. After successful commit, delete all `.bak` files.

  **Contract**: this is best-effort, not strictly atomic. If the process is killed or the disk fails during the commit phase, `.bak` files may be left behind. On next app startup, if `.bak` files are detected in the data directory, show a warning in the status message: `"Recovery files found from interrupted import. Data may be inconsistent — consider re-importing."` and clean up the `.bak` files.

### 3. Add config validation on import (`src/config.rs`)

Add a `Config::validate(&mut self, valid_language_keys: &[&str])` method that:
- Clamps `target_wpm` to 10..=200
- Clamps `word_count` to 5..=100
- Calls `normalize_code_language()` for code language validation
- Falls back to defaults for unrecognized theme names (via `Theme::load()` fallback, already handled)

This is called after merging imported config fields, before saving.

### 4. Add status message enum and app state fields (`src/app.rs`)

Add a structured status type:
```rust
pub enum StatusKind { Success, Error }
pub struct StatusMessage { pub kind: StatusKind, pub text: String }
```

New fields on `App`:
- `pub settings_confirm_import: bool` — controls the import warning dialog
- `pub settings_export_conflict: bool` — controls the export overwrite conflict dialog
- `pub settings_status_message: Option<StatusMessage>` — transient status, cleared on next keypress
- `pub settings_export_path: String` — editable export destination path
- `pub settings_import_path: String` — editable import source path
- `pub settings_editing_export_path: bool` — whether export path is being edited
- `pub settings_editing_import_path: bool` — whether import path is being edited

**Invariant**: at most one modal/edit state is active at a time. When entering any modal (confirm_import, export_conflict) or edit mode, clear all other modal/edit flags first.

Default export path: `dirs::download_dir()` / `keydr-export-{YYYY-MM-DD}.json`.
Default import path: same canonical filename (`dirs::download_dir()` / `keydr-export-{YYYY-MM-DD}.json`), editable.

If `dirs::download_dir()` returns `None`, fall back to `dirs::home_dir()`, then `"."`. On export, if the parent directory of the target path doesn't exist, return an error `"Directory does not exist: {parent}"` rather than silently creating it.

### 5. Add app methods (`src/app.rs`)

- `export_data()` — builds `ExportData` from current state, writes JSON to `settings_export_path` via **atomic write** (write to `.tmp` in same directory, then rename to final path). If file already exists at that path, sets `settings_export_conflict = true` instead of writing. Sets `StatusMessage` on success/error.
- `export_data_overwrite()` — calls the same atomic-write logic without the existence check. The rename atomically replaces the old file; no pre-delete needed.
- `export_data_rename()` — delegates to `next_available_path()`, a free function that implements **conditional suffix normalization**: strips a trailing `-N` suffix only when the base file (without suffix) exists in the same directory. This prevents accidental stripping of intrinsic name components (e.g. date segments like `-01`). Then scans for the lowest unused `-N` suffix. Works for any filename. E.g. if `my-backup.json` and `my-backup-1.json` exist, picks `my-backup-2.json`. If called with `my-backup-1.json` (and `my-backup.json` exists), normalizes to `my-backup` then picks `-2`. Updates `settings_export_path` and writes via atomic write.
- `import_data()` — reads file at `settings_import_path`, validates `keydr_export_version` (reject if != 1 with error message), calls `store.import_all()`, then reloads all in-memory state (config with path fields preserved, profile, key_stats, ranked_key_stats, drill_history, skill_tree). Calls `Config::validate()` and `Config::save()`. Checks if imported theme loaded successfully and appends fallback note to success message if not. Sets `StatusMessage` on success/error.

### 6. Add settings entries (`src/main.rs` — `render_settings`)

Add four new rows at the bottom of the settings field list:

- **"Export Path"** — editable path field, shows `settings_export_path` (same pattern as Code Download Dir)
- **"Export Data"** — action button, label: `"Export now"`
- **"Import Path"** — editable path field, shows `settings_import_path`
- **"Import Data"** — action button, label: `"Import now"`

Update `MAX_SETTINGS` accordingly in `handle_settings_key`.

### 7. Handle key input (`src/main.rs` — `handle_settings_key`)

**Priority order at top of function:**

1. If `settings_status_message.is_some()` — any keypress clears it and returns (message dismissed).
2. If `settings_export_conflict` — handle conflict dialog:
   - `'d'` → `export_data_overwrite()`, clear conflict flag
   - `'r'` → `export_data_rename()`, clear conflict flag
   - `Esc` → clear conflict flag
   - Return early.
3. If `settings_confirm_import` — handle import confirmation:
   - `'y'` → `import_data()`, clear flag
   - `'n'` / `Esc` → clear flag
   - Return early.
4. If editing export/import path — handle typing (same pattern as `settings_editing_download_dir`).

For the Enter handler on the new indices:
- Export Path → enter editing mode (clear other edit/modal flags first)
- Export Data → call `export_data()`
- Import Path → enter editing mode (clear other edit/modal flags first)
- Import Data → set `settings_confirm_import = true` (clear other flags first)

Add new indices to the exclusion lists for left/right cycling.

### 8. Render dialogs (`src/main.rs` — `render_settings`)

**Import confirmation dialog** (when `settings_confirm_import` is true):
- Dialog size: ~52x7, centered
- Border title: `" Confirm Import "`, border color: `colors.error()`
- Line 1: `"This will erase your current data."`
- Line 2: `"Export first if you want to keep it."`
- Line 3: `"Proceed? (y/n)"`

**Export conflict dialog** (when `settings_export_conflict` is true):
- Dialog size: ~52x7, centered
- Border title: `" File Exists "`, border color: `colors.error()`
- Line 1: `"A file already exists at this path."`
- Line 2: `"[d] Overwrite  [r] Rename  [Esc] Cancel"`

**Status message dialog** (when `settings_status_message` is `Some`):
- Small centered dialog showing the message text
- `StatusKind::Success` → accent color border. `StatusKind::Error` → error color border.
- Footer: `"Press any key"`

Dialog rendering priority: status message > export conflict > import confirmation (only one shown at a time).

### 9. Automated tests (`src/store/json_store.rs` or new test module)

Add tests for:
- **Round-trip**: export then import produces identical data
- **Transactional safety (supplemental)**: use a `tempdir`, write valid data, then import into a read-only tempdir and verify original files are unchanged
- **Staged write failure**: `import_all` with a poisoned `ExportData` (e.g. containing data that serializes but whose target path is manipulated to fail) verifies `.tmp` cleanup and original file preservation — this provides deterministic failure coverage without platform-dependent permission tricks
- **Version rejection**: import with `keydr_export_version: 99` returns error containing `"Unsupported export version"`
- **Config validation**: import with out-of-range values (target_wpm=0, word_count=999) gets clamped to valid ranges
- **Smart rename suffix**: create files `stem.json`, `stem-1.json` in a tempdir, verify rename picks `stem-2.json`; also test with custom (non-canonical) filenames
- **Modal invariant**: verify that setting any modal/edit flag clears all others

## Key Files to Modify

| File | Changes |
|------|---------|
| `src/store/schema.rs` | Add `ExportData` struct |
| `src/store/json_store.rs` | Add `export_all()`, transactional `import_all()` with rollback, `.bak` cleanup on startup, tests |
| `src/app.rs` | Add `StatusKind`/`StatusMessage`, state fields, export/import/rename methods, `.bak` check on init |
| `src/main.rs` | Settings UI entries, key handling, 3 dialog types, path editing |
| `src/config.rs` | Add `validate()` method |

## Deferred / Out of Scope

- **Settings enum refactor**: The hard-coded index pattern is pre-existing across the entire settings system. Refactoring to an enum/action map is worthwhile but out of scope for this feature.
- **Splitting config into portable vs machine-local structs**: Handled pragmatically by preserving path fields during import rather than restructuring Config.
- **IO abstraction for injectable writers**: The existing codebase uses direct `fs` calls throughout. Adding a trait-based abstraction for testability is a larger refactor. We use a poisoned-data test and a supplemental read-only tempdir test instead.

## Verification

1. `cargo build` — compiles without errors
2. `cargo test` — all new tests pass (round-trip, staged failure, version rejection, validation, rename suffix, modal invariant)
3. Launch app → Settings → verify Export Path / Export Data / Import Path / Import Data rows appear
4. Edit export path → verify typing/backspace works
5. Export → verify JSON file created at specified path with correct structure
6. Export again same day → verify conflict dialog appears; `d` overwrites atomically, `r` renames to `-1`
7. Export a third time → verify `r` renames to `-2` (smart suffix increment)
8. Export with custom filename → verify rename appends `-1` correctly
9. Import with bad version → verify error: `"Unsupported export version: 99 (expected 1)"`
10. Import → verify warning dialog appears; `n`/`Esc` cancels without changes
11. Import → `y` → verify data loaded, config preferences updated, paths preserved
12. Import with unavailable theme → verify success message includes fallback note
13. Verify only one modal/edit state can be active: e.g. while editing export path, pressing a key that would open import confirm does not open it
14. Round-trip: export, change settings, do a drill, import the export, verify original state restored
