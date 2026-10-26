//! `canbec-servo <url> [out.png]`: loads a page through canbec's core and filters on Servo,
//! headless (software rendering, no window), saves a screenshot and prints what canbec's
//! agent view (`readable()`) sees. It proves the Engine port works on a second engine; a windowed
//! Servo build needs input and compositing glue (see docs/research/05).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use canbec_application::ports::StateStore;
use canbec_application::ui::UiCommand;
use canbec_application::{BrowserService, Snapshot};
use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::host_of;
use canbec_domain::tabs::TabPolicy;
use canbec_domain::trackers::TrackerList;
use canbec_infrastructure::{blocklists, config_dir, content_script, privacy_script, random_token};
use canbec_servo::{allows_navigation, blocks_resource, ServoEngine};
use dpi::PhysicalSize;
use euclid::{Box2D, Point2D};
use servo::{
    JSValue, LoadStatus, NavigationRequest, Preferences, RenderingContext, ServoBuilder, SoftwareRenderingContext,
    UserContentManager, UserScript, WebResourceLoad, WebResourceResponse, WebView, WebViewBuilder, WebViewDelegate,
};

const W: u32 = 1280;
const H: u32 = 800;

/// A one-off run starts on a blank tab (not the home page) and never touches the user's tabs.
struct BlankTab;
impl StateStore for BlankTab {
    fn load(&self) -> Option<Snapshot> {
        let mut snapshot = Snapshot::default();
        snapshot.tabs.open("about:blank", &TabPolicy::default(), now()).ok()?;
        Some(snapshot)
    }
    fn save(&self, _: &Snapshot) {}
}

/// What Servo reports, queued for the main loop (the delegate can't borrow the service).
enum Seen {
    Started(String),
    Finished(String, Option<String>),
}

struct Delegate {
    adult: HostBlocklist,
    trackers: TrackerList,
    blocked: Cell<u32>,
    refused: RefCell<Vec<String>>,
    seen: RefCell<Vec<Seen>>,
}

impl WebViewDelegate for Delegate {
    fn request_navigation(&self, _: WebView, request: NavigationRequest) {
        if allows_navigation(&self.adult, &request.url) {
            request.allow();
        } else {
            self.refused.borrow_mut().push(request.url.to_string());
            request.deny();
        }
    }
    fn load_web_resource(&self, webview: WebView, load: WebResourceLoad) {
        let (url, main_frame) = (load.request().url.clone(), load.request().is_for_main_frame);
        // Loads the app starts itself skip request_navigation, so pages are checked here too.
        if main_frame && !allows_navigation(&self.adult, &url) {
            self.refused.borrow_mut().push(url.to_string());
            load.intercept(WebResourceResponse::new(url)).cancel();
        } else if !main_frame && blocks_resource(&self.trackers, &url, webview.url().as_ref()) {
            self.blocked.set(self.blocked.get() + 1);
            load.intercept(WebResourceResponse::new(url)).cancel();
        }
        // not intercepted: dropping `load` lets it through
    }
    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        let url = webview.url().map(|u| u.to_string()).unwrap_or_default();
        match status {
            LoadStatus::Started => self.seen.borrow_mut().push(Seen::Started(url)),
            LoadStatus::Complete => self.seen.borrow_mut().push(Seen::Finished(url, webview.page_title())),
            _ => {}
        }
    }
    fn notify_crashed(&self, _: WebView, reason: String, _: Option<String>) {
        eprintln!("canbec-servo: page crashed: {reason}");
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(url) = args.get(1).cloned() else {
        eprintln!("usage: canbec-servo <url> [out.png]");
        std::process::exit(2);
    };
    let out = args.get(2).cloned().unwrap_or_else(|| "canbec-servo.png".into());
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // The same lists and scripts the browser uses (cached copies or the built-in seeds).
    let dir = config_dir();
    let lists = dir.join("blocklists");
    let adult = blocklists::load_adult(&lists.join("adult.txt"));
    let ads = blocklists::load_ads(&lists.join("ads.txt"));
    let trackers = blocklists::load_trackers(&lists.join("trackers.txt"));

    let ctx: Rc<dyn RenderingContext> =
        Rc::new(SoftwareRenderingContext::new(PhysicalSize::new(W, H)).expect("software rendering context"));
    let _ = ctx.make_current();
    let servo = ServoBuilder::default().preferences(Preferences::default()).build();
    // ponytail: page world and no window.ipc on Servo yet, so content messages (ad counts,
    // shortcuts) don't reach the core; the flags and ad rules the core sends do work.
    let scripts = Rc::new(UserContentManager::new(&servo));
    scripts.add_script(Rc::new(UserScript::new(privacy_script(&random_token()), None)));
    scripts.add_script(Rc::new(UserScript::new(content_script(&random_token(), &ads.generic, ""), None)));
    let delegate = Rc::new(Delegate {
        adult: adult.clone(),
        trackers,
        blocked: Cell::new(0),
        refused: RefCell::default(),
        seen: RefCell::default(),
    });
    let webview = WebViewBuilder::new(&servo, ctx.clone())
        .user_content_manager(scripts)
        .delegate(delegate.clone() as Rc<dyn WebViewDelegate>)
        .build();

    // Servo drops a load asked for while another one is still pending (its first about:blank,
    // then the service's blank tab), so each one finishes first.
    let settle = || {
        while !delegate.seen.take().iter().any(|s| matches!(s, Seen::Finished(..))) {
            servo.spin_event_loop();
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    settle();
    let mut service = BrowserService::start(ServoEngine { webview: webview.clone() }, BlankTab, adult, now());
    service.set_ad_rules(ads);
    settle();
    service.handle(UiCommand::Navigate { url: url.clone() }, now()); // the core's own checks first
    if let Some(notice) = service.state(now()).notice {
        println!("notice:   {notice}");
        return;
    }

    let start = Instant::now();
    let mut done_at = None;
    while done_at.is_none_or(|t: Instant| t.elapsed() < Duration::from_secs(3)) {
        servo.spin_event_loop();
        std::thread::sleep(Duration::from_millis(2));
        for seen in delegate.seen.take() {
            match seen {
                Seen::Started(u) => service.on_page(&u, None, true, now()),
                Seen::Finished(u, title) => {
                    service.on_page(&u, title.as_deref(), false, now()); // pushes slop/feed flags
                    if host_of(&u).is_some() && host_of(&u) == host_of(&url) {
                        done_at.get_or_insert_with(Instant::now); // the page we asked for, not the start page
                    }
                }
            }
        }
        if !delegate.refused.borrow().is_empty() {
            service.on_blocked();
            break;
        }
        if done_at.is_none() && start.elapsed() > Duration::from_secs(60) {
            eprintln!("canbec-servo: load timed out");
            std::process::exit(1);
        }
    }

    let state = service.state(now());
    println!("page:     {} ({})", state.current.title, state.current.url);
    println!("refused:  {:?}", delegate.refused.borrow());
    println!("trackers: {} requests blocked", delegate.blocked.get());
    if let Some(notice) = state.notice {
        println!("notice:   {notice}");
    }
    if delegate.refused.borrow().is_empty() {
        let readable = Rc::new(RefCell::new(None));
        let r = readable.clone();
        webview.evaluate_javascript("JSON.stringify(window.__canbec && window.__canbec.readable())", move |v| {
            *r.borrow_mut() = Some(v);
        });
        while readable.borrow().is_none() {
            servo.spin_event_loop();
            std::thread::sleep(Duration::from_millis(2));
        }
        match readable.take() {
            Some(Ok(JSValue::String(json))) => println!("readable: {} chars of JSON, starts {:?}", json.len(), json.chars().take(160).collect::<String>()),
            other => println!("readable: {other:?}"),
        }
        webview.paint();
        let image = ctx.read_to_image(Box2D::new(Point2D::new(0, 0), Point2D::new(W as i32, H as i32)));
        match image.map(|i| i.save(&out)) {
            Some(Ok(())) => println!("saved:    {out}"),
            other => eprintln!("canbec-servo: screenshot failed: {other:?}"),
        }
    }
}
