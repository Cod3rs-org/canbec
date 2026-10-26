# UI contract: Rust core ↔ browser chrome

The browser window holds **two webviews**: the *chrome* (Zen-style vertical sidebar, our own
HTML/CSS/JS in `src/ui/`) and the *content* (the website). The chrome never touches websites
directly. It only:

1. **receives state:** the core calls `window.canbec.render(state)` whenever something changes;
2. **sends commands:** `window.ipc.postMessage(JSON.stringify(command))`.

No frameworks, no build step, no network. Plain HTML, CSS and JS, so it stays lightweight.

## State (core → UI)

```jsonc
{
  "current":   { "url": "https://example.org/", "title": "Example", "slop": false, "loading": false,
                 "canGoBack": true, "canGoForward": false, "adsHidden": 3 },
  "tabs":      [ { "id": 1, "title": "Example", "url": "https://example.org/", "active": true, "slop": false } ],
  "tabLimit":  20,                       // soft cap: UI turns amber at 16, red at 20
  "archive":   [ { "folder": "GitHub", "tabs": [ { "id": 7, "title": "...", "url": "...", "idleDays": 4 } ] } ],
  "essentials":[ { "title": "Mail", "url": "https://mail.example/" } ],
  "workspaces":[ { "id": "personal", "name": "Personal", "color": "#ff7a45", "active": true } ],
  "feedLock":  { "locked": true, "unlockedUntil": null },  // News-feed blocker; unlock max 6 h, settings only
  "stayLoggedIn": ["mail.example"],  // (current.aiImages: AI-labelled images blurred on this page)     // sites whose cookies survive a restart; current.stayLoggedIn for this page
  "slopCount": 12, "sharedSlopCount": 340, // own slop tags; entries from followed lists (slop-lists.txt)
  "notice":    null                     // one-line message, e.g. "Blocked: adult content"
}
```

## Commands (UI → core)

| `type` | Extra fields | Meaning |
|---|---|---|
| `navigate` | `url` (text the user typed: URL or search terms) | Load in the active tab |
| `back` / `forward` / `reload` | | Navigation |
| `newTab` | | Refused by the core at 20 open tabs (the UI shows why) |
| `activateTab` / `closeTab` / `archiveTab` | `id` | Tab actions |
| `restoreTab` | `id` | Archived tab → open tab |
| `tagSlop` | `scope`: `"domain"` \| `"page"` | Mark the current site as AI slop |
| `untagSlop` | | Remove the mark |
| `unlockFeeds` | `hours` (1–6, the core enforces the cap) | Temporary unlock, from the settings panel only (after a 10 s wait) |
| `relockFeeds` | | Lock feeds again now |
| `pinPage` | | Pin the current page to essentials (max 9) |
| `unpinEssential` | `index` | Remove a pinned site |
| `stayLoggedIn` | | Keep the current site's cookies across restarts (everything else starts fresh) |
| `forgetLogin` | `domain` | Stop keeping that site's cookies (gone at the next start) |
| `openTab` | `url` | New background tab (adult check + tab cap); loads when activated |
| `importSlop` | `text` | Add a shared slop list (one host or URL per line) |
| `find` | `text`, `backwards?` | Select the next/previous match in the page (wraps around) |
| `zoom` | `step` (1, -1, 0) | Zoom in/out/reset; remembered per site |

App-level messages, handled in `src/app` (not browser state):

| `type` | Extra fields | Meaning |
|---|---|---|
| `window` | `action`: `drag` \| `minimize` \| `maximize` \| `close` | Own window buttons (Windows / Linux) |
| `setIcon` | `name` (a built-in icon) | Saves `<config>/icon` and swaps the icon live; the app answers with `window.canbec.setIcon(name)` |
| `openConfig` | | Shows the config folder in the file manager |
| `switchWorkspace` | `id` | Switch profile: saves logins, clears site data, loads that profile's tabs and logins |
| `addProfile` | `name` | New profile (max 6) and switch to it |
| `setAgents` | `id`, `on` | Allow AI agents (`canbec --mcp`) to browse as that profile |
| `exportSlop` | | Saves the user's slop tags to Downloads as `canbec-slop.txt` |

The settings panel is part of the sidebar (the gear toggles it). The app also injects
`window.__platform`, `window.__version`, `window.__icons` (built-in icons with previews) and
`window.__icon` (the active one).

## Rules

- The core is the source of truth. The UI never changes its own state optimistically; it waits
  for the next `render`.
- The core decides about the tab cap, slop matching and lock timing (domain layer). The UI only
  displays them.
