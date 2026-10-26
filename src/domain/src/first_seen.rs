//! "Obsolete" search results: pages that first appeared on the web after ChatGPT's launch
//! (2022-11-30). Dated by their first Wayback Machine capture (docs/research/02).

/// ChatGPT launch day, as a Wayback timestamp prefix (YYYYMMDD).
pub const AI_ERA_START: &str = "20221130";

/// `timestamp` is a Wayback capture time (`YYYYMMDDhhmmss`).
pub fn is_obsolete(timestamp: &str) -> bool {
    timestamp.len() >= 8 && timestamp.as_bytes()[..8].iter().all(u8::is_ascii_digit) && &timestamp[..8] >= AI_ERA_START
}

/// Year of a Wayback timestamp, for the badge.
pub fn year(timestamp: &str) -> Option<&str> {
    timestamp.get(..4).filter(|y| y.bytes().all(|b| b.is_ascii_digit()))
}
