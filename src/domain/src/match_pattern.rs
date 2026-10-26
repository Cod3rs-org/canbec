//! Chrome extension match patterns (`<all_urls>`, `*://*.example.com/*`, `https://a.example/path*`).
//! https://developer.chrome.com/docs/extensions/develop/concepts/match-patterns

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPattern {
    /// `*` (http or https), `http`, `https`.
    pub scheme: String,
    /// `*` any host, `*.example.com` that domain and subdomains, or an exact host.
    pub host: String,
    /// Glob where `*` matches anything.
    pub path: String,
}

impl MatchPattern {
    pub fn parse(pattern: &str) -> Option<Self> {
        if pattern == "<all_urls>" {
            return Some(Self { scheme: "*".into(), host: "*".into(), path: "/*".into() });
        }
        let (scheme, rest) = pattern.split_once("://")?;
        if !matches!(scheme, "*" | "http" | "https") {
            return None; // file:, ftp:, chrome-extension: aren't supported
        }
        let (host, path) = rest.find('/').map(|i| rest.split_at(i))?;
        let host_ok = host == "*" || host.strip_prefix("*.").unwrap_or(host).chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
        if host.is_empty() || !host_ok || host[1..].contains('*') && host != "*" {
            return None;
        }
        Some(Self { scheme: scheme.into(), host: host.to_ascii_lowercase(), path: path.into() })
    }

    pub fn matches(&self, url: &str) -> bool {
        let Ok(url) = url::Url::parse(url) else { return false };
        let scheme_ok = match self.scheme.as_str() {
            "*" => matches!(url.scheme(), "http" | "https"),
            s => url.scheme() == s,
        };
        let host = url.host_str().unwrap_or_default();
        let host_ok = match self.host.strip_prefix("*.") {
            _ if self.host == "*" => true,
            Some(domain) => host == domain || host.ends_with(&format!(".{domain}")),
            None => host == self.host,
        };
        let path = match url.query() {
            Some(q) => format!("{}?{q}", url.path()),
            None => url.path().to_owned(),
        };
        scheme_ok && host_ok && glob(&self.path, &path)
    }

    /// The same rule as a JavaScript regex source, for scripts that match `location.href`
    /// (without its `#fragment`) in the page. Any port matches, as in Chrome.
    pub fn to_regex(&self) -> String {
        let esc = |s: &str| s.chars().map(|c| if "\\^$.|?*+()[]{}/".contains(c) { format!("\\{c}") } else { c.to_string() }).collect::<String>();
        let scheme = if self.scheme == "*" { "https?".to_owned() } else { self.scheme.clone() };
        let host = match self.host.strip_prefix("*.") {
            _ if self.host == "*" => "[^/:]+".to_owned(),
            Some(domain) => format!("([^/:]+\\.)?{}", esc(domain)),
            None => esc(&self.host),
        };
        let path = self.path.split('*').map(esc).collect::<Vec<_>>().join(".*");
        format!("^{scheme}:\\/\\/{host}(:\\d+)?{path}$")
    }
}

/// `*` matches any run of characters, everything else literally.
pub fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !text.starts_with(first) || !text[first.len()..].ends_with(last) || text.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(i) => rest = &rest[i + part.len()..],
            None => return false,
        }
    }
    true
}
