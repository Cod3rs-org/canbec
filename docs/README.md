# canbec docs

Everything we've learned and decided, in reading order. These files are the source of truth; the
[GitHub Wiki](https://github.com/Cod3rs-org/canbec/wiki) is a generated mirror (`python3 tools/sync_wiki.py`).

## Start here

| Doc | What's inside |
|---|---|
| [../README.md](../README.md) | What canbec is, screenshots, the journey so far |
| [../CLAUDE.md](../CLAUDE.md) | Commands, architecture rules, product rules, gotchas (for humans and AI) |
| [architecture/ui-contract.md](architecture/ui-contract.md) | The sidebar ↔ core protocol (state + commands) |

## Research & decisions

| # | Doc | Decision / outcome |
|---|---|---|
| 01 | [Initial findings](research/01-initial-findings.md) | Goals; real apps need WASM + WebGL + Canvas, so "basic JS" isn't enough |
| 02 | [Search without AI](research/02-search.md) | NoAI DuckDuckGo now → own meta-search with Wayback "first seen" dating → own index (stretch) |
| 03 | [Open-source inspiration](research/03-inspiration.md) | Servo, Zen, adblock-rust, uBlacklist, HUGE AI Blocklist, Marginalia, Stract … |
| 04 | [UI, tabs & slop tagging](research/04-ui-and-tabs.md) | Zen-style sidebar; 20-tab cap; auto-archive; never hard-delete |
| 05 | [Engine bake-off](research/05-engine-bakeoff.md) | wry default (5/6), Servo optional (4/6 + shim), Blitz dropped |
| 06 | [Extensibility](research/06-extensibility.md) | Any-engine harness, MV3 subset + porting skill, feed lock, MCP, profiles |
| 07 | [Roadmap notes](research/07-roadmap-notes.md) | Simplicity rule, UI customization, benchmarks, Hacker News launch |
| 08 | [No tracking](research/08-privacy.md) | Fresh browser every start, stay-logged-in sites, fingerprint noise, tracking params, privacy tools |

## Benchmarks

[2026-10-25 macOS](benchmarks/2026-10-25-macos.md): canbec vs Safari vs Edge (size, time to window, memory). Reproduce with `tools/bench.py`.

## Screenshots

| | |
|:-:|:-:|
| ![Wikipedia](img/real-wikipedia.png) Reading Wikipedia | ![YouTube](img/real-youtube-feed-locked.png) YouTube with the feed locked |
| ![Slop](img/real-slop-tagged.png) A site you tagged as AI slop | ![Probe](img/canbec-probe.png) Engine probe: Blazor WASM + ad hatching |

## Brand

Default icon: **sprout** (`img/logos/05-sprout-cursor.svg`). All 7 designs ship with the app and are
selectable. See the README's Icons section and [contact sheet](img/logos/contact-sheet.svg).

**Guides:** [family-dns.md](family-dns.md): block adult sites in every app (and how to undo it).

**What's next:** [ROADMAP.md](ROADMAP.md): all open work as GitHub issues, by milestone.
