//! `canbec-servo --window [url]`: canbec in a window on Servo (#32). The same sidebar
//! (`src/ui/sidebar.html`) and core as the wry app, both drawn by Servo:
//!
//! - Two webviews, sidebar and website, each painting offscreen; every frame both are copied into
//!   the window (the sidebar fills it, the website card goes where `domain::layout` says).
//! - Mouse, wheel and keys go to the webview under the cursor / with focus.
//! - Servo has no `window.ipc`, so a tiny shim posts messages as a fetch to
//!   `https://canbec-ipc.invalid/?m=…`, which `load_web_resource` catches and cancels. Only
//!   messages from the sidebar webview are commands; the website's go through the same harmless
//!   filter as on wry (counters, and shortcuts with this run's token).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use canbec_application::ui::{key_command, key_sidebar_call, UiCommand};
use canbec_application::{BrowserService, HOME_URL};
use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::layout::Layout;
use canbec_domain::trackers::TrackerList;
use canbec_infrastructure::store::JsonFileStore;
use canbec_infrastructure::{blocklists, config_dir, content_script, privacy_script, random_token};
use euclid::{Point2D, Rect, Scale, Size2D};
use servo::{
    Code, DevicePoint, EventLoopWaker, InputEvent, Key, KeyState, KeyboardEvent, LoadStatus, Location, Modifiers,
    MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent, NamedKey, NavigationRequest,
    OffscreenRenderingContext, Preferences, RenderingContext, Servo, ServoBuilder, UserContentManager, UserScript, WebResourceLoad,
    WebResourceResponse, WebView, WebViewBuilder, WebViewDelegate, WebViewId, WheelDelta, WheelEvent, WheelMode,
    WindowRenderingContext,
};
use url::Url;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};

use crate::{blocks_resource, now, ServoEngine};

const SIDEBAR_HTML: &str = include_str!("../../../ui/sidebar.html");
const IPC_HOST: &str = "canbec-ipc.invalid";
const IPC_SHIM: &str = r#"window.ipc={postMessage:function(m){fetch("https://canbec-ipc.invalid/?m="+encodeURIComponent(m),{mode:"no-cors"}).catch(function(){})}};"#;

pub fn run(url: Option<String>) {
    let event_loop = EventLoop::<Wake>::with_user_event().build().expect("event loop");
    let mut app = App::Starting { url, waker: Waker(event_loop.create_proxy()) };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("canbec-servo: {e}");
    }
}

#[derive(Debug)]
struct Wake;

#[derive(Clone)]
struct Waker(EventLoopProxy<Wake>);
impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }
    fn wake(&self) {
        let _ = self.0.send_event(Wake);
    }
}

/// What the website's webview did, queued for the app (delegates can't borrow the service).
enum Seen {
    Started(String),
    Finished(String, Option<String>),
    SidebarReady,
}

struct Delegate {
    adult: HostBlocklist,
    trackers: TrackerList,
    sidebar: Cell<Option<WebViewId>>,
    sidebar_url: Url,
    page: RefCell<Option<Url>>,
    /// (from the sidebar, message JSON)
    messages: RefCell<Vec<(bool, String)>>,
    seen: RefCell<Vec<Seen>>,
    refused: Cell<bool>,
    redraw: Cell<bool>,
}

impl Delegate {
    fn is_sidebar(&self, webview: &WebView) -> bool {
        self.sidebar.get() == Some(webview.id())
    }
}

impl WebViewDelegate for Delegate {
    fn request_navigation(&self, webview: WebView, request: NavigationRequest) {
        let ok = if self.is_sidebar(&webview) { request.url == self.sidebar_url } else { !self.adult.blocks_url(request.url.as_str()) };
        if ok {
            request.allow();
        } else {
            self.refused.set(!self.is_sidebar(&webview));
            request.deny();
        }
    }
    fn load_web_resource(&self, webview: WebView, load: WebResourceLoad) {
        let (url, main_frame) = (load.request().url.clone(), load.request().is_for_main_frame);
        if url.host_str() == Some(IPC_HOST) {
            if let Some((_, m)) = url.query_pairs().find(|(k, _)| k == "m") {
                self.messages.borrow_mut().push((self.is_sidebar(&webview), m.into_owned()));
            }
            load.intercept(WebResourceResponse::new(url)).cancel();
            return;
        }
        if self.is_sidebar(&webview) {
            return; // the sidebar only loads its own file
        }
        if main_frame {
            *self.page.borrow_mut() = Some(url.clone());
        }
        if self.adult.blocks_url(url.as_str()) {
            self.refused.set(true);
            load.intercept(WebResourceResponse::new(url)).cancel();
        } else if !main_frame && blocks_resource(&self.trackers, &url, self.page.borrow().as_ref().or(webview.url().as_ref())) {
            load.intercept(WebResourceResponse::new(url)).cancel();
        }
    }
    fn notify_load_status_changed(&self, webview: WebView, status: LoadStatus) {
        let url = || webview.url().map(|u| u.to_string()).unwrap_or_default();
        let seen = match (self.is_sidebar(&webview), status) {
            (true, LoadStatus::Complete) => Seen::SidebarReady,
            (false, LoadStatus::Started) => Seen::Started(self.page.borrow().as_ref().map(|u| u.to_string()).unwrap_or_else(url)),
            // A page without a <title> is listed by its URL, as other browsers do.
            (false, LoadStatus::Complete) => Seen::Finished(url(), webview.page_title().or_else(|| webview.url().map(|u| u.to_string()))),
            _ => return,
        };
        self.seen.borrow_mut().push(seen);
    }
    fn notify_page_title_changed(&self, webview: WebView, title: Option<String>) {
        if !self.is_sidebar(&webview) && webview.load_status() == LoadStatus::Complete {
            let url = webview.url().map(|u| u.to_string()).unwrap_or_default();
            self.seen.borrow_mut().push(Seen::Finished(url, title));
        }
    }
    fn notify_new_frame_ready(&self, _: WebView) {
        self.redraw.set(true);
    }
    fn notify_crashed(&self, _: WebView, reason: String, _: Option<String>) {
        eprintln!("canbec-servo: page crashed: {reason}");
    }
}

#[allow(clippy::large_enum_variant)]
enum App {
    Starting { url: Option<String>, waker: Waker },
    Running(Running),
    Done,
}

struct Running {
    window: Window,
    servo: Servo,
    ctx: Rc<WindowRenderingContext>,
    sidebar: WebView,
    side_ctx: Rc<OffscreenRenderingContext>,
    content: WebView,
    page_ctx: Rc<OffscreenRenderingContext>,
    delegate: Rc<Delegate>,
    service: BrowserService<ServoEngine, JsonFileStore>,
    token: String,
    /// Opened once the first blank page settled (Servo drops a load asked for during another).
    first_url: Option<String>,
    layout: Layout,
    peek: bool,
    settings: bool,
    cursor: PhysicalPosition<f64>,
    modifiers: Modifiers,
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let App::Starting { url, waker } = self {
            *self = match Running::start(event_loop, url.take(), waker.clone()) {
                Some(r) => App::Running(r),
                None => {
                    event_loop.exit();
                    App::Done
                }
            };
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, _: Wake) {
        if let App::Running(r) = self {
            r.pump(event_loop);
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let App::Running(r) = self {
            r.window_event(event_loop, event);
            r.pump(event_loop);
        }
    }
}

impl Running {
    fn start(event_loop: &ActiveEventLoop, url: Option<String>, waker: Waker) -> Option<Running> {
        let attrs = Window::default_attributes().with_title("canbec (Servo)").with_inner_size(PhysicalSize::new(1280, 820));
        let window = event_loop.create_window(attrs).map_err(|e| eprintln!("canbec-servo: window: {e}")).ok()?;
        let display = event_loop.display_handle().ok()?;
        let handle = window.window_handle().ok()?;
        let ctx = Rc::new(
            WindowRenderingContext::new(display, handle, window.inner_size())
                .map_err(|e| eprintln!("canbec-servo: rendering context: {e:?}"))
                .ok()?,
        );
        let _ = ctx.make_current();
        // The settings page is a CSS grid, which Servo 0.1 ships switched off.
        let prefs = Preferences { layout_grid_enabled: true, ..Preferences::default() };
        let servo = ServoBuilder::default().preferences(prefs).event_loop_waker(Box::new(waker)).build();

        let dir = config_dir();
        let lists = dir.join("blocklists");
        let adult = blocklists::load_adult(&lists.join("adult.txt"));
        let ads = blocklists::load_ads(&lists.join("ads.txt"));
        let trackers = blocklists::load_trackers(&lists.join("trackers.txt"));
        let feeds = std::fs::read_to_string(dir.join("feeds.txt")).unwrap_or_default();
        let layout = Layout::parse(&std::fs::read_to_string(dir.join("layout")).unwrap_or_default());

        // The sidebar as a file next to the config (Servo loads file: URLs; no IPC protocol).
        let side_file = dir.join("servo").join("sidebar.html");
        let _ = std::fs::create_dir_all(side_file.parent()?);
        std::fs::write(&side_file, SIDEBAR_HTML).map_err(|e| eprintln!("canbec-servo: {e}")).ok()?;
        let sidebar_url = Url::from_file_path(&side_file).ok()?;

        let delegate = Rc::new(Delegate {
            adult: adult.clone(),
            trackers,
            sidebar: Cell::new(None),
            sidebar_url: sidebar_url.clone(),
            page: RefCell::default(),
            messages: RefCell::default(),
            seen: RefCell::default(),
            refused: Cell::new(false),
            redraw: Cell::new(true),
        });
        let scale = Scale::new(window.scale_factor() as f32);

        let side_scripts = Rc::new(UserContentManager::new(&servo));
        let theme = std::fs::read_to_string(dir.join("theme.css")).unwrap_or_default();
        side_scripts.add_script(Rc::new(UserScript::new(
            format!(
                "{IPC_SHIM}window.__platform={:?};window.__version={:?};window.__layout={:?};\
                 addEventListener('DOMContentLoaded',function(){{var s=document.createElement('style');s.textContent={};document.head.appendChild(s)}});",
                std::env::consts::OS,
                env!("CARGO_PKG_VERSION"),
                layout.name(),
                serde_json::Value::from(theme)
            ),
            None,
        )));
        let side_ctx = Rc::new(ctx.offscreen_context(window.inner_size()));
        let sidebar = WebViewBuilder::new(&servo, side_ctx.clone())
            .user_content_manager(side_scripts)
            .hidpi_scale_factor(scale)
            .delegate(delegate.clone() as Rc<dyn WebViewDelegate>)
            .build();
        delegate.sidebar.set(Some(sidebar.id())); // loads its file once about:blank settled

        let token = random_token();
        let page_scripts = Rc::new(UserContentManager::new(&servo));
        page_scripts.add_script(Rc::new(UserScript::new(IPC_SHIM.to_owned(), None)));
        page_scripts.add_script(Rc::new(UserScript::new(privacy_script(&random_token()), None)));
        // Servo runs user scripts in every frame; the content script is main-frame only elsewhere.
        // ponytail: page world (Servo has no isolated worlds), and a site's CSP connect-src can
        // block the IPC fetch, which only loses the ad counter and in-page shortcuts there.
        let content_js = format!("if (window === window.top) {{\n{}\n}}", content_script(&token, &ads.generic, &feeds));
        page_scripts.add_script(Rc::new(UserScript::new(content_js, None)));
        let page_ctx = Rc::new(ctx.offscreen_context(PhysicalSize::new(1, 1)));
        let content = WebViewBuilder::new(&servo, page_ctx.clone())
            .user_content_manager(page_scripts)
            .hidpi_scale_factor(scale)
            .delegate(delegate.clone() as Rc<dyn WebViewDelegate>)
            .build();
        content.focus();

        // Its own state file: the wry app's tabs stay untouched. ponytail: no profiles here yet.
        let store = JsonFileStore::new(dir.join("servo").join("state.json"));
        let mut service = BrowserService::start(ServoEngine { webview: content.clone() }, store, adult, now());
        service.set_ad_rules(ads);

        let mut r = Running {
            window,
            servo,
            ctx,
            sidebar,
            side_ctx,
            content,
            page_ctx,
            delegate,
            service,
            token,
            first_url: Some(url.unwrap_or_else(|| HOME_URL.to_owned())),
            layout,
            peek: false,
            settings: false,
            cursor: PhysicalPosition::new(0.0, 0.0),
            modifiers: Modifiers::empty(),
        };
        r.relayout();
        Some(r)
    }

    /// The website card in physical pixels: (x, y, width, height), y from the top.
    fn card(&self) -> (i32, i32, u32, u32) {
        let size = self.window.inner_size();
        let s = self.window.scale_factor();
        let (x, y, w, h) = self.layout.card(size.width as f64 / s, size.height as f64 / s, self.peek);
        ((x * s) as i32, (y * s) as i32, (w * s).max(1.0) as u32, (h * s).max(1.0) as u32)
    }

    fn relayout(&mut self) {
        let size = self.window.inner_size();
        self.ctx.resize(size);
        self.sidebar.resize(size);
        let (_, _, w, h) = self.card();
        self.content.resize(PhysicalSize::new(w, h));
        self.window.request_redraw();
    }

    fn paint(&self) {
        let _ = self.ctx.make_current();
        self.sidebar.paint();
        if !self.settings {
            self.content.paint();
        }
        self.ctx.prepare_for_rendering();
        let gl = self.ctx.glow_gl_api();
        let size = self.window.inner_size();
        let (wh, ww) = (size.height as i32, size.width as i32);
        if let Some(blit) = self.side_ctx.render_to_parent_callback() {
            blit(&gl, Rect::new(Point2D::new(0, 0), Size2D::new(ww, wh)));
        }
        // ponytail: square card; rounding needs a stencil or shader pass in the blit.
        if !self.settings {
            if let Some(blit) = self.page_ctx.render_to_parent_callback() {
                let (x, y, w, h) = self.card();
                blit(&gl, Rect::new(Point2D::new(x, wh - y - h as i32), Size2D::new(w as i32, h as i32)));
            }
        }
        self.ctx.present();
    }

    /// The webview under a window point, and the point in its own coordinates.
    fn target(&self, p: PhysicalPosition<f64>) -> (&WebView, DevicePoint) {
        let (x, y, w, h) = self.card();
        let inside = p.x >= x as f64 && p.y >= y as f64 && p.x < (x + w as i32) as f64 && p.y < (y + h as i32) as f64;
        if inside && !self.settings {
            (&self.content, DevicePoint::new((p.x - x as f64) as f32, (p.y - y as f64) as f32))
        } else {
            (&self.sidebar, DevicePoint::new(p.x as f32, p.y as f32))
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.paint(),
            WindowEvent::Resized(_) => self.relayout(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = position;
                let (view, point) = self.target(position);
                view.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(point.into())));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    winit::event::MouseButton::Back => MouseButton::Back,
                    winit::event::MouseButton::Forward => MouseButton::Forward,
                    winit::event::MouseButton::Other(n) => MouseButton::Other(n),
                };
                let action = if state == ElementState::Pressed { MouseButtonAction::Down } else { MouseButtonAction::Up };
                let (view, point) = self.target(self.cursor);
                if action == MouseButtonAction::Down && !view.focused() {
                    // Focus follows clicks; leaving the sidebar fires its blur (folds a peek).
                    let other = if view.id() == self.sidebar.id() { &self.content } else { &self.sidebar };
                    other.blur();
                    view.focus();
                }
                view.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(action, button, point.into())));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y, mode) = match delta {
                    MouseScrollDelta::LineDelta(dx, dy) => (dx as f64 * 76.0, dy as f64 * 76.0, WheelMode::DeltaLine),
                    MouseScrollDelta::PixelDelta(d) => (d.x, d.y, WheelMode::DeltaPixel),
                };
                let (view, point) = self.target(self.cursor);
                view.notify_input_event(InputEvent::Wheel(WheelEvent::new(WheelDelta { x, y, z: 0.0, mode }, point.into())));
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                let mut out = Modifiers::empty();
                out.set(Modifiers::SHIFT, s.shift_key());
                out.set(Modifiers::CONTROL, s.control_key());
                out.set(Modifiers::ALT, s.alt_key());
                out.set(Modifiers::META, s.super_key());
                self.modifiers = out;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let state = if event.state == ElementState::Pressed { KeyState::Down } else { KeyState::Up };
                let view = if self.sidebar.focused() { &self.sidebar } else { &self.content };
                let key = KeyboardEvent::new_without_event(
                    state,
                    key(&event.logical_key),
                    code(event.physical_key),
                    Location::Standard,
                    self.modifiers,
                    event.repeat,
                    false,
                );
                view.notify_input_event(InputEvent::Keyboard(key));
            }
            _ => {}
        }
    }

    /// Runs Servo, then everything its delegate queued, then pushes state to the sidebar.
    fn pump(&mut self, event_loop: &ActiveEventLoop) {
        self.servo.spin_event_loop();
        let mut render = false;
        for seen in self.delegate.seen.take() {
            match seen {
                Seen::SidebarReady if self.sidebar.url().as_ref() != Some(&self.delegate.sidebar_url) => {
                    self.sidebar.load(self.delegate.sidebar_url.clone());
                }
                Seen::SidebarReady => {
                    let _ = self.sidebar.evaluate_javascript(format!("window.canbec.setLayout({:?})", self.layout.name()), |_| {});
                    render = true;
                }
                Seen::Started(url) => {
                    self.service.on_page(&url, None, true, now());
                    render = true;
                }
                Seen::Finished(url, title) => {
                    self.service.on_page(&url, title.as_deref(), false, now());
                    if let Some(first) = self.first_url.take() {
                        self.service.handle(UiCommand::Navigate { url: first }, now());
                    }
                    render = true;
                }
            }
        }
        if self.delegate.refused.take() {
            self.service.on_blocked();
            render = true;
        }
        for (from_sidebar, json) in self.delegate.messages.take() {
            render |= if from_sidebar { self.sidebar_message(event_loop, &json) } else { self.page_message(&json) };
        }
        if render {
            let state = serde_json::to_string(&self.service.state(now())).unwrap_or_default();
            let _ = self.sidebar.evaluate_javascript(format!("window.canbec && window.canbec.render({state})"), |_| {});
        }
        if self.delegate.redraw.take() {
            self.window.request_redraw();
        }
    }

    /// A command from the trusted sidebar. True when the sidebar needs fresh state.
    fn sidebar_message(&mut self, event_loop: &ActiveEventLoop, json: &str) -> bool {
        let msg: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
        match msg["type"].as_str().unwrap_or_default() {
            "key" => self.key(msg["key"].as_str().unwrap_or_default()),
            "window" => {
                match msg["action"].as_str() {
                    Some("drag") => drop(self.window.drag_window()),
                    Some("minimize") => self.window.set_minimized(true),
                    Some("maximize") => self.window.set_maximized(!self.window.is_maximized()),
                    Some("close") => event_loop.exit(),
                    _ => {}
                }
                false
            }
            "setLayout" => {
                self.layout = Layout::parse(msg["name"].as_str().unwrap_or_default());
                self.peek = false;
                let _ = std::fs::write(config_dir().join("layout"), self.layout.name());
                self.relayout();
                false
            }
            "peek" => {
                self.peek = msg["open"].as_bool() == Some(true);
                self.relayout();
                false
            }
            "settingsView" => {
                self.settings = msg["open"].as_bool() == Some(true);
                self.window.request_redraw();
                true
            }
            // ponytail: app-level settings the Servo window doesn't do yet (profiles, icon,
            // default browser, family DNS, AI text model, config folder, downloads, updates).
            "setIcon" | "openConfig" | "openDownloads" | "makeDefaultBrowser" | "switchWorkspace" | "addProfile"
            | "setAgents" | "enableFamilyDns" | "aiText" | "installUpdate" | "exportSlop" | "importSlop" => {
                self.service.notify("Not in the Servo build yet");
                true
            }
            _ => match serde_json::from_str::<UiCommand>(json) {
                Ok(cmd) => {
                    self.service.handle(cmd, now());
                    true
                }
                Err(e) => {
                    eprintln!("canbec-servo: bad UI command {json}: {e}");
                    false
                }
            },
        }
    }

    /// A message from the website: only counters, and shortcuts carrying this run's token.
    fn page_message(&mut self, json: &str) -> bool {
        let msg: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
        match (msg["type"].as_str(), msg["count"].as_u64()) {
            (Some("adsHidden"), Some(n)) => self.service.on_ads_hidden(n.min(10_000) as u32),
            (Some("aiImages"), Some(n)) => self.service.on_ai_images(n.min(10_000) as u32),
            (Some("key"), _) if msg["token"].as_str() == Some(&self.token) => return self.key(msg["key"].as_str().unwrap_or_default()),
            _ => return false,
        }
        true
    }

    fn key(&mut self, key: &str) -> bool {
        if let Some(call) = key_sidebar_call(key) {
            self.content.blur();
            self.sidebar.focus();
            let _ = self.sidebar.evaluate_javascript(format!("window.canbec.{call}()"), |_| {});
        }
        match key_command(key, &self.service.state(now())) {
            Some(cmd) => {
                self.service.handle(cmd, now());
                true
            }
            None => false,
        }
    }
}

fn key(k: &winit::keyboard::Key) -> Key {
    use winit::keyboard::{Key as W, NamedKey as N};
    let named = match k {
        W::Character(s) => return Key::Character(s.to_string()),
        W::Named(N::Space) => return Key::Character(" ".into()),
        W::Named(n) => n,
        _ => return Key::Named(NamedKey::Unidentified),
    };
    Key::Named(match named {
        N::Enter => NamedKey::Enter,
        N::Tab => NamedKey::Tab,
        N::Backspace => NamedKey::Backspace,
        N::Delete => NamedKey::Delete,
        N::Escape => NamedKey::Escape,
        N::ArrowLeft => NamedKey::ArrowLeft,
        N::ArrowRight => NamedKey::ArrowRight,
        N::ArrowUp => NamedKey::ArrowUp,
        N::ArrowDown => NamedKey::ArrowDown,
        N::Home => NamedKey::Home,
        N::End => NamedKey::End,
        N::PageUp => NamedKey::PageUp,
        N::PageDown => NamedKey::PageDown,
        N::Shift => NamedKey::Shift,
        N::Control => NamedKey::Control,
        N::Alt => NamedKey::Alt,
        N::Super | N::Meta => NamedKey::Meta,
        N::F5 => NamedKey::F5,
        // ponytail: the keys a browser chrome and forms need; add more as they come up.
        _ => NamedKey::Unidentified,
    })
}

/// winit's key codes use the same W3C names as `Code`, so the name carries over.
fn code(k: winit::keyboard::PhysicalKey) -> Code {
    match k {
        winit::keyboard::PhysicalKey::Code(c) => format!("{c:?}").parse().unwrap_or(Code::Unidentified),
        _ => Code::Unidentified,
    }
}
