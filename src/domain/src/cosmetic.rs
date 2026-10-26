//! Cosmetic ad rules in EasyList syntax: `##sel` hides everywhere, `a.com,b.com##sel` only on
//! those sites, `~a.com##sel` everywhere but there, `a.com#@#sel` makes an exception there. Script
//! and extended syntaxes are skipped.

use crate::host_matches;

#[derive(Debug, Default, Clone)]
pub struct CosmeticRules {
    /// Selectors hidden on every site (the same for every page, so injected once at start).
    pub generic: Vec<String>,
    /// (sites, sites excluded, selector)
    specific: Vec<(Vec<String>, Vec<String>, String)>,
    exceptions: Vec<(Vec<String>, String)>,
}

/// What to add for one site on top of the generic selectors.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SiteCosmetics {
    pub hide: Vec<String>,
    pub allow: Vec<String>,
}

/// Syntax that is not plain CSS (uBlock/AdGuard extensions, scriptlets). Skipped, not guessed.
const UNSUPPORTED: &[&str] =
    ["#?#", "#$#", "#%#", "##+js", ":-abp-", ":has-text(", ":xpath(", ":style(", ":remove(", ":upward(", ":matches-"].as_slice();

impl CosmeticRules {
    pub fn parse(text: &str) -> Self {
        let mut rules = Self::default();
        let mut generic_exceptions = Vec::new();
        for line in text.lines().map(str::trim) {
            if line.starts_with('!') || line.starts_with('[') || UNSUPPORTED.iter().any(|u| line.contains(u)) {
                continue;
            }
            let (domains, selector, exception) = match (line.split_once("#@#"), line.split_once("##")) {
                (Some((d, s)), _) => (d, s, true),
                (None, Some((d, s))) => (d, s, false),
                _ => continue,
            };
            let (excluded, domains): (Vec<String>, Vec<String>) =
                domains.split(',').filter(|d| !d.is_empty()).map(str::to_ascii_lowercase).partition(|d| d.starts_with('~'));
            let excluded: Vec<String> = excluded.into_iter().map(|d| d[1..].to_owned()).collect();
            // ponytail: `~site.com#@#sel` (an exception everywhere but a site) is skipped.
            if selector.is_empty() || exception && !excluded.is_empty() {
                continue;
            }
            let selector = selector.to_owned();
            match (domains.is_empty(), exception) {
                (true, false) if !excluded.is_empty() => {
                    rules.generic.push(selector.clone());
                    rules.exceptions.push((excluded, selector));
                }
                (true, false) => rules.generic.push(selector),
                (true, true) => generic_exceptions.push(selector),
                (false, false) => rules.specific.push((domains, excluded, selector)),
                (false, true) => rules.exceptions.push((domains, selector)),
            }
        }
        rules.generic.retain(|s| !generic_exceptions.contains(s));
        rules
    }

    pub fn len(&self) -> usize {
        self.generic.len() + self.specific.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn for_host(&self, host: &str) -> SiteCosmetics {
        let on = |ds: &[String]| ds.iter().any(|d| host_matches(host, d));
        let hide = self.specific.iter().filter(|(ds, not, _)| on(ds) && !on(not)).map(|(_, _, s)| s.clone()).collect();
        let allow = self.exceptions.iter().filter(|(ds, _)| on(ds)).map(|(_, s)| s.clone()).collect();
        SiteCosmetics { hide, allow }
    }
}
