//! Adapter tests (no network, no window): `cargo test -p canbec-infrastructure`.

use canbec_application::ports::StateStore;
use canbec_application::Snapshot;
use canbec_domain::slop::SlopScope;
use canbec_infrastructure::blocklists::{load_adult, load_ads, load_trackers, needs_refresh};
use canbec_infrastructure::store::JsonFileStore;
use canbec_infrastructure::{content_script, privacy_script, random_token, CONTENT_SCRIPT};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("canbec-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn store_round_trips_and_leaves_no_temp_file() {
    let dir = temp_dir("store");
    let store = JsonFileStore::new(dir.join("state.json"));
    assert!(store.load().is_none());

    let mut snap = Snapshot::default();
    snap.slop.tag("https://slop.example/", SlopScope::Domain, 1);
    store.save(&snap);
    assert_eq!(store.load().unwrap().slop.tags.len(), 1);
    assert!(!dir.join("state.tmp").exists());
}

#[test]
fn corrupt_state_is_moved_aside_not_lost() {
    let dir = temp_dir("corrupt");
    std::fs::write(dir.join("state.json"), "{ not json").unwrap();
    assert!(JsonFileStore::new(dir.join("state.json")).load().is_none());
    assert_eq!(std::fs::read_to_string(dir.join("state.broken.json")).unwrap(), "{ not json");
}

#[test]
fn adult_seed_blocks_offline_and_cache_extends_it() {
    let dir = temp_dir("adult");
    let cache = dir.join("adult.txt");
    let list = load_adult(&cache);
    assert!(list.is_blocked("www.pornhub.com"));
    assert!(needs_refresh(&cache));

    std::fs::write(&cache, "0.0.0.0 cached-only.example\n").unwrap();
    assert!(load_adult(&cache).is_blocked("cached-only.example"));
    assert!(!needs_refresh(&cache));
}

#[test]
fn ad_seed_works_offline_and_user_extras_are_added() {
    let dir = temp_dir("ads");
    let cache = dir.join("ads.txt");
    assert!(load_ads(&cache).generic.iter().any(|s| s == "ins.adsbygoogle"));
    std::fs::write(dir.join("ads-extra.txt"), "##.my-own-ad\nnews.example##.promo\n").unwrap();
    let rules = load_ads(&cache);
    assert!(rules.generic.iter().any(|s| s == ".my-own-ad"));
    assert_eq!(rules.for_host("news.example").hide, [".promo"]);
}

#[test]
fn tracker_seed_works_offline() {
    let list = load_trackers(&temp_dir("trackers").join("trackers.txt"));
    assert!(list.len() >= 10);
    assert!(list.to_content_blocker_json().contains(r"google-analytics\\.com"));
}

#[test]
fn content_script_exposes_the_hooks_the_engine_calls() {
    for hook in ["window.__canbec", "flags(slop, feedsLocked)", "ads(hide, allow)", "adsHidden", "aiImages", "trainedAlgorithmicMedia", "canbec-feeds-locked"] {
        assert!(CONTENT_SCRIPT.contains(hook), "content script lost `{hook}`");
    }
}

#[test]
fn shortcut_token_is_random_and_filled_in() {
    let (a, b) = (random_token(), random_token());
    assert_ne!(a, b);
    assert_eq!(a.len(), 32);
    let script = content_script(&a, &[".ad-banner".into()], "");
    assert!(script.contains(&a) && !script.contains("__CANBEC_KEY_TOKEN__"));
    assert!(script.contains(r#"const GENERIC_ADS = [".ad-banner"];"#));
}

#[test]
fn user_feed_rules_are_parsed_and_filled_in() {
    let text = "! my feeds\nexample.com,www.news.example##.stream\nbad.example##x { color: red }\nnot a rule";
    let rules = canbec_infrastructure::feed_rules(text);
    assert_eq!(rules.len(), 2);
    assert_eq!(rules["news.example"], vec![".stream"]);
    let script = content_script("t", &[], text);
    assert!(script.contains(r#"Object.entries({"example.com":[".stream"],"news.example":[".stream"]})"#));
    assert!(!script.contains("__CANBEC_"));
}

#[test]
fn privacy_script_gets_seed_and_shared_tracking_list() {
    let script = privacy_script("seed123");
    assert!(script.contains(r#"const FP_SEED = "seed123";"#));
    assert!(script.contains(r#""fbclid""#) && !script.contains("__CANBEC_"));
}

mod icon {
    use super::temp_dir;
    use canbec_infrastructure::icon::{load_icon, BUILT_IN, DEFAULT_ICON};

    #[test]
    fn default_is_sprout_and_all_built_ins_decode() {
        let icon = load_icon(&temp_dir("icon-default"));
        assert_eq!((icon.source.as_str(), icon.width, icon.height), (DEFAULT_ICON, 256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        for (name, _, _) in BUILT_IN {
            let dir = temp_dir(&format!("icon-{name}"));
            std::fs::write(dir.join("icon"), format!("{name}\n")).unwrap();
            assert_eq!(load_icon(&dir).source, *name);
        }
    }

    #[test]
    fn custom_png_path_relative_to_config_dir() {
        let dir = temp_dir("icon-custom");
        let (_, _, png) = BUILT_IN.iter().find(|(n, _, _)| *n == "spoiler").unwrap();
        std::fs::write(dir.join("mine.png"), png).unwrap();
        std::fs::write(dir.join("icon"), "mine.png").unwrap();
        let icon = load_icon(&dir);
        assert!(icon.source.ends_with("mine.png"));
        assert_eq!(icon.png, *png);
    }

    #[test]
    fn unknown_name_or_broken_png_falls_back_to_default() {
        let dir = temp_dir("icon-bad");
        std::fs::write(dir.join("icon"), "no-such-icon").unwrap();
        assert_eq!(load_icon(&dir).source, DEFAULT_ICON);
        std::fs::write(dir.join("broken.png"), b"not a png").unwrap();
        std::fs::write(dir.join("icon"), "broken.png").unwrap();
        assert_eq!(load_icon(&dir).source, DEFAULT_ICON);
    }
}

mod mcp {
    use canbec_infrastructure::mcp::{handle_line, search_results, tool_result, Call, Incoming};
    use serde_json::{json, Value};

    fn reply(line: &str) -> Value {
        match handle_line(line) {
            Incoming::Reply(s) => serde_json::from_str(&s).unwrap(),
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[test]
    fn handshake_and_tool_list() {
        let init = reply(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}}"#);
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "canbec");
        assert_eq!(handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#), Incoming::Ignore);
        let names: Vec<_> = reply(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)["result"]["tools"]
            .as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_owned()).collect();
        assert_eq!(names, ["search", "open", "read", "list_tabs"]);
        assert_eq!(reply(r#"{"jsonrpc":"2.0","id":3,"method":"nope"}"#)["error"]["code"], -32601);
        assert_eq!(reply("not json")["error"]["code"], -32700);
    }

    #[test]
    fn tool_calls_become_browser_calls() {
        let call = handle_line(r#"{"jsonrpc":"2.0","id":"a","method":"tools/call","params":{"name":"open","arguments":{"url":" https://x.example "}}}"#);
        assert_eq!(call, Incoming::Call { id: json!("a"), call: Call::Open { url: "https://x.example".into() } });
        assert_eq!(reply(r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"search","arguments":{}}}"#)["error"]["code"], -32602);
        let done: Value = serde_json::from_str(&tool_result(&json!(5), "hi", false)).unwrap();
        assert_eq!(done["result"]["content"][0]["text"], "hi");
    }

    #[test]
    fn search_results_come_from_duckduckgo_redirect_links() {
        let links = vec![
            ("Rust".to_owned(), "https://duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F&rut=1".to_owned()),
            ("rust-lang.org".to_owned(), "https://duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.rust-lang.org%2F".to_owned()),
            ("Settings".to_owned(), "https://duckduckgo.com/settings".to_owned()),
            ("The Book".to_owned(), "https://doc.rust-lang.org/book/".to_owned()),
            ("doc.rust-lang.org/book/".to_owned(), "https://doc.rust-lang.org/book/".to_owned()),
        ];
        let titles: Vec<_> = search_results(&links).into_iter().map(|(t, _)| t).collect();
        assert_eq!(titles, ["Rust", "The Book"]);
    }
}

#[test]
fn sealed_logins_open_only_with_their_key() {
    use canbec_infrastructure::secret::{open, seal};
    let (key, other) = ([7u8; 32], [8u8; 32]);
    let sealed = seal(&key, br#"["sid=1"]"#).unwrap();
    assert!(!sealed.windows(5).any(|w| w == b"sid=1"), "no plain text in the file");
    assert_eq!(open(&key, &sealed).unwrap(), br#"["sid=1"]"#);
    assert!(open(&other, &sealed).is_none());
    let mut broken = sealed.clone();
    *broken.last_mut().unwrap() ^= 1;
    assert!(open(&key, &broken).is_none());
    assert_ne!(seal(&key, b"x").unwrap(), seal(&key, b"x").unwrap(), "fresh nonce every time");
}

#[test]
fn update_downloads_must_match_the_release_checksum() {
    use canbec_infrastructure::updater::checksum_ok;
    // sha256("hello")
    let sums = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824  canbec-0.2.0-macos-arm64.zip\n";
    assert!(checksum_ok(sums, "canbec-0.2.0-macos-arm64.zip", b"hello"));
    assert!(!checksum_ok(sums, "canbec-0.2.0-macos-arm64.zip", b"hellO"));
    assert!(!checksum_ok(sums, "other.zip", b"hello"));
}

#[test]
fn extensions_load_content_scripts_and_guard_storage() {
    use canbec_infrastructure::extensions::{load, load_storage, save_storage, script, STORAGE_LIMIT};
    let dir = temp_dir("extensions");
    let ext = dir.join("extensions").join("hello-ext");
    std::fs::create_dir_all(&ext).unwrap();
    std::fs::write(
        ext.join("manifest.json"),
        r#"{"manifest_version":3,"name":"Hello","content_scripts":[
            {"matches":["*://*.example.com/*","file:///*"],"js":["a.js","../secret.js"],"css":["s.css"],"run_at":"document_end"},
            {"matches":["nonsense"],"js":["a.js"]}]}"#,
    )
    .unwrap();
    std::fs::write(ext.join("a.js"), "chrome.storage.local.set({hi: 1});").unwrap();
    std::fs::write(ext.join("s.css"), "body{color:red}").unwrap();
    std::fs::write(dir.join("extensions").join("secret.js"), "LEAKED").unwrap();
    std::fs::create_dir_all(dir.join("extensions").join("bad name")).unwrap();

    let exts = load(&dir);
    assert_eq!(exts.len(), 1);
    assert_eq!((exts[0].id.as_str(), exts[0].name.as_str()), ("hello-ext", "Hello"));
    let js = script(&exts, &Default::default());
    assert!(js.contains("chrome.storage.local.set({hi: 1});") && js.contains("body{color:red}") && js.contains("document_end"));
    assert!(js.contains(r"example\\.com") && !js.contains("LEAKED") && !js.contains("__CANBEC_"));
    assert_eq!(js.matches("__canbecExt(\"hello-ext\"").count(), 1, "entry without valid matches is dropped");
    assert!(script(&[], &Default::default()).is_empty());

    let mut storage = load_storage(&dir, &exts);
    assert!(save_storage(&dir, &exts, &mut storage, "hello-ext", &serde_json::json!({"hi": 1})));
    assert!(!save_storage(&dir, &exts, &mut storage, "other", &serde_json::json!({})), "unknown extension");
    assert!(!save_storage(&dir, &exts, &mut storage, "hello-ext", &serde_json::json!([1])), "not an object");
    let big = serde_json::json!({ "x": "a".repeat(STORAGE_LIMIT) });
    assert!(!save_storage(&dir, &exts, &mut storage, "hello-ext", &big), "over the cap");
    assert_eq!(load_storage(&dir, &exts)["hello-ext"]["hi"], 1);
}

#[test]
fn ported_example_extension_loads() {
    let dir = temp_dir("ported-example");
    let ext = dir.join("extensions").join("reading-time");
    std::fs::create_dir_all(&ext).unwrap();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/extensions/reading-time/canbec");
    for file in ["manifest.json", "content.js", "badge.css"] {
        std::fs::copy(src.join(file), ext.join(file)).unwrap();
    }
    let exts = canbec_infrastructure::extensions::load(&dir);
    let js = canbec_infrastructure::extensions::script(&exts, &Default::default());
    assert_eq!(exts[0].name, "Reading time");
    assert!(js.contains("min read") && js.contains(".reading-time") && !js.contains("sendMessage"));
}
