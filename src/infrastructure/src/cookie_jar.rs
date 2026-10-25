//! "Stay logged in" without a persistent cookie store. The content webview runs in incognito
//! mode, so every start is a brand-new browser for trackers (cookies, storage, cache: all gone).
//! Only cookies of sites the user chose to stay logged in on are saved here and put back.

use std::path::Path;

use wry::cookie::Cookie;

/// Saves the live cookies that `keep(domain)` allows. Returns how many were saved.
pub fn save(view: &wry::WebView, path: &Path, keep: impl Fn(&str) -> bool) -> Result<usize, String> {
    let cookies = view.cookies().map_err(|e| e.to_string())?;
    let lines: Vec<String> =
        cookies.iter().filter(|c| c.domain().is_some_and(&keep)).map(|c| c.to_string()).collect();
    write_private(path, &serde_json::to_vec(&lines).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(lines.len())
}

/// Puts saved cookies back into a fresh webview. Call before the first page loads.
pub fn restore(view: &wry::WebView, path: &Path) -> usize {
    let lines: Vec<String> = std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    lines.iter().filter_map(|l| Cookie::parse(l.as_str()).ok()).filter(|c| view.set_cookie(c).is_ok()).count()
}

/// Atomic write, readable only by the user (login cookies are as good as passwords).
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(&tmp)?.write_all(bytes)?;
    std::fs::rename(tmp, path)
}
