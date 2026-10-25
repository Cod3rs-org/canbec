//! Host blocklists (adult content, ad servers). Parses hosts-file and plain-domain lists.

use std::collections::HashSet;

use crate::host_matches;

#[derive(Debug, Default, Clone)]
pub struct HostBlocklist {
    hosts: HashSet<String>,
}

impl HostBlocklist {
    /// Accepts `0.0.0.0 example.com`, `127.0.0.1 example.com`, `example.com` and `||example.com^`
    /// lines; `#` / `!` comments are ignored.
    pub fn parse(text: &str) -> Self {
        let mut list = Self::default();
        list.extend(text);
        list
    }

    pub fn extend(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() || line.starts_with('!') {
                continue;
            }
            let host = line.split_whitespace().last().unwrap_or("");
            let host = host.trim_start_matches("||").trim_end_matches('^').to_ascii_lowercase();
            if host.contains('.') && host != "0.0.0.0" && !host.starts_with("localhost") {
                self.hosts.insert(host);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.hosts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    /// Blocked if the host or any parent domain is listed.
    pub fn is_blocked(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        // Walk "a.b.example.com" -> "b.example.com" -> "example.com" -> "com".
        let mut rest = host.as_str();
        loop {
            if self.hosts.contains(rest) {
                debug_assert!(host_matches(&host, rest));
                return true;
            }
            match rest.split_once('.') {
                Some((_, parent)) => rest = parent,
                None => return false,
            }
        }
    }
}
