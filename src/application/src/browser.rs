//! The browser's use cases. Every driving adapter (sidebar, MCP) goes through here.

use std::collections::BTreeMap;

use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::feed_lock::FeedLock;
use canbec_domain::slop::SlopList;
use canbec_domain::tabs::{TabPolicy, TabState, Tabs};
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
}

pub struct BrowserService<E: Engine, S: StateStore> {
    engine: E,
    store: S,
    snapshot: Snapshot,
    policy: TabPolicy,
    adult: HostBlocklist,
    notice: Option<String>,
    loading: bool,
    ads_hidden: u32,
}

impl<E: Engine, S: StateStore> BrowserService<E, S> {
    /// Restores the last session (or opens the home page) and loads the active tab.
    pub fn start(engine: E, store: S, adult: HostBlocklist, now: Timestamp) -> Self {
        let snapshot = store.load().unwrap_or_default();
        let mut service =
            Self { engine, store, snapshot, policy: TabPolicy::default(), adult, notice: None, loading: false, ads_hidden: 0 };
        service.snapshot.tabs.sweep(&service.policy, now);
        if service.snapshot.tabs.active_tab().is_none() {
            let _ = service.snapshot.tabs.open(HOME_URL, &service.policy, now);
        }
        service.load_active();
        service
    }

    /// The engine adapter, e.g. for resizing its view.
    pub fn engine(&self) -> &E {
        &self.engine
    }

    pub fn set_adult_blocklist(&mut self, list: HostBlocklist) {
        self.adult = list;
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
                let url = resolve_input(&url);
                if host_of(&url).is_some_and(|h| self.adult.is_blocked(&h)) {
                    self.notice = Some("Blocked: adult content".into());
                    return;
                }
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
            UiCommand::Back => self.engine.back(),
            UiCommand::Forward => self.engine.forward(),
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
            // ponytail: one workspace until profiles exist (docs/research/06).
            UiCommand::SwitchWorkspace { .. } => {}
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
        self.loading = loading;
        if let Some(id) = self.snapshot.tabs.active {
            self.snapshot.tabs.update(id, url, title, now);
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
        let slop = |url: &str| self.snapshot.slop.find(url).is_some();
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
                // ponytail: the engine port doesn't report history yet; buttons stay enabled.
                can_go_back: true,
                can_go_forward: true,
                ads_hidden: self.ads_hidden,
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
            workspaces: vec![Workspace {
                id: "personal".into(),
                name: "Personal".into(),
                color: "#ff7a45".into(),
                active: true,
            }],
            feed_lock: FeedLockView {
                locked: self.snapshot.feed_lock.is_locked(now),
                unlocked_until: self.snapshot.feed_lock.unlocked_until.filter(|&u| u > now),
            },
            notice: self.notice.clone(),
        }
    }

    fn load_active(&mut self) {
        if let Some(url) = self.snapshot.tabs.active_tab().map(|t| t.url.clone()) {
            self.loading = true;
            self.ads_hidden = 0;
            self.engine.navigate(&url);
        }
    }

    fn push_flags(&self, now: Timestamp) {
        let slop = self.snapshot.tabs.active_tab().is_some_and(|t| self.snapshot.slop.find(&t.url).is_some());
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
