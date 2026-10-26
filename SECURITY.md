# Security

canbec is early alpha. Please report vulnerabilities privately through GitHub's
**Security → Report a vulnerability** on this repo, not in public issues.

## Threat model (what a malicious website can and can't do)

| Surface | Protection | Status |
|---|---|---|
| **Running code on your computer** | Websites run inside the OS web engine's sandbox (WebKit / WebView2 / WebKitGTK). canbec adds no native code paths that websites can call. | ✅ |
| **Talking to the browser core** | On macOS and Linux canbec's scripts post through a handler only their own isolated JS world can reach, and the page world's `window.ipc` is ignored. On Windows websites can post to `window.ipc` too. Content messages are parsed as JSON. Only `adsHidden` and `aiImages` (numbers, clamped), `emailField` (shows a tip), shortcuts and Cmd/Ctrl+click "open in new tab" are accepted. Shortcuts and new tabs must carry a random per-run token that only canbec's content script knows (captured before page scripts run), and come from real key presses (`isTrusted`). The worst a page could do with them is open or close a tab, and new tabs still go through the adult check and the 20-tab cap. Everything else is ignored. | ✅ |
| **Taking over the trusted sidebar** | The sidebar is a separate webview. All website-controlled text (titles, URLs) is HTML-escaped before display. The sidebar can't navigate anywhere or open windows (locked in `app`). | ✅ |
| **Popups bypassing filters** | `window.open` / `target=_blank` never create native windows; they're loaded in the current tab, through the adult check. Fixture: `test/fixtures/popup.html`. | ✅ |
| **Script injection through state** | State goes to the sidebar as `serde_json` output; page flags are booleans only. No string concatenation of untrusted text into scripts. | ✅ |
| **Blocklist downloads** | HTTPS only; parsed as hostnames, never executed. A broken or small download (fewer than 1,000 hosts) is rejected. The worst case is that a bad list blocks too much. It can't unblock anything. | ✅ |
| **Removing the slop frame / feed lock on its own page** | On macOS and Linux the content script and extensions run in canbec's own JS world: pages can't see, call or overwrite them, or read extension storage. They share the DOM, though, so a site *can* still remove its own overlay. This is cosmetic only (not a security boundary). Windows (WebView2) has no JS worlds; there they run in the page. | ⚠️ known |
| **Engine bugs** | We rely on the OS engine's patches: Safari/WebKit updates with macOS, WebView2 auto-updates; **WebKitGTK depends on your Linux distro**, so keep it updated. Servo (optional build) is younger and less hardened. | ⚠️ inherent |
| **Supply chain** | `cargo audit` (RustSec) runs in CI on every push. | ✅ |
| **Tracking across runs** | Websites run in an incognito (in-memory) profile; cookies, storage and cache are gone after quitting. Only "stay logged in" sites' cookies are kept, encrypted (ChaCha20-Poly1305) in `logins.enc`, with the key in the macOS Keychain, the Linux Secret Service or wrapped by Windows DPAPI. Without a secret store (Linux without `secret-tool`) they stay a plain owner-only `logins.json`. Fingerprint readouts are randomized per site and run. See docs/research/08-privacy.md. | ✅ |
| **Tracker requests within a session** | macOS: EasyPrivacy host and path rules (~50k, with per-site exceptions) are compiled into WebKit's content blocker; tracker hosts are blocked as third parties, analytics paths also on the site itself. Pages themselves are never blocked. Test page: `test/fixtures/trackers/` (`python3 tools/serve.py test/fixtures/trackers 5091`). Linux compiles the same JSON in WebKitGTK; Windows checks the same rules per request. Both not yet checked by hand. | ✅ macOS / 🧪 others |
| **Camera, microphone, location** | Never granted silently: the engine asks you per site (WebKit / WebView2 prompt). Notifications, motion sensors, local-font lists and MIDI are always refused (nagging or fingerprinting). Linux: camera/mic are refused until canbec draws its own prompt. | ✅ |

## What canbec never does

- No telemetry, no accounts, no remote config. The only call home is the daily update check: a
  plain GET of the latest release from api.github.com (no ID, no data sent). Updates are checked
  against the release's SHA256SUMS.txt before the app is replaced.
- No local servers listening on the network (`tools/serve.py` is a dev tool bound to `127.0.0.1`).
- The MCP server (`canbec --mcp`) is **off by default**: it refuses to start unless you allowed
  AI agents on a profile in settings. It talks over stdio only (the agent starts it, no port),
  browses with the same filters as you, can't use the camera/mic or download files, never
  touches your saved tabs, and logs every call to `agent.log` in that profile's folder.
  Page text it returns is untrusted: a page can still try prompt injection on the agent.
