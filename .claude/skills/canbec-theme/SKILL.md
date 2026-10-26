---
name: canbec-theme
description: Write a theme.css for the canbec browser sidebar from a plain description ("calm forest", "high contrast", "like my terminal"). Use when someone asks to restyle, recolour or theme canbec.
---

# Restyle canbec

canbec's sidebar reads `theme.css` from the config folder at start and lets it override CSS
variables. You write that file. Nothing else needs to change.

Config folder: `~/Library/Application Support/canbec` (macOS), `~/.config/canbec` (Linux),
`%APPDATA%\canbec` (Windows). Changes show after a restart (Settings → Look → Open config folder).

## The variables (all of them)

| Variable | Used for |
|---|---|
| `--bg` | window frame and sidebar background |
| `--fg` | main text and icons |
| `--mut` | secondary text, section titles |
| `--card` | chips, address bar, essentials tiles, settings boxes (usually `--fg` at 6–7% alpha) |
| `--hov` | hover background (usually `--fg` at 9–11% alpha) |
| `--act` | active tab, focused address bar, buttons |
| `--line` | thin separators |
| `--acc` | accent: slop shield, focus ring, links, warnings |
| `--amber` / `--red` | tab counter near / at the 20-tab limit, error notices |
| `--sh` | shadow under the active tab and buttons |

The defaults live at the top of `src/ui/sidebar.html` (`:root` and its dark-mode block).

## How to write one

1. Pick `--bg`, `--fg` and `--acc` from the description, then derive the rest:
   `--card`/`--hov`/`--line` as `--fg` with low alpha, `--act` a bit lighter (light theme) or
   lighter-than-bg (dark theme) than `--bg`, `--mut` halfway between `--fg` and `--bg`.
2. Contrast: `--fg` on `--bg` and on `--act` at least 7:1, `--mut` on `--bg` at least 4.5:1,
   `--acc` against `--bg` at least 3:1 (it's used for focus rings).
3. Give a dark variant under `@media (prefers-color-scheme: dark)` unless the theme is always
   dark or always light on purpose (say so in the top comment).
4. Only set variables. No selectors, no `@import`, no URLs: the file is injected as a
   stylesheet into the trusted sidebar.
5. Start with a one-line comment naming the theme.

## Examples

`docs/themes/calm-forest.css`, `docs/themes/high-contrast.css`, `docs/themes/night-owl.css`.
To try one: copy it to `theme.css` in the config folder and restart canbec.
