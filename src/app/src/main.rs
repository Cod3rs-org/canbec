//! canbec — composition root. One window, two webviews:
//! the sidebar (our UI, `src/ui/sidebar.html`) and the content (websites).
//! All events funnel into the event loop, which owns the `BrowserService`.

use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use canbec_application::ui::UiCommand;
use canbec_application::BrowserService;
use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::host_of;
use canbec_infrastructure::engine_wry::WryEngine;
use canbec_infrastructure::store::JsonFileStore;
use canbec_infrastructure::icon::{load_icon, AppIcon, BUILT_IN};
use canbec_infrastructure::{blocklists, config_dir, CONTENT_SCRIPT};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Icon, Window, WindowBuilder};
use wry::{dpi, PageLoadEvent, Rect, WebViewBuilder};

const SIDEBAR_HTML: &str = include_str!("../../ui/sidebar.html");
const SIDEBAR_WIDTH: f64 = 300.0;
const CARD_GAP: f64 = 8.0;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const CARD_RADIUS: f64 = 12.0;

enum AppEvent {
    Ui(String),
    Content(String),
    PageLoad { url: String, finished: bool },
    Popup(String),
    Title(String),
    Blocked,
    SidebarReady,
    AdultList(HostBlocklist),
    Tick,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The sidebar webview fills the window (its background is the frame); the website floats
/// on top as an inset card. ponytail: square corners; rounding needs a native layer per OS.
fn bounds(window: &Window, sidebar: bool) -> Rect {
    let size = window.inner_size().to_logical::<f64>(window.scale_factor());
    let (x, y, w, h) = if sidebar {
        (0.0, 0.0, size.width, size.height)
    } else {
        (SIDEBAR_WIDTH, CARD_GAP, size.width - SIDEBAR_WIDTH - CARD_GAP, size.height - 2.0 * CARD_GAP)
    };
    Rect { position: dpi::LogicalPosition::new(x, y).into(), size: dpi::LogicalSize::new(w.max(0.0), h.max(0.0)).into() }
}

/// User theme: `<config>/theme.css` overrides the sidebar's CSS variables (see docs/research/07).
fn theme_script() -> String {
    let css = std::fs::read_to_string(config_dir().join("theme.css")).unwrap_or_default();
    format!(
        "addEventListener('DOMContentLoaded',()=>{{const s=document.createElement('style');s.textContent={};document.head.appendChild(s)}})",
        serde_json::to_string(&css).unwrap_or_else(|_| "''".into())
    )
}

/// Built-in icons for the settings picker, plus which one is active.
fn icons_script(current: &str) -> String {
    use base64::Engine as _;
    let list: Vec<_> = BUILT_IN
        .iter()
        .map(|(name, meaning, png)| {
            serde_json::json!({ "name": name, "meaning": meaning,
                "src": format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)) })
        })
        .collect();
    format!("window.__icons={};window.__icon={}", serde_json::Value::from(list), serde_json::Value::from(current))
}

/// Shows the config folder (theme.css, icon, state) in the OS file manager.
fn open_config_folder(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    let opener = if cfg!(target_os = "macos") { "open" } else if cfg!(windows) { "explorer" } else { "xdg-open" };
    if let Err(e) = std::process::Command::new(opener).arg(dir).spawn() {
        eprintln!("canbec: could not open {}: {e}", dir.display());
    }
}

fn main() -> wry::Result<()> {
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let dir = config_dir();
    let icon = load_icon(&dir);
    let builder = WindowBuilder::new()
        .with_window_icon(Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height).ok())
        .with_title("canbec")
        .with_inner_size(LogicalSize::new(1280.0, 820.0))
        .with_position(LogicalPosition::new(80.0, 80.0));
    // macOS: no title bar strip; the traffic lights sit in the sidebar's top spacer.
    // Windows/Linux: no native title bar; the sidebar draws minimize/maximize/close.
    #[cfg(target_os = "macos")]
    let builder = {
        use tao::platform::macos::WindowBuilderExtMacOS;
        builder.with_titlebar_transparent(true).with_title_hidden(true).with_fullsize_content_view(true)
    };
    #[cfg(not(target_os = "macos"))]
    let builder = builder.with_decorations(false);
    let window = builder.build(&event_loop).expect("window");
    set_dock_icon(&icon);

    let adult_cache = dir.join("blocklists").join("adult.txt");
    let adult = Arc::new(RwLock::new(blocklists::load_adult(&adult_cache)));

    let sidebar = {
        let (p1, p2) = (proxy.clone(), proxy.clone());
        WebViewBuilder::new()
            .with_html(SIDEBAR_HTML)
            .with_initialization_script(&theme_script())
            .with_initialization_script(&format!("window.__platform={:?};window.__version={:?}", std::env::consts::OS, env!("CARGO_PKG_VERSION")))
            .with_initialization_script(&icons_script(&icon.source))
            .with_bounds(bounds(&window, true))
            // The sidebar has the trusted IPC channel, so it may never load anything else.
            .with_navigation_handler(|url| url.starts_with("about:") || url.starts_with("data:"))
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_ipc_handler(move |req| send(&p1, AppEvent::Ui(req.into_body())))
            .with_on_page_load_handler(move |e, _| {
                if matches!(e, PageLoadEvent::Finished) {
                    send(&p2, AppEvent::SidebarReady)
                }
            })
            .build_as_child(&window)?
    };

    let content = {
        let (p1, p2, p3, p4) = (proxy.clone(), proxy.clone(), proxy.clone(), proxy.clone());
        let adult = adult.clone();
        WebViewBuilder::new()
            .with_initialization_script(CONTENT_SCRIPT)
            .with_bounds(bounds(&window, false))
            // Adult blocking can't be bypassed from the UI: every navigation is checked here.
            .with_navigation_handler(move |url| {
                let blocked = host_of(&url).is_some_and(|h| adult.read().map(|l| l.is_blocked(&h)).unwrap_or(true));
                if blocked {
                    send(&p1, AppEvent::Blocked);
                }
                !blocked
            })
            // Popups would be unfiltered native windows; open them in this tab (and its checks) instead.
            .with_new_window_req_handler({
                let p = proxy.clone();
                move |url, _| {
                    send(&p, AppEvent::Popup(url));
                    wry::NewWindowResponse::Deny
                }
            })
            .with_ipc_handler(move |req| send(&p2, AppEvent::Content(req.into_body())))
            .with_on_page_load_handler(move |e, url| {
                send(&p3, AppEvent::PageLoad { url, finished: matches!(e, PageLoadEvent::Finished) })
            })
            .with_document_title_changed_handler(move |title| send(&p4, AppEvent::Title(title)))
            .build_as_child(&window)?
    };

    round_corners(&content);

    let snapshot_list = adult.read().map(|l| l.clone()).unwrap_or_default();
    let store = JsonFileStore::new(dir.join("state.json"));
    let mut service = BrowserService::start(WryEngine { view: content }, store, snapshot_list, now());
    if let Some(url) = std::env::args().nth(1) {
        service.handle(UiCommand::Navigate { url }, now());
    }

    // Background: weekly adult-list refresh and a once-a-minute housekeeping tick.
    if blocklists::needs_refresh(&adult_cache) {
        let p = proxy.clone();
        std::thread::spawn(move || match blocklists::refresh_adult(&adult_cache) {
            Ok(list) => send(&p, AppEvent::AdultList(list)),
            Err(e) => eprintln!("canbec: adult list refresh failed (seed + cache still active): {e}"),
        });
    }
    {
        let p = proxy.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(60));
            send(&p, AppEvent::Tick);
        });
    }

    let mut last_url = String::new();
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        let render = match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *flow = ControlFlow::Exit;
                false
            }
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                let _ = sidebar.set_bounds(bounds(&window, true));
                let _ = service.engine().view.set_bounds(bounds(&window, false));
                false
            }
            // Window chrome and app settings, not browser state: handled here instead of in BrowserService.
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"window""#) => {
                let msg: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                match msg["action"].as_str() {
                    Some("drag") => drop(window.drag_window()),
                    Some("minimize") => window.set_minimized(true),
                    Some("maximize") => window.set_maximized(!window.is_maximized()),
                    Some("close") => *flow = ControlFlow::Exit,
                    _ => {}
                }
                false
            }
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"setIcon""#) => {
                let msg: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                // Only built-in names from the picker; custom PNGs stay a hand-edited file.
                if let Some(name) = msg["name"].as_str().filter(|n| BUILT_IN.iter().any(|(b, _, _)| b == n)) {
                    match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join("icon"), name)) {
                        Ok(()) => {
                            let icon = load_icon(&dir);
                            window.set_window_icon(Icon::from_rgba(icon.rgba.clone(), icon.width, icon.height).ok());
                            set_dock_icon(&icon);
                            let _ = sidebar.evaluate_script(&format!("window.canbec.setIcon({})", serde_json::Value::from(icon.source)));
                        }
                        Err(e) => eprintln!("canbec: could not save icon choice: {e}"),
                    }
                }
                false
            }
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"openConfig""#) => {
                open_config_folder(&dir);
                false
            }
            Event::UserEvent(AppEvent::Ui(json)) => match serde_json::from_str::<UiCommand>(&json) {
                Ok(cmd) => {
                    service.handle(cmd, now());
                    true
                }
                Err(e) => {
                    eprintln!("canbec: bad UI command {json}: {e}");
                    false
                }
            },
            // Websites can post to window.ipc too, so only harmless counters are accepted from content.
            Event::UserEvent(AppEvent::Content(json)) => {
                let msg: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                match (msg["type"].as_str(), msg["count"].as_u64()) {
                    (Some("adsHidden"), Some(n)) => {
                        service.on_ads_hidden(n.min(10_000) as u32);
                        true
                    }
                    _ => false,
                }
            }
            Event::UserEvent(AppEvent::PageLoad { url, finished }) => {
                last_url = url.clone();
                service.on_page(&url, None, !finished, now());
                true
            }
            Event::UserEvent(AppEvent::Title(title)) => {
                service.on_page(&last_url, Some(&title), false, now());
                true
            }
            Event::UserEvent(AppEvent::Popup(url)) => {
                service.handle(UiCommand::Navigate { url }, now());
                true
            }
            Event::UserEvent(AppEvent::Blocked) => {
                service.on_blocked();
                true
            }
            Event::UserEvent(AppEvent::AdultList(list)) => {
                if let Ok(mut shared) = adult.write() {
                    *shared = list.clone();
                }
                service.set_adult_blocklist(list);
                false
            }
            Event::UserEvent(AppEvent::Tick) => service.tick(now()),
            Event::UserEvent(AppEvent::SidebarReady) => true,
            _ => false,
        };
        if render {
            let state = serde_json::to_string(&service.state(now())).unwrap_or_default();
            let _ = sidebar.evaluate_script(&format!("window.canbec && window.canbec.render({state})"));
        }
    });
}

/// Rounds the website card. ponytail: macOS only; WebView2 / WebKitGTK need their own region APIs.
/// macOS ignores window icons; the Dock icon is set on the application instead.
/// ponytail: Finder/.app bundle icon needs packaging (icns) — comes with release builds.
#[cfg(target_os = "macos")]
fn set_dock_icon(icon: &AppIcon) {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let Some(mtm) = MainThreadMarker::new() else { return };
    let data = NSData::with_bytes(&icon.png);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        // SAFETY: called on the main thread with a valid image.
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

#[cfg(not(target_os = "macos"))]
fn set_dock_icon(_icon: &AppIcon) {}

#[cfg(target_os = "macos")]
fn round_corners(view: &wry::WebView) {
    use wry::WebViewExtMacOS;
    let native = view.webview();
    native.setWantsLayer(true);
    if let Some(layer) = native.layer() {
        layer.setCornerRadius(CARD_RADIUS);
        layer.setMasksToBounds(true);
    }
}

#[cfg(not(target_os = "macos"))]
fn round_corners(_view: &wry::WebView) {}

fn send(proxy: &EventLoopProxy<AppEvent>, event: AppEvent) {
    let _ = proxy.send_event(event);
}
