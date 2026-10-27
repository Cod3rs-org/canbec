# 11 · Settings: what other browsers show, and what canbec shows

Settings moved from the sidebar to a full-window page, like `chrome://settings` or Firefox's
`about:preferences`: a category menu on the left, one scrolling page, and a search box at the top.

## How the big browsers group settings

| Chrome | Firefox | Safari (macOS) |
|---|---|---|
| You and Google (sync) | General (startup, tabs, language, files and downloads, updates) | General (startup, homepage, downloads) |
| Autofill and passwords | Home | Tabs |
| Privacy and security (clear data, cookies, site permissions, Safe Browsing) | Search | AutoFill, Passwords |
| Performance (memory saver, battery) | Privacy & Security (tracking protection, cookies, permissions, HTTPS-only) | Search |
| Appearance (theme, fonts, zoom) | Sync | Security, Privacy |
| Search engine | Extensions & themes (separate page) | Websites (per-site permissions) |
| Default browser | | Profiles |
| On startup | | Extensions |
| Languages, Downloads, Accessibility, System, Reset | | Advanced |
| About Chrome (version, updates) | | |

Sources: [Chrome settings overview](https://www.customguide.com/google-chrome/access-chrome-settings),
[Firefox settings](https://support.mozilla.org/kb/firefox-options-preferences-and-settings),
[Safari settings](https://support.apple.com/guide/safari/ibrwcd8bc28e/mac).

## What canbec took, and what it left out on purpose

| Their section | canbec | Why |
|---|---|---|
| Profiles / You and Google | **Profiles** (local only, no accounts) | Built (#27) |
| Privacy and security | **Privacy**, **Privacy tools**, **Adult content** | Fresh browser every start; nothing to clear between runs |
| Site permissions | One line in Privacy: sites ask, canbec doesn't save the answers | There is no stored list to manage |
| Search engine | **Search** (read-only) | Fixed to `noai.duckduckgo.com` until canbec's own search exists |
| Downloads | **Downloads**: where files go, open the folder | No history to show; no "ask where to save" yet |
| Appearance | **Appearance**: app icon, `theme.css` | Light/dark follows the OS |
| Extensions | **Extensions** | The small MV3 subset (#29) |
| About (version, updates) | **About canbec**: version, install update, what's new, report a problem | |
| Keyboard shortcuts (Vivaldi, Arc) | **Keyboard shortcuts** (read-only list) | Same map as `content.js` and the sidebar |
| canbec only | **News feeds**, **AI slop**, **AI text hints**, **Pinned sites** | The point of canbec |
| Autofill, passwords | left out | Use a password manager (linked in Privacy tools); storing secrets is a big attack surface |
| Sync | left out | No accounts, no servers |
| Performance, memory saver | left out | 20-tab cap and auto-archive already do this |
| On startup | left out | canbec always reopens your tabs |
| Languages, accessibility, fonts, reset | left out for now | The OS settings apply; add when someone asks |
| Default browser | **Default browser**: macOS asks Launch Services (the bundle declares http/https; links arrive as tao `Opened` events). Windows registers under `HKCU` and opens Default apps, where the user picks canbec. Linux writes `canbec.desktop` and runs `xdg-settings` | One canbec at a time: a second start (a clicked link on Windows/Linux) hands its URL to the running one and quits: a Unix socket `<config>/instance.sock` (owner-only, websites can't reach it) on macOS/Linux, 127.0.0.1 + a token from `<config>/instance` on Windows |

## How it works

The sidebar webview already spans the whole window (the site is a card on top of it). Opening
settings sends `settingsView {open: true}`; the core hides the website card and the sidebar's
settings page takes the full width. Done, Esc, any browser shortcut or a link that navigates
closes it again (`settingsView {open: false}`). Security stays the same: settings commands still
come only from the sidebar, never from websites.
