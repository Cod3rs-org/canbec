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
    fn back(&self) {
        self.js("history.back()");
    }
    fn forward(&self) {
        self.js("history.forward()");
    }
    fn reload(&self) {
        self.js("location.reload()");
    }
    fn page_flags(&self, slop: bool, feeds_locked: bool) {
        self.js(&format!("window.__canbec && window.__canbec.flags({slop}, {feeds_locked})"));
    }
}
