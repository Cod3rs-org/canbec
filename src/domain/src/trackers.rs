//! Tracker request blocking: EasyPrivacy's `||host^` rules turned into a content-blocker rule
//! list (the JSON format WebKit's `WKContentRuleList` / WebKitGTK's `UserContentFilter` compile).
//! Only third-party loads are blocked, so visiting a tracker's own site still works.

use std::collections::BTreeSet;

#[derive(Debug, Default, Clone)]
pub struct TrackerList {
    block: BTreeSet<String>,
    allow: BTreeSet<String>,
}

impl TrackerList {
    /// Keeps `||host^` and `||host^$third-party` rules and global `@@||host^…` exceptions. Path rules and
    /// other options are skipped (ponytail: they are <20% of EasyPrivacy and need a real matcher).
    pub fn parse(text: &str) -> Self {
        let mut list = Self::default();
        for line in text.lines().map(str::trim) {
            let (exception, rule) = match line.strip_prefix("@@") {
                Some(rest) => (true, rest),
                None => (false, line),
            };
            let Some(rest) = rule.strip_prefix("||") else { continue };
            let Some((host, options)) = rest.split_once('^') else { continue };
            let plain = host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') && host.contains('.');
            // Site-specific exceptions (`domain=`) would unblock the host everywhere: skipped.
            let applies = if exception { !options.contains("domain=") } else { options.is_empty() || options == "$third-party" };
            if !plain || !applies {
                continue;
            }
            let set = if exception { &mut list.allow } else { &mut list.block };
            set.insert(host.to_ascii_lowercase());
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
    /// Hosts are validated to `[a-z0-9.-]` in `parse`, so plain formatting is safe JSON.
    pub fn to_content_blocker_json(&self) -> String {
        let rule = |host: &String, action: &str| {
            format!(
                r#"{{"trigger":{{"url-filter":"^[^:]+://+([^:/]+\\.)?{}[:/]","load-type":["third-party"]}},"action":{{"type":"{action}"}}}}"#,
                host.replace('.', "\\\\.")
            )
        };
        let rules: Vec<String> =
            self.block.iter().map(|h| rule(h, "block")).chain(self.allow.iter().map(|h| rule(h, "ignore-previous-rules"))).collect();
        format!("[{}]", rules.join(","))
    }
}
