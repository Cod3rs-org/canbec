//! Engine bake-off: Servo.
//! Usage: engine-servo <url> <out.png> [wait_secs]
//! Loads the page, waits `wait_secs` after the load event (so WASM apps like Blazor can boot),
//! then saves a screenshot. Adapted from simonw/research servo-shot (MPL-2.0).

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use dpi::PhysicalSize;
use euclid::{Box2D, Point2D};
use servo::{
    LoadStatus, Preferences, RenderingContext, ServoBuilder, SoftwareRenderingContext, WebView,
    WebViewBuilder, WebViewDelegate,
};

const W: u32 = 1280;
const H: u32 = 800;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let url = url::Url::parse(args.get(1).context("usage: engine-servo <url> <out.png> [wait]")?)?;
    let out = args.get(2).map(String::as_str).unwrap_or("servo.png");
    let wait: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);

    let ctx: Rc<dyn RenderingContext> = Rc::new(
        SoftwareRenderingContext::new(PhysicalSize::new(W, H))
            .map_err(|e| anyhow::anyhow!("rendering context: {e:?}"))?,
    );
    let _ = ctx.make_current();

    let mut prefs = Preferences::default();
    prefs.network_http_proxy_uri = String::new();
    prefs.network_https_proxy_uri = String::new();
    let servo = ServoBuilder::default().preferences(prefs).build();
    servo.setup_logging();

    let delegate = Rc::new(Delegate::default());
    let _webview = WebViewBuilder::new(&servo, ctx.clone())
        .url(url)
        .delegate(delegate.clone() as Rc<dyn WebViewDelegate>)
        .build();

    let start = Instant::now();
    let mut loaded_at = None;
    loop {
        servo.spin_event_loop();
        std::thread::sleep(Duration::from_millis(2));
        if loaded_at.is_none() && delegate.loaded.get() {
            loaded_at = Some(Instant::now());
            eprintln!("servo: load complete after {:?}", start.elapsed());
        }
        match loaded_at {
            Some(t) if t.elapsed() > Duration::from_secs(wait) => break,
            None if start.elapsed() > Duration::from_secs(60) => bail!("load timed out"),
            _ => {}
        }
    }

    let img = ctx
        .read_to_image(Box2D::new(Point2D::new(0, 0), Point2D::new(W as i32, H as i32)))
        .context("read_to_image")?;
    img.save(out)?;
    eprintln!("servo: wrote {out} ({} frames painted)", delegate.frames.get());
    Ok(())
}

#[derive(Default)]
struct Delegate {
    loaded: Cell<bool>,
    frames: Cell<u32>,
}

impl WebViewDelegate for Delegate {
    fn notify_load_status_changed(&self, _: WebView, status: LoadStatus) {
        if matches!(status, LoadStatus::Complete) {
            self.loaded.set(true);
        }
    }
    // Servo only fills the framebuffer if the embedder calls paint() on every new frame.
    fn notify_new_frame_ready(&self, webview: WebView) {
        webview.paint();
        self.frames.set(self.frames.get() + 1);
    }
}
