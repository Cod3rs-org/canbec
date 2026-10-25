//! Adult-content blocklist: a small built-in seed (works offline, first run) plus the
//! StevenBlack porn-only hosts list, cached on disk and refreshed weekly.

use std::path::Path;
use std::time::{Duration, SystemTime};

use canbec_domain::blocklist::HostBlocklist;

pub const ADULT_LIST_URL: &str =
    "https://raw.githubusercontent.com/StevenBlack/hosts/master/alternates/porn-only/hosts";
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
const SEED: &str = include_str!("scripts/adult-seed.txt");

/// Seed + cached list (if any). Never fails: blocking must work offline.
pub fn load_adult(cache: &Path) -> HostBlocklist {
    let mut list = HostBlocklist::parse(SEED);
    if let Ok(text) = std::fs::read_to_string(cache) {
        list.extend(&text);
    }
    list
}

pub fn needs_refresh(cache: &Path) -> bool {
    let age = std::fs::metadata(cache).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok());
    age.is_none_or(|age| age > MAX_AGE)
}

/// Downloads the full list into `cache`. Run off the UI thread.
pub fn refresh_adult(cache: &Path) -> Result<HostBlocklist, String> {
    let text = ureq::get(ADULT_LIST_URL)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let parsed = HostBlocklist::parse(&text);
    // Refuse obviously broken downloads (captive portals, error pages) instead of weakening the block.
    if parsed.len() < 1000 {
        return Err(format!("downloaded list too small ({} hosts)", parsed.len()));
    }
    if let Some(dir) = cache.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(cache, &text).map_err(|e| e.to_string())?;
    Ok(load_adult(cache))
}
