# 02 — Search without AI slop

The aim: search that never shows AI answers, hides AI images, and marks results that **first
appeared after the AI boom** as *obsolete*.

## Options compared

| Option | Own index? | AI-free today | API / cost (2026) | Fit |
|---|---|---|---|---|
| **DuckDuckGo NoAI** (`noai.duckduckgo.com`) | No (mostly Bing) | ✅ No AI answers; AI images hidden using uBO / uBlacklist lists | No official results API | Good **default homepage** today, but we can't post-process its results without scraping |
| **Brave Search API** | ✅ Yes | Raw results, no AI unless you ask for it | About $5 per 1k requests, $5 free credit each month | Strongest independent index we can access through an API |
| **Mojeek API** | ✅ Yes (UK, privacy-first) | ✅ | Pay as you go, about £1–2 CPM | Cheap second source |
| **Marginalia** | ✅ Yes (small, old and indie web) | ✅ by design | Free shared "public" key; email for your own | Perfect "human web" source; CC-BY-NC-SA data |
| **Mwmbl** | ✅ Yes (non-profit, community crawl) | ✅ | Open source (AGPL), free | Index is small, but we could contribute to it |
| **SearXNG** (self-hosted) | No, meta-search | Depends on its sources | Free, AGPL | Good model for merging results from several engines |
| **Kagi** | Partly | Has *SlopStop*, community AI-slop reports | Paid, closed | Inspiration: per-domain slop scoring (a domain counts as slop when >80% of its pages are AI) |
| **Own crawler + index** | ✅ | ✅ | Servers + years of work | Long-term stretch goal ("aim high") |

## Recommended plan

1. **Phase 1 (now):** the browser homepage opens `noai.duckduckgo.com`. No AI on day one, zero cost.
2. **Phase 2:** our own **meta-search** page built into the browser. It combines Brave, Mojeek and
   Marginalia results, then:
   - **Obsolete badge:** checks the Wayback Machine CDX API for the URL's first capture
     (`https://web.archive.org/cdx/search/cdx?url=<url>&limit=1&fl=timestamp`). If the first
     capture is after **2022-11-30** (ChatGPT launch), the result is marked *obsolete*. Results are
     cached locally.
   - **Blocklists:** drops or flags domains on the uBlockOrigin "HUGE AI Blocklist" and on our own
     user-tagged slop list (see 04).
   - **Ranking:** pre-2023 pages rank higher.
3. **Phase 3 (stretch):** our own focused crawler of the *pre-AI web*, seeded from Common Crawl
   snapshots taken before 2023 plus Marginalia and Mwmbl data. Stract (Rust, AGPL, now archived)
   is a useful reference for how to build the index.

## Caveats

- Google and Bing don't expose "first indexed" dates. Wayback first-capture is the best free proxy,
  but newly *archived* old pages can show up as false "obsolete" results. Also check
  `<meta article:published_time>`, sitemap `lastmod` and the HTTP `Last-Modified` header.
- An old domain can still publish new AI content, so the date check is per **URL**, not per domain.

Sources: [DuckDuckGo NoAI](https://noai.duckduckgo.com),
[Vice](https://www.vice.com/en/article/duckduckgo-no-ai-mode),
[Brave pricing change](https://www.implicator.ai/brave-drops-free-search-api-tier-puts-all-developers-on-metered-billing.md),
[Mojeek API](https://www.mojeek.com/services/search/web-search-api/),
[Marginalia API](https://www.marginalia.nu/marginalia-search/api/),
[Kagi SlopStop](https://help.kagi.com/kagi/features/slopstop.html),
[HUGE AI Blocklist](https://github.com/laylavish/uBlockOrigin-HUGE-AI-Blocklist),
[AI blocklist for SearXNG](https://zff.dev/HalfofBilly/AI-Blocklist-for-SearXNG).
