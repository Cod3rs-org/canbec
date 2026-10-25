//! User-tagged AI slop: one click marks a domain or a single page.

use serde::{Deserialize, Serialize};

use crate::{host_matches, host_of, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SlopScope {
    Domain,
    Page,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlopTag {
    /// Host for `Domain`, URL without fragment for `Page`.
    pub pattern: String,
    pub scope: SlopScope,
    pub created_at: Timestamp,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SlopList {
    pub tags: Vec<SlopTag>,
}

fn page_key(url: &str) -> String {
    url.split('#').next().unwrap_or(url).to_owned()
}

impl SlopList {
    /// Returns false for URLs without a host (about:blank, data:) — nothing to tag.
    pub fn tag(&mut self, url: &str, scope: SlopScope, now: Timestamp) -> bool {
        let pattern = match scope {
            SlopScope::Domain => match host_of(url) {
                Some(h) => h,
                None => return false,
            },
            SlopScope::Page => page_key(url),
        };
        if !self.tags.iter().any(|t| t.pattern == pattern && t.scope == scope) {
            self.tags.push(SlopTag { pattern, scope, created_at: now });
        }
        true
    }

    /// Removes every tag that matches `url` (domain and page).
    pub fn untag(&mut self, url: &str) {
        let host = host_of(url);
        let page = page_key(url);
        self.tags.retain(|t| !Self::matches(t, host.as_deref(), &page));
    }

    pub fn find(&self, url: &str) -> Option<&SlopTag> {
        let host = host_of(url);
        let page = page_key(url);
        self.tags.iter().find(|t| Self::matches(t, host.as_deref(), &page))
    }

    fn matches(tag: &SlopTag, host: Option<&str>, page: &str) -> bool {
        match tag.scope {
            SlopScope::Domain => host.is_some_and(|h| host_matches(h, &tag.pattern)),
            SlopScope::Page => tag.pattern == page,
        }
    }
}
