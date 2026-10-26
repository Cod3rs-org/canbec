//! Domain rule tests: `cargo test -p canbec-domain`.

use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::cosmetic::{CosmeticRules, SiteCosmetics};
use canbec_domain::feed_lock::FeedLock;
use canbec_domain::privacy::{keeps_cookie, strip_tracking};
use canbec_domain::slop::{SlopList, SlopScope};
use canbec_domain::trackers::TrackerList;
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
         ~skip.example##.neg\nexample.org##div:has(> .ad)\nnews.example,~old.news.example##.nag\n~x.example#@#.neg\n",
    );
    assert_eq!(rules.generic, [".ad-banner", ".neg"]); // ##sponsor removed by the generic exception
    assert_eq!(rules.len(), 5);
    assert_eq!(rules.for_host("skip.example").allow, [".neg"], "~site: everywhere but there");
    assert_eq!(rules.for_host("old.news.example").hide, [".promo"], "excluded subdomain");
    assert_eq!(rules.for_host("news.example").hide, [".promo", ".nag"]);
    assert_eq!(rules.for_host("m.news.example"), SiteCosmetics { hide: vec![".promo".into(), ".nag".into()], allow: vec![] });
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

#[test]
fn tracker_rules_become_content_blocker_json() {
    let list = TrackerList::parse(
        "! EasyPrivacy\n||stats.example^\n||pixel.example^$third-party\n||path.example/track.js\n\
         ||img.example^$image\n@@||stats.example^$script\n||bad host^\n/collect?tid=\n\
         ||mx.example^$third-party\n@@||mx.example^$script,domain=shop.example\n\
         /^regex$/\n||x.example^$csp=none\n||y.example^$domain=a.example|~b.example\n/ab^\n",
    );
    let json: serde_json::Value = serde_json::from_str(&list.to_content_blocker_json()).unwrap();
    let rules = json.as_array().unwrap();
    let find = |part: &str| rules.iter().find(|r| r["trigger"]["url-filter"].as_str().unwrap().contains(part)).unwrap().clone();
    assert_eq!(list.len(), 6); // stats, pixel, path, img, collect, mx
    assert_eq!(rules.len(), 8);

    let pixel = find(r"pixel\.example");
    assert_eq!(pixel["trigger"]["url-filter"], r"^[^:]+://+([^:/]+\.)?pixel\.example[:/]");
    assert_eq!(pixel["trigger"]["load-type"][0], "third-party");
    assert!(!pixel["trigger"]["resource-type"].as_array().unwrap().contains(&"document".into()), "pages are never blocked");
    assert_eq!(find("stats")["trigger"]["load-type"][0], "third-party", "plain host rules spare the tracker's own site");

    let path = find("track");
    assert_eq!(path["trigger"]["url-filter"], r"^[^:]+://+([^:/]+\.)?path\.example/track\.js");
    assert!(path["trigger"]["load-type"].is_null(), "path rules also block first-party analytics");
    assert_eq!(find("collect")["trigger"]["url-filter"], r"/collect\?tid=");
    assert_eq!(find("img")["trigger"]["resource-type"], serde_json::json!(["image"]));

    let site_exception = rules.iter().find(|r| r["action"]["type"] == "ignore-previous-rules" && r["trigger"]["url-filter"].as_str().unwrap().contains("mx")).unwrap();
    assert_eq!(site_exception["action"]["type"], "ignore-previous-rules");
    assert_eq!(site_exception["trigger"]["if-domain"], serde_json::json!(["*shop.example"]));
}

#[test]
fn zoom_steps_stop_at_the_ends_and_reset() {
    use canbec_domain::zoom::{step, LEVELS};
    assert_eq!(step(1.0, 1), 1.1);
    assert_eq!(step(1.0, -1), 0.9);
    assert_eq!(step(3.0, 1), 3.0);
    assert_eq!(step(0.5, -1), 0.5);
    assert_eq!(step(1.75, 0), 1.0);
    assert_eq!(step(1.2, 1), 1.5); // an odd value snaps to the next level up first
    assert!(LEVELS.contains(&1.0));
}

#[test]
fn tracker_requests_match_like_the_content_blocker() {
    let list = TrackerList::parse(
        "||stats.example^\n||ok.example^\n@@||ok.example^\n||p.example/x.js\n/collect?tid=\n\
         ||img.example^$image\n||mx.example^\n@@||mx.example^$script,domain=shop.example\n||fb.example/tr^\n|https://ad.example/pix|\n",
    );
    let blocks = |url: &str, page: &str, ty: &str| list.blocks(url, Some(page), ty);
    assert!(blocks("https://stats.example/a.js", "news.example", "script"));
    assert!(blocks("https://eu.stats.example/a.js", "news.example", "script"), "subdomains");
    assert!(!blocks("https://mystats.example/a.js", "news.example", "script"), "lookalike host");
    assert!(!blocks("https://stats.example/", "stats.example", "script"), "plain host rules spare the tracker's own site");
    assert!(!blocks("https://ok.example/a.js", "news.example", "script"), "global exception");
    assert!(blocks("https://cdn.p.example/x.js?v=1", "p.example", "script"), "path rules block first-party too");
    assert!(!blocks("https://p.example/y.js", "news.example", "script"));
    assert!(blocks("https://shop.example/g/COLLECT?tid=1", "shop.example", "fetch"), "anywhere in the URL, any case");
    assert!(blocks("https://img.example/1.gif", "news.example", "image") && !blocks("https://img.example/1.js", "news.example", "script"));
    assert!(blocks("https://mx.example/m.js", "news.example", "script") && !blocks("https://mx.example/m.js", "a.shop.example", "script"), "site exception");
    assert!(blocks("https://fb.example/tr?id=1", "news.example", "image") && blocks("https://fb.example/tr", "news.example", "image"), "^ is a separator or the end");
    assert!(!blocks("https://fb.example/track", "news.example", "image"));
    assert!(blocks("https://ad.example/pix", "news.example", "image") && !blocks("https://ad.example/pix2", "news.example", "image"), "| anchors");
}

#[test]
fn slop_lists_export_and_import_as_plain_text() {
    use canbec_domain::slop::{SlopList, SlopScope};
    let mut mine = SlopList::default();
    mine.tag("https://www.slop.example/a", SlopScope::Domain, 1);
    mine.tag("https://news.example/ai-article#top", SlopScope::Page, 1);
    let text = mine.export();
    assert!(text.contains("slop.example\n") && text.contains("https://news.example/ai-article\n"));

    let mut theirs = SlopList::default();
    assert_eq!(theirs.import(&format!("{text}\n! comment\nnot a host\njavascript:alert(1)\nslop.example\n"), 2), 2);
    assert!(theirs.find("https://sub.slop.example/x").is_some());
    assert!(theirs.find("https://news.example/ai-article").is_some());
    assert!(theirs.find("https://news.example/other").is_none());
}

#[test]
fn profiles_get_safe_unique_ids_and_a_cap() {
    use canbec_domain::profiles::{Profiles, DEFAULT_PROFILE, MAX_PROFILES};
    let mut p = Profiles::default();
    assert_eq!(p.active().id, DEFAULT_PROFILE);
    assert_eq!(p.add("Work / Client A").unwrap(), "work---client-a");
    assert!(p.add("work / client a").is_err(), "names are unique");
    assert_eq!(p.add("../..").unwrap(), "profile", "ids can't escape the profiles folder");
    assert!(p.add("  ").is_err());
    assert!(p.activate("profile") && !p.activate("profile") && !p.activate("nope"));
    assert_eq!(p.active().name, "../..");
    while p.list.len() < MAX_PROFILES {
        p.add(&format!("p{}", p.list.len())).unwrap();
    }
    assert!(p.add("one more").is_err());
}

#[test]
fn agents_only_get_profiles_that_allow_them() {
    use canbec_domain::profiles::Profiles;
    let mut p = Profiles::default();
    let agent = p.add("Agent").unwrap();
    assert!(p.for_agents(None).is_none(), "off by default");
    p.set_agents(&agent, true);
    assert_eq!(p.for_agents(None).unwrap().id, agent);
    assert!(p.for_agents(Some("personal")).is_none());
}

#[test]
fn results_first_seen_after_chatgpt_are_obsolete() {
    use canbec_domain::first_seen::{is_obsolete, year};
    assert!(!is_obsolete("20110904062405"));
    assert!(!is_obsolete("20221129235959"));
    assert!(is_obsolete("20221130000000"));
    assert!(is_obsolete("20240301120000"));
    assert!(!is_obsolete("garbage") && !is_obsolete(""));
    assert_eq!(year("20240301120000"), Some("2024"));
}

#[test]
fn family_dns_is_on_only_without_other_servers() {
    use canbec_domain::family_dns::is_family;
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert!(is_family(&s(&["1.1.1.3", "1.0.0.3"])));
    assert!(!is_family(&s(&["1.1.1.3", "8.8.8.8"])));
    assert!(!is_family(&s(&[])));
}

#[test]
fn chrome_match_patterns() {
    use canbec_domain::match_pattern::MatchPattern;
    let m = |p: &str, u: &str| MatchPattern::parse(p).unwrap().matches(u);
    assert!(m("<all_urls>", "https://a.example/x?y=1"));
    assert!(m("*://*.example.com/*", "http://example.com/"));
    assert!(m("*://*.example.com/*", "https://news.example.com/a/b"));
    assert!(!m("*://*.example.com/*", "https://badexample.com/"));
    assert!(m("https://a.example/docs/*", "https://a.example/docs/intro"));
    assert!(!m("https://a.example/docs/*", "http://a.example/docs/intro"));
    assert!(m("https://*/*q=*", "https://s.example/find?q=rust"));
    assert!(!m("*://a.example/x", "https://a.example/xy"));
    let re = |p: &str| MatchPattern::parse(p).unwrap().to_regex();
    assert_eq!(re("*://*.example.com/*"), r"^https?:\/\/([^/:]+\.)?example\.com(:\d+)?\/.*$");
    assert_eq!(re("<all_urls>"), r"^https?:\/\/[^/:]+(:\d+)?\/.*$");
    for bad in ["file:///*", "https://a.*.example/*", "*://*", "nonsense"] {
        assert!(MatchPattern::parse(bad).is_none(), "{bad}");
    }
}

#[test]
fn only_real_newer_versions_count() {
    use canbec_domain::version::is_newer;
    assert!(is_newer("v0.2.0", "0.1.0"));
    assert!(is_newer("0.10.0", "0.9.9"));
    assert!(!is_newer("v0.1.0", "0.1.0"));
    assert!(!is_newer("v0.0.9", "0.1.0"));
    assert!(!is_newer("v0.2.0-beta", "0.1.0"), "pre-releases are skipped");
    assert!(!is_newer("latest", "0.1.0"));
}

#[test]
fn persona_is_stable_per_site_differs_between_sites_and_never_real() {
    use canbec_domain::persona::{lists, pick, site_seed};
    for (os, real) in [("macos", 10), ("windows", 8), ("linux", 4), ("linux", 1), ("windows", 64)] {
        let l = lists(os, real);
        assert!(!l.cores.is_empty() && !l.cores.contains(&real), "{os} {real}: {:?}", l.cores);
        assert!(!l.gpus.is_empty() && l.gpus.iter().all(|g| g.1 != "WebKit WebGL"));
    }
    assert!(lists("windows", 8).gpus[0].1.starts_with("ANGLE"), "Windows looks like Chromium");
    let l = lists("windows", 8);
    let cores = |host: &str| pick(&l.cores, Some(&8), site_seed("run1", host), 1).unwrap();
    assert_eq!(cores("a.example"), cores("a.example"), "same site, same run");
    let hosts = ["a.example", "b.example", "c.example", "d.example", "e.example", "f.example"];
    assert!(hosts.iter().any(|h| cores(h) != cores("a.example")), "sites differ");
    for seed in 0..500 {
        assert_ne!(pick(&l.screens, Some(&(1920, 1080)), seed, 3), Some((1920, 1080)));
    }
    assert_eq!(pick(&[5], Some(&5), 1, 1), None, "only the real value: nothing to fake with");
    // Same answers as privacy.js's pick (computed there with node).
    let screens = [(1366, 768), (1536, 864), (1920, 1080), (1440, 900)];
    for (host, c, s) in [("a.example", 10, (1440, 900)), ("b.example", 10, (1536, 864)), ("news.example.org", 16, (1366, 768))] {
        let seed = site_seed("run1", host);
        assert_eq!(pick(&[4, 6, 10, 12, 14, 16], Some(&8), seed, 1), Some(c), "{host}");
        assert_eq!(pick(&screens, Some(&(1920, 1080)), seed, 3), Some(s), "{host}");
    }
}

#[test]
fn ai_text_hints_need_a_high_score() {
    use canbec_domain::ai_text::hint;
    assert_eq!(hint(0.5), None, "the model's 50% flags 5% of human texts");
    assert_eq!(hint(0.849), None);
    assert_eq!(hint(0.85), Some(85));
    assert_eq!(hint(0.996), Some(100));
    assert_eq!(hint(f64::NAN), None);
    assert_eq!(hint(1.5), None);
}
