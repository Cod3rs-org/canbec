//! Downloaded lists, each = a small built-in seed (works offline, first run) + a cached copy of
//! a public list, refreshed weekly:
//! - adult hosts: StevenBlack porn-only hosts list;
//! - cosmetic ad rules: EasyList (`##selector` rules only; see domain::cosmetic);
//! - tracker hosts: EasyPrivacy (`||host^` rules only; see domain::trackers).

use std::path::Path;
use std::time::{Duration, SystemTime};

use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::cosmetic::CosmeticRules;
use canbec_domain::slop::SlopList;
use canbec_domain::trackers::TrackerList;

pub const ADULT_LIST_URL: &str =
    "https://raw.githubusercontent.com/StevenBlack/hosts/master/alternates/porn-only/hosts";
pub const ADS_LIST_URL: &str = "https://easylist.to/easylist/easylist.txt";
pub const TRACKERS_LIST_URL: &str = "https://easylist.to/easylist/easyprivacy.txt";
const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
const SEED: &str = include_str!("scripts/adult-seed.txt");
const ADS_SEED: &str = include_str!("scripts/ads-seed.txt");
const TRACKERS_SEED: &str = include_str!("scripts/trackers-seed.txt");

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
    download(ADULT_LIST_URL, cache, 1000, |text| HostBlocklist::parse(text).len())?;
    Ok(load_adult(cache))
}

/// Seed + cached EasyList + the user's own `ads-extra.txt` next to the cache.
pub fn load_ads(cache: &Path) -> CosmeticRules {
    let mut text = ADS_SEED.to_owned();
    for file in [cache.to_path_buf(), cache.with_file_name("ads-extra.txt")] {
        if let Ok(more) = std::fs::read_to_string(file) {
            text.push('\n');
            text.push_str(&more);
        }
    }
    CosmeticRules::parse(&text)
}

/// Downloads EasyList into `cache`. Run off the UI thread. New rules apply from the next start.
pub fn refresh_ads(cache: &Path) -> Result<(), String> {
    download(ADS_LIST_URL, cache, 1000, |text| CosmeticRules::parse(text).len())
}

/// Seed + cached EasyPrivacy.
pub fn load_trackers(cache: &Path) -> TrackerList {
    let cached = std::fs::read_to_string(cache).unwrap_or_default();
    TrackerList::parse(&format!("{TRACKERS_SEED}\n{cached}"))
}

/// Downloads EasyPrivacy into `cache`. Run off the UI thread. New rules apply from the next start.
pub fn refresh_trackers(cache: &Path) -> Result<(), String> {
    download(TRACKERS_LIST_URL, cache, 1000, |text| TrackerList::parse(text).len())
}

/// `<config>/slop-lists.txt`: https URLs of shared slop lists, one per line (`#` comments).
pub fn slop_subscriptions(config: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(config.join("slop-lists.txt")).unwrap_or_default();
    text.lines().map(str::trim).filter(|l| l.starts_with("https://")).map(str::to_owned).collect()
}

fn slop_cache(config: &Path, url: &str) -> std::path::PathBuf {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hash = DefaultHasher::new();
    url.hash(&mut hash);
    config.join("blocklists").join(format!("slop-{:x}.txt", hash.finish()))
}

/// All subscribed slop lists from their cached copies (read-only for the user).
pub fn load_shared_slop(config: &Path) -> SlopList {
    let mut list = SlopList::default();
    for url in slop_subscriptions(config) {
        if let Ok(text) = std::fs::read_to_string(slop_cache(config, &url)) {
            list.import(&text, 0);
        }
    }
    list
}

/// Downloads subscribed slop lists older than a week. Run off the UI thread.
pub fn refresh_shared_slop(config: &Path) -> Result<SlopList, String> {
    for url in slop_subscriptions(config) {
        let cache = slop_cache(config, &url);
        if needs_refresh(&cache) {
            download(&url, &cache, 1, |text| SlopList::default().import(text, 0))?;
        }
    }
    Ok(load_shared_slop(config))
}

/// Fetches `url` and saves it to `cache` only if it parses into at least `min` entries.
/// Broken downloads (captive portals, error pages) are refused instead of weakening a list.
fn download(url: &str, cache: &Path, min: usize, entries: impl Fn(&str) -> usize) -> Result<(), String> {
    let text = ureq::get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(16 * 1024 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let n = entries(&text);
    if n < min {
        return Err(format!("downloaded list too small ({n} entries)"));
    }
    if let Some(dir) = cache.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(cache, &text).map_err(|e| e.to_string())
}
