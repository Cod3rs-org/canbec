---
name: port-chrome-extension
description: Port a Chrome (MV3 or MV2) extension to canbec's small extension subset — keep what runs, rewrite what can be rewritten, list the rest. Use when someone wants a Chrome extension in canbec, asks "can canbec run <extension>", or points at an unpacked extension folder or its source.
---

# Port a Chrome extension to canbec

canbec runs a small part of Chrome's extension API. You read the extension, keep what runs,
rewrite what can move into a content script, and write down everything else. **Never fake an
API** (no stubs that silently do nothing): unsupported parts are dropped and listed.

## What canbec runs

Source of truth: `src/infrastructure/src/extensions.rs` and `scripts/extensions.js`
(read them if anything below seems off).

| Manifest key / API | canbec |
|---|---|
| `content_scripts`: `matches`, `exclude_matches`, `js`, `css`, `run_at`, `all_frames` | ✅ |
| `chrome.storage.local` / `.sync` (`get`, `set`, `remove`, `clear`, callbacks and promises), `storage.onChanged` | ✅ (sync = local, 1 MB, onChanged only in the same page) |
| `chrome.runtime.id`; `browser.*` alias for `chrome.*` | ✅ |
| background / service worker, `runtime.sendMessage`/`onMessage`, `tabs`, `action`, popups, options pages, `contextMenus`, `alarms`, `notifications`, `i18n`, `web_accessible_resources`, `declarativeNetRequest`, `webRequest` | ❌ |
| Match patterns: `<all_urls>`, `http`/`https`/`*` schemes | ✅ (`file:` and others are skipped) |

Scripts run in the page's own JavaScript world (no isolated world yet), so the page can see
what they define.

## Steps

1. **Read everything**: `manifest.json`, every script it names, HTML pages and the scripts they
   load. If you got a `.crx`/`.zip`, unpack it into its own scratch folder first and don't run
   anything from it.
2. **Sort each part** into: runs as is · can be rewritten · drop.
   Common rewrites:
   - Content script → `runtime.sendMessage` → background that only reads/writes storage:
     do the storage call in the content script directly.
   - Background that injects CSS/JS into matching pages (`scripting.executeScript`,
     `insertCSS`): turn it into a `content_scripts` entry with the same matches.
   - Options page that only sets storage values: keep the defaults in the `get({...})` calls and
     document the keys (the user edits `<config>/extension-storage/<id>.json`).
   - `chrome.i18n.getMessage` for a few strings: inline the English text.
   - `__MSG_name__` in the manifest name: replace with the English name.
   Drop (and list): toolbar badges/popups, notifications, network blocking, anything that
   needs another tab or a background page that does real work (fetching, timers).
3. **Write the port** into a new folder named with letters, digits, `-`, `_` only (that's the
   extension id): a trimmed `manifest.json` with only `manifest_version`, `name`, `version`,
   `description`, `content_scripts` (and `permissions: ["storage"]` if used), plus the files it
   names. Mark each changed line with a `// canbec:` comment saying what it replaced.
4. **Write `PORTING.md`** in that folder: a table of every part with ✅ runs / ✅ rewritten /
   ❌ dropped / TODO, one line on what changed, and the install line:
   copy the folder to `<config>/extensions/<id>/` and restart canbec.
5. **Check**: every file in the manifest exists, paths have no `..`, the JS still parses
   (`node --check file.js`), and no `chrome.*` call outside the ✅ list is left.
   If a core feature of the extension can't run, say so plainly at the top of `PORTING.md`
   instead of shipping a half-working port.

Config folder: `~/Library/Application Support/canbec` (macOS), `~/.config/canbec` (Linux),
`%APPDATA%\canbec` (Windows). Settings → Extensions lists what loaded.

## Example

`docs/examples/extensions/reading-time/`: `chrome/` is the original (background counter,
toolbar badge, options page), `canbec/` is the port with its `PORTING.md`.
