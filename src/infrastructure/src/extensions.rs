//! Unpacked Chrome extensions (MV3 subset) from `<config>/extensions/<name>/manifest.json`:
//! `content_scripts` (`matches`, `exclude_matches`, `js`, `css`, `run_at`, `all_frames`) and
//! `chrome.storage.local`/`.sync`, saved in `<config>/extension-storage/<name>.json`.
//! Everything else in the manifest is ignored (see docs/research/06-extensibility.md).

use std::path::Path;

use canbec_domain::match_pattern::MatchPattern;
use serde_json::{json, Map, Value};

pub const RUNTIME: &str = include_str!("scripts/extensions.js");
/// Per extension; chrome.storage.local's own quota is 10 MB.
pub const STORAGE_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Extension {
    /// The folder name: letters, digits, `-`, `_`.
    pub id: String,
    pub name: String,
    /// Ready-to-run `__canbecExt(...)` calls, one per content_scripts entry.
    calls: Vec<String>,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Every extension folder with a readable manifest. Broken ones are skipped with a message.
pub fn load(config: &Path) -> Vec<Extension> {
    let Ok(entries) = std::fs::read_dir(config.join("extensions")) else { return Vec::new() };
    let mut found: Vec<Extension> = entries
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let ext = valid_id(&id).then(|| load_one(&e.path(), &id)).flatten();
            if ext.is_none() && e.path().is_dir() {
                eprintln!("canbec: skipped extension folder {id} (bad name or manifest)");
            }
            ext
        })
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

fn load_one(dir: &Path, id: &str) -> Option<Extension> {
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).ok()?).ok()?;
    // Files stay inside the extension folder.
    let read = |file: &Value| -> Option<String> {
        let file = file.as_str()?;
        (!file.split(['/', '\\']).any(|p| p == "..")).then(|| std::fs::read_to_string(dir.join(file)).ok()).flatten()
    };
    let regexes = |key: &Value| -> Vec<String> {
        key.as_array().into_iter().flatten().filter_map(|p| MatchPattern::parse(p.as_str()?)).map(|p| p.to_regex()).collect()
    };
    let calls = manifest["content_scripts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let matches = regexes(&entry["matches"]);
            if matches.is_empty() {
                return None;
            }
            let js: Vec<String> = entry["js"].as_array().into_iter().flatten().filter_map(read).collect();
            let css: String = entry["css"].as_array().into_iter().flatten().filter_map(read).collect::<Vec<_>>().join("\n");
            let settings = json!({
                "matches": matches,
                "exclude": regexes(&entry["exclude_matches"]),
                "css": css,
                "run_at": entry["run_at"].as_str().unwrap_or("document_idle"),
                "all_frames": entry["all_frames"].as_bool().unwrap_or(false),
            });
            // All files of one entry share a scope, as in Chrome's isolated world.
            Some(format!("__canbecExt({}, {settings}, function (chrome) {{ const browser = chrome;\n{}\n}});", json!(id), js.join(";\n")))
        })
        .collect::<Vec<_>>();
    let name = manifest["name"].as_str().filter(|n| !n.starts_with("__MSG_")).unwrap_or(id).to_owned();
    Some(Extension { id: id.to_owned(), name, calls })
}

/// One init script for every page and iframe: the runtime, then each extension's scripts.
/// Empty when nothing is installed.
pub fn script(extensions: &[Extension], storage: &Map<String, Value>) -> String {
    if extensions.is_empty() {
        return String::new();
    }
    let runtime = RUNTIME.replace("\"__CANBEC_EXT_STORAGE__\"", &Value::Object(storage.clone()).to_string());
    let calls: Vec<&str> = extensions.iter().flat_map(|e| e.calls.iter().map(String::as_str)).collect();
    format!("{runtime}\n{}", calls.join("\n"))
}

pub fn load_storage(config: &Path, extensions: &[Extension]) -> Map<String, Value> {
    extensions
        .iter()
        .filter_map(|e| {
            let text = std::fs::read_to_string(config.join("extension-storage").join(format!("{}.json", e.id))).ok()?;
            Some((e.id.clone(), serde_json::from_str::<Value>(&text).ok().filter(Value::is_object)?))
        })
        .collect()
}

/// Saves a page's `extStorageSet`. Pages can send these too, so only known extensions, only
/// objects, and at most [`STORAGE_LIMIT`].
pub fn save_storage(config: &Path, extensions: &[Extension], storage: &mut Map<String, Value>, id: &str, items: &Value) -> bool {
    let text = items.to_string();
    if !items.is_object() || text.len() > STORAGE_LIMIT || !extensions.iter().any(|e| e.id == id) {
        return false;
    }
    storage.insert(id.to_owned(), items.clone());
    let dir = config.join("extension-storage");
    std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(format!("{id}.json")), text)).is_ok()
}
