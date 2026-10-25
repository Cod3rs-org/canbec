//! canbec infrastructure: concrete adapters for the application ports.

pub mod blocklists;
pub mod cookie_jar;
pub mod engine_wry;
pub mod icon;
pub mod store;

/// Injected into every website at document start (ads, slop border, feed blocker, shortcuts).
pub const CONTENT_SCRIPT: &str = include_str!("scripts/content.js");

/// The content script with this run's shortcut token and the generic ad selectors filled in.
pub fn content_script(key_token: &str, generic_ads: &[String]) -> String {
    let ads = serde_json::to_string(generic_ads).unwrap_or_else(|_| "[]".into());
    CONTENT_SCRIPT.replace("__CANBEC_KEY_TOKEN__", key_token).replace("\"__CANBEC_GENERIC_ADS__\"", &ads)
}

/// Injected into every page *and iframe*: fingerprint noise, tracking-parameter removal, GPC.
pub const PRIVACY_SCRIPT: &str = include_str!("scripts/privacy.js");

/// The privacy script with this run's fingerprint seed and the tracking-parameter list.
pub fn privacy_script(fingerprint_seed: &str) -> String {
    let params = serde_json::to_string(canbec_domain::privacy::TRACKING_PARAMS).unwrap_or_else(|_| "[]".into());
    PRIVACY_SCRIPT.replace("__CANBEC_FP_SEED__", fingerprint_seed).replace("\"__CANBEC_TRACKING_PARAMS__\"", &params)
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
