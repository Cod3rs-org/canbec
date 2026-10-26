//! Messages exchanged with the sidebar UI. Mirrors docs/architecture/ui-contract.md.

use canbec_domain::tabs::TabId;
use canbec_domain::slop::SlopScope;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiCommand {
    Navigate { url: String },
    /// Opens `url` in a new background tab (Cmd/Ctrl+click, middle click). Same checks as navigate.
    OpenTab { url: String },
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
    /// From the settings panel only (the UI waits a few seconds before it allows this).
    UnlockFeeds { hours: u64 },
    RelockFeeds,
    /// Pins the current page to the essentials grid.
    PinPage,
    UnpinEssential { index: usize },
    /// Keep cookies of the current site across restarts (every other site starts fresh).
    StayLoggedIn,
    /// Forget a site's login at the next start.
    ForgetLogin { domain: String },
    /// Find `text` in the page; again with the same text jumps to the next match.
    Find { text: String, #[serde(default)] backwards: bool },
    /// `1` zoom in, `-1` zoom out, `0` reset. Remembered per site.
    Zoom { step: i8 },
    /// Adds a shared slop list (plain text, one host or URL per line).
    ImportSlop { text: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state", content = "percent")]
pub enum AiTextMode {
    Off,
    /// Downloading the model, percent done.
    Downloading(u8),
    On,
}

/// Closed tabs sent to the sidebar; older ones stay in state.json.
pub const CLOSED_SHOWN: usize = 200;

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
    /// Sites whose cookies survive a restart. Everything else starts fresh on every launch.
    pub stay_logged_in: Vec<String>,
    /// Sites/pages tagged by the user, and entries from subscribed lists.
    pub slop_count: usize,
    pub shared_slop_count: usize,
    /// The computer uses the family DNS (adult sites blocked in every app). `None`: not checked yet.
    pub family_dns: Option<bool>,
    /// Version of a newer release, if there is one.
    pub update: Option<String>,
    /// Closed tabs, most recently used first (at most [`CLOSED_SHOWN`]). Restore with `restoreTab`.
    pub closed: Vec<ArchivedTab>,
    /// Names of the installed extensions (`<config>/extensions/`).
    pub extensions: Vec<String>,
    /// The optional AI text model: off (not downloaded), downloading, or on.
    pub ai_text: AiTextMode,
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
    /// Images on this page whose metadata says they're AI-generated (blurred until clicked).
    pub ai_images: u32,
    /// "Reads like AI", in percent, when the optional detector is fairly sure (see domain::ai_text).
    pub ai_text: Option<u8>,
    /// The current site keeps its cookies across restarts.
    pub stay_logged_in: bool,
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
    /// AI agents may use this profile (`canbec --mcp`).
    pub agents: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedLockView {
    pub locked: bool,
    pub unlocked_until: Option<u64>,
}
