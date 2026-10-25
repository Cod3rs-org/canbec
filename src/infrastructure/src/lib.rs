//! canbec infrastructure: concrete adapters for the application ports.

pub mod blocklists;
pub mod engine_wry;
pub mod icon;
pub mod store;

/// Injected into every website at document start (ads, slop border, feed blocker).
pub const CONTENT_SCRIPT: &str = include_str!("scripts/content.js");

/// Per-user config folder: `~/.config/canbec` (Linux), `~/Library/Application Support/canbec`
/// (macOS), `%APPDATA%\canbec` (Windows). Users edit `theme.css` here.
/// `CANBEC_CONFIG_DIR` overrides it (screenshots, benchmarks, throwaway profiles).
pub fn config_dir() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("CANBEC_CONFIG_DIR") {
        return dir.into();
    }
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("canbec")
}
