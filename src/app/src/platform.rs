//! Per-OS glue the composition root needs: Dock icon and menu (macOS), tracker blocking
//! (WebKit content blocker, WebKitGTK filter store, WebView2 request filter), crash hook and the
//! rounded website card. Each function has one version per OS and a no-op fallback.

use canbec_infrastructure::icon::AppIcon;
use wry::WebViewBuilder;

/// Corner radius of the website card.
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
const CARD_RADIUS: f64 = 12.0;

/// macOS ignores window icons; the Dock icon is set on the application instead.
/// ponytail: Finder/.app bundle icon needs packaging (icns) — comes with release builds.
#[cfg(target_os = "macos")]
pub fn set_dock_icon(icon: &AppIcon) {
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
pub fn set_dock_icon(_icon: &AppIcon) {}

/// macOS routes Cmd+C/V/X/A/Z and Cmd+Q through the main menu; without one they do nothing.
/// Browser shortcuts (Cmd+T, W, L, …) come from the page/sidebar key handlers instead.
#[cfg(target_os = "macos")]
pub fn set_main_menu() {
    use objc2::{sel, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem};
    use objc2_foundation::NSString;
    let Some(mtm) = MainThreadMarker::new() else { return };
    let bar = NSMenu::new(mtm);
    let submenu = |title: &str, items: &[(&str, objc2::runtime::Sel, &str)]| {
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
        for (name, action, key) in items {
            // SAFETY: standard responder-chain selectors with no arguments beyond the sender.
            unsafe { menu.addItemWithTitle_action_keyEquivalent(&NSString::from_str(name), Some(*action), &NSString::from_str(key)) };
        }
        let item = NSMenuItem::new(mtm);
        item.setSubmenu(Some(&menu));
        bar.addItem(&item);
    };
    submenu("canbec", &[("Hide canbec", sel!(hide:), "h"), ("Quit canbec", sel!(terminate:), "q")]);
    submenu(
        "Edit",
        &[
            ("Undo", sel!(undo:), "z"),
            ("Redo", sel!(redo:), "Z"),
            ("Cut", sel!(cut:), "x"),
            ("Copy", sel!(copy:), "c"),
            ("Paste", sel!(paste:), "v"),
            ("Select All", sel!(selectAll:), "a"),
        ],
    );
    NSApplication::sharedApplication(mtm).setMainMenu(Some(&bar));
}

#[cfg(not(target_os = "macos"))]
pub fn set_main_menu() {}

/// Blocks tracker requests with WebKit's built-in content blocker (the one Safari blockers use;
/// compiled once, then cached by WebKit under an id that changes with the list). `compiled` runs
/// when the list had to be compiled first (new list), so the open page can reload.
#[cfg(target_os = "macos")]
pub fn block_trackers(view: &wry::WebView, list: &canbec_domain::trackers::TrackerList, compiled: impl Fn() + 'static) {
    use std::hash::{DefaultHasher, Hash, Hasher};
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_foundation::{NSError, NSString};
    use objc2_web_kit::{WKContentRuleList, WKContentRuleListStore};
    use wry::WebViewExtMacOS;

    let Some(mtm) = MainThreadMarker::new() else { return };
    let json = list.to_content_blocker_json();
    let mut hash = DefaultHasher::new();
    json.hash(&mut hash);
    let id = NSString::from_str(&format!("canbec-trackers-{:x}", hash.finish()));
    // SAFETY: main thread; the controller outlives the webview's configuration; blocks run on main.
    unsafe {
        let Some(store) = WKContentRuleListStore::defaultStore(mtm) else { return };
        let controller = view.webview().configuration().userContentController();
        let add = {
            let controller = controller.clone();
            move |list: *mut WKContentRuleList| {
                if let Some(list) = list.as_ref() {
                    controller.addContentRuleList(list);
                }
            }
        };
        let compiled = std::rc::Rc::new(compiled);
        let compile = {
            let (store, add) = (store.clone(), add.clone());
            let (id, json) = (id.clone(), NSString::from_str(&json));
            move || {
                let add = add.clone();
                let compiled = compiled.clone();
                let done = RcBlock::new(move |list: *mut WKContentRuleList, err: *mut NSError| match err.as_ref() {
                    Some(err) => eprintln!("canbec: tracker blocker failed to compile: {err:?}"),
                    None => {
                        add(list);
                        compiled();
                    }
                });
                store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(Some(&id), Some(&json), Some(&done));
            }
        };
        let found = RcBlock::new(move |list: *mut WKContentRuleList, _: *mut NSError| {
            if list.is_null() { compile() } else { add(list) }
        });
        store.lookUpContentRuleListForIdentifier_completionHandler(Some(&id), Some(&found));
    }
}

/// Linux: WebKitGTK compiles the same JSON with its UserContentFilterStore (cached on disk in
/// `<config>/content-filters`, looked up first by an id that changes with the list).
/// ponytail: raw FFI because webkit2gtk-rs doesn't bind the store; old compiled lists stay on disk.
#[cfg(target_os = "linux")]
pub fn block_trackers(view: &wry::WebView, list: &canbec_domain::trackers::TrackerList, compiled: impl Fn() + 'static) {
    use std::ffi::{CStr, CString};
    use std::hash::{DefaultHasher, Hash, Hasher};
    use std::ptr::null_mut;
    use webkit2gtk::glib::translate::ToGlibPtr;
    use webkit2gtk::{ffi, gio, glib, WebViewExt};
    use wry::WebViewExtUnix;

    struct Job {
        store: *mut ffi::WebKitUserContentFilterStore,
        manager: webkit2gtk::UserContentManager,
        id: CString,
        json: glib::Bytes,
        compiled: Box<dyn Fn()>,
    }
    unsafe fn add(job: &Job, filter: *mut ffi::WebKitUserContentFilter) {
        ffi::webkit_user_content_manager_add_filter(job.manager.to_glib_none().0, filter);
        ffi::webkit_user_content_filter_unref(filter);
    }
    unsafe fn take_error(err: *mut glib::ffi::GError) -> String {
        if err.is_null() {
            return "unknown error".into();
        }
        let msg = CStr::from_ptr((*err).message).to_string_lossy().into_owned();
        glib::ffi::g_error_free(err);
        msg
    }
    unsafe extern "C" fn saved(_: *mut glib::gobject_ffi::GObject, res: *mut gio::ffi::GAsyncResult, data: glib::ffi::gpointer) {
        let job = Box::from_raw(data as *mut Job);
        let mut err = null_mut();
        let filter = ffi::webkit_user_content_filter_store_save_finish(job.store, res, &mut err);
        if filter.is_null() {
            eprintln!("canbec: tracker blocker failed to compile: {}", take_error(err));
        } else {
            add(&job, filter);
            (job.compiled)();
        }
    }
    unsafe extern "C" fn loaded(_: *mut glib::gobject_ffi::GObject, res: *mut gio::ffi::GAsyncResult, data: glib::ffi::gpointer) {
        let job = data as *mut Job;
        let mut err = null_mut();
        let filter = ffi::webkit_user_content_filter_store_load_finish((*job).store, res, &mut err);
        if !filter.is_null() {
            add(&Box::from_raw(job), filter);
            return;
        }
        let _ = take_error(err); // not compiled yet: compile now
        let (store, id, json) = ((*job).store, (*job).id.as_ptr(), ToGlibPtr::<*const glib::ffi::GBytes>::to_glib_none(&(*job).json).0 as *mut _);
        ffi::webkit_user_content_filter_store_save(store, id, json, null_mut(), Some(saved), job as glib::ffi::gpointer);
    }

    let Some(manager) = view.webview().user_content_manager() else { return };
    let json = list.to_content_blocker_json();
    let mut hash = DefaultHasher::new();
    json.hash(&mut hash);
    let dir = canbec_infrastructure::config_dir().join("content-filters");
    let (Ok(path), Ok(id)) = (CString::new(dir.to_string_lossy().as_bytes()), CString::new(format!("canbec-trackers-{:x}", hash.finish()))) else {
        return;
    };
    // SAFETY: GTK main thread; the job (and the refs it holds) lives until the last callback frees it.
    unsafe {
        let store = ffi::webkit_user_content_filter_store_new(path.as_ptr());
        let job = Box::into_raw(Box::new(Job { store, manager, id, json: glib::Bytes::from_owned(json.into_bytes()), compiled: Box::new(compiled) }));
        ffi::webkit_user_content_filter_store_load(store, (*job).id.as_ptr(), null_mut(), Some(loaded), job as glib::ffi::gpointer);
    }
}

/// Windows: WebView2 has no content blocker, so every request passes this handler and tracker
/// hosts (third-party only) get an empty 403. Pages themselves are never blocked.
/// ponytail: host rules only; EasyPrivacy path rules need a URL matcher here.
#[cfg(windows)]
pub fn block_trackers(view: &wry::WebView, list: &canbec_domain::trackers::TrackerList, _compiled: impl Fn() + 'static) {
    use canbec_domain::{host_matches, host_of};
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use webview2_com::{take_pwstr, WebResourceRequestedEventHandler};
    use windows_core::{Interface, HSTRING, PWSTR};
    use wry::WebViewExtWindows;

    let (webview, env, list) = (view.webview(), view.environment(), list.clone());
    // Edge's own "Strict" tracking prevention on top: known trackers lose cookie and storage
    // access in third-party frames. ponytail: not every third-party cookie, like WebKit does.
    // SAFETY: UI thread; plain COM casts and a setter.
    let strict = unsafe {
        webview
            .cast::<ICoreWebView2_13>()
            .and_then(|w| w.Profile())
            .and_then(|p| p.cast::<ICoreWebView2Profile3>())
            .and_then(|p| p.SetPreferredTrackingPreventionLevel(COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT))
    };
    if let Err(e) = strict {
        eprintln!("canbec: strict tracking prevention unavailable: {e}");
    }
    let page = webview.clone();
    let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
        // SAFETY: WebView2 calls this on the UI thread with valid args; out-params are ours.
        unsafe {
            args.ResourceContext(&mut context)?;
            if context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT {
                return Ok(());
            }
            let (mut uri, mut source) = (PWSTR::null(), PWSTR::null());
            args.Request()?.Uri(&mut uri)?;
            page.Source(&mut source)?;
            let (Some(host), page_host) = (host_of(&take_pwstr(uri)), host_of(&take_pwstr(source))) else { return Ok(()) };
            let first_party = page_host.is_some_and(|p| host_matches(&host, &p) || host_matches(&p, &host));
            if !first_party && list.blocks_host(&host) {
                let response = env.CreateWebResourceResponse(None, 403, &HSTRING::from("Blocked"), &HSTRING::new())?;
                args.SetResponse(&response)?;
            }
        }
        Ok(())
    }));
    let mut token = 0i64;
    // SAFETY: called once on the UI thread right after the webview was built.
    let result = unsafe {
        webview
            .AddWebResourceRequestedFilter(&HSTRING::from("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL)
            .and_then(|_| webview.add_WebResourceRequested(&handler, &mut token))
    };
    if let Err(e) = result {
        eprintln!("canbec: tracker blocker failed: {e}");
    }
}

/// macOS: Safari's own user agent. WKWebView leaves out `Version/… Safari/…`, which makes canbec
/// stand out from the crowd of Safari users. WebView2 and WebKitGTK already send stock strings.
#[cfg(target_os = "macos")]
pub fn user_agent(builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
    let out = std::process::Command::new("sw_vers").arg("-productVersion").output().ok();
    let macos: u32 = out.and_then(|o| String::from_utf8_lossy(&o.stdout).split('.').next()?.trim().parse().ok()).unwrap_or(26);
    let safari = if macos >= 26 { macos } else { macos + 3 }; // macOS 15 shipped Safari 18; 26 → 26
    builder.with_user_agent(format!("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{safari}.0 Safari/605.1.15"))
}

#[cfg(not(target_os = "macos"))]
pub fn user_agent(builder: WebViewBuilder<'_>) -> WebViewBuilder<'_> {
    builder
}

/// ponytail: macOS only; wry doesn't expose WebView2's ProcessFailed or WebKitGTK's
/// web-process-terminated yet, so there a crashed page stays white until reload.
#[cfg(target_os = "macos")]
pub fn on_crash(builder: WebViewBuilder<'_>, crashed: impl Fn() + 'static) -> WebViewBuilder<'_> {
    use wry::WebViewBuilderExtDarwin;
    builder.with_on_web_content_process_terminate_handler(crashed)
}

#[cfg(not(target_os = "macos"))]
pub fn on_crash(builder: WebViewBuilder<'_>, _crashed: impl Fn() + 'static) -> WebViewBuilder<'_> {
    builder
}

#[cfg(target_os = "macos")]
pub fn round_corners(view: &wry::WebView) {
    use wry::WebViewExtMacOS;
    let native = view.webview();
    native.setWantsLayer(true);
    if let Some(layer) = native.layer() {
        layer.setCornerRadius(CARD_RADIUS);
        layer.setMasksToBounds(true);
    }
}

/// Windows: clips wry's container window (sized to the card) to a rounded region. Called again
/// on every resize. ponytail: GDI regions aren't anti-aliased; Linux stays square (GTK3 can't
/// clip a WebKit widget to rounded corners).
#[cfg(windows)]
pub fn round_corners(view: &wry::WebView) {
    use windows_sys::Win32::Foundation::{HWND, RECT};
    use windows_sys::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;
    use wry::WebViewExtWindows;
    let mut parent = Default::default();
    // SAFETY: UI thread; the container window lives as long as the webview. After SetWindowRgn
    // succeeds the system owns the region.
    unsafe {
        if view.controller().ParentWindow(&mut parent).is_err() {
            return;
        }
        let hwnd = parent.0 as HWND;
        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if GetClientRect(hwnd, &mut rect) == 0 {
            return;
        }
        let diameter = (2.0 * CARD_RADIUS * GetDpiForWindow(hwnd).max(96) as f64 / 96.0) as i32;
        let region = CreateRoundRectRgn(0, 0, rect.right + 1, rect.bottom + 1, diameter, diameter);
        SetWindowRgn(hwnd, region, 1);
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn round_corners(_view: &wry::WebView) {}
