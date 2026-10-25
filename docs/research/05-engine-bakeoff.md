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
