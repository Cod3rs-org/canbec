//! Use-case tests with a fake engine and in-memory store: `cargo test -p canbec-application`.

use std::cell::RefCell;
use std::rc::Rc;

use canbec_application::ports::{Engine, StateStore};
use canbec_application::ui::UiCommand;
use canbec_application::{resolve_input, BrowserService, Snapshot, HOME_URL, MAX_ESSENTIALS};
use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::cosmetic::CosmeticRules;
use canbec_domain::slop::SlopScope;
use canbec_domain::{DAY, HOUR};

#[derive(Clone, Default)]
struct FakeEngine {
    log: Rc<RefCell<Vec<String>>>,
}

impl Engine for FakeEngine {
    fn navigate(&self, url: &str) {
        self.log.borrow_mut().push(format!("nav {url}"));
    }
    fn reload(&self) {
        self.log.borrow_mut().push("reload".into());
    }
    fn page_flags(&self, slop: bool, feeds_locked: bool) {
        self.log.borrow_mut().push(format!("flags slop={slop} feeds_locked={feeds_locked}"));
    }
    fn page_ads(&self, hide: &[String], allow: &[String]) {
        self.log.borrow_mut().push(format!("ads hide={hide:?} allow={allow:?}"));
    }
    fn find(&self, text: &str, backwards: bool) {
        self.log.borrow_mut().push(format!("find {text} back={backwards}"));
    }
    fn zoom(&self, level: f64) {
        self.log.borrow_mut().push(format!("zoom {level}"));
    }
}

#[derive(Clone, Default)]
struct MemStore {
    saved: Rc<RefCell<Option<Snapshot>>>,
}

impl StateStore for MemStore {
    fn load(&self) -> Option<Snapshot> {
        self.saved.borrow().clone()
    }
    fn save(&self, snapshot: &Snapshot) {
        *self.saved.borrow_mut() = Some(snapshot.clone());
    }
}

fn service() -> (BrowserService<FakeEngine, MemStore>, FakeEngine, MemStore) {
    let (engine, store) = (FakeEngine::default(), MemStore::default());
    let adult = HostBlocklist::parse("0.0.0.0 adult.example");
    (BrowserService::start(engine.clone(), store.clone(), adult, 0), engine, store)
}

fn nav(url: &str) -> UiCommand {
    UiCommand::Navigate { url: url.into() }
}

#[test]
fn first_start_opens_no_ai_home() {
    let (svc, engine, _) = service();
    assert_eq!(engine.log.borrow().as_slice(), [format!("nav {HOME_URL}")]);
    assert_eq!(svc.state(0).tabs.len(), 1);
}

#[test]
fn address_bar_input_resolution() {
    assert_eq!(resolve_input("https://example.org/x"), "https://example.org/x");
    assert_eq!(resolve_input("example.org"), "https://example.org");
    assert_eq!(resolve_input("rust borrow checker"), "https://noai.duckduckgo.com/?q=rust+borrow+checker");
}

#[test]
fn adult_sites_are_blocked_including_subdomains() {
    let (mut svc, engine, _) = service();
    svc.handle(nav("https://cdn.adult.example/video"), 1);
    assert_eq!(svc.state(1).notice.as_deref(), Some("Blocked: adult content"));
    assert!(!engine.log.borrow().iter().any(|l| l.contains("adult")));
    assert!(!svc.is_allowed("https://adult.example/"));
    assert!(svc.is_allowed("https://example.org/"));
}

#[test]
fn twenty_first_tab_is_refused_with_a_notice() {
    let (mut svc, _, _) = service();
    for _ in 0..19 {
        svc.handle(UiCommand::NewTab, 1);
    }
    assert_eq!(svc.state(1).tabs.len(), 20);
    svc.handle(UiCommand::NewTab, 1);
    let state = svc.state(1);
    assert_eq!(state.tabs.len(), 20);
    assert!(state.notice.unwrap().contains("close or archive"));
}

#[test]
fn slop_tag_flags_page_and_survives_restart() {
    let (mut svc, engine, store) = service();
    svc.handle(nav("https://slop.example/article"), 1);
    svc.handle(UiCommand::TagSlop { scope: SlopScope::Domain }, 2);
    assert!(svc.state(2).current.slop);
    assert!(engine.log.borrow().iter().any(|l| l == "flags slop=true feeds_locked=true"));

    // Restart from the saved snapshot: the tag is remembered on the next visit.
    let svc2 = BrowserService::start(FakeEngine::default(), store.clone(), HostBlocklist::default(), 3);
    assert!(svc2.state(3).current.slop);
    assert!(svc2.state(3).tabs[0].slop);
}

#[test]
fn feeds_unlock_for_at_most_six_hours() {
    let (mut svc, _, _) = service();
    assert!(svc.state(0).feed_lock.locked);
    svc.handle(UiCommand::UnlockFeeds { hours: 24 }, 0);
    assert!(!svc.state(6 * HOUR - 1).feed_lock.locked);
    assert!(svc.state(6 * HOUR).feed_lock.locked);
}

#[test]
fn idle_tabs_archive_into_folders_and_restore() {
    let (mut svc, _, _) = service();
    svc.handle(nav("https://github.com/rust-lang/rust"), 0);
    svc.handle(UiCommand::NewTab, 0); // now the GitHub tab is in the background
    assert!(svc.tick(3 * DAY));
    let state = svc.state(3 * DAY);
    assert_eq!(state.archive[0].folder, "Code");
    let id = state.archive[0].tabs[0].id;

    svc.handle(UiCommand::RestoreTab { id }, 3 * DAY);
    assert!(svc.state(3 * DAY).tabs.iter().any(|t| t.id == id && t.active));
}

#[test]
fn closing_last_tab_opens_home_instead_of_empty_window() {
    let (mut svc, _, _) = service();
    let id = svc.state(0).tabs[0].id;
    svc.handle(UiCommand::CloseTab { id }, 1);
    let state = svc.state(1);
    assert_eq!(state.tabs.len(), 1);
    assert_eq!(state.current.url, HOME_URL);
}

#[test]
fn ui_commands_parse_from_contract_json() {
    let cmd: UiCommand = serde_json::from_str(r#"{"type":"tagSlop","scope":"page"}"#).unwrap();
    assert_eq!(cmd, UiCommand::TagSlop { scope: SlopScope::Page });
    let cmd: UiCommand = serde_json::from_str(r#"{"type":"unlockFeeds","hours":3}"#).unwrap();
    assert_eq!(cmd, UiCommand::UnlockFeeds { hours: 3 });
}

#[test]
fn expired_unlock_relocks_the_open_page_on_tick() {
    let (mut svc, engine, _) = service();
    svc.handle(UiCommand::UnlockFeeds { hours: 1 }, 0);
    assert!(engine.log.borrow().last().unwrap().ends_with("feeds_locked=false"));
    assert!(!svc.tick(HOUR - 1));
    assert!(svc.tick(HOUR));
    assert!(engine.log.borrow().last().unwrap().ends_with("feeds_locked=true"));
    assert!(svc.state(HOUR).feed_lock.unlocked_until.is_none());
}

#[test]
fn pin_and_unpin_essentials_with_a_cap() {
    let (mut svc, _, _) = service();
    svc.handle(nav("https://example.org/"), 1);
    svc.handle(UiCommand::PinPage, 1);
    svc.handle(UiCommand::PinPage, 1); // same page twice: still one pin
    assert_eq!(svc.state(1).essentials.len(), 1);
    for i in 1..MAX_ESSENTIALS + 1 {
        svc.handle(nav(&format!("https://site{i}.example/")), 1);
        svc.handle(UiCommand::PinPage, 1);
    }
    let state = svc.state(1);
    assert_eq!(state.essentials.len(), MAX_ESSENTIALS);
    assert!(state.notice.unwrap().contains("unpin"));
    svc.handle(UiCommand::UnpinEssential { index: 0 }, 1);
    svc.handle(UiCommand::UnpinEssential { index: 99 }, 1); // out of range is ignored
    assert_eq!(svc.state(1).essentials.len(), MAX_ESSENTIALS - 1);
}

#[test]
fn site_ad_rules_are_sent_when_a_page_starts_loading() {
    let (mut svc, engine, _) = service();
    svc.set_ad_rules(CosmeticRules::parse("news.example##.promo\nnews.example#@#.ad"));
    svc.on_page("https://news.example/today", None, true, 1);
    assert_eq!(engine.log.borrow().last().unwrap(), r#"ads hide=[".promo"] allow=[".ad"]"#);
    svc.on_page("https://other.example/", None, true, 2); // no rules for this site: nothing sent
    assert!(!engine.log.borrow().last().unwrap().starts_with("ads"));
}

#[test]
fn typed_urls_lose_tracking_params() {
    let (mut svc, engine, _) = service();
    svc.handle(nav("https://shop.example/item?id=3&utm_source=mail&fbclid=x"), 1);
    assert_eq!(engine.log.borrow().last().unwrap(), "nav https://shop.example/item?id=3");
}

#[test]
fn stay_logged_in_is_per_site_and_can_be_forgotten() {
    let (mut svc, _, _) = service();
    svc.handle(nav("https://www.app.example/inbox"), 1);
    assert!(!svc.keeps_cookie(".app.example"));
    svc.handle(UiCommand::StayLoggedIn, 1);
    let state = svc.state(1);
    assert!(state.current.stay_logged_in);
    assert_eq!(state.stay_logged_in, ["app.example"]);
    assert!(svc.keeps_cookie(".app.example") && !svc.keeps_cookie(".tracker.example"));
    svc.handle(UiCommand::ForgetLogin { domain: "app.example".into() }, 1);
    assert!(!svc.keeps_cookie(".app.example"));
}

#[test]
fn back_and_forward_walk_each_tabs_own_history() {
    let (mut svc, engine, _) = service();
    let finish = |svc: &mut BrowserService<FakeEngine, MemStore>, url: &str| svc.on_page(url, None, false, 0);
    finish(&mut svc, HOME_URL);
    assert!(!svc.state(0).current.can_go_back);
    svc.handle(nav("https://a.example/"), 0);
    finish(&mut svc, "https://a.example/");
    svc.handle(nav("https://b.example/"), 0);
    finish(&mut svc, "https://b.example/");
    assert!(svc.state(0).current.can_go_back && !svc.state(0).current.can_go_forward);

    svc.handle(UiCommand::Back, 0);
    assert_eq!(engine.log.borrow().last().unwrap(), "nav https://a.example/");
    finish(&mut svc, "https://a.example/");
    assert!(svc.state(0).current.can_go_forward);

    // A second tab has its own (empty) history; the first keeps its place.
    svc.handle(UiCommand::NewTab, 0);
    finish(&mut svc, HOME_URL);
    assert!(!svc.state(0).current.can_go_back);
    let first = svc.state(0).tabs[0].id;
    svc.handle(UiCommand::ActivateTab { id: first }, 0);
    finish(&mut svc, "https://a.example/");
    svc.handle(UiCommand::Forward, 0);
    assert_eq!(engine.log.borrow().last().unwrap(), "nav https://b.example/");

    // Navigating from the middle drops the forward entries.
    svc.handle(UiCommand::Back, 0);
    finish(&mut svc, "https://a.example/");
    svc.handle(nav("https://c.example/"), 0);
    finish(&mut svc, "https://c.example/");
    assert!(!svc.state(0).current.can_go_forward);
}

#[test]
fn zoom_is_remembered_per_site_and_reapplied_on_load() {
    let (mut svc, engine, store) = service();
    svc.handle(nav("https://a.example/"), 0);
    svc.handle(UiCommand::Zoom { step: 1 }, 0);
    svc.handle(UiCommand::Zoom { step: 1 }, 0);
    assert_eq!(engine.log.borrow().last().unwrap(), "zoom 1.25");
    assert_eq!(svc.state(0).notice.as_deref(), Some("Zoom 125 %"));
    svc.on_page("https://b.example/", None, true, 0);
    assert!(engine.log.borrow().contains(&"zoom 1".to_string()));
    svc.on_page("https://a.example/x", None, true, 0);
    assert_eq!(engine.log.borrow().iter().rev().find(|l| l.starts_with("zoom")).unwrap(), "zoom 1.25");
    svc.handle(UiCommand::Zoom { step: 0 }, 0);
    assert!(store.saved.borrow().as_ref().unwrap().zoom.is_empty());
}

#[test]
fn open_tab_adds_a_background_tab_with_the_usual_checks() {
    let (mut svc, engine, _) = service();
    let loads = engine.log.borrow().len();
    svc.handle(UiCommand::OpenTab { url: "https://a.example/?utm_source=x".into() }, 0);
    let state = svc.state(0);
    assert_eq!(state.tabs.len(), 2);
    assert_eq!(state.tabs[1].url, "https://a.example/");
    assert!(state.tabs[0].active, "the current tab stays in front");
    assert_eq!(engine.log.borrow().len(), loads, "nothing loads until the tab is opened");
    svc.handle(UiCommand::OpenTab { url: "https://adult.example/".into() }, 0);
    assert_eq!(svc.state(0).tabs.len(), 2);
    assert_eq!(svc.state(0).notice.as_deref(), Some("Blocked: adult content"));
}
