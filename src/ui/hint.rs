//! Key token constants for keyboard hint strings.
//!
//! Key names are not translatable — "ESC" is "ESC" in every language — so they
//! live here as constants rather than embedded in locale YAML.  Locale files
//! store only the translated label text (e.g. "Back").  Hints are assembled at
//! render time via [`hint`].
//!
//! Using the same constant in both the hint builder and the mouse-handler match
//! arm keeps the two in sync: changing a keybinding here updates both what the
//! footer displays and what a mouse click on that hint triggers.

// ── Navigation / back ─────────────────────────────────────────────────────────

/// ESC only — screens where `q` is reserved for another action (drill, settings,
/// keyboard explorer).
pub const K_ESC: &str = "ESC";

/// `q` or ESC — screens where both keys go back (stats, skill tree, select
/// screens, intro screens).
pub const K_Q_ESC: &str = "q/ESC";

/// `q` only — menu quit and drill-result "go to menu" (ESC also works on both,
/// but the hint intentionally shows just `q` to keep it short).
pub const K_Q: &str = "q";

// ── Universal keys ────────────────────────────────────────────────────────────
pub const K_ENTER: &str = "Enter";
pub const K_ENTER_SPACE: &str = "Enter/Space";
pub const K_TAB: &str = "Tab";
pub const K_BACKSPACE: &str = "Backspace";

// ── Menu ──────────────────────────────────────────────────────────────────────
pub const K_1_3: &str = "1-3";
pub const K_T: &str = "t";
pub const K_B: &str = "b";
pub const K_S: &str = "s";
pub const K_C: &str = "c";

// ── Dashboard / drill result ──────────────────────────────────────────────────
pub const K_C_ENTER_SPACE: &str = "c/Enter/Space";
pub const K_R: &str = "r";
pub const K_X: &str = "x";

// ── Stats ─────────────────────────────────────────────────────────────────────
pub const K_1_6: &str = "1-6";
pub const K_J_K: &str = "j/k";
pub const K_PGUP_PGDN: &str = "PgUp/PgDn";

// ── Settings ──────────────────────────────────────────────────────────────────
pub const K_ENTER_ARROWS: &str = "Enter/arrows";
pub const K_ENTER_ON_PATH: &str = "Enter on path";
pub const K_ARROW_LR: &str = "←→";

// ── Select screens ────────────────────────────────────────────────────────────
pub const K_UP_DOWN_PGUP_PGDN: &str = "Up/Down/PgUp/PgDn";

// ── Intro screens ─────────────────────────────────────────────────────────────
pub const K_UP_DOWN: &str = "Up/Down";
pub const K_LEFT_RIGHT: &str = "Left/Right";
pub const K_TYPE_BACKSPACE: &str = "Type/Backspace";

// ── Skill tree ────────────────────────────────────────────────────────────────
pub const K_UD_JK: &str = "↑↓/jk";
pub const K_SCROLL_KEYS: &str = "PgUp/PgDn or Ctrl+U/Ctrl+D";

/// Assembles a single hint entry: `"[key] label"`.
///
/// The returned `String` owns its content and can be held alongside other hint
/// strings before passing `&str` slices to `pack_hint_lines` / `hint_token_at`.
pub fn hint(key: &str, label: &str) -> String {
    format!("[{key}] {label}")
}
