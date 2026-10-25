<div align="center">

<img src="docs/img/logo.svg" width="128" alt="canbec logo: cut puppet strings above a free human">

# canbec

### **C**ontrol **A**I, **N**ot **B**e **C**ontrolled

*A lightweight, cross-platform browser written in Rust that hides AI slop, ads and adult content,
so you can browse the web as a human, not a parrot.*

![status](https://img.shields.io/badge/status-research%20%26%20prototyping-orange)
![rust](https://img.shields.io/badge/built%20with-Rust-b7410e?logo=rust)
![platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20Linux%20%7C%20macOS-blue)
![license](https://img.shields.io/badge/license-Unlicense-lightgrey)

</div>

---

## 🛤️ Journey so far

```mermaid
timeline
    title canbec — what is built
    2026-10-25 : 1 · Repo set up
               : 2 · Research & decisions
               : 3a · Engine bake-off — wry ✅
               : 3b · Engine bake-off — Servo ⏳
               : 3c · Engine bake-off — Blitz ⏳
               : 4 · Search, inspiration, UI design docs ✅
```

| # | Step | Status | Read more |
|:-:|------|:------:|-----------|
| 1 | Repo set up: `src/`, `test/`, `docs/` | ✅ | |
| 2 | Kickoff research: what my apps need from an engine | ✅ | [01-initial-findings](docs/research/01-initial-findings.md) |
| 3 | Engine bake-off: wry vs Servo vs Blitz | 🔄 | [src/prototypes/](src/prototypes/) · [05-engine-bakeoff](docs/research/05-engine-bakeoff.md) |
| 4 | Search without AI: comparison and plan | ✅ | [02-search](docs/research/02-search.md) |
| 5 | Open-source inspiration | ✅ | [03-inspiration](docs/research/03-inspiration.md) |
| 6 | UI: vertical tabs, 20-tab discipline, slop tagging | ✅ design | [04-ui-and-tabs](docs/research/04-ui-and-tabs.md) |

## 🧪 Engine bake-off

Test page: [the local Blazor test app](http://localhost:5080). It's a Blazor WebAssembly app
that uses MapLibre (WebGL), Chart.js (canvas) and a QR scanner (camera).

| wry (system webview) | Servo | Blitz |
|:-:|:-:|:-:|
| <img src="docs/img/engine-wry-testapp.png" width="280"> | ⏳ building | ⏳ building |
| ✅ Renders fully · 1.5 MB binary | | |

## 🎯 Features

| Feature | Idea | Progress |
|---|---|---|
| 🧱 **Engine** | Runs real web apps (WASM, WebGL, Canvas) on all 3 OSes | `▓▓▓░░░░░░░` prototyping |
| 🔞 **Adult lock** | Built into the browser **and** system DNS (family resolver); no off switch | `▓░░░░░░░░░` researched |
| 🔎 **Human search** | Results first seen after 2023 marked *obsolete*; NoAI DuckDuckGo first | `▓▓░░░░░░░░` designed |
| 🫥 **AI spoilers** | AI images hidden behind a click-to-reveal overlay (C2PA / IPTC first) | `▓░░░░░░░░░` researched |
| ⚠️ **Slop tagging** | One click marks a site; it gets a warning border on every later visit | `▓▓░░░░░░░░` designed |
| 🚫 **No ads** | Ads blurred and covered in grey hatching | `▓░░░░░░░░░` researched |
| 🗂️ **Tab discipline** | Vertical tabs, at most 20 open, auto-archive to folders, soft-delete after 2 months | `▓▓░░░░░░░░` designed |

## 📁 Layout

```
src/          the browser (coming next)
test/         tests
docs/         research notes, decisions, screenshots
src/prototypes/throwaway engine experiments (one crate each)
tools/        small helper scripts (e.g. window screenshots)
```
