# Adaptive Auto-Continue Input Lock Overlay

## Context

In adaptive mode, when a drill completes with no milestone popups to show, the app auto-continues to the next drill immediately (`finish_drill` → `start_drill()` with no intermediate screen). The existing 800ms input lock (`POST_DRILL_INPUT_LOCK_MS`) is only armed when there IS an intermediate screen (DrillResult or milestone popup). This means trailing keystrokes from the previous drill can bleed into the next drill as unintended inputs.

The fix: arm the same 800ms lock during adaptive auto-continue, block drill input while it's active, and show a small countdown popup overlay on the drill screen so the user knows why their input is temporarily ignored.

## Changes

### 1. Arm the lock on adaptive auto-continue

**`src/app.rs` — `finish_drill()`**

Currently the auto-continue path does not arm the lock:
```rust
if self.drill_mode == DrillMode::Adaptive && self.milestone_queue.is_empty() {
    self.start_drill();
}
```

Add `arm_post_drill_input_lock()` after `start_drill()`. It must come after because `start_drill()` calls `clear_post_drill_input_lock()` as its first action (to clear stale locks from manual continues). Re-arming immediately after means the 800ms window starts from when the new drill begins:
```rust
if self.drill_mode == DrillMode::Adaptive && self.milestone_queue.is_empty() {
    self.start_drill();
    self.arm_post_drill_input_lock();
}
```

**Event ordering safety**: The event loop in `run_app()` is single-threaded: `draw` → `events.next()` → `handle_key` → loop. `finish_drill()` runs inside a `handle_key()` call, so both `start_drill()` and `arm_post_drill_input_lock()` complete within the same event iteration. Any buffered key events are processed in subsequent loop iterations, where the lock is already active.

### 2. Allow Ctrl+C through the lock and add Drill screen to lock guard

**`src/main.rs` — `handle_key()`**

Move the Ctrl+C quit handler ABOVE the input lock guard so it always works, even during lockout. Then add `AppScreen::Drill` to the lock guard:

```rust
// Ctrl+C always quits, even during input lock
if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
    app.should_quit = true;
    return;
}

// Briefly block all input right after a drill completes to avoid accidental
// popup dismissal or continuation from trailing keystrokes.
if app.post_drill_input_lock_remaining_ms().is_some()
    && (!app.milestone_queue.is_empty()
        || app.screen == AppScreen::DrillResult
        || app.screen == AppScreen::Drill)
{
    return;
}
```

This is a behavior change for the existing DrillResult/milestone lock too: previously Ctrl+C was blocked during the 800ms window, now it passes through. All other keys remain blocked. The 800ms window is short enough that blocking everything else is not disruptive.

### 3. Render lock overlay on the drill screen

**`src/main.rs` — end of `render_drill()`**

After all existing drill UI is rendered, if the lock is active, draw a small centered popup overlay on top of the typing area:

- Check `app.post_drill_input_lock_remaining_ms()` — if `None`, skip overlay entirely
- **Size**: 3 rows tall (top border + message + bottom border), width = message length + 4 (border + padding), centered within the full `area` rect
- **Clear** the overlay rect with `ratatui::widgets::Clear`
- **Block**: `Block::bordered()` with `colors.accent()` border style and `colors.bg()` background — same pattern as `render_milestone_overlay`
- **Message**: `"Keys re-enabled in {ms}ms"` as a `Paragraph` with `colors.text_pending()` style — matches the milestone overlay footer color
- Render inside the block's `inner()` area

This overlay is intentionally small (single bordered line) since the drill content should remain visible behind it and it only appears for ≤800ms.

**Countdown repainting**: The event loop (`run_app()`) uses `EventHandler::new(Duration::from_millis(100))` which sends `AppEvent::Tick` every 100ms when idle. Each tick triggers `terminal.draw()`, which re-renders the drill screen. `post_drill_input_lock_remaining_ms()` recomputes the remaining time from `Instant::now()` on each call, so the countdown value updates every ~100ms without any additional machinery.

### 4. Tests

**`src/app.rs`** — add to the existing `#[cfg(test)] mod tests`:

1. **`adaptive_auto_continue_arms_input_lock`**: Create `App`, verify it starts in adaptive mode with a drill. Simulate completing the drill by calling `finish_drill()` (set up drill state as complete first). Assert `post_drill_input_lock_remaining_ms().is_some()` and `screen == AppScreen::Drill` after auto-continue.

2. **`adaptive_auto_continue_lock_not_armed_with_milestones`**: Same setup but push a milestone into `milestone_queue` before calling `finish_drill()`. Assert `screen == AppScreen::DrillResult` (not auto-continued) and lock is armed via the existing milestone path.

## Files to modify

- `src/app.rs` — 1-line addition in `finish_drill()` auto-continue path; 2 tests
- `src/main.rs` — extend input lock guard condition in `handle_key()`; add overlay rendering at end of `render_drill()`

## Verification

1. `cargo test` — all existing and new tests pass
2. Manual: start adaptive drill, complete it. Verify small popup appears briefly over the next drill, countdown decrements every ~100ms, then disappears and typing works normally
3. Manual: complete adaptive drill that triggers a milestone popup. Verify milestone popup still works as before (no double-lock or interference)
4. Manual: complete Code or Passage drill. Verify DrillResult screen lockout still works as before
