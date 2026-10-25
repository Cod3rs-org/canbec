//! Engine port implemented with wry (WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux).

use canbec_application::ports::Engine;

pub struct WryEngine {
    pub view: wry::WebView,
}

impl WryEngine {
    fn js(&self, script: &str) {
        if let Err(e) = self.view.evaluate_script(script) {
            eprintln!("canbec: script failed: {e}");
        }
    }
}

impl Engine for WryEngine {
    fn navigate(&self, url: &str) {
        if let Err(e) = self.view.load_url(url) {
            eprintln!("canbec: navigation to {url} failed: {e}");
        }
    }
    fn reload(&self) {
        self.js("location.reload()");
    }
    fn page_flags(&self, slop: bool, feeds_locked: bool) {
        self.js(&format!("window.__canbec && window.__canbec.flags({slop}, {feeds_locked})"));
    }
    fn page_ads(&self, hide: &[String], allow: &[String]) {
        let json = |list: &[String]| serde_json::to_string(list).unwrap_or_else(|_| "[]".into());
        self.js(&format!("window.__canbec && window.__canbec.ads({}, {})", json(hide), json(allow)));
    }
    fn find(&self, text: &str, backwards: bool) {
        // window.find is non-standard but built into WebKit and Chromium; it wraps around.
        let text = serde_json::to_string(text).unwrap_or_else(|_| "''".into());
        self.js(&format!("window.find({text}, false, {backwards}, true)"));
    }
    fn zoom(&self, level: f64) {
        if let Err(e) = self.view.zoom(level) {
            eprintln!("canbec: zoom failed: {e}");
        }
    }
}
