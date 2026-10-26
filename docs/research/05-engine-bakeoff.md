# 05 — Engine bake-off: wry vs Servo vs Blitz

Test machine: macOS 27 (arm64), Rust 1.98.1, .NET 9.

Test page: [`test/engine-probe`](../../test/engine-probe), a local **Blazor WebAssembly** app that
checks each feature real apps need and shows ✅ or ❌ for each one. Run it like this:

```sh
dotnet publish test/engine-probe -c Release -o target/engine-probe
python3 tools/serve.py target/engine-probe/wwwroot 5080
```

Prototypes live in [`src/prototypes/`](../../src/prototypes/), one crate each.

## Results

| | wry 0.57 (system webview) | Servo 0.1.4 LTS | Blitz 0.3 beta (+ Boa JS) |
|---|---|---|---|
| Release build time (cold) | **47 s** | 7 m 45 s | 4 m 21 s |
| Binary size | **1.5 MB** | 143 MB | 51 MB |
| Blazor WASM boots | ✅ | ✅ with `?nostream` shim ¹ | ❌ blank page |
| Canvas 2D / WebGL | ✅ / ✅ | ✅ / ✅ | ❌ |
| Camera API (`getUserMedia`) | ❌ ² | ❌ | ❌ |
| Service worker | ✅ | ❌ | ❌ |
| **Probe score** | **5 / 6** | 4 / 6 | 0 |
| Engine written in Rust | ❌ (WebKit / WebView2 / WebKitGTK) | ✅ | ✅ |
| Owns the network stack | ❌ (a proxy can be configured) | ✅ | ✅ |

¹ Servo 0.1.4's `WebAssembly.compileStreaming` rejects valid modules ("failed to match magic
number"). Removing `compileStreaming` / `instantiateStreaming` makes .NET fall back to
ArrayBuffer compilation, which works. The browser can inject this shim for Servo automatically.
<br>² WKWebView needs camera permission handling in the app (`NSCameraUsageDescription` and a
permission delegate). This is a gap in the embedding setup, not an engine limitation.

| wry | Servo | Blitz |
|:-:|:-:|:-:|
| ![wry](../img/engine-wry-probe.png) | ![servo](../img/engine-servo-probe.png) | ![blitz](../img/engine-blitz-probe.png) |

## Decision

**Engine-agnostic core with pluggable engines.** The browser depends on an `Engine` port
(interface) only.

| Adapter | Role |
|---|---|
| `engine-wry` | **Default.** Best compatibility, tiny binary, Windows / Linux / macOS |
| `engine-servo` | Optional "pure Rust" build (cargo feature). Gets the streaming shim |
| `engine-cef` (later) | Chromium via CEF, for exact Chrome compatibility |
| others (later) | Gecko / Ladybird if they offer embedding APIs |

Blitz is dropped as a page engine. It might later draw our own UI (sidebar, tabs).

## How filtering works per engine

| Need | wry | Servo |
|---|---|---|
| Block adult and ad **domains** | `with_proxy_config` → local Rust proxy (sees every request's host, including HTTPS CONNECT) + `with_navigation_handler` for top-level pages | Request hook in Servo's net layer |
| Ad overlay, slop border, AI spoilers, feed blocker | `with_initialization_script`: injected JS/CSS | The same (user scripts) |
| Per-URL (path) ad rules | Platform APIs (WebView2 `WebResourceRequested`, WebKit content rule lists), so code differs per OS | Uniform |
| AI image metadata (C2PA) | Injected script fetches suspect images and sends them to Rust over IPC | Direct access to image bytes |

## Cross-platform notes

- **Windows:** WebView2 (Chromium-based, preinstalled on Win 10/11).
- **macOS:** WKWebView. `with_proxy_config` needs macOS 14+.
- **Linux:** WebKitGTK 4.1 from the distro.
- Servo renders the same on every OS, because the engine ships inside our binary.

## Servo adapter (#32)

`src/engines/servo` (crate `canbec-servo`, outside the main workspace with its own lockfile, so the
default build and CI stay small). `ServoEngine` implements the same `Engine` port as wry, and the
filters use Servo's own hooks:

| canbec needs | Servo hook |
|---|---|
| Adult block on every page and frame | `BrowserService` for typed URLs; `request_navigation` → `deny()` for links; main-frame loads in `load_web_resource` (loads the app starts skip `request_navigation`) |
| Tracker requests | `WebViewDelegate::load_web_resource` → `TrackerList::blocks` → `intercept(..).cancel()` (same rules as the other engines; no resource type yet, so `$image`-style rules don't apply) |
| privacy.js, content.js | `UserContentManager` user scripts (page world) |
| Core → page (flags, ad rules, find) | `WebView::evaluate_javascript`, sharing `infrastructure::content_calls` with wry |
| Zoom | `WebView::set_page_zoom` |

Run it headless (software rendering, no window):

```sh
cd src/engines/servo
cargo run --release -- https://example.com out.png   # ~8 min the first time
```

Checked on `test/fixtures/trackers`: the same five verdicts as WebKit (3 blocked, app.js and jQuery
load), and an adult seed host is refused. Servo quirk: a load asked for while another is still
pending is dropped, so the runner lets each one finish first.

It drives `BrowserService` like the browser does, prints the page, blocked requests and canbec's
agent view, and saves a screenshot. **Not yet:** a Servo window inside canbec (Servo needs the
app to forward mouse, keyboard and scroll events and to composite its frames; servoshell shows how)
and content → core messages (no `window.ipc`; Servo would need a message hook for embedders).

## Notes for a CEF (Chromium) adapter

Not built. What it would take, for whoever picks it up:

- **Crate:** `cef` (tauri-apps/cef-rs), the Rust bindings Tauri uses for its experimental CEF
  runtime. It downloads the CEF binary distribution (Chromium as a library), about 100+ MB per OS.
- **Size and packaging:** the app grows from ~2 MB to a few hundred MB. On macOS CEF needs helper
  apps inside the bundle (renderer, GPU, plugin) and its own framework, so `tools/bundle-macos.sh`
  changes; on Windows and Linux the CEF files ship next to the binary.
- **Filters map well:** `CefRequestHandler::OnBeforeBrowse` (adult block), the resource request
  handler's `OnBeforeResourceLoad` → cancel (trackers, with the resource type, so typed rules work),
  `CefFrame::ExecuteJavaScript` and a render-process handler for our scripts, and a process
  message from the renderer for content → core messages (only our renderer code can send them,
  so no token is needed).
- **Privacy:** an in-memory `CefRequestContext` (empty cache path) matches the incognito profile;
  `CefCookieManager` can restore the "stay logged in" cookies like `cookie_jar` does.
- **Why it's last:** exact Chrome compatibility is the only gain over WebView2 (already Chromium on
  Windows), at the cost of shipping and updating a whole browser engine ourselves. Worth it only if
  a site we need breaks in WebKit.
