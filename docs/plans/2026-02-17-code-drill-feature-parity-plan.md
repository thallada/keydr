# Code Drill Feature Parity Plan

## Context

The code drill feature is significantly less developed than the passage drill. The passage drill has a full onboarding flow, lazy downloads with progress bars, configurable network/cache settings, and rich content from Project Gutenberg. The code drill only has 4 hardcoded languages with ~20-30 built-in snippets each, a basic language selection screen, and a partially-implemented synchronous GitHub fetch that blocks the UI thread. There's also a completely dead `github_code.rs` file that's never used.

This plan is split into three delivery phases:
1. **Phase 1**: Feature parity with passage drill (onboarding, downloads, progress bar, config)
2. **Phase 2**: Language expansion and extraction improvements
3. **Phase 3**: Custom repo support

## Current Code Drill Analysis

### What exists:
- **`generator/code_syntax.rs`**: `CodeSyntaxGenerator` with built-in snippets for 4 languages (rust, python, javascript, go), a `try_fetch_code()` that synchronously fetches from hardcoded GitHub URLs (blocking UI), `extract_code_snippets()` for parsing functions from source
- **`generator/code_patterns.rs`**: Post-processor that inserts code-like expressions into adaptive drill text (unrelated to code drill mode)
- **`generator/github_code.rs`**: **Dead code** - `GitHubCodeGenerator` struct with `#[allow(dead_code)]`, never referenced outside its own file
- **Config**: Only `code_language: String` - no download/network/onboarding settings
- **Screens**: `CodeLanguageSelect` only - no intro, no download progress
- **Languages**: rust, python, javascript, go, "all"

### What passage drill has that code drill doesn't:
- Onboarding intro screen (`PassageIntro`) with config for downloads/dir/limits
- `passage_onboarding_done` flag (shows intro only on first use)
- `passage_downloads_enabled` toggle
- `passage_download_dir` configurable path
- `passage_paragraphs_per_book` content limit
- Lazy download: on drill start, downloads one book if not cached
- Background download thread with atomic progress reporting
- Download progress screen (`PassageDownloadProgress`) with byte-level progress bar
- Fallback to built-in content when downloads off

### Built-in snippet whitespace review:
- **Rust**: 4-space indent - idiomatic
- **Python**: 4-space indent - idiomatic
- **JavaScript**: 4-space indent - idiomatic
- **Go**: `\t` tab indent - idiomatic

All whitespace is correct. The escaped string format (`\n`, `\t`, `\"`) is hard to read. Converting to raw strings (`r#"..."#`) improves maintainability.

---

## Phase 1: Feature Parity with Passage Drill

Goal: Give code drill the same onboarding, download, caching, and config infrastructure as passage drill. Keep the existing 4 languages. No language expansion yet.

### Step 1.1: Delete dead code

- Delete `src/generator/github_code.rs` entirely
- Remove `pub mod github_code;` from `src/generator/mod.rs`

### Step 1.2: Convert built-in snippets to raw strings

**File**: `src/generator/code_syntax.rs`

Convert all 4 language snippet arrays from escaped strings to `r#"..."#` raw strings. Example:

Before: `"fn main() {\n    println!(\"hello\");\n}"`
After:
```rust
r#"fn main() {
    println!("hello");
}"#
```

Go snippets: `\t` becomes actual tab characters inside raw strings (correct for Go).

Keep all existing snippets at their current count (~20-30 per language). Do NOT reduce them -- since downloads default to off, these are the primary content source for new users.

Validation: run `cargo test` after conversion. Add a focused test that asserts a sample snippet's char content matches expectations (catches any accidental whitespace changes).

### Step 1.3: Add config fields for code drill

**File**: `src/config.rs`

Add fields mirroring passage drill config:

```rust
#[serde(default = "default_code_downloads_enabled")]
pub code_downloads_enabled: bool,    // default: false
#[serde(default = "default_code_download_dir")]
pub code_download_dir: String,       // default: dirs::data_dir()/keydr/code/
#[serde(default = "default_code_snippets_per_repo")]
pub code_snippets_per_repo: usize,   // default: 50
#[serde(default = "default_code_onboarding_done")]
pub code_onboarding_done: bool,      // default: false
```

`code_download_dir` default uses `dirs::data_dir()` (same pattern as `default_passage_download_dir`) for cross-platform portability.

`code_snippets_per_repo` is a **download-time extraction cap**: when fetching from a repo, extract at most this many snippets and write them to cache. The generator reads whatever is in the cache without re-filtering.

Update `Default` impl. Add `default_*` functions.

**Config normalization**: After deserialization in `App::new()` (not `Config::load()`, to avoid coupling config to generator internals), validate `code_language` against `code_language_options()`. If invalid (e.g., old/renamed key), reset to `"rust"`.

**Old cache migration**: The old `DiskCache("code_cache")` entries (in `~/.local/share/keydr/code_cache/`) are simply ignored. They used a different key format (`{lang}_snippets`) and location. No migration or cleanup needed -- they'll be naturally superseded by the new cache in `code_download_dir`.

### Step 1.4: Define language data structures

**File**: `src/generator/code_syntax.rs`

Add structures for the language registry. Phase 1 only populates the 4 existing languages + "all":

```rust
pub struct CodeLanguage {
    pub key: &'static str,         // filesystem-safe identifier (e.g. "rust", "bash")
    pub display_name: &'static str, // UI label (e.g. "Rust", "Shell/Bash")
    pub extensions: &'static [&'static str], // e.g. &[".rs"], &[".py", ".pyi"]
    pub repos: &'static [CodeRepo],
    pub has_builtin: bool,
}

pub struct CodeRepo {
    pub key: &'static str,        // filesystem-safe identifier for cache naming
    pub urls: &'static [&'static str], // raw.githubusercontent.com file URLs to fetch
}

pub const CODE_LANGUAGES: &[CodeLanguage] = &[
    CodeLanguage {
        key: "rust",
        display_name: "Rust",
        extensions: &[".rs"],
        repos: &[
            CodeRepo {
                key: "tokio",
                urls: &[
                    "https://raw.githubusercontent.com/tokio-rs/tokio/master/tokio/src/sync/mutex.rs",
                    "https://raw.githubusercontent.com/tokio-rs/tokio/master/tokio/src/net/tcp/stream.rs",
                ],
            },
            CodeRepo {
                key: "serde",
                urls: &[
                    "https://raw.githubusercontent.com/serde-rs/serde/master/serde/src/ser/mod.rs",
                ],
            },
        ],
        has_builtin: true,
    },
    // ... python, javascript, go with similar structure
    // Move existing hardcoded URLs from try_fetch_code() into these repo definitions
];
```

Helper functions:
```rust
pub fn code_language_options() -> Vec<(&'static str, String)>
// Returns [("rust", "Rust"), ("python", "Python"), ..., ("all", "All (random)")]

pub fn language_by_key(key: &str) -> Option<&'static CodeLanguage>

pub fn is_language_cached(cache_dir: &str, key: &str) -> bool
// Checks if any {key}_*.txt files exist in cache_dir AND have non-empty content (>0 bytes)
// Uses direct filesystem scanning (NOT DiskCache -- DiskCache has no list/glob API)
```

### Step 1.5: Generalize download job struct

**File**: `src/app.rs`

Rename `PassageDownloadJob` to `DownloadJob`. It's already generic (just `Arc<AtomicU64>`, `Arc<AtomicBool>`, and a thread handle). Update all passage references to use the renamed type. No behavior change.

### Step 1.6: Add code drill app state

**File**: `src/app.rs`

Add `CodeDownloadCompleteAction` enum (parallels `PassageDownloadCompleteAction`):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeDownloadCompleteAction {
    StartCodeDrill,
    ReturnToSettings,
}
```

Add screen variants:
```rust
CodeIntro,              // Onboarding screen for code drill
CodeDownloadProgress,   // Download progress for code files
```

Add app fields:
```rust
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
pub code_download_queue: Vec<usize>,  // repo indices within current language's repos array
pub code_drill_language_override: Option<String>,
pub code_download_action: CodeDownloadCompleteAction,
code_download_job: Option<DownloadJob>,
```

### Step 1.7: Remove blocking fetch from generator

**File**: `src/generator/code_syntax.rs`

Remove `try_fetch_code()` from `CodeSyntaxGenerator`. All network I/O moves to the app layer with background threads.

Update constructor:
```rust
pub fn new(rng: SmallRng, language: &str, cache_dir: &str) -> Self
```

Update `load_cached_snippets()`: scan `cache_dir` for files matching `{language}_*.txt`, read each, split on `---SNIPPET---` delimiter. This replaces the `DiskCache("code_cache")` approach with direct filesystem reads (since `DiskCache` has no listing/glob API and the cache dir is now user-configurable).

### Step 1.8: Add download function

**File**: `src/generator/code_syntax.rs`

```rust
pub fn download_code_repo_to_cache_with_progress<F>(
    cache_dir: &str,
    language_key: &str,
    repo: &CodeRepo,
    snippets_limit: usize,
    on_progress: F,
) -> bool
where
    F: FnMut(u64, Option<u64>),
```

This function:
1. Creates `cache_dir` if needed (`fs::create_dir_all`)
2. Fetches each URL in `repo.urls` using `fetch_url_bytes_with_progress` (already exists in `cache.rs`)
3. Runs `extract_code_snippets()` on each fetched file
4. Combines all snippets, truncates to `snippets_limit`
5. Writes to `{cache_dir}/{language_key}_{repo.key}.txt` with `---SNIPPET---` delimiter
6. Returns `true` on success

**Error handling**: If any individual URL fails (404, timeout, network error), skip it and continue with others. If zero snippets extracted from all URLs, return `false`. The app layer treats `false` as "skip this repo, continue queue" (same as passage drill's failure behavior).

### Step 1.9: Implement code drill flow methods

**File**: `src/app.rs`

**`go_to_code_intro()`**: Initialize intro screen state (downloads toggle, dir, snippets limit from config). Set `code_download_action = CodeDownloadCompleteAction::StartCodeDrill`. Set screen to `CodeIntro`.

**`start_code_drill()`**: Lazy download logic with explicit language resolution:

```rust
pub fn start_code_drill(&mut self) {
    // Step 1: Resolve concrete language (never download with "all" selected)
    if self.code_drill_language_override.is_none() {
        let chosen = if self.config.code_language == "all" {
            // Pick from languages with built-in OR cached content only
            // Never pick a network-only language that isn't cached
            let available = languages_with_content(&self.config.code_download_dir);
            if available.is_empty() {
                "rust".to_string() // ultimate fallback
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

    // Step 2: Check if we need to download
    if self.config.code_downloads_enabled
        && !is_language_cached(&self.config.code_download_dir, &chosen)
    {
        if let Some(lang) = language_by_key(&chosen) {
            if !lang.repos.is_empty() {
                // Pick one random repo to download
                let repo_idx = self.rng.gen_range(0..lang.repos.len());
                self.code_download_queue = vec![repo_idx];
                self.code_intro_download_total = 1;
                self.code_intro_downloaded = 0;
                self.code_intro_downloading = true;
                self.code_intro_current_repo = format!("{}", lang.repos[repo_idx].key);
                self.code_download_action = CodeDownloadCompleteAction::StartCodeDrill;
                self.code_download_job = None;
                self.screen = AppScreen::CodeDownloadProgress;
                return;
            }
        }
        // Language has no repos or unknown: fall through to built-in
    }

    // Step 3: If language has no built-in AND no cache AND downloads off → fallback
    if !is_language_cached(&self.config.code_download_dir, &chosen) {
        if let Some(lang) = language_by_key(&chosen) {
            if !lang.has_builtin {
                // Network-only language with no cache: fall back to "rust"
                self.code_drill_language_override = Some("rust".to_string());
            }
        }
    }

    // Step 4: Start the drill
    self.drill_mode = DrillMode::Code;
    self.drill_scope = DrillScope::Global;
    self.start_drill();
}
```

Key behavior: `"all"` only selects from `languages_with_content()` (built-in OR cached). This prevents the dead-end loop of repeatedly picking uncached network-only languages and forcing download screens. In Phase 2, once network-only languages get cached via manual download, they are automatically included in `"all"` selection.

**`languages_with_content(cache_dir: &str) -> Vec<&'static str>`**: Returns language keys that have either `has_builtin: true` or non-empty cache files in `cache_dir`.

**`process_code_download_tick()`**, **`spawn_code_download_job()`**: Same pattern as passage equivalents, using `download_code_repo_to_cache_with_progress` and `DownloadJob`.

**`start_code_downloads_from_settings()`**: Mirror `start_passage_downloads_from_settings()` with `CodeDownloadCompleteAction::ReturnToSettings`.

### Step 1.10: Update code language select flow

**File**: `src/main.rs`

Update `handle_code_language_key()` and `render_code_language_select()`:
- Still shows the same 4+1 languages for now (Phase 2 expands this)
- Wire Enter to `confirm_code_language_and_continue()`:

```rust
fn confirm_code_language_and_continue(app: &mut App, langs: &[&str]) {
    if app.code_language_selected >= langs.len() { return; }
    app.config.code_language = langs[app.code_language_selected].to_string();
    let _ = app.config.save();
    if app.config.code_onboarding_done {
        app.start_code_drill();
    } else {
        app.go_to_code_intro();
    }
}
```

### Step 1.11: Add event handlers and renderers

**File**: `src/main.rs`

Add to screen dispatch in `handle_key()` and `render()`:

**`handle_code_intro_key()`**: Same field navigation as `handle_passage_intro_key()` but operates on `code_intro_*` fields. 4 fields:
1. Enable network downloads (toggle)
2. Download directory (editable text)
3. Snippets per repo (numeric, adjustable)
4. Start code drill (confirm button)

On confirm: save config fields, set `code_onboarding_done = true`, call `start_code_drill()`.

**`handle_code_download_progress_key()`**: Esc/q to cancel. On cancel:
1. Clear `code_download_queue`
2. Set `code_intro_downloading = false`
3. If a `code_download_job` is in-flight, detach it (set to `None` without joining -- the thread will finish and write to cache, which is harmless; the `Arc` atomics keep the thread safe)
4. Reset `code_drill_language_override` to `None`
5. Go to menu

This matches the existing passage download cancel behavior (passage also does not join/abort in-flight threads on Esc).

**`render_code_intro()`**: Mirror `render_passage_intro()` layout. Title: "Code Downloads Setup". Explanatory text: "Configure code source settings before your first code drill." / "Downloads are lazy: code is fetched only when first needed."

**`render_code_download_progress()`**: Mirror `render_passage_download_progress()`. Title: "Downloading Code Source". Show repo name, byte progress bar.

Update tick handler:
```rust
if (app.screen == AppScreen::CodeIntro
    || app.screen == AppScreen::CodeDownloadProgress)
    && app.code_intro_downloading
{
    app.process_code_download_tick();
}
```

### Step 1.12: Update generate_text for Code mode

**File**: `src/app.rs`

Update `DrillMode::Code` in `generate_text()`:

```rust
DrillMode::Code => {
    let filter = CharFilter::new(('a'..='z').collect());
    let lang = self.code_drill_language_override
        .clone()
        .unwrap_or_else(|| self.config.code_language.clone());
    let rng = SmallRng::from_rng(&mut self.rng).unwrap();
    let mut generator = CodeSyntaxGenerator::new(
        rng, &lang, &self.config.code_download_dir,
    );
    self.code_drill_language_override = None;
    let text = generator.generate(&filter, None, word_count);
    (text, Some(generator.last_source().to_string()))
}
```

### Step 1.13: Settings integration

**Files**: `src/main.rs`, `src/app.rs`

Add settings rows after existing code language field (index 3):
- Index 4: Code Downloads: On/Off
- Index 5: Code Download Dir: editable path
- Index 6: Code Snippets per Repo: numeric
- Index 7: Download Code Now: action button

Shift existing passage settings indices up by 4. Update `settings_cycle_forward`/`settings_cycle_backward` and max `settings_selected` bound.

**"Download Code Now" behavior**: Downloads all uncached curated repos for the currently selected `code_language` only. If `code_language == "all"`, downloads all uncached repos for all curated languages. Does NOT include custom repos. Mirrors passage behavior where "Download Passages Now" downloads all uncached books.

**`start_code_downloads()`**: Queues all uncached repos for the currently selected language. Used by intro screen "confirm" flow when downloads are enabled.

### Phase 1 Verification

1. `cargo build` -- compiles
2. `cargo test` -- all existing tests pass, plus new tests:
   - `test_languages_with_content_includes_builtin` -- verifies built-in languages appear in `languages_with_content()` even with empty cache dir
   - `test_languages_with_content_excludes_uncached_network_only` -- verifies network-only languages without cache are not returned
   - `test_config_serde_defaults` -- verifies new config fields deserialize with correct defaults from empty/old configs
   - `test_raw_string_snippets_preserved` -- spot-check that raw string conversion didn't alter snippet content
3. `cargo build --no-default-features` -- compiles, network features gated
4. Manual tests:
   - Menu → Code Drill → language select → first time shows CodeIntro
   - CodeIntro with downloads off → confirms → starts drill with built-in snippets
   - CodeIntro with downloads on → confirms → shows CodeDownloadProgress → downloads repo → starts drill with downloaded content
   - Subsequent code drills skip onboarding
   - "all" language mode only picks from languages with content (never triggers download)
   - Settings shows code drill fields, values persist on restart
   - Passage drill flow completely unchanged
   - Esc during download progress → returns to menu, no crash

---

## Phase 2: Language Expansion and Extraction Improvements

Goal: Add 8 more built-in languages and ~18 network-only languages, improve snippet extraction.

### Step 2.1: Add 8 built-in language snippet sets

**File**: `src/generator/code_syntax.rs`

Add ~10-15 raw-string snippets each for: **typescript, java, c, cpp, ruby, swift, bash, lua**

Language keys: `typescript`/`ts`, `java`, `c`, `cpp`, `ruby`, `swift`, `bash` (display: "Shell/Bash"), `lua`

All with idiomatic whitespace:
- TypeScript: 4-space indent
- Java: 4-space indent
- C: 4-space indent
- C++: 4-space indent
- Ruby: 2-space indent
- Swift: 4-space indent
- Bash: 2-space indent (common convention)
- Lua: 2-space indent

Update `get_snippets()` match to include all 12 languages.

### Step 2.2: Expand language registry to ~30 languages

**File**: `src/generator/code_syntax.rs`

Add ~18 network-only entries to `CODE_LANGUAGES` with curated repos:

kotlin, scala, haskell, elixir, clojure, perl, php, r, dart, zig, nim, ocaml, erlang, julia, objective-c, groovy, csharp, fsharp

Each gets 2-3 repos with specific raw.githubusercontent.com file URLs. **Exclude SQL and CSS** -- their syntax is too different from procedural code for function-level extraction to work well.

This is a significant data curation subtask: for each language, identify 2-3 well-known repos with permissive licenses (MIT/Apache/BSD), select 2-5 representative source files per repo with functions/methods to extract.

**Acceptance threshold**: Each language must yield at least 10 extractable snippets from its curated repos (verified by running `extract_code_snippets` against fetched files). Languages that fall below this threshold should be dropped from the registry rather than shipped with poor content.

### Step 2.3: Improve snippet extraction

**File**: `src/generator/code_syntax.rs`

Add a `func_start_patterns` field to `CodeLanguage`:

```rust
pub struct CodeLanguage {
    // ... existing fields ...
    pub block_style: BlockStyle,
}

pub enum BlockStyle {
    Braces(&'static [&'static str]),       // fn/def/func patterns, brace-delimited (C, Java, Go, etc.)
    Indentation(&'static [&'static str]),  // def/class patterns, indentation-delimited (Python)
    EndDelimited(&'static [&'static str]), // def/class patterns, closed by `end` keyword (Ruby, Lua, Elixir)
}
```

Update `extract_code_snippets()` to accept `BlockStyle`:
- `Braces`: current behavior with configurable start patterns (C, Java, Go, JS, etc.)
- `Indentation`: track indent level changes to find block boundaries (Python only)
- `EndDelimited`: scan for matching `end` keyword at same indent level to close blocks (Ruby, Lua, Elixir)

Language-specific patterns:
- Java: `["public ", "private ", "protected ", "static ", "class ", "interface "]`
- Ruby: `["def ", "class ", "module "]` (EndDelimited style -- uses `end` keyword to close blocks)
- C/C++: `["int ", "void ", "char ", "float ", "double ", "struct ", "class ", "template"]`
- Swift: `["func ", "class ", "struct ", "enum ", "protocol "]`
- Bash: `["function ", "() {"]` (Braces style, simple)
- etc.

### Step 2.4: Make language select scrollable

**File**: `src/main.rs`

With 30+ languages, the selection screen needs scrolling. Add `code_language_scroll: usize` to `App`. Show a viewport of ~15 items. Add keybindings:
- Up/Down: navigate
- PageUp/PageDown: jump 10 items
- Home/End or `g`/`G`: jump to top/bottom
- `/`: type-to-filter (optional, nice-to-have)

Mark each language as "(built-in)" or "(download required)" in the list.

### Phase 2 Verification

1. `cargo build && cargo test`
2. Manual: verify all 12 built-in languages produce readable snippets with correct indentation
3. Manual: select a network-only language → triggers download → produces good snippets
4. Manual: scrollable language list works, indicators are accurate
5. Verify each built-in language's snippet whitespace is idiomatic

---

## Phase 3: Custom Repo Support

Goal: Let users specify their own GitHub repos to train on.

### Step 3.1: Design custom repo fetch strategy

Custom repos require solving problems that curated repos don't have:
- **Branch discovery**: Use GitHub API `GET /repos/{owner}/{repo}` to find `default_branch`. Requires `User-Agent` header (GitHub rejects requests without it; use `"keydr/{version}"`). Optionally support a `GITHUB_TOKEN` env var for authenticated requests (raises rate limit from 60 to 5000 req/hour).
- **File discovery**: Use GitHub API `GET /repos/{owner}/{repo}/git/trees/{branch}?recursive=1` to list all files, filter by language extensions. Same `User-Agent` and optional auth headers. If the response has `"truncated": true` (repos with >100k files), reject with a user-facing error: "Repository is too large for automatic file discovery. Please use a smaller repo or fork with fewer files."
- **Rate limiting**: Cache the tree response to disk. On 403/429 responses, show error: "GitHub API rate limit reached. Try again later or set GITHUB_TOKEN env var for higher limits."
- **File selection**: From matching files, randomly select 3-5 files to download via raw.githubusercontent.com (no API needed for file content)
- **Language detection**: Match file extensions against `CodeLanguage.extensions` field. If ambiguous or no match, prompt user.
- **All API requests**: Set `Accept: application/vnd.github.v3+json` header, timeout 10s.

### Step 3.2: Add config field and validation

**File**: `src/config.rs`

```rust
#[serde(default)]
pub code_custom_repos: Vec<String>,  // Format: "owner/repo" or "owner/repo@language"
```

Parse function:
```rust
pub fn parse_custom_repo(input: &str) -> Option<CustomRepo> {
    // Accepts: "owner/repo", "owner/repo@language", "https://github.com/owner/repo"
    // Validates: owner and repo contain only valid GitHub chars
    // Returns None on invalid input
}
```

### Step 3.3: Settings UI for custom repos

Add a settings section showing current custom repos as a scrollable list. Keybindings:
- `a`: add new repo (enters text input mode)
- `d`/`x`: delete selected repo
- Up/Down: navigate list

### Step 3.4: Code language select "Add custom repo" option

At the bottom of the language select list, add an "[ + Add custom repo ]" option. Selecting it enters a text input mode for `owner/repo`. On confirm:
1. Validate format
2. Add to `code_custom_repos` config
3. Auto-detect language from repo (via API tree listing file extensions)
4. If language ambiguous, show a small picker
5. Queue download of that repo

### Step 3.5: Integrate custom repos into download flow

When `start_code_drill()` runs for a language, include matching custom repos in the download candidates alongside curated repos.

### Phase 3 Verification

1. Add a custom repo → appears in settings list
2. Start drill → custom repo snippets appear
3. Invalid repo format → shows error, doesn't save
4. GitHub rate limit → shows informative error
5. Remove custom repo → removed from config and future drills

---

## Critical Files Summary

| File | Phase | Changes |
|------|-------|---------|
| `src/generator/github_code.rs` | 1 | Delete |
| `src/generator/mod.rs` | 1 | Remove github_code module |
| `src/generator/code_syntax.rs` | 1, 2 | Raw strings, new constructor, remove blocking fetch, language registry, download fn, new snippet sets, improved extraction |
| `src/config.rs` | 1, 3 | New code drill config fields, validation |
| `src/app.rs` | 1 | DownloadJob rename, new screens/state/flow methods, CodeDownloadCompleteAction |
| `src/main.rs` | 1, 2 | New handlers/renderers, updated settings, scrollable language list |
| `src/generator/cache.rs` | 1 | No changes (reuse existing `fetch_url_bytes_with_progress`) |

## Existing Code to Reuse

- `generator::cache::fetch_url_bytes_with_progress` -- already handles progress callbacks, used for passage downloads
- `generator::cache::DiskCache` -- NOT reused for code cache (no listing API); use direct `fs::read_dir` + `fs::read_to_string` instead
- `PassageDownloadJob` pattern (atomics + thread) -- generalized into `DownloadJob`
- `passage::extract_paragraphs` pattern -- referenced for extraction design but not directly reused
- `passage::download_book_to_cache_with_progress` -- structural template for `download_code_repo_to_cache_with_progress`

---

## Phase 2.5: Improve Snippet Extraction Quality

### Context

After Phase 2, the verification test (`test_verify_repo_urls`) shows many languages producing far fewer than 100 snippets. Root causes:
1. **Per-file cap of 50** in `extract_code_snippets()` (line 1869) limits output even from large source files
2. **Keyword-only matching** — extraction only starts when a line begins with a recognized keyword (e.g. `fn `, `def `, `class `). Many valid code blocks (anonymous functions, method chains, match arms, closures, etc.) are missed.
3. **Narrow keyword lists** — some languages are missing patterns for common constructs (e.g. `macro_rules!` in Rust, `@interface` in Objective-C)
4. **`code_snippets_per_repo` default of 50** caps total output per download

### Goal

Get every language to produce 100+ snippets from its curated repos, without sacrificing snippet quality. Do this by:
1. Widening keyword patterns to capture more language constructs
2. Adding a structural fallback that extracts well-formed code blocks by structure when keywords alone don't find enough
3. Raising the per-file and per-repo snippet caps

### Step 2.5.1: Raise snippet caps

**File**: `src/generator/code_syntax.rs`

Change `snippets.truncate(50)` → `snippets.truncate(200)` in `extract_code_snippets()`.

**File**: `src/config.rs`

Change `default_code_snippets_per_repo()` → `200`.

### Step 2.5.2: Widen keyword patterns

**File**: `src/generator/code_syntax.rs`

Add missing start patterns to existing languages. These are patterns that should have been there from the start — they represent common, well-defined constructs that produce good typing drill snippets:

| Language | Add patterns |
|----------|-------------|
| Rust | `"macro_rules! "`, `"mod "`, `"const "`, `"static "`, `"type "` |
| Python | `"async def "` is already there. Add `"@"` (decorators start blocks) |
| JavaScript | `"class "`, `"const "`, `"let "`, `"export "` |
| Go | No changes needed (already has `"func "`, `"type "`) |
| TypeScript | `"class "`, `"const "`, `"let "`, `"export "`, `"interface "` |
| Java | `"abstract "`, `"final "`, `"@"` (annotations start blocks) |
| C | `"typedef "`, `"#define "`, `"enum "` |
| C++ | `"namespace "`, `"typedef "`, `"#define "`, `"enum "`, `"constexpr "`, `"auto "` |
| Ruby | Add `"attr_"`, `"scope "`, `"describe "`, `"it "` |
| Swift | `"var "`, `"let "`, `"init("`, `"deinit "`, `"extension "`, `"typealias "` |
| Bash | `"if "`, `"for "`, `"while "`, `"case "` |
| Kotlin | `"override fun "` already there. Add `"val "`, `"var "`, `"enum "`, `"annotation "`, `"typealias "` |
| Scala | `"val "`, `"var "`, `"type "`, `"implicit "`, `"given "`, `"extension "` |
| PHP | `"class "`, `"interface "`, `"trait "`, `"enum "` |
| Dart | Add `"Widget "`, `"get "`, `"set "`, `"enum "`, `"typedef "`, `"extension "` |
| Elixir | `"defmacro "`, `"defstruct"`, `"defprotocol "`, `"defimpl "` |
| Zig | `"test "`, `"var "` |
| Haskell | Already broad. No changes. |
| Objective-C | `"@interface "`, `"@implementation "`, `"@protocol "`, `"typedef "` |
| Others | Review on a case-by-case basis during implementation |

### Step 2.5.3: Add structural fallback extraction

**File**: `src/generator/code_syntax.rs`

When keyword-based extraction yields fewer than 20 snippets from a file, run a second pass that extracts code blocks purely by structure. This captures anonymous functions, nested blocks, and other constructs that don't start with recognized keywords.

#### Design

Add a `structural_fallback: bool` field to each `BlockStyle` variant:

```rust
pub enum BlockStyle {
    Braces {
        patterns: &'static [&'static str],
        structural_fallback: bool,
    },
    Indentation {
        patterns: &'static [&'static str],
        structural_fallback: bool,
    },
    EndDelimited {
        patterns: &'static [&'static str],
        structural_fallback: bool,
    },
}
```

Set `structural_fallback: true` for all languages. This can be disabled per-language if it produces poor results.

Update `extract_code_snippets()`:

```rust
pub fn extract_code_snippets(source: &str, block_style: &BlockStyle) -> Vec<String> {
    let mut snippets = keyword_extract(source, block_style);

    if snippets.len() < 20 && has_structural_fallback(block_style) {
        let structural = structural_extract(source, block_style);
        // Add structural snippets that don't overlap with keyword ones
        for s in structural {
            if !snippets.contains(&s) {
                snippets.push(s);
            }
        }
    }

    snippets.truncate(200);
    snippets
}
```

#### Structural extraction for Braces languages

`structural_extract_braces(source)`:
1. Scan for lines containing `{` where brace depth transitions from 0→1 or 1→2
2. Capture from that line until depth returns to its starting level
3. Apply the same quality filters: 3-30 lines, 20+ non-whitespace chars, ≤800 bytes
4. Skip noise blocks: reject snippets where first non-blank line is only `{`, or where the block is just imports/use statements

#### Structural extraction for Indentation languages

`structural_extract_indent(source)`:
1. Scan for non-blank lines at indentation level 0 (top-level) that are followed by indented lines
2. Capture the top-level line + all subsequent lines with greater indentation
3. Apply same quality filters
4. Skip noise: reject if all body lines are `import`/`from`/`use`/`#include` statements

#### Structural extraction for EndDelimited languages

`structural_extract_end(source)`:
1. Scan for lines at top-level indentation followed by indented body ending with `end`
2. Same quality filters and noise rejection

#### Noise filtering

A snippet is "noise" and should be rejected if:
- First meaningful line (after stripping comments) is just `{` or `}`
- Body consists entirely of `import`, `use`, `from`, `require`, `include`, or blank lines
- It's a single-statement block (only 1 non-blank body line after the opening)

### Step 2.5.4: Add more source URLs for low-count languages

After implementing the extraction improvements, re-run `test_verify_repo_urls` to identify languages still under 100 snippets. For those, add 1-2 more source file URLs from the same or new repos to increase raw material.

This step is intentionally deferred until after extraction improvements, since better extraction may push many languages over the 100 threshold without needing more URLs.

### Phase 2.5 Verification

1. `cargo test` — all existing tests pass
2. Run `cargo test test_verify_repo_urls -- --ignored --nocapture` — verify all 30 languages produce 50+ snippets (ideally 100+)
3. Spot-check structural fallback snippets for 3-4 languages — verify they contain real code, not just import blocks or noise
4. `cargo build --no-default-features` — compiles without network features
5. Verify no change to built-in snippet behavior (built-in snippets don't go through extraction)
