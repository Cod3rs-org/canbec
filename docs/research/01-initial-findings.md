# 01 — Initial findings (2026-10-25)

## Goals (from the kickoff)

1. Lightweight browser written in Rust, runs on **Windows, Linux and macOS**.
   It must run the author's own apps, e.g. http://localhost:5080.
2. Adult-content blocking you can't switch off (browser + system DNS).
3. Search without AI slop: mark results that first appeared after 2023 as obsolete, and
   detect AI text and images and hide them behind a spoiler overlay, like a Discord spoiler.
4. No visible ads: cover each ad with a grey hatched overlay and heavy blur.

## Decisions so far

| Topic | Decision |
|---|---|
| Engine | Undecided. Build a prototype on each of three candidates and compare them by hand (see 02). |
| Adult block | Hardcoded in the browser **and** set system DNS to a family resolver |
| Search | Undecided. Compare options; the user likes DuckDuckGo's no-AI search |
| AI detection | Metadata first (C2PA / IPTC). Optional local model later |

## What the local Blazor test app needs from an engine

Checked by reading the page's HTML:

- **Blazor WebAssembly** (`_framework/blazor.webassembly.js`), so the engine needs **WASM**
- **MapLibre**, so it needs **WebGL**
- **Chart.js**, so it needs **Canvas 2D**
- **html5-qrcode**, so it needs camera access (`getUserMedia`)
- Bootstrap 5 JS, Google Fonts, a web manifest (PWA)

Conclusion: "basic JS" is not enough. The engine has to be close to a full browser engine.

## Engine candidates

| Candidate | Language | JS / WASM | Notes |
|---|---|---|---|
| [Servo](https://servo.org) | Rust | Yes (SpiderMonkey) | Released on crates.io as `servo` 0.1.0 in 2026. The API is built around `ServoBuilder` / `WebView` / a delegate. An LTS line is planned. |
| [wry](https://github.com/tauri-apps/wry) (Tauri) | Rust shell around the OS engine | Yes (full) | Uses WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux. Tiny binary; the engine itself isn't Rust. |
| [Blitz](https://github.com/DioxusLabs/blitz) | Rust | **No JS** | Stylo + Taffy + Vello. Fast HTML/CSS rendering. Can't run Blazor. |

## Filtering building blocks

- **Ads:** [`adblock-rust`](https://github.com/brave/adblock-rust) (Brave) reads EasyList and uBO-style
  filter lists, including cosmetic rules. We cover matched ads with an overlay instead of removing them.
- **Adult content:** StevenBlack hosts (porn extension), HaGeZi NSFW, OISD NSFW;
  Cloudflare `1.1.1.3` / `1.0.0.3` family DNS.
- **AI images:** C2PA Content Credentials, the IPTC `digitalSourceType = trainedAlgorithmicMedia` field.
- **AI-site blocklists:** uBlacklist "Huge AI Blocklist" and uBlockOrigin lists. DuckDuckGo uses these
  for its AI-image filter.
- **"First seen" date:** the Wayback Machine CDX API (first capture of a URL). Cutoff idea: before
  2022-11-30 (ChatGPT launch) counts as pre-AI.

## Search options

- **DuckDuckGo NoAI** — https://noai.duckduckgo.com turns off AI answers and hides AI images, using
  curated blocklists. No official public results API.
- **Brave Search API** — independent index. 2026 pricing: about $5 per 1,000 requests, with $5 of
  free credit each month.
- **Mojeek, Marginalia** — independent indexes; Marginalia favours the old and independent web.
  Pricing not yet checked.

Sources: [Simon Willison on the servo crate](https://simonwillison.net/2026/Apr/13/servo-crate-exploration/),
[Phoronix: Servo embed crates / LTS](https://phoronix.com/news/Servo-Embed-Crates-LTS),
[Blitz README](https://cdn.jsdelivr.net/gh/dioxuslabs/blitz@main/README.md),
[Vice: DuckDuckGo no-AI mode](https://www.vice.com/en/article/duckduckgo-no-ai-mode),
[Slashdot: DDG hides AI images](https://tech.slashdot.org/story/25/07/18/2046207/duckduckgo-now-lets-you-hide-ai-generated-images-in-search-results),
[Brave API pricing change](https://www.implicator.ai/brave-drops-free-search-api-tier-puts-all-developers-on-metered-billing.md).
