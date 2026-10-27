# canbec — notes for Claude

Lightweight cross-platform (Windows / Linux / macOS) Rust browser that hides AI slop, ads, adult
content and infinite feeds. Mission: **de-AI and un-addict your life**. Public repo:
`Cod3rs-org/canbec`.

## Commands

```sh
cargo test --workspace                 # all tests (test/*.rs)
cargo run --release -p canbec [url]    # the browser
# Engine probe (local Blazor WASM test page):
dotnet publish test/engine-probe -c Release -o target/engine-probe
python3 tools/serve.py target/engine-probe/wwwroot 5080   # then open http://127.0.0.1:5080
swift tools/winshot.swift <process-name> out.png          # screenshot one window (macOS)
```

## Architecture (clean, but keep it small)

```
src/domain          pure rules, no I/O, time passed in (Timestamp = unix secs)
src/application     BrowserService use cases + ports: Engine, StateStore; UI types (ui.rs)
src/infrastructure  adapters: wry engine, JSON store, blocklists, scripts/content.js
src/app             composition root: window, sidebar + content webviews, event loop;
                    platform.rs = per-OS glue, mcp.rs = `canbec --mcp` (agents)
src/ui              sidebar chrome: plain HTML/CSS/JS, no build step
src/engines/servo   optional Servo engine adapter (own workspace + lockfile; ~8 min build)
src/prototypes      engine bake-off crates (excluded from the workspace; Servo is a 143 MB build)
test/               Rust integration tests (wired via [[test]] path) + engine-probe Blazor app
docs/research       numbered research/decision notes; docs/architecture/ui-contract.md
```

- Dependencies point inward only: app → infrastructure → application → domain.
- **Simple to extend beats clever.** No new crates, traits or layers unless a second
  implementation really exists. A new rule = a domain module + a test. A new engine = one adapter
  implementing `Engine`.
- The sidebar ↔ core protocol is `docs/architecture/ui-contract.md`. Change the contract, Rust
  types (`application/src/ui.rs`) and `src/ui/sidebar.html` together.
- Websites can call `window.ipc` too. Content IPC accepts only harmless messages (`adsHidden`,
  `aiImages`, `emailField`, and `key` / `openTab` / `pageText` with the per-run token). Never act on content messages for anything
  security-relevant. content.js and extensions run in canbec's own JS world on macOS/Linux
  (`isolate` in `platform.rs`; call them with `engine.js()`, not `evaluate_script`); privacy.js
  stays in the page world.
- One canbec at a time: a second start hands its URL to the running one (`infrastructure/src/instance.rs`:
  Unix socket, or localhost + token on Windows) and quits; that's also how clicked links arrive on Windows/Linux.
- macOS needs a main menu for Cmd+C/V/Q (`set_main_menu` in `app`). Browser shortcuts are JS
  key handlers in `content.js` and `sidebar.html` (keep the two maps in sync).

## Product rules that must not regress

- Adult blocking can't be disabled from the UI. It's checked in the content navigation handler
  and in `BrowserService`. The seed list in `infrastructure/src/scripts/adult-seed.txt` keeps it
  working offline.
- Tabs: soft cap 20 open; idle 3 days → archived into a folder; archived 60 days → Deleted.
  **Never hard-delete** (no `remove` on tabs).
- Feed lock: on by default; unlock only via settings, clamped to at most 6 hours (domain-enforced).
- No tracking across runs: the content webview is incognito; only `stay_logged_in` sites' cookies
  are saved (`cookie_jar`). Fingerprint noise + tracking-param removal live in
  `scripts/privacy.js` (all frames). Don't add anything that persists site data otherwise.
- New tab / search → `https://noai.duckduckgo.com/` until our own search exists.
- App icon: default `sprout`; `<config>/icon` holds a built-in name or a PNG path
  (`infrastructure/src/icon.rs`, PNGs in `infrastructure/src/icons/`, sources in `docs/img/logos/`).

## Working conventions

- Small incremental commits; after each meaningful step update the README journey table/timeline.
- Record new findings and decisions in `docs/research/NN-*.md`.
- **Never put private URLs (e.g. the owner's own apps) in the repo.** Use the local
  `test/engine-probe` instead. History was rewritten once to remove one.
- Mark deliberate shortcuts with `// ponytail:` comments that name the limit and the upgrade path.

## Known gotchas

- Servo 0.1.x breaks `WebAssembly.compileStreaming`; the probe's `?nostream` shim works around it.
- Camera/mic: the permission handler (`permission` in `app`) lets WebKit/WebView2 ask per site.
  Run from the `.app` bundle (`tools/bundle-macos.sh`) to test it: plain `cargo run` has no
  `NSCameraUsageDescription`, and macOS kills the process when a site opens the camera.
- `python -m http.server` drops Blazor's parallel downloads (backlog 5). Use `tools/serve.py`.
- `with_proxy_config` (planned for the domain-level filter proxy) needs macOS 14+.
