//! Adapter tests (no network, no window): `cargo test -p canbec-infrastructure`.

use canbec_application::ports::StateStore;
use canbec_application::Snapshot;
use canbec_domain::slop::SlopScope;
use canbec_infrastructure::blocklists::{load_adult, needs_refresh};
use canbec_infrastructure::store::JsonFileStore;
use canbec_infrastructure::CONTENT_SCRIPT;

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
fn content_script_exposes_the_hooks_the_engine_calls() {
    for hook in ["window.__canbec", "flags(slop, feedsLocked)", "adsHidden", "canbec-feeds-locked"] {
        assert!(CONTENT_SCRIPT.contains(hook), "content script lost `{hook}`");
    }
}
