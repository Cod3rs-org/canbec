# 13 · Linux hand test (X11)

Run on Ubuntu 24.04 under Xvfb (no GPU, software rendering), driven with `xdotool`, with local
test pages (`test/fixtures`, served by `tools/serve.py`). Outside sites can't be reached from the
test machine, so everything was checked against local pages and a local copy of the lists.

| Check | Result | Issue |
|---|---|---|
| Tracker blocking (WebKitGTK content filter): EasyPrivacy-style path rule `/a8c-analytics.js` blocked, third-party host rule blocked, same-site script loads; also on the very first page after a fresh compile | ✅ | #12 |
| Isolated world: the page sees `window.__canbec` as `undefined` | ✅ | #20 |
| Device persona: 4 real cores / 1600×1000 / software GPU show as 8 cores / 1920×1080 / Intel UHD 630; getter looks native; GPC on | ✅ | #34 |
| Adult block from the address bar: "Blocked: adult content", page stays | ✅ | |
| Find in page (Ctrl+F), zoom (Ctrl+=, chip shows 125 %), new / close / reopen tab | ✅ | #7 #8 |
| Tag as AI slop: frame on the page, red dot and tab icon in the sidebar | ✅ | |
| Settings (Ctrl+,): full window, overview tiles, Escape closes | ✅ | |
| Rounded website card, sidebar and top bar layouts | ✅ | #15 |
| **Ctrl+L / Ctrl+F from the page: the bar opened but typing went nowhere** | 🐛 → ✅ fixed | #2 |

## The keyboard focus bug

On X11 wry puts every webview in its own X window. Moving GTK focus to the sidebar (what
`WebView::focus` does) left X sending keys to the page's window, so after Ctrl+L the address
was selected but typing never arrived. A mouse click worked because X moves focus itself then.
`platform::focus` now also calls `XSetInputFocus` on the webview's **own** GDK window: wry's
container window is "foreign" to GDK, which doesn't listen for keys on it, so focusing that one
didn't help either.

Not covered here: Wayland, a real GPU, camera/mic (denied on Linux until there's a prompt),
downloads, and anything needing outside sites (feed lock on real feeds, list downloads, updates).
