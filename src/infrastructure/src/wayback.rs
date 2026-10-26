//! First Wayback Machine capture of a URL (the CDX API, no account needed).

use std::time::Duration;

/// `Ok(None)`: never archived. Run off the UI thread.
pub fn first_capture(url: &str) -> Result<Option<String>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(12))).build().into();
    let text = agent
        .get("https://web.archive.org/cdx/search/cdx")
        .query("url", url)
        .query("limit", "1")
        .query("fl", "timestamp")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    Ok(text.lines().next().map(str::trim).filter(|t| t.len() >= 8 && t.bytes().all(|b| b.is_ascii_digit())).map(str::to_owned))
}
