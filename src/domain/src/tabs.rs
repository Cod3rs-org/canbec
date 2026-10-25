//! Tab discipline: at most 20 open tabs, idle tabs auto-archive into folders,
//! long-forgotten archived tabs move to Deleted. Nothing is ever hard-deleted.

use serde::{Deserialize, Serialize};

use crate::{host_matches, host_of, Timestamp, DAY};

pub type TabId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabState {
    Open,
    Archived,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tab {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub state: TabState,
    /// Set when archived; derived from the URL by [`folder_for`].
    pub folder: Option<String>,
    pub last_active: Timestamp,
    /// Pages finished in this tab, oldest first; `pos` is the current one. Back/forward walk it.
    /// ponytail: SPA pushState changes aren't reported by the engine, so they don't add entries.
    #[serde(default)]
    pub history: Vec<String>,
    #[serde(default)]
    pub pos: usize,
}

/// Back/forward entries kept per tab.
pub const HISTORY_LIMIT: usize = 50;

impl Tab {
    pub fn can_go_back(&self) -> bool {
        self.pos > 0 && !self.history.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        self.pos + 1 < self.history.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabPolicy {
    pub limit: usize,
    pub archive_after: u64,
    pub delete_after: u64,
}

impl Default for TabPolicy {
    fn default() -> Self {
        Self { limit: 20, archive_after: 3 * DAY, delete_after: 60 * DAY }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabLimitReached {
    pub limit: usize,
}

impl std::fmt::Display for TabLimitReached {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} tabs open — close or archive one first", self.limit)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Tabs {
    pub items: Vec<Tab>,
    pub active: Option<TabId>,
    next_id: TabId,
}

impl Tabs {
    pub fn open_count(&self) -> usize {
        self.items.iter().filter(|t| t.state == TabState::Open).count()
    }

    pub fn get(&self, id: TabId) -> Option<&Tab> {
        self.items.iter().find(|t| t.id == id)
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active.and_then(|id| self.get(id))
    }

    fn get_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.items.iter_mut().find(|t| t.id == id)
    }

    /// Opens and activates a new tab, unless the soft cap is reached.
    pub fn open(&mut self, url: &str, policy: &TabPolicy, now: Timestamp) -> Result<TabId, TabLimitReached> {
        if self.open_count() >= policy.limit {
            return Err(TabLimitReached { limit: policy.limit });
        }
        self.next_id += 1;
        let id = self.next_id;
        self.items.push(Tab {
            id,
            url: url.to_owned(),
            title: url.to_owned(),
            state: TabState::Open,
            folder: None,
            last_active: now,
            history: Vec::new(),
            pos: 0,
        });
        self.active = Some(id);
        Ok(id)
    }

    pub fn activate(&mut self, id: TabId, now: Timestamp) {
        if let Some(tab) = self.get_mut(id).filter(|t| t.state == TabState::Open) {
            tab.last_active = now;
            self.active = Some(id);
        }
    }

    /// Updates URL/title of a tab after navigation.
    pub fn update(&mut self, id: TabId, url: &str, title: Option<&str>, now: Timestamp) {
        if let Some(tab) = self.get_mut(id) {
            tab.url = url.to_owned();
            if let Some(title) = title {
                tab.title = title.to_owned();
            }
            tab.last_active = now;
        }
    }

    /// A page finished loading in the tab: a new history entry unless it's the current one
    /// (reloads, tab switches, back/forward landing where expected).
    pub fn commit(&mut self, id: TabId, url: &str) {
        let Some(tab) = self.get_mut(id) else { return };
        if tab.history.get(tab.pos).is_some_and(|u| u == url) {
            return;
        }
        if !tab.history.is_empty() {
            tab.history.truncate(tab.pos + 1);
        }
        tab.history.push(url.to_owned());
        if tab.history.len() > HISTORY_LIMIT {
            tab.history.remove(0);
        }
        tab.pos = tab.history.len() - 1;
    }

    /// Moves the tab one step back (`-1`) or forward (`1`) in its history; returns the URL to load.
    pub fn step(&mut self, id: TabId, delta: isize) -> Option<String> {
        let tab = self.get_mut(id)?;
        let pos = tab.pos.checked_add_signed(delta).filter(|&p| p < tab.history.len())?;
        tab.pos = pos;
        tab.url = tab.history[pos].clone();
        Some(tab.url.clone())
    }

    /// Closing keeps history: the tab becomes Deleted (hidden, searchable later).
    pub fn close(&mut self, id: TabId) {
        self.set_state(id, TabState::Deleted);
    }

    pub fn archive(&mut self, id: TabId) {
        self.set_state(id, TabState::Archived);
    }

    /// Archived or deleted tab back to Open; respects the cap and resets the idle timer.
    pub fn restore(&mut self, id: TabId, policy: &TabPolicy, now: Timestamp) -> Result<(), TabLimitReached> {
        if self.get(id).is_some_and(|t| t.state != TabState::Open) {
            if self.open_count() >= policy.limit {
                return Err(TabLimitReached { limit: policy.limit });
            }
            if let Some(tab) = self.get_mut(id) {
                tab.state = TabState::Open;
                tab.folder = None;
            }
            self.activate(id, now);
        }
        Ok(())
    }

    /// Periodic housekeeping. Returns how many tabs changed state.
    pub fn sweep(&mut self, policy: &TabPolicy, now: Timestamp) -> usize {
        let active = self.active;
        let mut changed = 0;
        for tab in &mut self.items {
            let idle = now.saturating_sub(tab.last_active);
            let next = match tab.state {
                TabState::Open if Some(tab.id) != active && idle >= policy.archive_after => TabState::Archived,
                TabState::Archived if idle >= policy.delete_after => TabState::Deleted,
                _ => continue,
            };
            if next == TabState::Archived {
                tab.folder = Some(folder_for(&tab.url));
            }
            tab.state = next;
            changed += 1;
        }
        changed
    }

    fn set_state(&mut self, id: TabId, state: TabState) {
        if let Some(tab) = self.get_mut(id) {
            if state == TabState::Archived {
                tab.folder = Some(folder_for(&tab.url));
            }
            tab.state = state;
        }
        if self.active == Some(id) && state != TabState::Open {
            self.active = self.items.iter().rev().find(|t| t.state == TabState::Open).map(|t| t.id);
        }
    }
}

/// Folder label for an archived tab. No AI: a small domain → category table, else the host.
pub fn folder_for(url: &str) -> String {
    // ponytail: static table; user-defined folder rules can extend it later.
    const RULES: &[(&str, &str)] = &[
        ("github.com", "Code"),
        ("gitlab.com", "Code"),
        ("stackoverflow.com", "Code"),
        ("docs.rs", "Docs"),
        ("developer.mozilla.org", "Docs"),
        ("wikipedia.org", "Reading"),
        ("news.ycombinator.com", "Reading"),
        ("youtube.com", "Video"),
        ("vimeo.com", "Video"),
        ("amazon.com", "Shopping"),
        ("ebay.com", "Shopping"),
        ("reddit.com", "Social"),
        ("x.com", "Social"),
        ("facebook.com", "Social"),
        ("linkedin.com", "Social"),
    ];
    let Some(host) = host_of(url) else { return "Other".into() };
    RULES
        .iter()
        .find(|(domain, _)| host_matches(&host, domain))
        .map(|(_, folder)| (*folder).to_owned())
        .unwrap_or(host)
}
