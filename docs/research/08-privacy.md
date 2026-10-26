# 08 — No tracking: a fresh browser on every start

Goal from the owner: *no tracking whatsoever. Never persist a cookie that can track the browser;
the user is always a new one.* Plus: suggest privacy tools (throwaway email and similar).

## What canbec does now

| Layer | How | Where |
|---|---|---|
| **Cookies, storage, cache** | The content webview runs in incognito mode (`with_incognito(true)`: `WKWebsiteDataStore.nonPersistent` on macOS, InPrivate on WebView2, ephemeral WebKitGTK). Everything a site stored is gone after quitting. | `src/app` |
| **Logins you want to keep** | "Stay logged in on this site" in settings. Only those sites' cookies are saved (`<config>/logins.enc`, encrypted) and put back on the next start. Every other cookie dies with the session. | `domain::privacy::keeps_cookie`, `infrastructure::cookie_jar` |
| **Fingerprinting** | Canvas, WebGL and audio readouts get tiny invisible changes, seeded per site and per run: stable within a page (so it doesn't look like noise), different on every other site and after every restart. Wrapped functions still print `[native code]`. Runs in iframes too. | `scripts/privacy.js` |
| **Fake device (persona)** | Sites see a common device similar to this one, never its real values: core count, `deviceMemory` (Chromium), screen size (never smaller than the window), WebGL GPU (Safari's "Apple GPU" on macOS, common ANGLE strings on Windows, Mesa strings on Linux) and Windows' `platformVersion`. Same per site until restart, different between sites. On macOS the user agent is Safari's own (WKWebView leaves out `Version/… Safari/…`). Check page: `test/fixtures/persona/` (serve it with `tools/serve.py`). | `domain::persona`, `scripts/privacy.js`, `platform::user_agent` |
| **Tracking parameters** | `utm_*`, `fbclid`, `gclid`, `msclkid`, `si`, … are removed from typed URLs (core), from links right before you follow them, and from the address of the page you landed on. One list in Rust, injected into the script. | `domain::privacy`, `scripts/privacy.js` |
| **Tracker requests** (macOS) | EasyPrivacy's ~47k `||host^` rules (weekly, plus a built-in seed) become a WebKit content-blocker list, the same engine Safari blockers use. Only third-party loads are blocked. Compiled once, cached by WebKit. Verified: Google Analytics, Hotjar, Mixpanel, Plausible blocked; jQuery CDN loads. | `domain::trackers`, `block_trackers` in `app` |
| **Global Privacy Control** | `navigator.globalPrivacyControl = true`: a legal opt-out signal in some places (e.g. California). | `scripts/privacy.js` |
| **Privacy tools** | Settings lists 10 Minute Mail, Firefox Relay, DuckDuckGo Email, Bitwarden, Have I Been Pwned and EFF's Cover Your Tracks. When a page focuses an email field, the sidebar suggests a throwaway address. | `src/ui/sidebar.html`, `scripts/content.js` |

Verified on macOS with a local test page: canvas hash changes per run and per site, a cookie
and a localStorage value set in one run are gone in the next, a stay-logged-in site's cookie
comes back, landing and link URLs lose `utm_*`/`fbclid`.

## Limits (honest list)

- **Third-party cookies on WebView2** are refused with Chromium's `--test-third-party-cookie-phaseout`
  switch (#14, `browser_args` in `app/platform.rs`); WebKit's ITP already blocks them on macOS and
  Linux. Not yet checked on a Windows machine. It's a test switch, so Edge may drop it one day.
- **Tracker request blocking** is in `block_trackers` in `app`: a WebKit content blocker on macOS,
  the same JSON in WebKitGTK's filter store on Linux, and on Windows the same rules checked per
  WebView2 request (`TrackerList::blocks`, about 80 µs a request with full EasyPrivacy).
  EasyPrivacy path rules and site exceptions are compiled too (#21).
- **Kept sites keep only cookies**, not localStorage/IndexedDB. Logins based on localStorage
  tokens need a re-login after restart; settings says so next to the list. Why not more: the
  storage would have to be put back by a document-start script, and wry fixes those scripts when
  the webview is built, so after a profile switch they'd hold the wrong profile's data. Upgrade
  path: one webview per profile, or a persistent data store per profile for kept sites only.
- Saved cookies are encrypted (`logins.enc`, ChaCha20-Poly1305) with a key in the OS secret
  store (`infrastructure::secret`). Linux without `secret-tool` falls back to a plain 0600 file.
- The fingerprint noise lives in the page's JavaScript world, so a determined script can detect
  that something is wrapped. It still breaks cross-site and cross-session matching, which is the
  point. Fonts, timezone, `devicePixelRatio` and the OS/engine are real on purpose (faking them breaks sites or gives canbec away). Your IP address needs a VPN.
- Persona gaps: Web Workers don't run the init script, so `hardwareConcurrency` is real there; the `Sec-CH-UA-*` headers on Windows are WebView2's own. A screen bigger than every listed size (e.g. 5K) keeps its real value.
