//! The browser's use cases. Every driving adapter (sidebar, MCP) goes through here.

use std::collections::BTreeMap;

use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::cosmetic::CosmeticRules;
use canbec_domain::feed_lock::FeedLock;
use canbec_domain::privacy::{keeps_cookie, strip_tracking};
use canbec_domain::slop::SlopList;
use canbec_domain::tabs::{TabPolicy, TabState, Tabs};
use canbec_domain::zoom;
use canbec_domain::{host_of, Timestamp, DAY};
use serde::{Deserialize, Serialize};

use crate::ports::{Engine, StateStore};
use crate::ui::*;

/// New tabs and search go to DuckDuckGo's no-AI mode until our own search exists (docs/research/02).
pub const HOME_URL: &str = "https://noai.duckduckgo.com/";

/// Essentials grid size (3 columns × 3 rows in the sidebar).
pub const MAX_ESSENTIALS: usize = 9;

/// Everything persisted between runs.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub tabs: Tabs,
    pub slop: SlopList,
    pub feed_lock: FeedLock,
    pub essentials: Vec<Essential>,
    /// Domains whose cookies survive a restart (see domain::privacy::keeps_cookie).
    #[serde(default)]
    pub stay_logged_in: Vec<String>,
    /// Zoom per host, only for sites the user zoomed (100 % isn't stored).
    #[serde(default)]
    pub zoom: BTreeMap<String, f64>,
}

pub struct BrowserService<E: Engine, S: StateStore> {
    engine: E,
    store: S,
    snapshot: Snapshot,
    policy: TabPolicy,
    adult: HostBlocklist,
    ads: CosmeticRules,
    notice: Option<String>,
    loading: bool,
    ads_hidden: u32,
    ai_images: u32,
    last_crash: Option<Timestamp>,
    /// Slop lists the user subscribed to (read-only, refreshed weekly).
    shared_slop: SlopList,
    /// Profiles for the sidebar dots (switching is done by the app: it swaps store and logins).
    workspaces: Vec<Workspace>,
}

impl<E: Engine, S: StateStore> BrowserService<E, S> {
    /// Restores the last session (or opens the home page) and loads the active tab.
    pub fn start(engine: E, store: S, adult: HostBlocklist, now: Timestamp) -> Self {
        let snapshot = store.load().unwrap_or_default();
        let mut service =
            Self { engine, store, snapshot, policy: TabPolicy::default(), adult, ads: CosmeticRules::default(), notice: None, loading: false, ads_hidden: 0, ai_images: 0, last_crash: None, shared_slop: SlopList::default(), workspaces: Vec::new() };
        service.snapshot.tabs.sweep(&service.policy, now);
        if service.snapshot.tabs.active_tab().is_none() {
            let _ = service.snapshot.tabs.open(HOME_URL, &service.policy, now);
        }
        service.load_active();
        service
    }

    /// Another profile: its own saved tabs, tags and settings. The app clears the engine's site
    /// data and puts that profile's logins back before calling this.
    pub fn switch_store(&mut self, store: S, now: Timestamp) {
        self.store.save(&self.snapshot);
        self.store = store;
        self.snapshot = self.store.load().unwrap_or_default();
        self.snapshot.tabs.sweep(&self.policy, now);
        if self.snapshot.tabs.active_tab().is_none() {
            let _ = self.snapshot.tabs.open(HOME_URL, &self.policy, now);
        }
        self.load_active();
    }

    pub fn set_workspaces(&mut self, workspaces: Vec<Workspace>) {
        self.workspaces = workspaces;
    }

    /// The engine adapter, e.g. for resizing its view.
    pub fn engine(&self) -> &E {
        &self.engine
    }

    pub fn set_adult_blocklist(&mut self, list: HostBlocklist) {
        self.adult = list;
    }

    /// Ad rules; the generic part is injected by the engine adapter, site rules go per page.
    pub fn set_ad_rules(&mut self, rules: CosmeticRules) {
        self.ads = rules;
    }

    pub fn set_shared_slop(&mut self, list: SlopList) {
        self.shared_slop = list;
    }

    /// The user's own tags as a shareable text list.
    pub fn export_slop(&self) -> String {
        self.snapshot.slop.export()
    }

    /// Adult content can't be switched off: every navigation goes through this check.
    pub fn is_allowed(&self, url: &str) -> bool {
        host_of(url).is_none_or(|host| !self.adult.is_blocked(&host))
    }

    pub fn handle(&mut self, cmd: UiCommand, now: Timestamp) {
        self.notice = None;
        let tabs = &mut self.snapshot.tabs;
        match cmd {
            UiCommand::Navigate { url } => {
                let Some(url) = self.checked(&url) else { return };
                let tabs = &mut self.snapshot.tabs;
                match tabs.active {
                    Some(id) => tabs.update(id, &url, None, now),
                    None => {
                        if let Err(e) = tabs.open(&url, &self.policy, now) {
                            self.notice = Some(e.to_string());
                        }
                    }
                }
                self.load_active();
            }
            UiCommand::OpenTab { url } => {
                let Some(url) = self.checked(&url) else { return };
                let tabs = &mut self.snapshot.tabs;
                let current = tabs.active;
                match tabs.open(&url, &self.policy, now) {
                    // Loads when the user switches to it: one engine view, so background = not loaded yet.
                    Ok(_) => tabs.active = current,
                    Err(e) => self.notice = Some(e.to_string()),
                }
            }
            UiCommand::Back | UiCommand::Forward => {
                let delta = if cmd == UiCommand::Back { -1 } else { 1 };
                if tabs.active.and_then(|id| tabs.step(id, delta)).is_some() {
                    self.load_active();
                }
            }
            UiCommand::Reload => self.engine.reload(),
            UiCommand::NewTab => match tabs.open(HOME_URL, &self.policy, now) {
                Ok(_) => self.load_active(),
                Err(e) => self.notice = Some(e.to_string()),
            },
            UiCommand::ActivateTab { id } => {
                tabs.activate(id, now);
                self.load_active();
            }
            UiCommand::CloseTab { id } | UiCommand::ArchiveTab { id } => {
                let was_active = tabs.active == Some(id);
                if matches!(cmd, UiCommand::CloseTab { .. }) {
                    tabs.close(id)
                } else {
                    tabs.archive(id)
                }
                if tabs.active.is_none() {
                    let _ = tabs.open(HOME_URL, &self.policy, now);
                }
                if was_active {
                    self.load_active();
                }
            }
            UiCommand::RestoreTab { id } => match tabs.restore(id, &self.policy, now) {
                Ok(()) => self.load_active(),
                Err(e) => self.notice = Some(e.to_string()),
            },
            UiCommand::TagSlop { scope } => {
                if let Some(url) = tabs.active_tab().map(|t| t.url.clone()) {
                    self.snapshot.slop.tag(&url, scope, now);
                }
                self.push_flags(now);
            }
            UiCommand::UntagSlop => {
                if let Some(url) = tabs.active_tab().map(|t| t.url.clone()) {
                    self.snapshot.slop.untag(&url);
                }
                self.push_flags(now);
            }
            UiCommand::UnlockFeeds { hours } => {
                self.snapshot.feed_lock.unlock(hours, now);
                self.push_flags(now);
            }
            UiCommand::RelockFeeds => {
                self.snapshot.feed_lock.relock();
                self.push_flags(now);
            }
            UiCommand::PinPage => {
                let ess = &mut self.snapshot.essentials;
                match tabs.active_tab() {
                    _ if ess.len() >= MAX_ESSENTIALS => self.notice = Some(format!("{MAX_ESSENTIALS} pinned sites max, unpin one first")),
                    Some(t) if !ess.iter().any(|e| e.url == t.url) => {
                        let title = if t.title.is_empty() { host_of(&t.url).unwrap_or_default() } else { t.title.clone() };
                        ess.push(Essential { title, url: t.url.clone() })
                    }
                    _ => {}
                }
            }
            UiCommand::StayLoggedIn => {
                if let Some(host) = tabs.active_tab().and_then(|t| host_of(&t.url)) {
                    if !self.snapshot.stay_logged_in.contains(&host) {
                        self.snapshot.stay_logged_in.push(host);
                    }
                }
            }
            UiCommand::ForgetLogin { domain } => self.snapshot.stay_logged_in.retain(|d| *d != domain),
            UiCommand::ImportSlop { text } => {
                let added = self.snapshot.slop.import(&text, now);
                self.notice = Some(format!("Imported {added} new slop tags"));
                self.push_flags(now);
            }
            UiCommand::Find { text, backwards } => self.engine.find(&text, backwards),
            UiCommand::Zoom { step } => {
                if let Some(host) = tabs.active_tab().and_then(|t| host_of(&t.url)) {
                    let level = zoom::step(self.zoom_for(&host), step);
                    if level == 1.0 {
                        self.snapshot.zoom.remove(&host);
                    } else {
                        self.snapshot.zoom.insert(host, level);
                    }
                    self.engine.zoom(level);
                    self.notice = Some(format!("Zoom {:.0} %", level * 100.0));
                }
            }
            UiCommand::UnpinEssential { index } => {
                if index < self.snapshot.essentials.len() {
                    self.snapshot.essentials.remove(index);
                }
            }
        }
        self.store.save(&self.snapshot);
    }

    /// Called by the engine adapter when a page finished loading (or changed URL/title).
    pub fn on_page(&mut self, url: &str, title: Option<&str>, loading: bool, now: Timestamp) {
        // A new document (also link clicks inside the page): fresh ad count, site ad rules.
        if loading {
            self.ads_hidden = 0;
            self.ai_images = 0;
            if let Some(host) = host_of(url) {
                self.engine.zoom(self.zoom_for(&host)); // the engine's zoom sticks across sites otherwise
                let site = self.ads.for_host(&host);
                if !site.hide.is_empty() || !site.allow.is_empty() {
                    self.engine.page_ads(&site.hide, &site.allow);
                }
            }
        }
        self.loading = loading;
        if let Some(id) = self.snapshot.tabs.active {
            self.snapshot.tabs.update(id, url, title, now);
            if !loading {
                self.snapshot.tabs.commit(id, url);
            }
        }
        if !loading {
            self.push_flags(now);
            self.store.save(&self.snapshot);
        }
    }

    /// The content script reports how many ads it covered on the current page.
    pub fn on_ads_hidden(&mut self, count: u32) {
        self.ads_hidden = count;
    }

    /// The content script reports how many AI-labelled images it blurred on the current page.
    pub fn on_ai_images(&mut self, count: u32) {
        self.ai_images = count;
    }

    /// Whether a cookie for `domain` may be restored after a restart.
    pub fn keeps_cookie(&self, domain: &str) -> bool {
        keeps_cookie(domain, &self.snapshot.stay_logged_in)
    }

    /// A one-line message for the user from outside the service (downloads, …).
    pub fn notify(&mut self, message: impl Into<String>) {
        self.notice = Some(message.into());
    }

    /// The engine's page process died (white page). Reload once; a second crash within a minute
    /// waits for the user, so a page that always crashes can't loop.
    pub fn on_crashed(&mut self, now: Timestamp) {
        if self.last_crash.is_some_and(|t| now.saturating_sub(t) < 60) {
            self.notice = Some("This page crashed again. Press reload to try once more".into());
            return;
        }
        self.last_crash = Some(now);
        self.notice = Some("This page crashed and was reloaded".into());
        self.load_active();
    }

    /// Navigation was refused by the adult blocklist (checked in the engine's navigation hook).
    pub fn on_blocked(&mut self) {
        self.notice = Some("Blocked: adult content".into());
    }

    /// Periodic housekeeping (auto-archive, soft delete, feed re-lock).
    pub fn tick(&mut self, now: Timestamp) -> bool {
        let mut changed = self.snapshot.tabs.sweep(&self.policy, now) > 0;
        // An expired unlock hides the feeds again on the open page, not only on the next load.
        if self.snapshot.feed_lock.unlocked_until.is_some() && self.snapshot.feed_lock.is_locked(now) {
            self.snapshot.feed_lock.relock();
            self.push_flags(now);
            changed = true;
        }
        if changed {
            self.store.save(&self.snapshot);
        }
        changed
    }

    pub fn state(&self, now: Timestamp) -> UiState {
        let tabs = &self.snapshot.tabs;
        let slop = |url: &str| self.is_slop(url);
        let active = tabs.active_tab();
        let mut archive: BTreeMap<String, Vec<ArchivedTab>> = BTreeMap::new();
        for t in tabs.items.iter().filter(|t| t.state == TabState::Archived) {
            archive.entry(t.folder.clone().unwrap_or_default()).or_default().push(ArchivedTab {
                id: t.id,
                title: t.title.clone(),
                url: t.url.clone(),
                idle_days: now.saturating_sub(t.last_active) / DAY,
            });
        }
        UiState {
            current: Current {
                url: active.map(|t| t.url.clone()).unwrap_or_default(),
                title: active.map(|t| t.title.clone()).unwrap_or_default(),
                slop: active.is_some_and(|t| slop(&t.url)),
                loading: self.loading,
                can_go_back: active.is_some_and(|t| t.can_go_back()),
                can_go_forward: active.is_some_and(|t| t.can_go_forward()),
                ads_hidden: self.ads_hidden,
                ai_images: self.ai_images,
                stay_logged_in: active.and_then(|t| host_of(&t.url)).is_some_and(|h| self.snapshot.stay_logged_in.contains(&h)),
            },
            tabs: tabs
                .items
                .iter()
                .filter(|t| t.state == TabState::Open)
                .map(|t| TabView {
                    id: t.id,
                    title: t.title.clone(),
                    url: t.url.clone(),
                    active: tabs.active == Some(t.id),
                    slop: slop(&t.url),
                })
                .collect(),
            tab_limit: self.policy.limit,
            archive: archive.into_iter().map(|(folder, tabs)| ArchiveFolder { folder, tabs }).collect(),
            essentials: self.snapshot.essentials.clone(),
            workspaces: self.workspaces.clone(),
            feed_lock: FeedLockView {
                locked: self.snapshot.feed_lock.is_locked(now),
                unlocked_until: self.snapshot.feed_lock.unlocked_until.filter(|&u| u > now),
            },
            stay_logged_in: self.snapshot.stay_logged_in.clone(),
            slop_count: self.snapshot.slop.tags.len(),
            shared_slop_count: self.shared_slop.tags.len(),
            notice: self.notice.clone(),
        }
    }

    /// Address-bar text → clean URL, or `None` (with a notice) when the adult list blocks it.
    fn checked(&mut self, input: &str) -> Option<String> {
        let url = resolve_input(input);
        let url = strip_tracking(&url).unwrap_or(url);
        if host_of(&url).is_some_and(|h| self.adult.is_blocked(&h)) {
            self.notice = Some("Blocked: adult content".into());
            return None;
        }
        Some(url)
    }

    /// Tagged by the user or on a list they follow.
    pub fn is_slop(&self, url: &str) -> bool {
        self.snapshot.slop.find(url).is_some() || self.shared_slop.find(url).is_some()
    }

    fn zoom_for(&self, host: &str) -> f64 {
        self.snapshot.zoom.get(host).copied().unwrap_or(1.0)
    }

    fn load_active(&mut self) {
        if let Some(url) = self.snapshot.tabs.active_tab().map(|t| t.url.clone()) {
            self.loading = true;
            self.ads_hidden = 0;
            self.engine.navigate(&url);
        }
    }

    fn push_flags(&self, now: Timestamp) {
        let slop = self.snapshot.tabs.active_tab().is_some_and(|t| self.is_slop(&t.url));
        self.engine.page_flags(slop, self.snapshot.feed_lock.is_locked(now));
    }
}

/// Turns address-bar text into a URL: full URLs pass through, bare hosts get https://,
/// anything else becomes a no-AI DuckDuckGo search.
pub fn resolve_input(text: &str) -> String {
    let text = text.trim();
    if url::Url::parse(text).is_ok_and(|u| u.has_host() || u.scheme() == "about") {
        return text.to_owned();
    }
    let looks_like_host = !text.contains(' ') && text.contains('.') && !text.ends_with('.');
    if looks_like_host {
        return format!("https://{text}");
    }
    let mut url = url::Url::parse(HOME_URL).expect("valid home URL");
    url.query_pairs_mut().append_pair("q", text);
    url.into()
}
