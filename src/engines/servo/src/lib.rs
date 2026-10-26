//! canbec's `Engine` port on Servo (#32). Servo is written in Rust and owns its network stack, so
//! filtering is the same on every OS: Servo asks before each navigation (adult block) and each
//! subresource load (tracker rules), and runs our user scripts. See docs/research/05.

use canbec_application::ports::Engine;
use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::host_of;
use canbec_domain::trackers::TrackerList;
use canbec_infrastructure::content_calls;
use servo::WebView;
use url::Url;

pub struct ServoEngine {
    pub webview: WebView,
}

impl ServoEngine {
    fn js(&self, script: String) {
        self.webview.evaluate_javascript(script, |result| {
            if let Err(e) = result {
                eprintln!("canbec-servo: script failed: {e:?}");
            }
        });
    }
}

impl Engine for ServoEngine {
    fn navigate(&self, url: &str) {
        match Url::parse(url) {
            Ok(url) => self.webview.load(url),
            Err(e) => eprintln!("canbec-servo: bad URL {url}: {e}"),
        }
    }
    fn reload(&self) {
        self.webview.reload();
    }
    fn page_flags(&self, slop: bool, feeds_locked: bool, read_text: bool) {
        self.js(content_calls::flags(slop, feeds_locked, read_text));
    }
    fn page_ads(&self, hide: &[String], allow: &[String]) {
        self.js(content_calls::ads(hide, allow));
    }
    fn find(&self, text: &str, backwards: bool) {
        self.js(content_calls::find(text, backwards));
    }
    fn zoom(&self, level: f64) {
        self.webview.set_page_zoom(level as f32);
    }
}

/// Top-level pages and frames: refused when the host is on the adult list.
pub fn allows_navigation(adult: &HostBlocklist, url: &Url) -> bool {
    !host_of(url.as_str()).is_some_and(|h| adult.is_blocked(&h))
}

/// Subresources: the same EasyPrivacy rules as the other engines. Servo doesn't say what kind of
/// resource a request is, so typed rules (`$image`, `$script`) don't apply here.
/// ponytail: guess the type from `Sec-Fetch-Dest` once Servo sends it to the embedder.
pub fn blocks_resource(trackers: &TrackerList, url: &Url, page: Option<&Url>) -> bool {
    trackers.blocks(url.as_str(), page.and_then(|p| host_of(p.as_str())).as_deref(), "other")
}
