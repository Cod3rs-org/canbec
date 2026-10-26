//! Encryption for the saved logins. A random 256-bit key lives in the OS secret store
//! (macOS Keychain, Linux Secret Service, Windows DPAPI); files are sealed with
//! ChaCha20-Poly1305 (`ring`, already in the tree through the HTTPS client).
//! No secret store (e.g. a Linux box without `secret-tool`)? `key()` is `None` and the
//! caller keeps the old plain, owner-only file.

use std::sync::OnceLock;

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305, NONCE_LEN};
use ring::rand::{SecureRandom, SystemRandom};

const MAGIC: &[u8] = b"CNBC1";

pub type Key = [u8; 32];

/// The logins key, created on first use. Looked up once per run.
pub fn key() -> Option<Key> {
    static KEY: OnceLock<Option<Key>> = OnceLock::new();
    *KEY.get_or_init(|| {
        if let Some(key) = store::load() {
            return Some(key);
        }
        let mut key = [0u8; 32];
        SystemRandom::new().fill(&mut key).ok()?;
        store::save(&key).then_some(key)
    })
}

pub fn seal(key: &Key, plain: &[u8]) -> Option<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new().fill(&mut nonce).ok()?;
    let sealing = LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, key).ok()?);
    let mut data = plain.to_vec();
    sealing.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::from(MAGIC), &mut data).ok()?;
    Some([MAGIC, &nonce, &data].concat())
}

/// `None` for anything not sealed with this key (wrong key, damaged or foreign file).
pub fn open(key: &Key, sealed: &[u8]) -> Option<Vec<u8>> {
    let rest = sealed.strip_prefix(MAGIC)?;
    if rest.len() < NONCE_LEN {
        return None;
    }
    let (nonce, data) = rest.split_at(NONCE_LEN);
    let opening = LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, key).ok()?);
    let mut data = data.to_vec();
    let plain = opening.open_in_place(Nonce::try_assume_unique_for_key(nonce).ok()?, Aad::from(MAGIC), &mut data).ok()?;
    Some(plain.to_vec())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn from_hex(text: &str) -> Option<Key> {
    let text = text.trim();
    let mut key = [0u8; 32];
    if text.len() != 64 {
        return None;
    }
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(key)
}

/// macOS: a generic password in the login Keychain, through the `security` tool. The key goes
/// in on stdin (`security -i`), never on the command line where `ps` could see it.
#[cfg(target_os = "macos")]
mod store {
    use std::io::Write;
    use std::process::{Command, Stdio};

    const ARGS: [&str; 4] = ["-a", "canbec", "-s", "canbec logins key"];

    pub fn load() -> Option<super::Key> {
        let out = Command::new("security").arg("find-generic-password").args(ARGS).arg("-w").output().ok()?;
        out.status.success().then(|| super::from_hex(&String::from_utf8_lossy(&out.stdout))).flatten()
    }

    pub fn save(key: &super::Key) -> bool {
        let Ok(mut child) = Command::new("security").arg("-i").stdin(Stdio::piped()).stdout(Stdio::null()).spawn() else { return false };
        let line = format!("add-generic-password -U -a canbec -s \"canbec logins key\" -w {}\n", super::hex(key));
        let wrote = child.stdin.take().is_some_and(|mut stdin| stdin.write_all(line.as_bytes()).is_ok());
        child.wait().is_ok_and(|s| s.success()) && wrote && load() == Some(*key)
    }
}

/// Linux: the desktop's Secret Service (GNOME Keyring, KWallet) through `secret-tool`.
#[cfg(target_os = "linux")]
mod store {
    use std::io::Write;
    use std::process::{Command, Stdio};

    const ATTRS: [&str; 4] = ["application", "canbec", "purpose", "logins"];

    pub fn load() -> Option<super::Key> {
        let out = Command::new("secret-tool").arg("lookup").args(ATTRS).output().ok()?;
        out.status.success().then(|| super::from_hex(&String::from_utf8_lossy(&out.stdout))).flatten()
    }

    pub fn save(key: &super::Key) -> bool {
        let Ok(mut child) =
            Command::new("secret-tool").args(["store", "--label=canbec logins key"]).args(ATTRS).stdin(Stdio::piped()).spawn()
        else {
            return false;
        };
        let wrote = child.stdin.take().is_some_and(|mut stdin| stdin.write_all(super::hex(key).as_bytes()).is_ok());
        child.wait().is_ok_and(|s| s.success()) && wrote && load() == Some(*key)
    }
}

/// Windows: the key file is wrapped with DPAPI, so only this Windows user can unwrap it.
#[cfg(windows)]
mod store {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};

    fn path() -> std::path::PathBuf {
        crate::config_dir().join("logins.key")
    }

    /// Runs DPAPI `protect` or `unprotect` over `data`.
    fn dpapi(data: &[u8], protect: bool) -> Option<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let null = std::ptr::null();
        // SAFETY: valid input blob; DPAPI allocates the output, freed with LocalFree below.
        unsafe {
            let ok = if protect {
                CryptProtectData(&input, null, null, null, null, 0, &mut output)
            } else {
                CryptUnprotectData(&input, std::ptr::null_mut(), null, null, null, 0, &mut output)
            };
            if ok == 0 || output.pbData.is_null() {
                return None;
            }
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData as _);
            Some(bytes)
        }
    }

    pub fn load() -> Option<super::Key> {
        dpapi(&std::fs::read(path()).ok()?, false)?.try_into().ok()
    }

    pub fn save(key: &super::Key) -> bool {
        let Some(wrapped) = dpapi(key, true) else { return false };
        let _ = std::fs::create_dir_all(crate::config_dir());
        std::fs::write(path(), wrapped).is_ok() && load() == Some(*key)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod store {
    pub fn load() -> Option<super::Key> {
        None
    }
    pub fn save(_: &super::Key) -> bool {
        false
    }
}
