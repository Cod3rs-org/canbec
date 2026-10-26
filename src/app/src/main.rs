//! canbec — composition root. One window, two webviews:
//! the sidebar (our UI, `src/ui/sidebar.html`) and the content (websites).
//! All events funnel into the event loop, which owns the `BrowserService`.

// No console window next to the browser on Windows (debug builds keep it for eprintln!).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use canbec_application::ui::{AiTextMode, UiCommand};
use canbec_application::BrowserService;
use canbec_domain::blocklist::HostBlocklist;
use canbec_domain::host_of;
use canbec_infrastructure::engine_wry::WryEngine;
use canbec_infrastructure::profiles as profile_files;
use canbec_infrastructure::updater;
use canbec_infrastructure::store::JsonFileStore;
use canbec_domain::profiles::Profiles;
use canbec_infrastructure::icon::{load_icon, BUILT_IN};
use canbec_infrastructure::{ai_text, blocklists, config_dir, content_script, cookie_jar, privacy_script, random_token, save_to_downloads};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Icon, Window, WindowBuilder};
use wry::{dpi, PageLoadEvent, PermissionKind, PermissionResponse, Rect, WebViewBuilder};

const SIDEBAR_HTML: &str = include_str!("../../ui/sidebar.html");
const SIDEBAR_WIDTH: f64 = 300.0;
const CARD_GAP: f64 = 8.0;

enum AppEvent {
    Ui(String),
    /// A browser shortcut (from the sidebar, or from a page with the right token).
    Key(String),
    Content(String),
    PageLoad { url: String, finished: bool },
    Popup(String),
    Title(String),
    Blocked,
    /// The page's web process died.
    Crashed,
    /// A new tracker list finished compiling (the first page loaded without it).
    TrackersReady,
    /// Download started (file name) or finished (`None` name, success flag).
    Download(Option<String>, bool),
    SidebarReady,
    AdultList(HostBlocklist),
    SharedSlop(canbec_domain::slop::SlopList),
    /// Whether the system uses the family DNS; with an error if turning it on failed.
    FamilyDns(bool, Option<String>),
    /// A newer release (checked once a day), or the result of installing it.
    Update(updater::Release),
    UpdateInstalled(Result<std::path::PathBuf, String>),
    /// First Wayback captures for search results (`None`: never archived).
    FirstSeen(Vec<(String, Option<String>)>),
    /// From the hidden AI text page: `{url, p}`, `{ready}` or `{error}`.
    AiText(String),
    /// Model download: percent done, `Ok(None)` when finished.
    AiTextDownload(Result<Option<u8>, String>),
    Tick,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The sidebar webview fills the window (its background is the frame); the website floats
/// on top as an inset card, rounded by `round_corners` (macOS, Windows).
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

/// Site permissions. Without a handler wry grants camera/mic silently; `Default` makes the engine
/// ask the user with its own per-site prompt instead. Things that only help tracking or nagging
/// (notifications, sensors, local fonts for fingerprinting) are refused outright.
/// ponytail: Linux treats `Default` as deny, so camera/mic stay off there until we draw a prompt.
fn permission(kind: PermissionKind) -> PermissionResponse {
    match kind {
        PermissionKind::Notifications
        | PermissionKind::Sensors
        | PermissionKind::LocalFonts
        | PermissionKind::Midi
        | PermissionKind::WindowManagement => PermissionResponse::Deny,
        _ => PermissionResponse::Default,
    }
}

/// The hidden page that runs the optional AI text model (#25). It only loads its own files from
/// the `canbec-ai` protocol, and its messages are scores. Zero size and invisible.
/// Created on the first page to score and dropped after 10 idle minutes: it holds ~500 MB.
/// ponytail: the protocol handler reads the 77 MB model on the UI thread each time it loads.
fn ai_text_view(window: &Window, dir: &std::path::Path, proxy: &EventLoopProxy<AppEvent>) -> Option<wry::WebView> {
    let (dir, p) = (dir.to_owned(), proxy.clone());
    let url = if cfg!(windows) { "http://canbec-ai.localhost/" } else { "canbec-ai://localhost/" };
    let view = browser_args(WebViewBuilder::new())
        .with_custom_protocol("canbec-ai".into(), move |_, req| {
            let (body, mime) = ai_text::file(&dir, req.uri().path()).unwrap_or_default();
            let status = if body.is_empty() { 404 } else { 200 };
            wry::http::Response::builder().status(status).header("Content-Type", mime).body(body.into()).unwrap_or_default()
        })
        .with_url(url)
        .with_navigation_handler(move |u| u.starts_with(url))
        .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
        .with_permission_handler(|_| PermissionResponse::Deny)
        .with_ipc_handler(move |req| send(&p, AppEvent::AiText(req.into_body())))
        .with_bounds(Rect { position: dpi::LogicalPosition::new(0.0, 0.0).into(), size: dpi::LogicalSize::new(0.0, 0.0).into() })
        .with_visible(false)
        .build_as_child(window);
    view.map_err(|e| eprintln!("canbec: AI text page failed: {e}")).ok()
}

/// Shows the config folder (theme.css, icon, state) in the OS file manager.
fn open_config_folder(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    let opener = if cfg!(target_os = "macos") { "open" } else if cfg!(windows) { "explorer" } else { "xdg-open" };
    if let Err(e) = std::process::Command::new(opener).arg(dir).spawn() {
        eprintln!("canbec: could not open {}: {e}", dir.display());
    }
}

mod mcp;
mod platform;

use platform::{block_trackers, browser_args, evaluate_in_world, isolate, on_crash, round_corners, set_dock_icon, set_main_menu, user_agent, watch_crashes, HAS_WORLDS};

fn main() -> wry::Result<()> {
    if std::env::args().any(|a| a == "--mcp") {
        return mcp::run();
    }
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let dir = config_dir();
    let mut profiles = profile_files::load(&dir);
    let profile_dir = profile_files::dir(&dir, &profiles.active);
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
    set_main_menu();
    let key_token = random_token();

    let adult_cache = dir.join("blocklists").join("adult.txt");
    let adult = Arc::new(RwLock::new(blocklists::load_adult(&adult_cache)));
    let ads_cache = dir.join("blocklists").join("ads.txt");
    let ads = blocklists::load_ads(&ads_cache);
    let extensions = canbec_infrastructure::extensions::load(&dir);
    let mut ext_storage = canbec_infrastructure::extensions::load_storage(&dir, &extensions);

    let sidebar = {
        let (p1, p2) = (proxy.clone(), proxy.clone());
        browser_args(WebViewBuilder::new())
            .with_html(SIDEBAR_HTML)
            .with_initialization_script(theme_script())
            .with_initialization_script(format!("window.__platform={:?};window.__version={:?}", std::env::consts::OS, env!("CARGO_PKG_VERSION")))
            .with_initialization_script(icons_script(&icon.source))
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

    // Extensions (all frames) and content.js (main frame): in canbec's own JS world where the
    // engine has one, out of the page scripts' reach (#20); in the page world otherwise.
    let world_scripts: Vec<(String, bool)> = [
        (canbec_infrastructure::extensions::script(&extensions, &ext_storage), false),
        (content_script(&key_token, &ads.generic, &std::fs::read_to_string(dir.join("feeds.txt")).unwrap_or_default()), true),
    ]
    .into_iter()
    .filter(|(script, _)| !script.is_empty())
    .collect();
    let content = {
        let (p1, p2, p3, p4) = (proxy.clone(), proxy.clone(), proxy.clone(), proxy.clone());
        let adult = adult.clone();
        let builder = on_crash(user_agent(browser_args(WebViewBuilder::new())), { let p = proxy.clone(); move || send(&p, AppEvent::Crashed) })
            // No tracking across runs: a fresh, in-memory browser profile on every start.
            // Logins the user wants to keep are put back by cookie_jar.
            .with_incognito(true)
            .with_initialization_script_for_main_only(privacy_script(&random_token()), false);
        let page_world = if HAS_WORLDS { &[][..] } else { &world_scripts[..] };
        page_world
            .iter()
            .fold(builder, |b, (script, main_only)| b.with_initialization_script_for_main_only(script.as_str(), *main_only))
            .with_bounds(bounds(&window, false))
            .with_permission_handler(permission)
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
            // Downloads land in the OS Downloads folder (wry fills in a free file name there).
            .with_download_started_handler({
                let p = proxy.clone();
                move |_, path| {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    send(&p, AppEvent::Download(Some(name), true));
                    true
                }
            })
            .with_download_completed_handler({
                let p = proxy.clone();
                move |_, _, ok| send(&p, AppEvent::Download(None, ok))
            })
            // Where worlds exist our scripts post through their own world's handler (`isolate`),
            // so this channel only carries what pages send themselves: ignored. WebView2 has
            // only this one.
            .with_ipc_handler(move |req| {
                if !HAS_WORLDS {
                    send(&p2, AppEvent::Content(req.into_body()))
                }
            })
            .with_on_page_load_handler(move |e, url| {
                send(&p3, AppEvent::PageLoad { url, finished: matches!(e, PageLoadEvent::Finished) })
            })
            .with_document_title_changed_handler(move |title| send(&p4, AppEvent::Title(title)))
            .build_as_child(&window)?
    };

    isolate(&content, &world_scripts, { let p = proxy.clone(); move |msg| send(&p, AppEvent::Content(msg)) });
    drop(world_scripts); // main() never returns; the engine has its own copies
    round_corners(&content);
    watch_crashes(&content, { let p = proxy.clone(); move || send(&p, AppEvent::Crashed) });
    let trackers_cache = dir.join("blocklists").join("trackers.txt");
    {
        let p = proxy.clone();
        block_trackers(&content, &blocklists::load_trackers(&trackers_cache), move || send(&p, AppEvent::TrackersReady));
    }

    let mut cookie_file = profile_dir.join("logins.json");
    cookie_jar::restore(&content, &cookie_file);
    let snapshot_list = adult.read().map(|l| l.clone()).unwrap_or_default();
    let store = JsonFileStore::new(profile_dir.join("state.json"));
    let mut service = BrowserService::start(WryEngine { view: content, eval: evaluate_in_world }, store, snapshot_list, now());
    service.set_ad_rules(ads);
    service.set_shared_slop(blocklists::load_shared_slop(&dir));
    service.set_workspaces(workspaces(&profiles));
    service.set_extensions(extensions.iter().map(|e| e.name.clone()).collect());
    // AI text hints: on while the model is downloaded; the page that runs it only while in use.
    let mut ai_view: Option<wry::WebView> = None;
    let (mut ai_ready, mut ai_pending, mut ai_used) = (false, None::<String>, 0);
    if ai_text::installed(&dir) {
        service.set_ai_text_mode(AiTextMode::On, now());
    }
    if let Some(url) = std::env::args().nth(1) {
        service.handle(UiCommand::Navigate { url }, now());
    }

    // Background: weekly list refreshes and a once-a-minute housekeeping tick.
    if blocklists::needs_refresh(&trackers_cache) {
        std::thread::spawn(move || {
            if let Err(e) = blocklists::refresh_trackers(&trackers_cache) {
                eprintln!("canbec: tracker list refresh failed (seed + cache still active): {e}");
            }
        });
    }
    if blocklists::needs_refresh(&ads_cache) {
        std::thread::spawn(move || match blocklists::refresh_ads(&ads_cache) {
            Ok(()) => eprintln!("canbec: ad rules updated, active from the next start"),
            Err(e) => eprintln!("canbec: ad list refresh failed (seed + cache still active): {e}"),
        });
    }
    check_family_dns(&proxy, false);
    if updater::due(&dir) {
        let p = proxy.clone();
        std::thread::spawn(move || match updater::check(env!("CARGO_PKG_VERSION")) {
            Ok(Some(release)) => send(&p, AppEvent::Update(release)),
            Ok(None) => {}
            Err(e) => eprintln!("canbec: update check failed: {e}"),
        });
    }
    if !blocklists::slop_subscriptions(&dir).is_empty() {
        let (p, dir) = (proxy.clone(), dir.clone());
        std::thread::spawn(move || match blocklists::refresh_shared_slop(&dir) {
            Ok(list) => send(&p, AppEvent::SharedSlop(list)),
            Err(e) => eprintln!("canbec: slop list refresh failed (cached copies still active): {e}"),
        });
    }
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

    // First-seen dates for search results. ponytail: in memory for this run only; writing them to
    // disk would leave a trace of what you searched.
    let first_seen: Arc<std::sync::Mutex<std::collections::HashMap<String, Option<String>>>> = Arc::default();
    let mut release: Option<updater::Release> = None;
    let mut last_url = String::new();
    let mut downloading = String::new();
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        let render = match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *flow = ControlFlow::Exit;
                false
            }
            Event::LoopDestroyed => {
                save_logins(&service, &cookie_file);
                false
            }
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                let _ = sidebar.set_bounds(bounds(&window, true));
                let _ = service.engine().view.set_bounds(bounds(&window, false));
                round_corners(&service.engine().view);
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
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"key""#) => {
                let msg: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                send(&proxy, AppEvent::Key(msg["key"].as_str().unwrap_or_default().to_owned()));
                false
            }
            Event::UserEvent(AppEvent::Key(key)) => {
                let cmd = match key.as_str() {
                    "newTab" => Some(UiCommand::NewTab),
                    "reload" => Some(UiCommand::Reload),
                    "back" => Some(UiCommand::Back),
                    "forward" => Some(UiCommand::Forward),
                    "pinPage" => Some(UiCommand::PinPage),
                    "zoomIn" => Some(UiCommand::Zoom { step: 1 }),
                    "zoomOut" => Some(UiCommand::Zoom { step: -1 }),
                    "zoomReset" => Some(UiCommand::Zoom { step: 0 }),
                    "closeTab" => service.state(now()).tabs.iter().find(|t| t.active).map(|t| UiCommand::CloseTab { id: t.id }),
                    "reopenTab" => service.state(now()).closed.first().map(|t| UiCommand::RestoreTab { id: t.id }),
                    "focusAddress" | "settings" | "find" => {
                        let _ = sidebar.focus();
                        let call = match key.as_str() { "settings" => "toggleSettings", "find" => "find", _ => "focusUrl" };
                        let _ = sidebar.evaluate_script(&format!("window.canbec.{call}()"));
                        None
                    }
                    _ => None,
                };
                match cmd {
                    Some(cmd) => {
                        service.handle(cmd, now());
                        true
                    }
                    None => false,
                }
            }
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"switchWorkspace""#) || json.contains(r#""type":"addProfile""#) => {
                let msg: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                let id = match (msg["type"].as_str(), msg["name"].as_str(), msg["id"].as_str()) {
                    (Some("addProfile"), Some(name), _) => match profiles.add(name) {
                        Ok(id) => Some(id),
                        Err(e) => {
                            service.notify(e);
                            None
                        }
                    },
                    (Some("switchWorkspace"), _, Some(id)) => Some(id.to_owned()),
                    _ => None,
                };
                if let Some(id) = id {
                    switch_profile(&id, &mut profiles, &mut service, &mut cookie_file, &dir);
                }
                true
            }
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"setAgents""#) => {
                let msg: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                if let (Some(id), Some(on)) = (msg["id"].as_str(), msg["on"].as_bool()) {
                    profiles.set_agents(id, on);
                    profile_files::save(&dir, &profiles);
                    service.set_workspaces(workspaces(&profiles));
                }
                true
            }
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"installUpdate""#) => {
                match release.clone() {
                    // macOS app: swap it in place. Elsewhere (or a source build): the release page.
                    Some(r) if cfg!(target_os = "macos") && updater::current_bundle().is_some() => {
                        service.notify(format!("Downloading canbec {}…", r.version));
                        let p = proxy.clone();
                        std::thread::spawn(move || send(&p, AppEvent::UpdateInstalled(updater::install(&r))));
                    }
                    Some(r) => service.handle(UiCommand::Navigate { url: r.page }, now()),
                    None => {}
                }
                true
            }
            Event::UserEvent(AppEvent::Update(r)) => {
                service.set_update(Some(r.version.clone()));
                release = Some(r);
                true
            }
            Event::UserEvent(AppEvent::UpdateInstalled(result)) => {
                match result {
                    Ok(bundle) => {
                        updater::relaunch(&bundle);
                        *flow = ControlFlow::Exit; // logins are saved on the way out
                    }
                    Err(e) => service.notify(format!("Update failed: {e}")),
                }
                true
            }
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"aiText""#) => {
                let msg: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                if matches!(service.ai_text_mode(), AiTextMode::Downloading(_)) {
                    // a double click; the button is disabled after the next render
                } else if msg["on"].as_bool() == Some(true) {
                    service.set_ai_text_mode(AiTextMode::Downloading(0), now());
                    let (p, dir) = (proxy.clone(), dir.clone());
                    std::thread::spawn(move || {
                        let result = ai_text::install(&dir, |pct| send(&p, AppEvent::AiTextDownload(Ok(Some(pct)))));
                        send(&p, AppEvent::AiTextDownload(result.map(|_| None)));
                    });
                } else {
                    (ai_view, ai_ready) = (None, false);
                    ai_text::remove(&dir);
                    service.set_ai_text_mode(AiTextMode::Off, now());
                }
                true
            }
            Event::UserEvent(AppEvent::AiTextDownload(progress)) => {
                match progress {
                    Ok(Some(pct)) => service.set_ai_text_mode(AiTextMode::Downloading(pct.min(99)), now()),
                    Ok(None) => service.set_ai_text_mode(AiTextMode::On, now()),
                    Err(e) => {
                        service.set_ai_text_mode(AiTextMode::Off, now());
                        service.notify(format!("AI text model download failed: {e}"));
                    }
                }
                true
            }
            Event::UserEvent(AppEvent::AiText(json)) => {
                let msg: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                if let (Some(url), Some(p)) = (msg["url"].as_str(), msg["p"].as_f64()) {
                    service.on_ai_text(url, p);
                } else if msg["ready"].as_bool() == Some(true) {
                    ai_ready = true;
                    if let (Some(view), Some(script)) = (&ai_view, ai_pending.take()) {
                        let _ = view.evaluate_script(&script);
                    }
                } else if let Some(e) = msg["error"].as_str() {
                    eprintln!("canbec: AI text model: {e}");
                }
                true
            }
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"enableFamilyDns""#) => {
                check_family_dns(&proxy, true);
                false
            }
            Event::UserEvent(AppEvent::Ui(json)) if json.contains(r#""type":"exportSlop""#) => {
                match save_to_downloads("canbec-slop.txt", &service.export_slop()) {
                    Ok(path) => service.notify(format!("Saved your slop list to {}", path.display())),
                    Err(e) => service.notify(format!("Could not save the slop list: {e}")),
                }
                true
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
            // Websites can post to window.ipc too: only harmless counters, and shortcuts that carry
            // this run's secret token (which only our content script knows), are accepted.
            Event::UserEvent(AppEvent::Content(json)) => {
                let msg: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
                match (msg["type"].as_str(), msg["count"].as_u64()) {
                    (Some("adsHidden"), Some(n)) => {
                        service.on_ads_hidden(n.min(10_000) as u32);
                        true
                    }
                    (Some("aiImages"), Some(n)) => {
                        service.on_ai_images(n.min(10_000) as u32);
                        true
                    }
                    // Worst case a page shows this tip when it shouldn't.
                    (Some("emailField"), _) => {
                        service.notify("Tip: a throwaway email keeps this site out of your inbox (Settings → Privacy tools)");
                        true
                    }
                    // Only from the search page, and only asks archive.org about those URLs.
                    (Some("dateResults"), _)
                        if msg["token"].as_str() == Some(&key_token)
                            && host_of(&last_url).is_some_and(|h| canbec_domain::host_matches(&h, "duckduckgo.com")) =>
                    {
                        let urls: Vec<String> = msg["urls"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|u| u.as_str())
                            .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
                            .take(30)
                            .map(str::to_owned)
                            .collect();
                        let (p, cache) = (proxy.clone(), first_seen.clone());
                        std::thread::spawn(move || {
                            let answers = std::thread::scope(|s| {
                                let jobs: Vec<_> = urls
                                    .into_iter()
                                    .map(|url| {
                                        let cache = cache.clone();
                                        s.spawn(move || {
                                            let known = cache.lock().ok().and_then(|c| c.get(&url).cloned());
                                            let seen = match known {
                                                Some(seen) => Some(seen),
                                                None => canbec_infrastructure::wayback::first_capture(&url).ok(),
                                            };
                                            if let (Some(seen), Ok(mut c)) = (&seen, cache.lock()) {
                                                c.insert(url.clone(), seen.clone());
                                            }
                                            seen.map(|s| (url, s))
                                        })
                                    })
                                    .collect();
                                jobs.into_iter().filter_map(|j| j.join().ok().flatten()).collect::<Vec<_>>()
                            });
                            send(&p, AppEvent::FirstSeen(answers));
                        });
                        false
                    }
                    // Text for the optional AI text hint, from our content script (token) and only
                    // for the page the tab is on now. Scored under the core's URL, not the page's.
                    (Some("pageText"), _)
                        if service.ai_text_mode() == AiTextMode::On
                            && msg["token"].as_str() == Some(&key_token)
                            && msg["url"].as_str() == Some(last_url.as_str()) =>
                    {
                        let text: String = msg["text"].as_str().unwrap_or_default().chars().take(4000).collect();
                        let script = format!("classify({}, {})", serde_json::Value::from(last_url.as_str()), serde_json::Value::from(text));
                        ai_used = now();
                        match &ai_view {
                            Some(view) if ai_ready => drop(view.evaluate_script(&script)),
                            _ => {
                                ai_pending = Some(script); // only the newest page waits for the model
                                if ai_view.is_none() {
                                    ai_view = ai_text_view(&window, &dir, &proxy);
                                }
                            }
                        }
                        false
                    }
                    // Extension storage: pages can send these too, so they're limited to known
                    // extensions and a size cap (see extensions::save_storage).
                    (Some("extStorageGet"), _) => {
                        let all = serde_json::Value::Object(ext_storage.clone());
                        service.engine().js(&format!("window.__canbecExtStorage && window.__canbecExtStorage({all})"));
                        false
                    }
                    (Some("extStorageSet"), _) => {
                        let id = msg["id"].as_str().unwrap_or_default();
                        canbec_infrastructure::extensions::save_storage(&dir, &extensions, &mut ext_storage, id, &msg["items"]);
                        false
                    }
                    (Some("openTab"), _) if msg["token"].as_str() == Some(&key_token) => {
                        let url = msg["url"].as_str().unwrap_or_default().to_owned();
                        service.handle(UiCommand::OpenTab { url }, now());
                        true
                    }
                    (Some("key"), _) if msg["token"].as_str() == Some(&key_token) => {
                        send(&proxy, AppEvent::Key(msg["key"].as_str().unwrap_or_default().to_owned()));
                        false
                    }
                    _ => false,
                }
            }
            Event::UserEvent(AppEvent::PageLoad { url, finished }) => {
                last_url = url.clone();
                service.on_page(&url, None, !finished, now());
                if finished && !service.state(now()).stay_logged_in.is_empty() {
                    save_logins(&service, &cookie_file); // a login just happened? keep it even after a crash
                }
                true
            }
            Event::UserEvent(AppEvent::Title(title)) => {
                // Single-page apps change the URL with pushState and no page load, but mostly the
                // title too: ask the engine (not the page) where the tab is now.
                if let Ok(url) = service.engine().view.url() {
                    last_url = url;
                }
                service.on_page(&last_url, Some(&title), false, now());
                true
            }
            Event::UserEvent(AppEvent::Popup(url)) => {
                service.handle(UiCommand::Navigate { url }, now());
                true
            }
            Event::UserEvent(AppEvent::Download(Some(name), _)) => {
                service.notify(format!("Downloading {name}…"));
                downloading = name;
                true
            }
            Event::UserEvent(AppEvent::Download(None, ok)) => {
                let name = std::mem::take(&mut downloading);
                service.notify(if ok { format!("Downloaded {name} to Downloads") } else { format!("Download failed: {name}") });
                true
            }
            Event::UserEvent(AppEvent::TrackersReady) => {
                service.handle(UiCommand::Reload, now());
                false
            }
            Event::UserEvent(AppEvent::Crashed) => {
                service.on_crashed(now());
                true
            }
            Event::UserEvent(AppEvent::Blocked) => {
                service.on_blocked();
                true
            }
            Event::UserEvent(AppEvent::FirstSeen(answers)) => {
                use canbec_domain::first_seen::{is_obsolete, year};
                let map: serde_json::Map<String, serde_json::Value> = answers
                    .into_iter()
                    .map(|(url, seen)| {
                        let info = serde_json::json!({
                            "year": seen.as_deref().and_then(year),
                            "obsolete": seen.as_deref().is_some_and(is_obsolete),
                            "slop": service.is_slop(&url),
                        });
                        (url, info)
                    })
                    .collect();
                let script = format!("window.__canbec && window.__canbec.dates({})", serde_json::Value::Object(map));
                service.engine().js(&script);
                false
            }
            Event::UserEvent(AppEvent::FamilyDns(on, error)) => {
                service.set_family_dns(on);
                if let Some(e) = error {
                    service.notify(format!("Family DNS not turned on: {e}"));
                }
                true
            }
            Event::UserEvent(AppEvent::SharedSlop(list)) => {
                service.set_shared_slop(list);
                true
            }
            Event::UserEvent(AppEvent::AdultList(list)) => {
                if let Ok(mut shared) = adult.write() {
                    *shared = list.clone();
                }
                service.set_adult_blocklist(list);
                false
            }
            Event::UserEvent(AppEvent::Tick) => {
                save_logins(&service, &cookie_file); // also every minute, in case of a crash
                if ai_view.is_some() && now().saturating_sub(ai_used) > 600 {
                    (ai_view, ai_ready) = (None, false); // frees the model's memory
                }
                service.tick(now())
            }
            Event::UserEvent(AppEvent::SidebarReady) => true,
            _ => false,
        };
        if render {
            let state = serde_json::to_string(&service.state(now())).unwrap_or_default();
            let _ = sidebar.evaluate_script(&format!("window.canbec && window.canbec.render({state})"));
        }
    });
}

/// Reads the system DNS in the background (after switching it to the family servers if asked;
/// the OS shows its admin prompt for that).
fn check_family_dns(proxy: &EventLoopProxy<AppEvent>, enable: bool) {
    use canbec_infrastructure::system_dns;
    let p = proxy.clone();
    std::thread::spawn(move || {
        let error = if enable { system_dns::enable().err() } else { None };
        send(&p, AppEvent::FamilyDns(canbec_domain::family_dns::is_family(&system_dns::servers()), error));
    });
}

fn workspaces(profiles: &Profiles) -> Vec<canbec_application::ui::Workspace> {
    profiles
        .list
        .iter()
        .map(|p| canbec_application::ui::Workspace { id: p.id.clone(), name: p.name.clone(), color: p.color.clone(), active: p.id == profiles.active, agents: p.agents })
        .collect()
}

/// Another identity: the current profile's logins are saved, the engine forgets every site
/// (cookies, storage), the new profile's logins come back and its tabs load.
fn switch_profile(id: &str, profiles: &mut Profiles, service: &mut BrowserService<WryEngine, JsonFileStore>, cookie_file: &mut std::path::PathBuf, config: &std::path::Path) {
    if !profiles.activate(id) {
        return;
    }
    save_logins(service, cookie_file);
    if let Err(e) = service.engine().view.clear_all_browsing_data() {
        eprintln!("canbec: could not clear site data: {e}");
    }
    let dir = profile_files::dir(config, id);
    *cookie_file = dir.join("logins.json");
    cookie_jar::restore(&service.engine().view, cookie_file);
    service.switch_store(JsonFileStore::new(dir.join("state.json")), now());
    service.set_workspaces(workspaces(profiles));
    profile_files::save(config, profiles);
    service.notify(format!("Profile: {}", profiles.active().name));
}

/// Keeps cookies only for "stay logged in" sites; every other site starts fresh next time.
fn save_logins<S: canbec_application::ports::StateStore>(service: &BrowserService<WryEngine, S>, file: &std::path::Path) {
    if let Err(e) = cookie_jar::save(&service.engine().view, file, |domain| service.keeps_cookie(domain)) {
        eprintln!("canbec: could not save logins: {e}");
    }
}

fn send(proxy: &EventLoopProxy<AppEvent>, event: AppEvent) {
    let _ = proxy.send_event(event);
}
