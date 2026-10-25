//! Privacy rules: tracking parameters in URLs, and which cookies survive a restart.

use crate::host_matches;

/// Query parameters that only exist to track you across sites (ad clicks, newsletters, shares).
pub const TRACKING_PARAMS: &[&str] = &[
    "fbclid", "gclid", "dclid", "gbraid", "wbraid", "msclkid", "yclid", "ttclid", "twclid", "li_fat_id",
    "igshid", "mc_cid", "mc_eid", "_hsenc", "_hsmi", "mkt_tok", "oly_anon_id", "oly_enc_id", "vero_id",
    "srsltid", "epik", "si", "ref_src",
];

pub fn is_tracking_param(name: &str) -> bool {
    name.starts_with("utm_") || TRACKING_PARAMS.contains(&name)
}

/// The URL without tracking parameters, or `None` when there was nothing to remove.
pub fn strip_tracking(url: &str) -> Option<String> {
    let mut parsed = url::Url::parse(url).ok()?;
    let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
    let kept: Vec<_> = pairs.iter().filter(|(k, _)| !is_tracking_param(k)).collect();
    if kept.len() == pairs.len() {
        return None;
    }
    if kept.is_empty() {
        parsed.set_query(None);
    } else {
        parsed.query_pairs_mut().clear().extend_pairs(kept);
    }
    Some(parsed.into())
}

/// Cookies are wiped on every start, except for sites you chose to stay logged in on.
/// A cookie for `.example.com` is kept for `app.example.com` (logins often live on the parent).
pub fn keeps_cookie(cookie_domain: &str, stay_logged_in: &[String]) -> bool {
    let domain = cookie_domain.trim_start_matches('.').to_ascii_lowercase();
    !domain.is_empty() && stay_logged_in.iter().any(|kept| host_matches(&domain, kept) || host_matches(kept, &domain))
}
