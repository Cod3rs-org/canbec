//! Optional system-wide adult block: Cloudflare's "1.1.1.1 for Families" DNS (malware + adult).
//! The browser blocks adult sites itself; this extends it to every app on the computer.

pub const FAMILY_DNS: &[&str] = &["1.1.1.3", "1.0.0.3", "2606:4700:4700::1113", "2606:4700:4700::1003"];

/// On when the system resolves only through the family servers (one other server would be a
/// way around it).
pub fn is_family(servers: &[String]) -> bool {
    !servers.is_empty() && servers.iter().all(|s| FAMILY_DNS.contains(&s.trim()))
}
