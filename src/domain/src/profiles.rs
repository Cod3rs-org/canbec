//! Profiles: separate identities (personal, work, …). Each has its own tabs, slop tags and
//! logins; the browser keeps one active at a time.

use serde::{Deserialize, Serialize};

pub const DEFAULT_PROFILE: &str = "personal";
pub const MAX_PROFILES: usize = 6;
const COLORS: &[&str] = &["#ff7a45", "#3b82f6", "#22a06b", "#a855f7", "#e5484d", "#d98a00"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profiles {
    pub list: Vec<Profile>,
    pub active: String,
}

impl Default for Profiles {
    fn default() -> Self {
        let personal = Profile { id: DEFAULT_PROFILE.into(), name: "Personal".into(), color: COLORS[0].into() };
        Self { list: vec![personal], active: DEFAULT_PROFILE.into() }
    }
}

impl Profiles {
    pub fn active(&self) -> &Profile {
        self.list.iter().find(|p| p.id == self.active).unwrap_or(&self.list[0])
    }

    /// Adds a profile and returns its id. Ids are lowercase ASCII, safe as folder names.
    pub fn add(&mut self, name: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 24 {
            return Err("Profile names are 1 to 24 characters".into());
        }
        if self.list.len() >= MAX_PROFILES {
            return Err(format!("{MAX_PROFILES} profiles max"));
        }
        if self.list.iter().any(|p| p.name.eq_ignore_ascii_case(name)) {
            return Err(format!("There is already a profile called {name}"));
        }
        let slug: String = name.to_ascii_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        let slug = slug.trim_matches('-');
        let base = if slug.is_empty() { "profile" } else { slug };
        let id = (1..).map(|n| if n == 1 { base.to_owned() } else { format!("{base}-{n}") }).find(|id| self.list.iter().all(|p| p.id != *id)).unwrap_or_default();
        let color = COLORS[self.list.len() % COLORS.len()].to_owned();
        self.list.push(Profile { id: id.clone(), name: name.to_owned(), color });
        Ok(id)
    }

    /// Makes `id` the active profile; false if there is no such profile or it's already active.
    pub fn activate(&mut self, id: &str) -> bool {
        if self.active == id || !self.list.iter().any(|p| p.id == id) {
            return false;
        }
        self.active = id.to_owned();
        true
    }
}
