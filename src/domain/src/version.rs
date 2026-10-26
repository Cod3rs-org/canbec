//! Release versions (`v0.2.0`, `0.10.1`): is a release newer than the running build?

/// `None` for anything that isn't `[v]MAJOR.MINOR.PATCH` (pre-releases are ignored on purpose).
pub fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u64>().ok());
    let v = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(v)
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((parse(candidate), parse(current)), (Some(a), Some(b)) if a > b)
}
