# DeAiYourLife

> A lightweight, cross-platform browser written in Rust that hides AI slop, ads and adult content,
> so you can browse the web as a human.

## Journey

Each step is one commit (or a few). The newest step is at the bottom.

| # | Step | Status | Notes |
|---|------|--------|-------|
| 1 | Repo set up: `src/`, `test/`, `docs/` | ✅ | |
| 2 | Initial research & decisions | ✅ | [docs/research/01-initial-findings.md](docs/research/01-initial-findings.md) |

```
 [1 init] ──▶ [2 research] ──▶ ( 3 engine bake-off ) ──▶ ( 4 search ) ──▶ ( 5 inspiration ) ──▶ ...
```

## Planned features

- 🧱 **Engine** that runs real web apps (WASM, WebGL, Canvas). Choosing between Servo, wry and Blitz.
- 🔞 **Adult-content lock**, enforced in the browser and through system DNS.
- 🔎 **Human search**: results first seen after 2023 are marked *obsolete*.
- 🫥 **AI spoilers**: AI images and text are hidden behind a click-to-reveal overlay.
- 🚫 **No ads**: ads are blurred and covered with grey hatching.
