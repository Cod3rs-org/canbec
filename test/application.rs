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
    fn back(&self) {
        self.log.borrow_mut().push("back".into());
    }
    fn forward(&self) {
        self.log.borrow_mut().push("forward".into());
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
    let before = engine.log.borrow().len();
    svc.on_page("https://other.example/", None, true, 2); // no rules for this site: nothing sent
    assert_eq!(engine.log.borrow().len(), before);
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
