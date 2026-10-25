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
    for hook in ["window.__canbec", "flags(slop, feedsLocked)", "ads(hide, allow)", "adsHidden", "canbec-feeds-locked"] {
        assert!(CONTENT_SCRIPT.contains(hook), "content script lost `{hook}`");
    }
}

#[test]
fn shortcut_token_is_random_and_filled_in() {
    let (a, b) = (random_token(), random_token());
    assert_ne!(a, b);
    assert_eq!(a.len(), 32);
    let script = content_script(&a, &[".ad-banner".into()]);
    assert!(script.contains(&a) && !script.contains("__CANBEC_KEY_TOKEN__"));
    assert!(script.contains(r#"const GENERIC_ADS = [".ad-banner"];"#));
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
