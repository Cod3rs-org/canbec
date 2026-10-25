//! News-feed blocker lock. Locked by default; unlock only from settings, at most 6 hours.

use serde::{Deserialize, Serialize};

use crate::{Timestamp, HOUR};

pub const MAX_UNLOCK_HOURS: u64 = 6;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedLock {
    pub unlocked_until: Option<Timestamp>,
}

impl FeedLock {
    pub fn is_locked(&self, now: Timestamp) -> bool {
        self.unlocked_until.is_none_or(|until| now >= until)
    }

    /// Unlock for `hours`, clamped to 1..=6. Never extends past `now + 6h`, even if called repeatedly.
    pub fn unlock(&mut self, hours: u64, now: Timestamp) -> Timestamp {
        let until = now + hours.clamp(1, MAX_UNLOCK_HOURS) * HOUR;
        self.unlocked_until = Some(until);
        until
    }

    pub fn relock(&mut self) {
        self.unlocked_until = None;
    }
}
