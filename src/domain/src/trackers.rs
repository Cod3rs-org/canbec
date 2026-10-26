//! Tracker request blocking: EasyPrivacy network rules turned into a content-blocker rule list
//! (the JSON format WebKit's `WKContentRuleList` / WebKitGTK's `UserContentFilter` compile).
//! Pages themselves are never blocked, only what they load (scripts, pixels, requests, …).

use std::collections::BTreeSet;

/// Resource types a rule applies to when it names none: everything except the page itself.
const SUBRESOURCES: &[&str] = &["image", "style-sheet", "script", "font", "raw", "svg-document", "media", "ping", "fetch", "websocket", "other"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Rule {
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
            if let Some(rule) = parse_rule(rule, exception) {
                if exception { &mut list.allow } else { &mut list.block }.insert(rule);
            }
        }
        list
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
    let mut rule = Rule { filter: String::new(), load: None, types: Vec::new(), if_domain: Vec::new(), unless_domain: Vec::new() };
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
