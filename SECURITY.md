# Security

canbec is early alpha. Please report vulnerabilities privately through GitHub's
**Security → Report a vulnerability** on this repo, not in public issues.

## Threat model (what a malicious website can and can't do)

| Surface | Protection | Status |
|---|---|---|
| **Running code on your computer** | Websites run inside the OS web engine's sandbox (WebKit / WebView2 / WebKitGTK). canbec adds no native code paths that websites can call. | ✅ |
| **Talking to the browser core** | Websites can post to `window.ipc`. Content messages are parsed as JSON. Only `adsHidden` (a number, clamped), `emailField` (shows a tip), shortcuts and Cmd/Ctrl+click "open in new tab" are accepted. Shortcuts and new tabs must carry a random per-run token that only canbec's content script knows (captured before page scripts run), and come from real key presses (`isTrusted`). The worst a page could do with them is open or close a tab, and new tabs still go through the adult check and the 20-tab cap. Everything else is ignored. | ✅ |
| **Taking over the trusted sidebar** | The sidebar is a separate webview. All website-controlled text (titles, URLs) is HTML-escaped before display. The sidebar can't navigate anywhere or open windows (locked in `app`). | ✅ |
| **Popups bypassing filters** | `window.open` / `target=_blank` never create native windows; they're loaded in the current tab, through the adult check. Fixture: `test/fixtures/popup.html`. | ✅ |
| **Script injection through state** | State goes to the sidebar as `serde_json` output; page flags are booleans only. No string concatenation of untrusted text into scripts. | ✅ |
| **Blocklist downloads** | HTTPS only; parsed as hostnames, never executed. A broken or small download (fewer than 1,000 hosts) is rejected. The worst case is that a bad list blocks too much. It can't unblock anything. | ✅ |
| **Removing the slop frame / feed lock on its own page** | The content script lives in the page, so a site *can* remove its own overlay. This is cosmetic only (not a security boundary). Hardening idea: isolated-world scripts. | ⚠️ known |
| **Engine bugs** | We rely on the OS engine's patches: Safari/WebKit updates with macOS, WebView2 auto-updates; **WebKitGTK depends on your Linux distro**, so keep it updated. Servo (optional build) is younger and less hardened. | ⚠️ inherent |
| **Supply chain** | `cargo audit` (RustSec) runs in CI on every push. | ✅ |
| **Tracking across runs** | Websites run in an incognito (in-memory) profile; cookies, storage and cache are gone after quitting. Only "stay logged in" sites' cookies are kept, in `<config>/logins.json` (mode 0600 on macOS/Linux, not encrypted). Fingerprint readouts are randomized per site and run. See docs/research/08-privacy.md. | ✅ |
| **Tracker requests within a session** | macOS: third-party requests to ~47k EasyPrivacy tracker hosts are blocked by WebKit's content blocker. Windows/Linux: not yet. | ✅ macOS / ⚠️ others |
| **Camera, microphone, location** | Never granted silently: the engine asks you per site (WebKit / WebView2 prompt). Notifications, motion sensors, local-font lists and MIDI are always refused (nagging or fingerprinting). Linux: camera/mic are refused until canbec draws its own prompt. | ✅ |

## What canbec never does

- No telemetry, no accounts, no remote config.
- No local servers listening on the network (`tools/serve.py` is a dev tool bound to `127.0.0.1`).
- The planned MCP server is **off by default**, local-only, and restricted per profile
  (see docs/research/06-extensibility.md).
