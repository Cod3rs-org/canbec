//! One canbec at a time: a second start hands its URL (from a clicked link, or none) to the
//! running instance and quits, so two processes never write the same state. The running one
//! listens on 127.0.0.1 and writes `port token` to `<config>/instance`; only callers that can read
//! that file (the same user) know the token, so websites poking localhost get nothing.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;

const FILE: &str = "instance";

/// Sends `url` (empty = just come to the front) to a running canbec. `true` when one took it.
pub fn hand_over(config: &Path, url: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(config.join(FILE)) else { return false };
    let Some((port, token)) = text.trim().split_once(' ') else { return false };
    let Ok(port) = port.parse::<u16>() else { return false };
    let Ok(mut stream) = TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_millis(500)) else { return false };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut reply = String::new();
    writeln!(stream, "{token}\n{}", url.replace(['\r', '\n'], "")).is_ok() && stream.read_to_string(&mut reply).is_ok() && reply == "ok"
}

/// Becomes the running instance: `on_url` gets every URL a later start hands over.
pub fn listen(config: &Path, token: &str, on_url: impl Fn(String) + Send + 'static) -> std::io::Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    std::fs::create_dir_all(config)?;
    write_private(&config.join(FILE), &format!("{} {token}", listener.local_addr()?.port()))?;
    let token = token.to_owned();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut lines = BufReader::new((&stream).take(8192)).lines();
            if lines.next().and_then(Result::ok).as_deref() != Some(token.as_str()) {
                continue; // not canbec: drop the connection
            }
            let url = lines.next().and_then(Result::ok).unwrap_or_default();
            let _ = stream.write_all(b"ok");
            on_url(url);
        }
    });
    Ok(())
}

fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(text.as_bytes())
}
