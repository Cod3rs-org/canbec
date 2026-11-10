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

/// Downloads every file that's missing or doesn't match its checksum; `progress` gets the percent
/// done as bytes arrive. A broken download keeps its `.part` file, and trying again continues
/// from there; `installed` only counts complete files, and `remove` clears partial ones too.
pub fn install(config: &Path, progress: impl Fn(u8)) -> Result<(), String> {
    fetch_all(&dir(config), progress)
}

fn fetch_all(dir: &Path, progress: impl Fn(u8)) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let total: u64 = FILES.iter().map(|f| f.3).sum();
    let (mut done, mut shown) = (0, 0);
    for (name, base, sha, size) in FILES {
        let path = dir.join(name);
        if !file_is(&path, sha) {
            download(&format!("{base}/{name}"), &path, size, sha, |got| {
                let pct = ((done + got) * 100 / total) as u8;
                if pct != shown {
                    shown = pct;
                    progress(pct);
                }
            })
            .map_err(|e| format!("{name}: {e}"))?;
        }
        done += size;
    }
    progress(100);
    Ok(())
}

/// Downloads `url` to `path` (`size` bytes, checked against `sha`), streaming to `path.part`.
/// A `.part` left by an earlier try is continued with an HTTP range request; a server that
/// ignores ranges sends everything again, and a part that turns out wrong is deleted.
/// `progress` gets the bytes of this file so far.
pub fn download(url: &str, path: &Path, size: u64, sha: &str, mut progress: impl FnMut(u64)) -> Result<(), String> {
    use std::io::{Read, Write};
    let part = path.with_extension("part");
    let have = part.metadata().map(|m| m.len()).unwrap_or(0).min(size);
    if have == size && file_is(&part, sha) {
        return std::fs::rename(&part, path).map_err(|e| e.to_string()); // finished last time
    }
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(600))).build().into();
    let mut request = agent.get(url);
    if have > 0 {
        request = request.header("Range", format!("bytes={have}-"));
    }
    let mut response = request.call().map_err(|e| e.to_string())?;
    let resumed = have > 0 && response.status() == 206;
    let mut out = std::fs::OpenOptions::new().create(true).append(resumed).write(true).truncate(!resumed).open(&part).map_err(|e| e.to_string())?;
    let mut got = if resumed { have } else { 0 };
    let mut reader = response.body_mut().with_config().limit(size - got + 1).reader();
    let mut chunk = vec![0; 1 << 20];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        out.write_all(&chunk[..n]).map_err(|e| e.to_string())?;
        got += n as u64;
        progress(got);
    }
    drop(out);
    if !file_is(&part, sha) {
        let _ = std::fs::remove_file(&part); // wrong bytes: the next try starts clean
        return Err("doesn't match its checksum".into());
    }
    std::fs::rename(&part, path).map_err(|e| e.to_string())
}

/// Whether the file at `path` has the SHA-256 `hex` (read in chunks, not all at once).
fn file_is(path: &Path, hex: &str) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buf = vec![0; 1 << 20];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => ctx.update(&buf[..n]),
            Err(_) => return false,
        }
    }
    ctx.finish().as_ref().iter().map(|b| format!("{b:02x}")).collect::<String>() == hex
}

pub fn remove(config: &Path) {
    let _ = std::fs::remove_dir_all(dir(config));
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
