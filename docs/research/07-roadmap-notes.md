# 07 — Roadmap notes (requests captured during kickoff)

## Guiding rule: simple to extend, simple to change

- 4 Rust crates, no more: `domain` (rules) → `application` (use cases + 2 ports) → `infrastructure`
  (adapters) → `app` (wiring). A new engine is one adapter file. A new rule is one domain module
  plus one test.
- The UI is plain HTML/CSS/JS (`src/ui/`). No framework, no build step: edit and reload.
- Config, themes, filters and extensions are **plain files** in the user's config folder, so you can
  change them with a text editor or an AI.

## UI customization

- **Theme tokens:** `~/.config/canbec/theme.css` overrides the sidebar's CSS custom properties
  (`--bg`, `--fg`, `--accent`, radii, density). It's loaded after the built-in styles.
- **Layout switches:** sidebar left or right, compact mode, essentials on or off (in settings).
- **`user.css` / `user.js`:** advanced, loaded into the sidebar only (never into websites).
- **AI skill `customize-canbec-ui`:** "make it look like Gruvbox / bigger tabs / no essentials" →
  edits `theme.css` and settings and shows a before/after screenshot. It never touches the Rust core.

## Benchmarks (canbec vs other browsers)

Run with `tools/bench` (to be written) on the same machine against `test/engine-probe` plus 5
public pages:

| Metric | How |
|---|---|
| Cold start → first paint | Time from process start to the page-load event |
| Idle memory (RSS) with 1 / 10 tabs | `ps` / Activity Monitor equivalent; summed over child processes |
| Install / binary size | Size on disk |
| Engine probe score | `test/engine-probe` ✅ count |
| Blocking quality | Fixture pages: ads covered, adult domain blocked, slop border shown |

Compared against: Safari, Chrome, Firefox, Zen, Brave (if installed). Results go into
`docs/benchmarks/` as dated tables, and the README shows the latest.

## Launch on Hacker News (when there's something people can try)

- **Prerequisites:** downloadable builds for macOS, Windows and Linux; a 30-second GIF; README with
  screenshots; a benchmarks table; a clear "what it is NOT" section.
- **Post:** `Show HN: canbec – a lightweight Rust browser that hides AI slop, ads and infinite feeds`.
  Post on a weekday morning US time; be in the thread for the first 2–3 hours.
- **First comment** (by the author): why (de-AI and un-addict your life), how it works (wry/Servo,
  adblock-rust, Wayback "first seen" dating), honest limitations (heuristics, false positives), and
  what feedback you want.
- **Expect questions about:** "isn't this just uBlock + uBlacklist?" (answer: hard locks, slop
  tagging, tab discipline, feed lock, all in one place); privacy (no telemetry); Wayback API load
  (results are cached).
