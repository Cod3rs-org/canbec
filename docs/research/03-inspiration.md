# 03 — Open-source projects to learn from

Stars and activity checked with the GitHub API on 2026-10-25.

## Engines & browser shells

| Project | Lang | ★ | Status | What to take from it |
|---|---|---|---|---|
| [servo/servo](https://github.com/servo/servo) | Rust | 38k | Active | Embeddable engine (`servo` crate, LTS 0.1.x); `servoshell` is a reference browser UI |
| [versotile-org/verso](https://github.com/versotile-org/verso) | Rust | 5.4k | **Archived 2025** | A Servo-based browser that got further than most. Its lessons on multi-process and UI work are useful |
| [DioxusLabs/blitz](https://github.com/DioxusLabs/blitz) | Rust | 4.3k | Active | Lightweight HTML/CSS (Stylo + Taffy + Vello); experimental Boa JS; good for our **own UI chrome** |
| [tauri-apps/wry](https://github.com/tauri-apps/wry) | Rust | 5.0k | Active | Cross-platform system webview: WebKit / WebView2 / WebKitGTK |
| [LadybirdBrowser/ladybird](https://github.com/LadybirdBrowser/ladybird) | C++ (→ Swift) | 67k | Active | Proof that a brand-new engine can be built; good architecture docs |
| [zen-browser/desktop](https://github.com/zen-browser/desktop) | Firefox fork | 45k | Active | **UI reference:** vertical tabs, workspaces, essentials, compact mode, split view |
| [Floorp-Projects/Floorp](https://github.com/Floorp-Projects/Floorp) | Firefox fork | 8.5k | Active | Vertical tabs and workspaces done another way |
| [minbrowser/min](https://github.com/minbrowser/min) | Electron | 9.2k | Active | Minimal UI; "tasks" as tab groups (a reference for archive folders) |
| [qutebrowser](https://github.com/qutebrowser/qutebrowser) | Python/Qt | 11.7k | Active | Keyboard-first, minimal, hackable |

## Blocking (ads, adult, AI)

| Project | ★ | What to take from it |
|---|---|---|
| [brave/adblock-rust](https://github.com/brave/adblock-rust) | 2.9k | **Use directly.** EasyList / uBO network and cosmetic filters, written in Rust (MPL-2.0) |
| [gorhill/uBlock](https://github.com/gorhill/uBlock) | 69k | Filter syntax, cosmetic filtering, how the element picker works |
| [iorate/ublacklist](https://github.com/iorate/ublacklist) | 6.7k | Hides domains on search-result pages; subscription format |
| [laylavish/uBlockOrigin-HUGE-AI-Blocklist](https://github.com/laylavish/uBlockOrigin-HUGE-AI-Blocklist) | 5.8k | CC0 list of AI-content sites. DuckDuckGo uses it too |
| [StevenBlack/hosts](https://github.com/StevenBlack/hosts) | 31k | Hosts lists with a porn extension (MIT) |
| [hagezi/dns-blocklists](https://github.com/hagezi/dns-blocklists) | 28k | NSFW, ads and tracking DNS lists (GPL-3.0, check the licence before bundling) |
| [hickory-dns/hickory-dns](https://github.com/hickory-dns/hickory-dns) | 5.5k | Rust DNS resolver with DoH support. Lets the browser use its own **filtered resolver** |
| [0xERR0R/blocky](https://github.com/0xERR0R/blocky) | 7.1k | Simple DNS blocker; reference for the system-level lock service |
| [contentauth/c2pa-rs](https://github.com/contentauth/c2pa-rs) | 0.4k | Reads C2PA Content Credentials, which mark AI images |

## Search

| Project | Lang | ★ | Status | What to take from it |
|---|---|---|---|---|
| [MarginaliaSearch](https://github.com/MarginaliaSearch/MarginaliaSearch) | Java | 2.2k | Active | Independent index focused on the human, non-commercial web |
| [mwmbl/mwmbl](https://github.com/mwmbl/mwmbl) | Python | 1.9k | Active | Non-profit, community-crawled index |
| [StractOrg/stract](https://github.com/StractOrg/stract) | Rust | 2.4k | **Archived** | A full Rust search engine (crawler, index, ranking) to learn from |
| [searxng/searxng](https://github.com/searxng/searxng) | Python | 39k | Active | Meta-search result merging; engine adapters |
| [yacy](https://github.com/yacy/yacy_search_server) | Java | 4.1k | Active | Peer-to-peer search. A possible future "human web" network |

## Licence notes

- This repo is **Unlicense** (public domain). Code copied from MPL-2.0 projects (Servo,
  adblock-rust) stays MPL in those files. Using them as libraries is fine.
- GPL lists (HaGeZi) and AGPL code (Stract, SearXNG) should be **downloaded at runtime**, not
  bundled or copied.
