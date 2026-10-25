//! Messages exchanged with the sidebar UI. Mirrors docs/architecture/ui-contract.md.

use canbec_domain::tabs::TabId;
use canbec_domain::slop::SlopScope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiCommand {
    Navigate { url: String },
    Back,
    Forward,
    Reload,
    NewTab,
    ActivateTab { id: TabId },
    CloseTab { id: TabId },
    ArchiveTab { id: TabId },
    RestoreTab { id: TabId },
    TagSlop { scope: SlopScope },
    UntagSlop,
    SwitchWorkspace { id: String },
    OpenSettings,
    UnlockFeeds { hours: u64 },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pub current: Current,
    pub tabs: Vec<TabView>,
    pub tab_limit: usize,
    pub archive: Vec<ArchiveFolder>,
    pub essentials: Vec<Essential>,
    pub workspaces: Vec<Workspace>,
    pub feed_lock: FeedLockView,
    /// One-line message for the user (e.g. "20 tabs open — close or archive one first").
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Current {
    pub url: String,
    pub title: String,
    pub slop: bool,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub ads_hidden: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TabView {
    pub id: TabId,
    pub title: String,
    pub url: String,
    pub active: bool,
    pub slop: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveFolder {
    pub folder: String,
    pub tabs: Vec<ArchivedTab>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedTab {
    pub id: TabId,
    pub title: String,
    pub url: String,
    pub idle_days: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Essential {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub color: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedLockView {
    pub locked: bool,
    pub unlocked_until: Option<u64>,
}
