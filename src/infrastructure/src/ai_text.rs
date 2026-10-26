//! Optional AI text hints (#25): the files the hidden detector page needs. Nothing is downloaded
//! until the user turns hints on in settings; then they go to `<config>/models/ai-text/`, each
//! checked against a pinned SHA-256. Turning hints off deletes the folder again.

use std::path::{Path, PathBuf};

/// The hidden page and its script, built in. Everything else comes from [`FILES`].
pub const PAGE: &str = include_str!("scripts/ai-text.html");
pub const SCRIPT: &str = include_str!("scripts/ai-text.js");

const MODEL: &str = "https://huggingface.co/Deckard618/textdet/resolve/8b677e1bd3bc6421ae710352cda13ca65502147b/ettin-68m";
const ORT: &str = "https://cdn.jsdelivr.net/npm/onnxruntime-web@1.22.0/dist";

/// (file name, where from, SHA-256, size in bytes). Ettin-68M is MIT, onnxruntime-web MIT.
pub const FILES: [(&str, &str, &str, u64); 5] = [
    ("model.onnx", MODEL, "d720129864ed3ecb7be5de85acf33d2fd62e5d4f8351de0e4cd1310de03adfe7", 77_291_480),
    ("tokenizer.json", MODEL, "c24b1448c7b6762d0e4bf1c0b7b1e74a360039228f9ecf5a96a5fc789c07f49b", 3_579_416),
    ("ort.wasm.min.js", ORT, "65e09376df69107e881b5c34d2d37aed333a366b6d941073ec518168e269b87d", 48_327),
    ("ort-wasm-simd-threaded.mjs", ORT, "30dd851d9c00622940500f71ddd2ff8820c5cb65270816080175b958705385a8", 20_856),
    ("ort-wasm-simd-threaded.wasm", ORT, "71aef04959c5c1b6de461b6538e2058e306610034a85aad2742d0c7fd4533fe4", 11_210_254),
];

pub fn dir(config: &Path) -> PathBuf {
    config.join("models").join("ai-text")
}

pub fn installed(config: &Path) -> bool {
    FILES.iter().all(|(name, _, _, size)| dir(config).join(name).metadata().is_ok_and(|m| m.len() == *size))
}

/// Downloads every missing file; `progress` gets the percent done after each one.
/// ponytail: no resume; a broken download starts that file over next time.
pub fn install(config: &Path, progress: impl Fn(u8)) -> Result<(), String> {
    let dir = dir(config);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let total: u64 = FILES.iter().map(|f| f.3).sum();
    let mut done = 0;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(600))).build().into();
    for (name, base, sha, size) in FILES {
        let path = dir.join(name);
        if !path.metadata().is_ok_and(|m| m.len() == size) {
            let bytes = agent
                .get(&format!("{base}/{name}"))
                .call()
                .map_err(|e| format!("{name}: {e}"))?
                .body_mut()
                .with_config()
                .limit(size + 1)
                .read_to_vec()
                .map_err(|e| format!("{name}: {e}"))?;
            if !sha256_is(&bytes, sha) {
                return Err(format!("{name} doesn't match its checksum"));
            }
            let part = path.with_extension("part");
            std::fs::write(&part, &bytes).and_then(|_| std::fs::rename(&part, &path)).map_err(|e| e.to_string())?;
        }
        done += size;
        progress((done * 100 / total) as u8);
    }
    Ok(())
}

pub fn remove(config: &Path) {
    let _ = std::fs::remove_dir_all(dir(config));
}

fn sha256_is(bytes: &[u8], hex: &str) -> bool {
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest.as_ref().iter().map(|b| format!("{b:02x}")).collect::<String>() == hex
}

/// A file for the hidden page, with its MIME type: the built-in page and script, or one of
/// [`FILES`]. Nothing else, so the page can't read the config folder.
pub fn file(config: &Path, path: &str) -> Option<(Vec<u8>, &'static str)> {
    let name = path.trim_start_matches('/');
    let mime = |n: &str| match n.rsplit('.').next() {
        Some("html") => "text/html",
        Some("js" | "mjs") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    };
    match name {
        "" | "index.html" => Some((PAGE.as_bytes().to_vec(), "text/html")),
        "ai-text.js" => Some((SCRIPT.as_bytes().to_vec(), "text/javascript")),
        _ if FILES.iter().any(|f| f.0 == name) => Some((std::fs::read(dir(config).join(name)).ok()?, mime(name))),
        _ => None,
    }
}
