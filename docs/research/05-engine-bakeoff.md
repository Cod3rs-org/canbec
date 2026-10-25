# 05 — Engine bake-off: wry vs Servo vs Blitz

Test machine: macOS 27 (arm64), Rust 1.98.1. Test page: http://localhost:5080
(Blazor WASM + MapLibre WebGL + Chart.js + QR camera).

Prototypes live in [`prototypes/`](../../prototypes/), one crate each.

## Results

| | wry 0.57 (system webview) | Servo 0.1.x LTS | Blitz 0.3 beta (+ Boa JS) |
|---|---|---|---|
| Release build time (cold) | **47 s** | _pending_ | _pending_ |
| Binary size | **1.5 MB** | _pending_ | _pending_ |
| Renders testapp login | ✅ pixel-perfect | _pending_ | _pending_ |
| Engine written in Rust | ❌ (WebKit / WebView2 / WebKitGTK) | ✅ | ✅ |
| Owns the network stack | ❌ (but a proxy can be configured) | ✅ | ✅ |

### wry

![wry rendering testapp](../img/engine-wry-testapp.png)

Run it: `cargo run --release --manifest-path prototypes/engine-wry/Cargo.toml -- http://localhost:5080`

## How filtering would work with each engine

| Need | wry | Servo / Blitz |
|---|---|---|
| Block adult and ad **domains** | `with_proxy_config` → our local Rust proxy (sees the host of every request, including HTTPS CONNECT) + `with_navigation_handler` for top-level pages | Our own resolver / request hook in the engine's net layer |
| Ad overlay, slop border, AI spoilers | `with_initialization_script`: injected JS/CSS on every page | The same, or native hooks in the DOM |
| Per-URL (path) ad rules | Only through platform APIs (WebView2 `WebResourceRequested`, WebKit content rule lists), so code differs per OS | Uniform, inside our code |
| AI image metadata (C2PA) | Proxy can't read HTTPS bodies, so the injected script fetches suspect images and sends them to Rust over IPC | Direct access to image bytes |

## Cross-platform notes

- **Windows:** wry uses WebView2 (Chromium-based, preinstalled on Win 10/11). Best compatibility.
- **macOS:** WKWebView (Safari engine). `with_proxy_config` needs macOS 14+.
- **Linux:** WebKitGTK 4.1 from the distro. Quality depends on the distro version; WebGL works.
- Servo and Blitz render the same on every OS, because the engine ships inside our binary.
