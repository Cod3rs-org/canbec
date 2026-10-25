//! Ports implemented by infrastructure adapters.

use crate::Snapshot;

/// A page engine (wry, Servo, CEF, ...). Deliberately tiny so new engines are cheap to add.
pub trait Engine {
    fn navigate(&self, url: &str);
    fn reload(&self);
    /// Tells the injected content script whether the current page is tagged as slop
    /// and whether feeds are locked.
    fn page_flags(&self, slop: bool, feeds_locked: bool);
    /// Site-specific ad selectors to cover, and generic ones this site is exempt from.
    fn page_ads(&self, hide: &[String], allow: &[String]);
}

/// Persistence for everything the user would hate to lose.
pub trait StateStore {
    fn load(&self) -> Option<Snapshot>;
    fn save(&self, snapshot: &Snapshot);
}
