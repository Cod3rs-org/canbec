//! Simple self-update from GitHub Releases. Once a day the browser asks for the latest release;
//! if it's newer, settings offer it. On macOS one click downloads the zip, checks it against the
//! release's SHA256SUMS.txt, swaps the .app and restarts. Windows/Linux open the release page.
//! ponytail: no delta updates, no signature beyond the checksum (the app isn't signed yet, #3).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use canbec_domain::version::is_newer;

const LATEST: &str = "https://api.github.com/repos/Cod3rs-org/canbec/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub page: String,
    /// This OS's package and the checksum list, when the release has them.
    pub package: Option<String>,
    pub sums: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(60))).build().into()
}

/// Package file name for this OS, as CI names them (`.github/workflows/ci.yml`).
fn package_name(version: &str) -> String {
    match std::env::consts::OS {
        "macos" => format!("canbec-{version}-macos-{}.zip", if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" }),
        "windows" => format!("canbec-{version}-windows-x64.zip"),
        _ => format!("canbec-{version}-linux-x64.tar.gz"),
    }
}

/// The latest release if it's newer than `current`. Run off the UI thread.
pub fn check(current: &str) -> Result<Option<Release>, String> {
    // CANBEC_UPDATE_FEED: a fake "latest release" JSON for testing the updater locally.
    let feed = std::env::var("CANBEC_UPDATE_FEED").unwrap_or_else(|_| LATEST.to_owned());
    let text = agent()
        .get(&feed)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let tag = json["tag_name"].as_str().unwrap_or_default();
    if !is_newer(tag, current) {
        return Ok(None);
    }
    let version = tag.trim_start_matches('v').to_owned();
    let asset = |name: &str| {
        json["assets"].as_array().into_iter().flatten().find(|a| a["name"] == name).and_then(|a| a["browser_download_url"].as_str()).map(str::to_owned)
    };
    Ok(Some(Release {
        package: asset(&package_name(&version)),
        sums: asset("SHA256SUMS.txt"),
        page: json["html_url"].as_str().unwrap_or("https://github.com/Cod3rs-org/canbec/releases").to_owned(),
        version,
    }))
}

/// Checks at most once a day (`<config>/update-check` holds the time of the last check).
pub fn due(config: &Path) -> bool {
    let stamp = config.join("update-check");
    let fresh = std::fs::metadata(&stamp).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|age| age < Duration::from_secs(24 * 3600));
    if !fresh {
        let _ = std::fs::create_dir_all(config).and_then(|_| std::fs::write(&stamp, b""));
    }
    !fresh
}

/// The `.app` this process runs from, if it is one (a `cargo run` build isn't).
pub fn current_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let bundle = exe.parent()?.parent()?.parent()?.to_path_buf();
    bundle.extension().is_some_and(|e| e == "app").then_some(bundle)
}

fn download(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    agent().get(url).call().map_err(|e| e.to_string())?.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| e.to_string())
}

/// `SHA256SUMS.txt` line for `name` matches `bytes`.
pub fn checksum_ok(sums: &str, name: &str, bytes: &[u8]) -> bool {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    let hex: String = digest.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    sums.lines().any(|l| {
        let mut parts = l.split_whitespace();
        parts.next() == Some(hex.as_str()) && parts.next().map(|n| n.trim_start_matches('*')) == Some(name)
    })
}

/// macOS: replaces the running `.app` with the release's. The old copy is removed only after
/// the new one is in place. Call [`relaunch`] afterwards.
pub fn install(release: &Release) -> Result<PathBuf, String> {
    let bundle = current_bundle().ok_or("canbec isn't running from an .app (built from source? run tools/install-macos.sh)")?;
    let (Some(package), Some(sums)) = (&release.package, &release.sums) else { return Err("this release has no package for your system".into()) };
    let zip = download(package, 200 * 1024 * 1024)?;
    let sums = String::from_utf8(download(sums, 64 * 1024)?).map_err(|e| e.to_string())?;
    if !checksum_ok(&sums, &package_name(&release.version), &zip) {
        return Err("download doesn't match its checksum".into());
    }
    let tmp = std::env::temp_dir().join(format!("canbec-update-{}", release.version));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    std::fs::write(tmp.join("canbec.zip"), &zip).map_err(|e| e.to_string())?;
    let unzip = std::process::Command::new("ditto").args(["-x", "-k"]).arg(tmp.join("canbec.zip")).arg(&tmp).status().map_err(|e| e.to_string())?;
    let fresh = tmp.join("canbec.app");
    if !unzip.success() || !fresh.join("Contents/MacOS/canbec").exists() {
        return Err("the package didn't contain canbec.app".into());
    }
    let old = bundle.with_extension("app.old");
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(&bundle, &old).map_err(|e| format!("can't replace {}: {e}", bundle.display()))?;
    let copied = std::process::Command::new("ditto").arg(&fresh).arg(&bundle).status().map_err(|e| e.to_string())?;
    if !copied.success() {
        let _ = std::fs::rename(&old, &bundle); // put the old app back
        return Err("copying the new app failed".into());
    }
    let _ = std::fs::remove_dir_all(&old);
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(bundle)
}

/// Starts the (new) app a moment after this process quits.
pub fn relaunch(bundle: &Path) {
    let _ = std::process::Command::new("/bin/sh").arg("-c").arg("sleep 1; open \"$0\"").arg(bundle).spawn();
}
