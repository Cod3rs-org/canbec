# 06 — Extensibility: engines, extensions, feed blocker, MCP

## 1. Engine harness ("bring any engine")

The core depends only on the `Engine` port (`src/application`). Each engine is one adapter:

```
           ┌───────────── application (ports) ─────────────┐
 UI ──▶ BrowserService ──▶ trait Engine ◀── engine-wry   (default: WebKit / WebView2 / WebKitGTK)
                                       ◀── engine-servo (pure Rust, cargo feature)
                                       ◀── engine-cef   (Chromium, later)
                                       ◀── engine-gecko (Firefox, if an embedding API appears)
```

The port API is deliberately small: create a view, navigate, inject a script at document start, run
JS, get notified on navigation, plus an IPC channel. Anything that needs more is a capability flag,
not a bigger trait.

## 2. Lightweight extensions (Chrome-compatible subset)

Goal: port most "content tweak" Chrome extensions without carrying a full WebExtensions runtime.

**Supported subset of Manifest V3 (v1):**

| MV3 key / API | Supported | Notes |
|---|---|---|
| `content_scripts` (`matches`, `js`, `css`, `run_at`) | ✅ | Injected through the engine's init-script hook |
| `chrome.storage.local` / `.sync` | ✅ | Backed by a per-extension JSON file (sync = local) |
| `chrome.runtime.sendMessage` / `onMessage` | ✅ | Bridged over IPC to a tiny background JS context |
| `declarativeNetRequest` (block rules) | ✅ | Translated into our filter engine (adblock-rust) |
| `action` popup | ⏳ later | Rendered in the sidebar |
| `webRequest` (blocking), `debugger`, `tabs.executeScript` everywhere | ❌ | Too heavy, or a privacy risk |

**Porting skill:** a Claude Code skill (`.claude/skills/port-chrome-extension/`). It reads a Chrome
extension, maps each API it uses onto the subset above, rewrites what isn't supported, and writes
`extensions/<name>/` with a `canbec.json` manifest. Anything it can't port goes into a TODO list
instead of being faked.

## 3. Feed blocker (News Feed Eradicator style), hard to undo

- Built in, **on by default**. Hides infinite feeds on Facebook, Instagram, X/Twitter, LinkedIn,
  Reddit home, YouTube home and Shorts, and TikTok. It shows a calm placeholder ("Nothing to scroll
  here") instead.
- Implemented as a built-in content script (CSS selectors per site) plus a domain rule `FeedLock`.
- **No unlock button on the page.** The only way out is *Settings → Feeds → Unlock for N hours*.
  N is capped at **6 hours** in the domain layer. When the time runs out it re-locks
  automatically, and the countdown is visible in the sidebar.

## 4. MCP: letting AI agents use the browser on *your* terms

The browser can run an **opt-in, local-only MCP server**. It's off by default and toggled in
settings. It listens on stdio or `127.0.0.1` only. Agents then see the web **through the same
filters a human does**.

| MCP tool | What it does |
|---|---|
| `search(query)` | Human search: results with `obsolete` (first seen after 2022-11-30) and `slop` flags |
| `open(url)` → `page_id` | Loads the page in a hidden tab (adult, ads and feed filters apply) |
| `read(page_id)` | Readable text + links; AI-marked images replaced by `[AI image hidden]` |
| `tag_slop(url)` | The agent can report slop. It goes into a separate list the user reviews |

Safety: no cookies or session from the user's own tabs (a separate profile). Every call is
logged in the sidebar. The user can see and kill agent tabs.

Implementation (done, v0.1): `canbec --mcp` is a stdio server. The JSON-RPC part is hand-written
in `infrastructure/src/mcp.rs` (four tools don't need `rmcp` and an async runtime). `app/src/mcp.rs`
runs a hidden browser with the same content scripts, blocklists and `BrowserService`, as the
first profile with "AI agents" ticked (or `--profile <id>`). Tools: `search`, `open`, `read`,
`list_tabs`. Not yet: `tag_slop` and a live agent log in the sidebar (calls go to `agent.log`).

## 5. Profiles (and per-profile MCP access)

A **profile** is a fully separate browser identity: its own cookies and storage (a separate webview
data directory), tabs, slop tags, essentials, theme and filter settings. Workspaces live *inside*
a profile.

| Profile setting | Example |
|---|---|
| Name, colour, icon | "Personal" 🟠, "Work" 🔵, "Agent" 🤖 |
| Filters | Adult lock is always on in every profile and can't be removed. Feed lock and ad overlay are per profile |
| **MCP access** | `off` (default) · `read-only` (search + read) · `full` (open, read, tag) |
| MCP scope | Allowed domains list, max open agent tabs, rate limit |

Rules:
- The MCP server only ever exposes profiles whose MCP access is not `off`. Agents choose a profile
  by name, and the default is a dedicated, empty "Agent" profile, so agents **never** see your
  personal cookies or logins unless you grant it explicitly.
- Every agent action is logged per profile and shown in the sidebar with a kill switch.
- Implementation: `profile_id` on the `Snapshot` store path (`<config>/profiles/<id>/`). wry's
  `WebContext::new(Some(data_dir))` isolates cookies per profile.
