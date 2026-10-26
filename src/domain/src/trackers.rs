//! Tracker request blocking: EasyPrivacy network rules turned into a content-blocker rule list
//! (the JSON format WebKit's `WKContentRuleList` / WebKitGTK's `UserContentFilter` compile).
//! Pages themselves are never blocked, only what they load (scripts, pixels, requests, …).

use std::collections::{BTreeSet, HashMap};

use crate::host_matches;

/// Resource types a rule applies to when it names none: everything except the page itself.
const SUBRESOURCES: &[&str] = &["image", "style-sheet", "script", "font", "raw", "svg-document", "media", "ping", "fetch", "websocket", "other"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Rule {
    /// The ABP pattern, lower-cased, for [`TrackerList::blocks`].
    pattern: String,
    /// `url-filter` regex (WebKit's small regex subset).
    filter: String,
    /// `third-party` / `first-party`, or none for both.
    load: Option<&'static str>,
    types: Vec<&'static str>,
    if_domain: Vec<String>,
    unless_domain: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct TrackerList {
    block: BTreeSet<Rule>,
    allow: BTreeSet<Rule>,
    /// The same rules for engines without a content blocker (WebView2 asks per request).
    block_index: Index,
    allow_index: Index,
}

/// `||host…` rules by host, so a request only checks the rules of its host and parent domains.
/// ponytail: the other rules (a few thousand in EasyPrivacy) are scanned, after a cheap
/// substring check; an Aho-Corasick prefilter if that ever shows up in page loads.
#[derive(Debug, Default, Clone)]
struct Index {
    by_host: HashMap<String, Vec<Rule>>,
    rest: Vec<Rule>,
}

impl Index {
    fn add(&mut self, rule: Rule) {
        let rest = rule.pattern.strip_prefix("||").unwrap_or_default();
        let key = rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '-')).next().unwrap_or_default();
        let ends_host = matches!(rest[key.len()..].chars().next(), None | Some('^' | '/' | ':' | '|'));
        if key.contains('.') && ends_host {
            self.by_host.entry(key.to_owned()).or_default().push(rule);
        } else {
            self.rest.push(rule);
        }
    }

    fn any(&self, req: &Request) -> bool {
        let parents = std::iter::successors(Some(req.host), |h| h.split_once('.').map(|(_, p)| p));
        parents.filter_map(|h| self.by_host.get(h)).flatten().chain(&self.rest).any(|r| r.applies(req))
    }
}

#[derive(Clone, Copy)]
struct Request<'a> {
    url: &'a str,
    host: &'a str,
    page: Option<&'a str>,
    ty: &'a str,
    /// Unknown type (`ty` empty) while checking exceptions: typed `@@` rules still apply.
    any_type: bool,
}

impl Rule {
    fn applies(&self, req: &Request) -> bool {
        let third_party = req.page.is_none_or(|p| !host_matches(req.host, p) && !host_matches(p, req.host));
        let on = |d: &String| req.page.is_some_and(|p| host_matches(p, &d[1..]));
        let literal = self.pattern.split(['*', '^', '|']).max_by_key(|l| l.len()).unwrap_or_default();
        match self.load {
            Some("third-party") if !third_party => return false,
            Some("first-party") if third_party => return false,
            _ => {}
        }
        (self.if_domain.is_empty() || self.if_domain.iter().any(on))
            && !self.unless_domain.iter().any(on)
            && (self.types.is_empty() || req.any_type || self.types.contains(&req.ty))
            && req.url.contains(literal)
            && abp_match(&self.pattern, req.url)
    }
}

/// ABP pattern against a lower-cased URL: `||` host anchor, `|` start/end anchors, `*` anything,
/// `^` a separator (or the end).
fn abp_match(pattern: &str, url: &str) -> bool {
    fn at(p: &[u8], t: &[u8], end: bool) -> bool {
        let sep = |c: u8| !(c.is_ascii_alphanumeric() || b"_-.%".contains(&c));
        match p.first() {
            None => !end || t.is_empty(),
            Some(b'*') => (0..=t.len()).any(|i| at(&p[1..], &t[i..], end)),
            Some(b'^') => match t.first() {
                None => at(&p[1..], t, end),
                Some(&c) => sep(c) && at(&p[1..], &t[1..], end),
            },
            Some(c) => t.first() == Some(c) && at(&p[1..], &t[1..], end),
        }
    }
    let (body, end) = pattern.strip_suffix('|').map_or((pattern, false), |b| (b, true));
    let (url, p) = (url.as_bytes(), body.as_bytes());
    if let Some(rest) = p.strip_prefix(b"||") {
        let Some(start) = url.windows(3).position(|w| w == b"://").map(|i| i + 3) else { return false };
        let host_end = url[start..].iter().position(|c| b"/:?#".contains(c)).map_or(url.len(), |i| start + i);
        (start..host_end).filter(|&i| i == start || url[i - 1] == b'.').any(|i| at(rest, &url[i..], end))
    } else if let Some(rest) = p.strip_prefix(b"|") {
        at(rest, url, end)
    } else {
        (0..=url.len()).any(|i| at(p, &url[i..], end))
    }
}

impl TrackerList {
    /// Keeps host and path rules (`||host^`, `||host/path`, `/path/x.js`, `|https://…`) with the
    /// `third-party`, `domain=` and resource-type options, plus their `@@` exceptions. Rules with
    /// other options or characters outside a safe set are skipped: one bad rule would make WebKit
    /// reject the whole list. ponytail: regex rules and `$document`/`$csp`/`$redirect` are skipped.
    pub fn parse(text: &str) -> Self {
        let mut list = Self::default();
        for line in text.lines().map(str::trim) {
            let (exception, rule) = match line.strip_prefix("@@") {
                Some(rest) => (true, rest),
                None => (false, line),
            };
            let Some(parsed) = parse_rule(rule, exception) else { continue };
            if exception { &mut list.allow } else { &mut list.block }.insert(parsed);
        }
        list.block.iter().for_each(|r| list.block_index.add(r.clone()));
        list.allow.iter().for_each(|r| list.allow_index.add(r.clone()));
        list
    }

    /// Request check for engines without a content blocker, same rules as the JSON: `ty` is a
    /// content-blocker resource type (`script`, `image`, `fetch`, …) or `""` when the engine doesn't
    /// say (only untyped block rules apply then, but every exception does), `page` the page's host.
    pub fn blocks(&self, url: &str, page: Option<&str>, ty: &str) -> bool {
        let url = url.to_ascii_lowercase();
        let Some(host) = url::Url::parse(&url).ok().and_then(|u| u.host_str().map(str::to_owned)) else { return false };
        // Untyped rules list every subresource type, so "other" picks exactly those.
        let req = Request { url: &url, host: &host, page, ty: if ty.is_empty() { "other" } else { ty }, any_type: false };
        self.block_index.any(&req) && !self.allow_index.any(&Request { any_type: ty.is_empty(), ..req })
    }

    pub fn len(&self) -> usize {
        self.block.len()
    }

    pub fn is_empty(&self) -> bool {
        self.block.is_empty()
    }

    /// Content-blocker JSON: block rules first, then exceptions (`ignore-previous-rules`).
    /// Filters only contain the safe characters checked in `parse`, so no quotes to escape.
    pub fn to_content_blocker_json(&self) -> String {
        let list = |items: &[String]| items.iter().map(|i| format!("\"{i}\"")).collect::<Vec<_>>().join(",");
        let rule = |r: &Rule, action: &str| {
            let mut trigger = format!(r#""url-filter":"{}""#, r.filter.replace('\\', "\\\\"));
            if let Some(load) = r.load {
                trigger += &format!(r#","load-type":["{load}"]"#);
            }
            if !r.types.is_empty() {
                trigger += &format!(r#","resource-type":[{}]"#, list(&r.types.iter().map(|t| t.to_string()).collect::<Vec<_>>()));
            }
            if !r.if_domain.is_empty() {
                trigger += &format!(r#","if-domain":[{}]"#, list(&r.if_domain));
            } else if !r.unless_domain.is_empty() {
                trigger += &format!(r#","unless-domain":[{}]"#, list(&r.unless_domain));
            }
            format!(r#"{{"trigger":{{{trigger}}},"action":{{"type":"{action}"}}}}"#)
        };
        let rules: Vec<String> =
            self.block.iter().map(|r| rule(r, "block")).chain(self.allow.iter().map(|r| rule(r, "ignore-previous-rules"))).collect();
        format!("[{}]", rules.join(","))
    }
}

fn parse_rule(line: &str, exception: bool) -> Option<Rule> {
    if line.is_empty() || line.starts_with('!') || line.starts_with('[') || line.contains("##") || line.contains("#@#") {
        return None;
    }
    let (pattern, options) = match line.rsplit_once('$') {
        Some((p, o)) => (p, o),
        None => (line, ""),
    };
    let mut rule = Rule { pattern: pattern.to_ascii_lowercase(), filter: String::new(), load: None, types: Vec::new(), if_domain: Vec::new(), unless_domain: Vec::new() };
    for option in options.split(',').filter(|o| !o.is_empty()) {
        let ty = match option {
            "third-party" | "3p" => {
                rule.load = Some("third-party");
                continue;
            }
            "~third-party" | "1p" | "first-party" => {
                rule.load = Some("first-party");
                continue;
            }
            "important" => continue,
            "script" => "script",
            "image" => "image",
            "stylesheet" => "style-sheet",
            "xmlhttprequest" => "fetch",
            "font" => "font",
            "media" => "media",
            "ping" => "ping",
            "websocket" => "websocket",
            "other" => "other",
            _ => {
                let domains = option.strip_prefix("domain=")?;
                for d in domains.split('|') {
                    let (negated, d) = d.strip_prefix('~').map_or((false, d), |d| (true, d));
                    if !is_host(d) {
                        return None;
                    }
                    let entry = format!("*{}", d.to_ascii_lowercase());
                    if negated { &mut rule.unless_domain } else { &mut rule.if_domain }.push(entry);
                }
                // WebKit takes either list, not both.
                if !rule.if_domain.is_empty() && !rule.unless_domain.is_empty() {
                    return None;
                }
                continue;
            }
        };
        rule.types.push(ty);
        if ty == "fetch" {
            rule.types.push("raw");
        }
    }
    rule.filter = to_regex(pattern)?;
    // Plain host rules only block third-party loads, so a tracker's own site still works.
    let plain_host = pattern.strip_prefix("||").and_then(|p| p.strip_suffix('^')).is_some_and(is_host);
    if plain_host && rule.load.is_none() && !exception {
        rule.load = Some("third-party");
    }
    if rule.types.is_empty() && !exception {
        rule.types = SUBRESOURCES.to_vec();
    }
    rule.types.sort_unstable();
    rule.types.dedup();
    Some(rule)
}

fn is_host(s: &str) -> bool {
    s.contains('.') && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// ABP pattern → url-filter regex. `None` for regex rules, unsafe characters, or patterns so
/// short they'd match almost anything.
fn to_regex(pattern: &str) -> Option<String> {
    if pattern.len() > 1 && pattern.starts_with('/') && pattern.ends_with('/') {
        return None; // ABP regex rule
    }
    let (mut regex, rest) = if let Some(rest) = pattern.strip_prefix("||") {
        (String::from(r"^[^:]+://+([^:/]+\.)?"), rest)
    } else if let Some(rest) = pattern.strip_prefix('|') {
        (String::from("^"), rest)
    } else {
        (String::new(), pattern)
    };
    let (body, end_anchor) = match rest.strip_suffix('|') {
        Some(b) => (b, true),
        None => (rest, false),
    };
    let safe = |c: char| c.is_ascii_alphanumeric() || "._~/-=&%,;:@!*^?".contains(c);
    if !body.chars().all(safe) || body.chars().filter(|c| *c != '*' && *c != '^').count() < 4 {
        return None;
    }
    let host_only = pattern.starts_with("||") && !body.contains(['/', '*', '?', '=']);
    let n = body.len();
    for (i, c) in body.char_indices() {
        match c {
            '*' => regex += ".*",
            '^' if i == n - 1 => regex += if host_only { "[:/]" } else { "" },
            '^' => regex += "[^a-zA-Z0-9_.%-]",
            '.' | '?' => {
                regex.push('\\');
                regex.push(c);
            }
            _ => regex.push(c),
        }
    }
    if end_anchor {
        regex.push('$');
    }
    Some(regex)
}
