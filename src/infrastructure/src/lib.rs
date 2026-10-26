//! canbec infrastructure: concrete adapters for the application ports.

pub mod blocklists;
pub mod cookie_jar;
pub mod ai_text;
pub mod engine_wry;
pub mod extensions;
pub mod icon;
pub mod mcp;
pub mod profiles;
pub mod secret;
pub mod store;
pub mod system_dns;
pub mod updater;
pub mod wayback;

/// Injected into every website at document start (ads, slop border, feed blocker, shortcuts).
pub const CONTENT_SCRIPT: &str = include_str!("scripts/content.js");

/// The content script with this run's shortcut token, the generic ad selectors and the user's
/// extra feed rules filled in.
pub fn content_script(key_token: &str, generic_ads: &[String], user_feeds: &str) -> String {
    let ads = serde_json::to_string(generic_ads).unwrap_or_else(|_| "[]".into());
    let feeds = serde_json::to_string(&feed_rules(user_feeds)).unwrap_or_else(|_| "{}".into());
    CONTENT_SCRIPT
        .replace("__CANBEC_KEY_TOKEN__", key_token)
        .replace("\"__CANBEC_GENERIC_ADS__\"", &ads)
        .replace("\"__CANBEC_USER_FEEDS__\"", &feeds)
}

/// `<config>/feeds.txt`: one `domain##selector` per line (EasyList style), `!` starts a comment.
/// Selectors with braces are dropped so a line can't break out of the feed stylesheet.
pub fn feed_rules(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut rules = std::collections::BTreeMap::<String, Vec<String>>::new();
    for line in text.lines().map(str::trim).filter(|l| !l.starts_with('!')) {
        let Some((domains, selector)) = line.split_once("##") else { continue };
        if selector.is_empty() || selector.contains(['{', '}']) {
            continue;
        }
        for domain in domains.split(',').map(|d| d.trim().trim_start_matches("www.").to_ascii_lowercase()).filter(|d| !d.is_empty()) {
            rules.entry(domain).or_default().push(selector.to_owned());
        }
    }
    rules
}

/// Injected into every page *and iframe*: fingerprint noise, tracking-parameter removal, GPC.
pub const PRIVACY_SCRIPT: &str = include_str!("scripts/privacy.js");

/// The privacy script with this run's fingerprint seed, the tracking-parameter list and the
/// fake-device lists for this OS and machine.
pub fn privacy_script(fingerprint_seed: &str) -> String {
    let params = serde_json::to_string(canbec_domain::privacy::TRACKING_PARAMS).unwrap_or_else(|_| "[]".into());
    let cores = std::thread::available_parallelism().map_or(8, |n| n.get() as u32);
    let persona = serde_json::to_string(&canbec_domain::persona::lists(std::env::consts::OS, cores)).unwrap_or_else(|_| "null".into());
    PRIVACY_SCRIPT
        .replace("__CANBEC_FP_SEED__", fingerprint_seed)
        .replace("\"__CANBEC_TRACKING_PARAMS__\"", &params)
        .replace("\"__CANBEC_PERSONA__\"", &persona)
}

/// A random per-run secret (128 bits from the std hasher's random keys; no extra crate).
pub fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || std::collections::hash_map::RandomState::new().build_hasher().finish();
    format!("{:016x}{:016x}", part(), part())
}

/// Writes `text` to the OS Downloads folder as `name` (or `name-2`, … if taken).
pub fn save_to_downloads(name: &str, text: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "txt"));
    let path = (1..100)
        .map(|i| dir.join(if i == 1 { format!("{stem}.{ext}") } else { format!("{stem}-{i}.{ext}") }))
        .find(|p| !p.exists())
        .ok_or_else(|| std::io::Error::other("too many files with that name"))?;
    std::fs::write(&path, text)?;
    Ok(path)
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
