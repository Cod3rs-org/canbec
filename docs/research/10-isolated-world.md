# 10 · Content scripts in an isolated world (#20)

**Problem.** content.js (ad cover, slop frame, feed lock, shortcuts) and extension scripts ran in
the page's own JavaScript world. A page could see `window.__canbec`, call or overwrite it, read an
extension's storage through `window.__canbecExt*`, and post fake content messages.

**Decision.** Run them in a named JS world, `canbec`, where the engine has one. A world shares the
page's DOM but not its globals, like Chrome's isolated world for extensions.

| Engine | Scripts | Messages to the core | Core → script |
|---|---|---|---|
| WebKit (macOS) | `WKUserScript` in `WKContentWorld "canbec"` | own `WKScriptMessageHandler` named `canbec`, reachable only from that world; page-world `window.ipc` is ignored | `evaluateJavaScript:inFrame:inContentWorld:` |
| WebKitGTK (Linux) | `webkit_user_script_new_for_world` | handler `canbec` registered in the world, own `script-message-received::canbec` listener; wry's listener hears every world, so its IPC is ignored | `run_javascript_in_world` |
| WebView2 (Windows) | page world, as before | page world | `ExecuteScript` |

The world gets its own `window.ipc` shim, so content.js and the extension runtime didn't change.
All of it lives in `isolate` / `evaluate_in_world` in `app/src/platform.rs`; `WryEngine::js` takes
the right one (`eval` field), and `canbec --mcp` keeps the page world.

**Stays in the page world.** privacy.js: its job is to change what page scripts see (canvas noise,
the fake device), so it has to patch the page's own objects. Its wrappers look native
(`toString`), which is the best a page-world script can do.

**Not solved by worlds.** The DOM is shared, so a page can still remove the slop frame or a feed
lock overlay from its own page. That's cosmetic (see SECURITY.md).

**Checked.** `window.find` (Cmd+F) works from the world too. A headless WKWebView (no window) with the real content.js and the ported
reading-time extension in the world: the page sees `undefined` for all three globals, the feed lock,
slop frame, ad cover and the extension's badge all apply, extension storage messages arrive through
the world's handler, and a page's fake `adsHidden` only reaches the (ignored) page-world handler.
Linux compiles in CI; not yet run by hand. WebView2 has no worlds (CDP's `worldName` exists, but
messaging from such a world isn't documented), so Windows stays as it is.
