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
  "feedLock":  { "locked": true, "unlockedUntil": null }   // News-feed blocker; unlock max 6 h, settings only
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
| `switchWorkspace` | `id` | |
| `openSettings` | | Settings page (the only place to unlock the feed blocker) |
| `unlockFeeds` | `hours` (1–6, the core enforces the cap) | Temporary unlock, from settings only |

## Rules

- The core is the source of truth. The UI never changes its own state optimistically; it waits
  for the next `render`.
- The core decides about the tab cap, slop matching and lock timing (domain layer). The UI only
  displays them.
