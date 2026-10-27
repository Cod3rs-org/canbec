//! One canbec at a time: a second start hands its URL (from a clicked link, or none) to the
//! running instance and quits, so two processes never write the same state.
//!
//! macOS / Linux: a Unix socket `<config>/instance.sock` (owner-only), invisible to websites.
//! Windows: 127.0.0.1 TCP; `<config>/instance` holds `port token`, and only callers that can read
//! that file know the token. ponytail: a site probing localhost ports can tell a port is open on
//! Windows; AF_UNIX via windows-sys would hide it.
//! ponytail: two starts in the same instant can both become the running one (no lock).

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[cfg(unix)]
const FILE: &str = "instance.sock";
#[cfg(windows)]
const FILE: &str = "instance";
const WAIT: Duration = Duration::from_secs(2);

/// Sends `url` (empty = just come to the front) to a running canbec. `true` when one took it.
pub fn hand_over(config: &Path, url: &str) -> bool {
    let url = url.replace(['\r', '\n'], "");
    let ask = |s: &mut dyn ReadWrite, first: &str| {
        let mut reply = String::new();
        s.write_all(format!("{first}{url}\n").as_bytes()).is_ok() && s.read_to_string(&mut reply).is_ok() && reply == "ok"
    };
    #[cfg(unix)]
    {
        let Ok(mut s) = std::os::unix::net::UnixStream::connect(config.join(FILE)) else { return false };
        let _ = s.set_read_timeout(Some(WAIT));
        ask(&mut s, "")
    }
    #[cfg(windows)]
    {
        use std::net::{Ipv4Addr, TcpStream};
        let Ok(text) = std::fs::read_to_string(config.join(FILE)) else { return false };
        let Some((port, token)) = text.trim().split_once(' ') else { return false };
        let Ok(port) = port.parse::<u16>() else { return false };
        let Ok(mut s) = TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), WAIT) else { return false };
        let _ = s.set_read_timeout(Some(WAIT));
        ask(&mut s, &format!("{token}\n"))
    }
}

/// Becomes the running instance: `on_url` gets every URL a later start hands over.
pub fn listen(config: &Path, on_url: impl Fn(String) + Send + Sync + 'static) -> std::io::Result<()> {
    std::fs::create_dir_all(config)?;
    let on_url = Arc::new(on_url);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = config.join(FILE);
        let _ = std::fs::remove_file(&path); // left by a crash: hand_over already found nobody there
        let listener = std::os::unix::net::UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        std::thread::spawn(move || {
            for s in listener.incoming().flatten() {
                let on_url = on_url.clone();
                std::thread::spawn(move || serve(s, None, &*on_url));
            }
        });
    }
    #[cfg(windows)]
    {
        use std::net::{Ipv4Addr, TcpListener};
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let token = crate::random_token();
        std::fs::write(config.join(FILE), format!("{} {token}", listener.local_addr()?.port()))?;
        let token: Arc<str> = token.into();
        std::thread::spawn(move || {
            for s in listener.incoming().flatten() {
                let (on_url, token) = (on_url.clone(), token.clone());
                std::thread::spawn(move || serve(s, Some(&token), &*on_url));
            }
        });
    }
    Ok(())
}

/// Stops later starts from handing over to this process (on quit, or before an update relaunch).
pub fn release(config: &Path) {
    let _ = std::fs::remove_file(config.join(FILE));
}

trait ReadWrite: Read + Write {}
impl<T: Read + Write> ReadWrite for T {}

/// One request: `[token\n]url\n`, answered with `ok`. Each connection has its own thread, so a
/// slow or silent caller can't hold up the next link.
fn serve<S: Read + Write + Timeout>(mut s: S, token: Option<&str>, on_url: &dyn Fn(String)) {
    s.timeout(WAIT);
    let mut lines = BufReader::new((&mut s).take(8192)).lines();
    if token.is_some_and(|t| lines.next().and_then(Result::ok).as_deref() != Some(t)) {
        return; // not canbec: drop the connection
    }
    let url = lines.next().and_then(Result::ok).unwrap_or_default();
    drop(lines);
    if s.write_all(b"ok").is_ok() {
        on_url(url);
    }
}

trait Timeout {
    fn timeout(&self, d: Duration);
}
#[cfg(unix)]
impl Timeout for std::os::unix::net::UnixStream {
    fn timeout(&self, d: Duration) {
        let _ = self.set_read_timeout(Some(d));
    }
}
#[cfg(windows)]
impl Timeout for std::net::TcpStream {
    fn timeout(&self, d: Duration) {
        let _ = self.set_read_timeout(Some(d));
    }
}
