//! Domain rule tests: `cargo test -p canbec-domain`.

use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::cosmetic::{CosmeticRules, SiteCosmetics};
use canbec_domain::feed_lock::FeedLock;
use canbec_domain::privacy::{keeps_cookie, strip_tracking};
use canbec_domain::slop::{SlopList, SlopScope};
use canbec_domain::tabs::{folder_for, TabPolicy, TabState, Tabs};
use canbec_domain::{host_of, DAY, HOUR};

#[test]
fn host_of_strips_www_and_rejects_hostless_urls() {
    assert_eq!(host_of("https://WWW.Example.org/a?b").as_deref(), Some("example.org"));
    assert_eq!(host_of("about:blank"), None);
}

#[test]
fn blocklist_parses_hosts_files_and_matches_subdomains() {
    let list = HostBlocklist::parse(
        "# comment\n0.0.0.0 adult.example\n127.0.0.1 localhost\n||ads.test^\nplain.test # trailing\n! abp comment\n",
    );
    assert_eq!(list.len(), 3);
    assert!(list.is_blocked("adult.example"));
    assert!(list.is_blocked("cdn.adult.example"));
    assert!(list.is_blocked("ADS.test"));
    assert!(!list.is_blocked("notadult.example"));
    assert!(!list.is_blocked("example"));
}

#[test]
fn feed_lock_caps_unlock_at_six_hours_and_relocks() {
    let mut lock = FeedLock::default();
    assert!(lock.is_locked(0));
    assert_eq!(lock.unlock(48, 1000), 1000 + 6 * HOUR);
    assert!(!lock.is_locked(1000 + 6 * HOUR - 1));
    assert!(lock.is_locked(1000 + 6 * HOUR));
    lock.unlock(0, 0); // clamped up to 1h
    assert_eq!(lock.unlocked_until, Some(HOUR));
}

#[test]
fn slop_tags_by_domain_or_page() {
    let mut slop = SlopList::default();
    assert!(slop.tag("https://www.slop.example/post/1", SlopScope::Domain, 1));
    assert!(slop.find("https://blog.slop.example/x").is_some());
    assert!(slop.find("https://notslop.example/").is_none());

    assert!(slop.tag("https://good.example/one#frag", SlopScope::Page, 2));
    assert!(slop.find("https://good.example/one").is_some());
    assert!(slop.find("https://good.example/two").is_none());

    assert!(!slop.tag("about:blank", SlopScope::Domain, 3));
    slop.untag("https://slop.example/");
    assert!(slop.find("https://slop.example/").is_none());
    assert_eq!(slop.tags.len(), 1);
}

#[test]
fn tab_cap_blocks_the_21st_tab_and_restore() {
    let policy = TabPolicy::default();
    let mut tabs = Tabs::default();
    for i in 0..20 {
        tabs.open(&format!("https://e{i}.example/"), &policy, 0).unwrap();
    }
    assert!(tabs.open("https://one-too-many.example/", &policy, 0).is_err());

    tabs.archive(1);
    assert_eq!(tabs.open_count(), 19);
    let extra = tabs.open("https://fits-now.example/", &policy, 0).unwrap();
    assert!(tabs.restore(1, &policy, 0).is_err(), "restore must respect the cap too");
    tabs.close(extra);
    assert!(tabs.restore(1, &policy, 0).is_ok());
    assert_eq!(tabs.get(extra).unwrap().state, TabState::Deleted, "closed tabs are kept, not removed");
}

#[test]
fn sweep_archives_idle_tabs_into_folders_then_soft_deletes() {
    let policy = TabPolicy::default();
    let mut tabs = Tabs::default();
    let gh = tabs.open("https://github.com/x/y", &policy, 0).unwrap();
    let active = tabs.open("https://example.org/", &policy, 0).unwrap();

    assert_eq!(tabs.sweep(&policy, 3 * DAY - 1), 0);
    assert_eq!(tabs.sweep(&policy, 3 * DAY), 1, "active tab is never auto-archived");
    let t = tabs.get(gh).unwrap();
    assert_eq!((t.state, t.folder.as_deref()), (TabState::Archived, Some("Code")));
    assert_eq!(tabs.get(active).unwrap().state, TabState::Open);

    tabs.sweep(&policy, 60 * DAY);
    assert_eq!(tabs.get(gh).unwrap().state, TabState::Deleted);
    assert_eq!(tabs.items.len(), 2, "never hard-deleted");
}

#[test]
fn folders_use_rules_then_host() {
    assert_eq!(folder_for("https://en.wikipedia.org/wiki/Rust"), "Reading");
    assert_eq!(folder_for("https://www.indie.blog/post"), "indie.blog");
    assert_eq!(folder_for("data:text/plain,hi"), "Other");
}

#[test]
fn cosmetic_rules_parse_generic_specific_and_exceptions() {
    let rules = CosmeticRules::parse(
        "! comment\n[Adblock Plus 2.0]\n##.ad-banner\n###sponsor\nnews.example,blog.example##.promo\n\
         shop.example#@#.ad-banner\n#@##sponsor\nsite.example##+js(noop)\nsite.example#?#div:-abp-has(.x)\n\
         ~skip.example##.neg\nexample.org##div:has(> .ad)\n",
    );
    assert_eq!(rules.generic, [".ad-banner"]); // ##sponsor removed by the generic exception
    assert_eq!(rules.len(), 3);
    assert_eq!(rules.for_host("m.news.example"), SiteCosmetics { hide: vec![".promo".into()], allow: vec![] });
    assert_eq!(rules.for_host("shop.example").allow, [".ad-banner"]);
    assert_eq!(rules.for_host("example.org").hide, ["div:has(> .ad)"]);
    assert_eq!(rules.for_host("other.example"), SiteCosmetics::default());
}

#[test]
fn tracking_params_are_stripped_and_real_ones_kept() {
    assert_eq!(strip_tracking("https://a.example/p?id=7&utm_source=x&fbclid=y").as_deref(), Some("https://a.example/p?id=7"));
    assert_eq!(strip_tracking("https://a.example/?gclid=1").as_deref(), Some("https://a.example/"));
    assert_eq!(strip_tracking("https://youtu.be/abc?si=share&t=42").as_deref(), Some("https://youtu.be/abc?t=42"));
    assert_eq!(strip_tracking("https://a.example/?q=rust"), None);
    assert_eq!(strip_tracking("about:blank"), None);
}

#[test]
fn only_stay_logged_in_sites_keep_cookies() {
    let kept = vec!["app.example.com".to_string()];
    assert!(keeps_cookie(".example.com", &kept)); // parent-domain login cookie
    assert!(keeps_cookie("app.example.com", &kept));
    assert!(keeps_cookie("eu.app.example.com", &kept));
    assert!(!keeps_cookie(".tracker.example", &kept));
    assert!(!keeps_cookie("", &kept));
    assert!(!keeps_cookie(".example.com", &[]));
}
