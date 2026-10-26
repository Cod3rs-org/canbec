//! Where each profile lives: `personal` keeps the original files in the config folder (so
//! existing users keep their tabs), others get `profiles/<id>/`.

use std::path::{Path, PathBuf};

use canbec_domain::profiles::{Profiles, DEFAULT_PROFILE};

pub fn load(config: &Path) -> Profiles {
    std::fs::read(config.join("profiles.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(config: &Path, profiles: &Profiles) {
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(config)?;
        let tmp = config.join("profiles.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(profiles)?)?;
        std::fs::rename(tmp, config.join("profiles.json"))
    };
    if let Err(e) = write() {
        eprintln!("canbec: could not save profiles: {e}");
    }
}

/// Folder with the profile's `state.json` and `logins.json`. Ids come from
/// `Profiles::add`, which only makes `[a-z0-9-]` ids.
pub fn dir(config: &Path, id: &str) -> PathBuf {
    if id == DEFAULT_PROFILE { config.to_path_buf() } else { config.join("profiles").join(id) }
}
