# Prevent Tests from Writing to Real User Data

## Context

Two tests in `src/app.rs` (`adaptive_auto_continue_arms_input_lock` and `adaptive_does_not_auto_continue_with_milestones`) call `App::new()` which connects to the real `JsonStore` at `~/.local/share/keydr/`. When they call `finish_drill()` → `save_data()`, fake drill results get persisted to the user's actual history file. All other app tests also use `App::new()` but happen to not call `finish_drill()`.

## Changes

### 1. Add `#[cfg(not(test))]` gate on `App::new()` (`src/app.rs:293`)

Mark `App::new()` with `#[cfg(not(test))]` so it cannot be called from test code at all. This is a compile-time guarantee — any future test that tries `App::new()` will fail to compile.

### 2. Add `App::new_test()` (`src/app.rs`, in `#[cfg(test)]` block)

Add a `pub fn new_test()` constructor inside a `#[cfg(test)] impl App` block that mirrors `App::new()` but sets `store: None`. This prevents any persistence to disk. All existing fields get their default/empty values (no loading from disk either).

Since most test fields just need defaults and a started drill, the test constructor can be minimal:
- `Config::default()`, `Theme::default()` (leaked), `Menu::new()`, `store: None`
- Default key stats, skill tree, profile, empty drill history
- `Dictionary::load()`, `TransitionTable`, `KeyboardModel` — same as production (needed for `start_drill()`)
- Call `start_drill()` at the end (same as `App::new()`)

### 3. Update all existing tests to use `App::new_test()`

Replace every `App::new()` call in the test module with `App::new_test()`. This covers all 7 tests in `#[cfg(test)] mod tests`.

## File to Modify

- `src/app.rs` — gate `new()`, add `new_test()`, update test calls

## Verification

1. `cargo test` — all tests pass
2. `cargo build` — production build still compiles (ungated `new()` available)
3. Temporarily add `App::new()` in a test → should fail to compile
