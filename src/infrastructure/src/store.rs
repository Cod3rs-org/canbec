//! JSON file store. Writes atomically (temp file + rename) so a crash never corrupts the session.

use std::path::PathBuf;

use canbec_application::ports::StateStore;
use canbec_application::Snapshot;

pub struct JsonFileStore {
    path: PathBuf,
}

impl JsonFileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl StateStore for JsonFileStore {
    fn load(&self) -> Option<Snapshot> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        match serde_json::from_str(&text) {
            Ok(snapshot) => Some(snapshot),
            Err(e) => {
                // Keep the unreadable file for recovery instead of overwriting it.
                let backup = self.path.with_extension("broken.json");
                eprintln!("canbec: state unreadable ({e}); moved to {}", backup.display());
                let _ = std::fs::rename(&self.path, backup);
                None
            }
        }
    }

    fn save(&self, snapshot: &Snapshot) {
        let write = || -> std::io::Result<()> {
            if let Some(dir) = self.path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let tmp = self.path.with_extension("tmp");
            std::fs::write(&tmp, serde_json::to_vec_pretty(snapshot)?)?;
            std::fs::rename(tmp, &self.path)
        };
        if let Err(e) = write() {
            eprintln!("canbec: could not save state: {e}");
        }
    }
}
