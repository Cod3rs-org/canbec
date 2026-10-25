//! canbec infrastructure: concrete adapters for the application ports.

pub mod blocklists;
pub mod engine_wry;
pub mod icon;
pub mod store;

/// Injected into every website at document start (ads, slop border, feed blocker, shortcuts).
pub const CONTENT_SCRIPT: &str = include_str!("scripts/content.js");

/// The content script with this run's shortcut token filled in (see `scripts/content.js`).
pub fn content_script(key_token: &str) -> String {
    CONTENT_SCRIPT.replace("__CANBEC_KEY_TOKEN__", key_token)
}

/// A random per-run secret (128 bits from the std hasher's random keys; no extra crate).
pub fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || std::collections::hash_map::RandomState::new().build_hasher().finish();
    format!("{:016x}{:016x}", part(), part())
}

/// Per-user config folder: `~/.config/canbec` (Linux), `~/Library/Application Support/canbec`
/// (macOS), `%APPDATA%\canbec` (Windows). Users edit `theme.css` here.
/// `CANBEC_CONFIG_DIR` overrides it (screenshots, benchmarks, throwaway profiles).
pub fn config_dir() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("CANBEC_CONFIG_DIR") {
        return dir.into();
    }
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("canbec")
}
