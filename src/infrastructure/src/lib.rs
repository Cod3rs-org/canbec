//! canbec infrastructure: concrete adapters for the application ports.

pub mod blocklists;
pub mod engine_wry;
pub mod store;

/// Injected into every website at document start (ads, slop border, feed blocker).
pub const CONTENT_SCRIPT: &str = include_str!("scripts/content.js");

/// Per-user config folder: `~/.config/canbec` (Linux), `~/Library/Application Support/canbec`
/// (macOS), `%APPDATA%\canbec` (Windows). Users edit `theme.css` here.
pub fn config_dir() -> std::path::PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("canbec")
}
