//! Engine port implemented with wry (WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux).

use canbec_application::ports::Engine;

use crate::content_calls;

pub struct WryEngine {
    pub view: wry::WebView,
    /// Runs a script where content.js lives: the page world ([`page_world`]) or the app's
    /// isolated world (#20, per OS in `app`).
    pub eval: fn(&wry::WebView, &str),
}

pub fn page_world(view: &wry::WebView, script: &str) {
    if let Err(e) = view.evaluate_script(script) {
        eprintln!("canbec: script failed: {e}");
    }
}

impl WryEngine {
    /// A script for content.js (`window.__canbec`) or the extension runtime.
    pub fn js(&self, script: &str) {
        (self.eval)(&self.view, script)
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
    fn page_flags(&self, slop: bool, feeds_locked: bool, read_text: bool) {
        self.js(&content_calls::flags(slop, feeds_locked, read_text));
    }
    fn page_ads(&self, hide: &[String], allow: &[String]) {
        self.js(&content_calls::ads(hide, allow));
    }
    fn find(&self, text: &str, backwards: bool) {
        self.js(&content_calls::find(text, backwards));
    }
    fn zoom(&self, level: f64) {
        if let Err(e) = self.view.zoom(level) {
            eprintln!("canbec: zoom failed: {e}");
        }
    }
}
