//! canbec domain: the browser's rules, with no I/O and no engine.
//! Everything here is deterministic: time is passed in as unix seconds.

pub mod blocklist;
pub mod feed_lock;
pub mod slop;
pub mod tabs;

/// Unix time in seconds. Passed in by callers so rules stay testable.
pub type Timestamp = u64;
pub const HOUR: u64 = 3600;
pub const DAY: u64 = 24 * HOUR;

/// Lower-cased host of a URL without a leading `www.`; `None` for non-host URLs (about:, data:).
pub fn host_of(url: &str) -> Option<String> {
    let host = url::Url::parse(url).ok()?.host_str()?.to_ascii_lowercase();
    Some(host.strip_prefix("www.").map(str::to_owned).unwrap_or(host))
}

/// True when `host` is `domain` or one of its subdomains.
pub fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.strip_suffix(domain).is_some_and(|rest| rest.ends_with('.'))
}
