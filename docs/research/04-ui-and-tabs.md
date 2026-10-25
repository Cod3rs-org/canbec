# 04 — Browser UI, tab discipline & slop tagging

## Layout (Zen-inspired)

```
┌──────────────┬─────────────────────────────────────────────┐
│ 🔍 search     │                                             │
│ ┌──┐┌──┐┌──┐ │                                             │
│ │★ ││★ ││★ │ │   ← essentials (pinned sites)                │
│ └──┘└──┘└──┘ │                                             │
│ ● Personal   │           page (rounded, floating card)     │
│  ▸ Tab       │                                             │
│  ▸ Tab   ⚠   │   ⚠ = tagged AI slop → orange border        │
│  + New tab   │                                             │
│              │                                             │
│ 12/20 tabs   │                                             │
│ ● ● ○   ⬇    │   ← workspaces, downloads                    │
└──────────────┴─────────────────────────────────────────────┘
```

Zen features we want: vertical tabs, workspaces, essentials, compact mode (sidebar hides itself).
Later: split view.

## Tab discipline: "be a human, close your tabs"

| Rule | Behaviour |
|---|---|
| **Soft cap of 20 open tabs** | Counter in the sidebar (`12/20`). It turns amber at 16. At 20, a new tab first asks you to close or archive one. You can't just keep opening tabs. |
| **Auto-archive** | A tab left idle for **3 days** (default) goes into an *archive folder*. It's labelled automatically by site / domain group, e.g. `GitHub`, `Docs`, `Shopping`. No AI: simple domain-to-category rules plus the user's own folders. |
| **Deleted after 2 months** | An archived tab not opened for **60 days** moves to *Deleted*. It's hidden from the UI but **never hard-deleted**; search can still find it. |
| **Restore** | Opening an archived or deleted entry brings it back as a live tab and resets its timer. |

Storage: a local SQLite table `tabs(url, title, folder, state[open|archived|deleted], last_active_at)`.
Nothing is ever removed with `DELETE`; the state only changes.

## "Tag as AI slop"

- A shield button in the address bar (the same idea as Kagi's SlopStop, but local).
- One click marks the **domain** (default) or **this page only**.
- Next visit: the page gets a **2–3 px warning border** (orange stripes) and a small dismissible banner:
  *"You marked this site as AI slop on 2026-10-25"*.
- Search results from tagged domains get flagged or ranked lower (see 02).
- Storage: a local SQLite table `slop_tags(pattern, scope, created_at, note)`. Import and export as a
  uBlacklist-compatible list, so tags can be shared later.

## AI spoiler overlay (from the kickoff)

- An image with C2PA / IPTC metadata marking it as AI-made is covered by a frosted overlay saying
  *"AI-generated — click to reveal"* (like a Discord spoiler).
- Later: a local classifier can mark more images. It is **opt-in** and labelled *"suspected"*.

## Ad overlay (from the kickoff)

- Ads matched by adblock-rust cosmetic and network rules **keep their layout space**, so the page
  doesn't jump.
- They get `filter: blur(24px)` plus grey diagonal hatching on top
  (`repeating-linear-gradient(45deg, #ddd 0 6px, #bbb 6px 12px)`).
