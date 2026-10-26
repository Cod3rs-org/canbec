# Roadmap

Every open piece of work is a GitHub issue. This page is the overview, grouped by milestone.
Live status: [issues](https://github.com/Cod3rs-org/canbec/issues) ·
[milestones](https://github.com/Cod3rs-org/canbec/milestones). Done work is in the README journey table.

## v0.1 · first release

Downloadable, works on macOS end to end, Windows and Linux verified by hand.

| # | Issue | Area | Done |
|--:|---|---|:-:|
| [#1](https://github.com/Cod3rs-org/canbec/issues/1) | Publish v0.1.0 release | release | |
| [#2](https://github.com/Cod3rs-org/canbec/issues/2) | Test Windows and Linux by hand | release, windows, linux | |
| [#3](https://github.com/Cod3rs-org/canbec/issues/3) | Sign and notarize the macOS app | release, macos | |
| [#4](https://github.com/Cod3rs-org/canbec/issues/4) | Windows: icon in the .exe (and maybe an installer) | release, windows | ✅ |
| [#5](https://github.com/Cod3rs-org/canbec/issues/5) | Publish the GitHub wiki | documentation | |
| [#6](https://github.com/Cod3rs-org/canbec/issues/6) | Real back/forward state | browser | ✅ |
| [#7](https://github.com/Cod3rs-org/canbec/issues/7) | Find in page (Cmd/Ctrl+F) | browser | ✅ |
| [#8](https://github.com/Cod3rs-org/canbec/issues/8) | Zoom in/out (Cmd/Ctrl +, -, 0) | browser | ✅ |
| [#9](https://github.com/Cod3rs-org/canbec/issues/9) | Open link in a new tab | browser | ✅ |
| [#10](https://github.com/Cod3rs-org/canbec/issues/10) | Camera and microphone permissions | browser, macos | ✅ |
| [#11](https://github.com/Cod3rs-org/canbec/issues/11) | Reload a tab when the web process crashes | browser | ✅ |
| [#33](https://github.com/Cod3rs-org/canbec/issues/33) | Hacker News launch (Show HN) | launch | |

## v0.2 · every OS equal

Windows and Linux get everything macOS has (tracker blocking, rounded card, DNS lock).

| # | Issue | Area | Done |
|--:|---|---|:-:|
| [#12](https://github.com/Cod3rs-org/canbec/issues/12) | Tracker blocking on Linux (WebKitGTK) | privacy, linux | |
| [#13](https://github.com/Cod3rs-org/canbec/issues/13) | Tracker blocking on Windows (WebView2) | privacy, windows | 🧪 |
| [#14](https://github.com/Cod3rs-org/canbec/issues/14) | Block third-party cookies within a session on Windows | privacy, windows | 🧪 |
| [#15](https://github.com/Cod3rs-org/canbec/issues/15) | Rounded content card on Windows and Linux | ui, windows, linux | |
| [#16](https://github.com/Cod3rs-org/canbec/issues/16) | Adult block: system DNS lock | blocking | ✅ |
| [#17](https://github.com/Cod3rs-org/canbec/issues/17) | Benchmarks on Windows and Linux | release | |
| [#18](https://github.com/Cod3rs-org/canbec/issues/18) | Encrypt the saved logins with the OS keychain | privacy | ✅ |
| [#19](https://github.com/Cod3rs-org/canbec/issues/19) | Stay-logged-in sites: keep localStorage/IndexedDB too | privacy | ✅ |
| [#20](https://github.com/Cod3rs-org/canbec/issues/20) | Content scripts in an isolated world | privacy, blocking | 🧪 |
| [#21](https://github.com/Cod3rs-org/canbec/issues/21) | EasyPrivacy path rules and site exceptions | blocking | ✅ |
| [#22](https://github.com/Cod3rs-org/canbec/issues/22) | More feed-lock sites and a user list | blocking | ✅ |
| [#34](https://github.com/Cod3rs-org/canbec/issues/34) | Fake device persona: common hardware, never the real values | privacy, windows, linux, macos | 🧪 |

## v0.3 · the human web

Own search, AI spoilers, AI text hints: the web as written by humans.

| # | Issue | Area | Done |
|--:|---|---|:-:|
| [#23](https://github.com/Cod3rs-org/canbec/issues/23) | Own search: mark results first seen after 2023 as "obsolete" | ai-detection | ✅ |
| [#24](https://github.com/Cod3rs-org/canbec/issues/24) | AI image spoilers (metadata first) | ai-detection | ✅ |
| [#25](https://github.com/Cod3rs-org/canbec/issues/25) | AI text hints with a small local model | ai-detection | 🧪 |
| [#26](https://github.com/Cod3rs-org/canbec/issues/26) | Shared slop lists | ai-detection, blocking | ✅ |

## v1.0 · agents & extensions

Profiles, MCP for AI agents, Chrome extension subset, AI skills.

| # | Issue | Area | Done |
|--:|---|---|:-:|
| [#27](https://github.com/Cod3rs-org/canbec/issues/27) | Profiles | extensibility | ✅ |
| [#28](https://github.com/Cod3rs-org/canbec/issues/28) | MCP server for AI agents (off by default, per profile) | extensibility | ✅ |
| [#29](https://github.com/Cod3rs-org/canbec/issues/29) | Chrome extension subset (MV3) | extensibility | 🧪 |
| [#30](https://github.com/Cod3rs-org/canbec/issues/30) | AI skill: port a Chrome extension to canbec | extensibility | ✅ |
| [#31](https://github.com/Cod3rs-org/canbec/issues/31) | AI skill: restyle canbec | extensibility, ui | ✅ |
| [#32](https://github.com/Cod3rs-org/canbec/issues/32) | Engine adapters: Servo and Chromium (CEF) | extensibility | 🧪 Servo headless; no window, IPC or CEF yet |

✅ done · 🧪 built, needs a hand test in the running app

## How to pick one

- Start from the top of v0.1. Each issue has **Done when** checkboxes and **Where to look**.
- Keep it simple (see CLAUDE.md): a new rule = a domain module + a test, a new engine = one adapter.
- New decisions go into `docs/research/NN-*.md`, finished steps into the README journey table.
