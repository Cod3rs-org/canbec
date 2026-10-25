<div align="center">

<img src="docs/img/logos/05-sprout-cursor.svg" width="128" alt="canbec logo: a mouse pointer with a green sprout growing from its tip">

# canbec

### **C**ontrol **A**I, **N**ot **B**e **C**ontrolled

**De-AI your life. Un-addict your life.**

*A lightweight, cross-platform browser written in Rust that hides AI slop, ads, adult content and
infinite feeds, so you browse the web as a human, not a parrot.*

![status](https://img.shields.io/badge/status-early%20alpha-orange)
![rust](https://img.shields.io/badge/built%20with-Rust-b7410e?logo=rust)
![platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-blue)
![tests](https://img.shields.io/badge/tests-23%20passing-brightgreen)
![binary](https://img.shields.io/badge/binary-~4%20MB-lightgrey)
![license](https://img.shields.io/badge/license-Unlicense-lightgrey)

<img src="docs/img/real-wikipedia.png" width="860" alt="canbec showing Wikipedia: a cream vertical sidebar with tabs on the left, the page as a rounded card on the right">

</div>

---

## ✨ 5 reasons to switch

The web got loud. AI-generated pages bury human ones, feeds are built to never end, and twenty
forgotten tabs nag in the background. canbec is a **quiet browser with hard limits you set once and
can't easily undo in a weak moment.**

1. 🔕 **Hard limits for weak moments.** Infinite feeds are gone, and unlocking them takes a trip to
   settings and lasts **6 hours at most**. Adult content has **no off switch** at all.
2. ⚠️ **AI slop: mark it once, warned forever.** One click on the shield, and the site wears an
   orange frame on every visit. AI images get spoilers, and search marks pages first seen after 2023.
3. 🗂️ **20 tabs, zero hoarding.** Idle tabs file themselves into folders and fade out after two
   months, but nothing is ever really deleted.
4. 🪶 **Featherweight.** A 4 MB app that opens in 0.3 s and uses about **3× less memory than Safari**
   ([benchmarks](docs/benchmarks/2026-10-25-macos.md)).
5. 🔒 **Private and yours.** No telemetry, no accounts. Theme and icon are plain files you edit,
   and AI agents get in only if you invite them, per profile.

| | |
|:-:|:-:|
| <img src="docs/img/real-youtube-feed-locked.png" width="420" alt="YouTube with the recommendation feed removed"> | <img src="docs/img/real-slop-tagged.png" width="420" alt="A page tagged as AI slop, with an orange frame and banner"> |
| 🔕 **No infinite feeds.** YouTube's home feed is simply gone. Unlocking needs a trip to settings and lasts **6 hours at most**. | ⚠️ **Slop you tagged stays tagged.** One click on the shield, and the site gets an orange frame on every visit. |
| <img src="docs/img/canbec-probe.png" width="420" alt="Engine probe page with an ad covered by grey hatching"> | <img src="docs/img/real-hn.png" width="420" alt="Hacker News in canbec"> |
| 🚫 **Ads become grey hatching.** The layout stays put; the ad doesn't get your attention. | 🧘 **Calm by default.** Vertical tabs, at most 20 open; idle tabs tidy themselves into folders. |

And underneath:
- 🔞 **Adult content is blocked** at every navigation, works offline, with no off switch (system DNS lock next).
- 🫥 **AI images get a spoiler overlay** (C2PA / IPTC metadata first). *In progress.*
- 🔎 **Human search**: results first seen after 2023 are marked *obsolete*. For now, search goes
  to [DuckDuckGo NoAI](https://noai.duckduckgo.com).

## 🛤️ Journey so far

```mermaid
timeline
    title canbec — built on 2026-10-25
    Research : Goals and engine needs
             : Search, inspiration, UI design
    Bake-off : wry ✅ 5/6
             : Servo ✅ 4/6 with shim
             : Blitz ❌
    Design   : UI contract + Zen-style sidebar
             : Extensions, MCP, profiles
             : 7 logo concepts
    Build    : Clean-architecture core
             : Ad hatching, slop frame, feed lock
             : Integrated title bar, rounded card
             : 20 tests + 3-OS CI
    Measure  : ~3x less memory than Safari
             : 4-6x less than Edge
    Polish   : 5 selling points
             : Sprout icon, customizable
```

| # | Step | Status | Read more |
|:-:|------|:------:|-----------|
| 1 | Repo set up | ✅ | |
| 2 | Kickoff research: what real apps need from an engine | ✅ | [01](docs/research/01-initial-findings.md) |
| 3 | Search without AI: comparison and plan | ✅ | [02](docs/research/02-search.md) |
| 4 | Open-source inspiration | ✅ | [03](docs/research/03-inspiration.md) |
| 5 | UI: vertical tabs, 20-tab discipline, slop tagging | ✅ | [04](docs/research/04-ui-and-tabs.md) |
| 6 | Engine bake-off with a local Blazor WASM probe | ✅ | [05](docs/research/05-engine-bakeoff.md) |
| 7 | Extensibility: any engine, Chrome extension subset, feed lock, MCP, profiles | ✅ design | [06](docs/research/06-extensibility.md) |
| 8 | Sidebar UI (vanilla HTML/CSS/JS, ~16 KB) | ✅ | [ui-contract](docs/architecture/ui-contract.md) |
| 9 | **First working browser** (domain / application / infrastructure / app) | ✅ | [src/](src/) |
| 10 | Integrated title bar (macOS) + own window buttons (Windows / Linux), rounded card | ✅ | |
| 11 | Benchmarks vs Safari and Edge: about 3× less memory | ✅ | [benchmarks](docs/benchmarks/2026-10-25-macos.md) |
| 12 | 5 selling points, sprout as the default icon, customizable app icon | ✅ | [icons](#-icons) |
| 13 | Roadmap: customization, local model, HN launch | 📝 | [07](docs/research/07-roadmap-notes.md) |

📚 **All docs:** [docs/README.md](docs/README.md)

## ⚡ Benchmarks

macOS, same pages, median of 3 runs ([details & caveats](docs/benchmarks/2026-10-25-macos.md)):

| Browser | Size | Time to window | Memory (Wikipedia) | Processes |
|---|--:|--:|--:|--:|
| **canbec** | **4 MB** | **0.33 s** | **323 MB** | **5** |
| Safari | 35 MB | 1.08 s | 976 MB | 11 |
| Microsoft Edge | 1,111 MB | 0.64 s | 1,963 MB | 15 |

## 🧪 Engine bake-off

The same local Blazor WebAssembly probe ([`test/engine-probe`](test/engine-probe)) in each engine:

| wry (system webview) | Servo 0.1.4 | Blitz 0.3 |
|:-:|:-:|:-:|
| <img src="docs/img/engine-wry-probe.png" width="260"> | <img src="docs/img/engine-servo-probe.png" width="260"> | <img src="docs/img/engine-blitz-probe.png" width="260"> |
| ✅ **5/6** · 1.5 MB · 47 s build | ✅ 4/6 with shim · 143 MB · 7 m 45 s | ❌ no WASM · 51 MB |

**Decision:** the core talks only to an `Engine` interface. wry is the default; Servo is the
pure-Rust option; Chromium (CEF) and others can be plugged in later.

## 🎯 Features

| Feature | Idea | Progress |
|---|---|---|
| 🧱 **Engine** | Real web apps (WASM, WebGL, Canvas) on 3 OSes; pluggable engines | `▓▓▓▓▓▓░░░░` wry works |
| 🗂️ **Tab discipline** | At most 20 open, auto-archive into folders, soft delete after 2 months | `▓▓▓▓▓▓░░░░` core + UI |
| ⚠️ **Slop tagging** | One click; orange frame on every later visit | `▓▓▓▓▓▓░░░░` works |
| 🔕 **Feed blocker** | Feeds hidden; unlock only in settings, max 6 h | `▓▓▓▓▓░░░░░` works, no settings UI yet |
| 🚫 **No ads** | Ads blurred + grey hatching | `▓▓▓▓░░░░░░` basic selectors (adblock-rust next) |
| 🔞 **Adult lock** | Every navigation checked + system DNS lock | `▓▓▓▓░░░░░░` browser part done |
| 👤 **Profiles** | Separate identities; per-profile MCP access | `▓░░░░░░░░░` designed |
| 🔎 **Human search** | "Obsolete" badge for results first seen after 2023 | `▓▓░░░░░░░░` designed |
| 🫥 **AI spoilers** | AI images behind a click-to-reveal overlay | `▓░░░░░░░░░` researched |
| 🧩 **Extensions** | Chrome MV3 subset + an AI skill that ports extensions | `▓░░░░░░░░░` designed |
| 🤖 **MCP** | Opt-in local server; agents browse through the same filters | `▓░░░░░░░░░` designed |
| 🎨 **Customization** | `theme.css` tokens (works now) + an AI skill to restyle | `▓▓▓░░░░░░░` theme.css |

## 🚀 Try it

```sh
git clone https://github.com/Cod3rs-org/canbec && cd canbec
cargo run --release -p canbec              # Linux also needs WebKitGTK 4.1 (see CI workflow)
cargo test --workspace                     # 23 tests
```

**Make it yours:** put CSS variables in `theme.css` in your config folder, and they override the
sidebar. The config folder is `~/Library/Application Support/canbec` on macOS, `~/.config/canbec`
on Linux and `%APPDATA%\canbec` on Windows.

```css
:root { --acc: #4fae6b; --bg: #fbfaf7; }
```

**Pick your icon:** write a built-in name (see [Icons](#-icons)) or a path to your own PNG into a
file named `icon` in the same folder:

```sh
echo spoiler > ~/Library/Application\ Support/canbec/icon      # macOS
echo ~/Pictures/my-icon.png > ~/.config/canbec/icon            # Linux, custom PNG (max 1024 px)
```

## 🏗️ How it's built

```
 sidebar (src/ui, HTML)  ──commands──▶  BrowserService (application)  ──▶  Engine port ──▶ wry | Servo | …
          ▲                                    │  rules (domain)            StateStore ──▶ JSON file
          └────────────── state ───────────────┘
```

```
src/domain          rules: tabs, slop, feed lock, blocklists (no I/O)
src/application     use cases + ports (Engine, StateStore)
src/infrastructure  wry engine, JSON store, blocklists, content script
src/app             the browser binary (wiring, window)
src/ui              sidebar: plain HTML/CSS/JS
src/prototypes      engine bake-off crates
test/               Rust tests, Blazor WASM engine probe, fixtures
docs/               research, decisions, screenshots, logos
```

## 🎨 Icons

**Default: `sprout`**, a pointer growing a sprout: taking your life online back, one human click
at a time. All seven ship with the app; choose one in the `icon` file (see [Try it](#-try-it)).

| <img src="docs/img/logos/05-sprout-cursor.svg" width="72"> | <img src="docs/img/logo.svg" width="72"> | <img src="docs/img/logos/01-fingerprint.svg" width="72"> | <img src="docs/img/logos/02-not-a-parrot.svg" width="72"> | <img src="docs/img/logos/03-broken-link.svg" width="72"> | <img src="docs/img/logos/04-spoiler.svg" width="72"> | <img src="docs/img/logos/06-calm-eye.svg" width="72"> |
|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| **`sprout`** ⭐ | `strings` | `fingerprint` | `parrot` | `broken-link` | `spoiler` | `calm-eye` |
| growing back online | cut puppet strings | human uniqueness | not a parrot | break the scroll | AI hidden | calm, protected |
