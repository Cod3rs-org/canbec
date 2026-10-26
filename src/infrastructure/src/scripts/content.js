// canbec content script — injected at document start into every website.
// Jobs: cover ads (blur + grey hatching), show the slop warning border, hide infinite feeds,
// and forward browser shortcuts (Cmd/Ctrl+T, W, L, R, …) while the page has focus.
// Edit freely: selectors are plain lists at the top.
(() => {
  if (window.__canbec) return;

  // Ad rules: generic EasyList selectors, filled in by the browser (seed + weekly download).
  // Site-specific rules arrive per page through __canbec.ads(). Edit lists in the config folder.
  const GENERIC_ADS = "__CANBEC_GENERIC_ADS__";
  // Ads keep their space (no layout jump) but become blurred grey stripes you can't click.
  // The 1 ms animation is only a cheap "an ad appeared" signal for the counter.
  const AD_STYLE = "filter:blur(10px) grayscale(1)!important;pointer-events:none!important;" +
    "-webkit-mask-image:repeating-linear-gradient(45deg,#000 0 6px,rgba(0,0,0,.3) 6px 12px)!important;" +
    "mask-image:repeating-linear-gradient(45deg,#000 0 6px,rgba(0,0,0,.3) 6px 12px)!important;" +
    "animation:canbec-ad-seen 1ms!important;";
  const AD_UNDO = "filter:none!important;pointer-events:auto!important;-webkit-mask-image:none!important;" +
    "mask-image:none!important;animation:none!important;";

  // Per-site feed containers. Hidden while the feed lock is on (unlock only via settings, max 6 h).
  const FEEDS = {
    "youtube.com": ["ytd-browse[page-subtype='home'] #contents", "ytd-reel-shelf-renderer", "ytd-rich-shelf-renderer[is-shorts]"],
    "facebook.com": ["[role='feed']"],
    "x.com": ["[aria-label^='Timeline']"],
    "twitter.com": ["[aria-label^='Timeline']"],
    "reddit.com": ["shreddit-feed"],
    "linkedin.com": [".scaffold-finite-scroll"],
    "instagram.com": ["main article"],
    "tiktok.com": ["[data-e2e='recommend-list-item-container']"],
    "bsky.app": ["[data-testid$='-feed-flatlist']"],
    "mastodon.social": ["[role='feed']"],
    "tumblr.com": ["[data-timeline-id]"],
    "pinterest.com": ["[data-test-id='masonry-container']"],
    "9gag.com": ["#list-view-2"], // from public filter lists; 9gag blocks headless checks
  };
  // The user's own rules from <config>/feeds.txt.
  for (const [domain, sel] of Object.entries("__CANBEC_USER_FEEDS__")) FEEDS[domain] = (FEEDS[domain] || []).concat(sel);

  const host = location.hostname.replace(/^www\./, "");
  const feedSelectors = Object.entries(FEEDS)
    .filter(([domain]) => host === domain || host.endsWith("." + domain))
    .flatMap(([, sel]) => sel);

  const css = `
    @keyframes canbec-ad-seen { from { outline-color: transparent } to { outline-color: transparent } }
    html.canbec-feeds-locked :is(${feedSelectors.join(",") || "#canbec-none"}) { display: none !important; }
    img.canbec-ai { filter: blur(24px) saturate(.6) !important; clip-path: inset(0) !important; cursor: pointer !important; }
    #canbec-slop { position: fixed; inset: 0; z-index: 2147483647; pointer-events: none;
      box-shadow: inset 0 0 0 6px #ff7a45, inset 0 0 0 8px #1f2328, inset 0 0 40px rgba(255,122,69,.35); }
    #canbec-slop-banner { position: fixed; top: 10px; left: 50%; transform: translateX(-50%); z-index: 2147483647;
      background: #1f2328; color: #f6f1e7; font: 600 13px system-ui, sans-serif; padding: 6px 12px; border-radius: 999px;
      box-shadow: 0 2px 10px rgba(0,0,0,.25); cursor: pointer; }`;

  // Captured before any page script runs, so a page can't wrap it to steal the shortcut token
  // (where this runs in the page world: --mcp and Windows).
  const post = window.ipc && window.ipc.postMessage.bind(window.ipc);
  const send = (msg) => post && post(JSON.stringify(msg));

  // Shortcuts. The token is a per-run secret filled in by the browser; pages can post to
  // window.ipc too, but without the token their fake shortcuts are ignored.
  const KEY_TOKEN = "__CANBEC_KEY_TOKEN__";
  const mac = /Mac/.test(navigator.platform);
  function shortcut(e) {
    const mod = mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
    const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    if (k === "F5") return "reload";
    if (e.altKey && !mod) return { ArrowLeft: "back", ArrowRight: "forward" }[k];
    if (!mod || e.altKey) return;
    if (e.shiftKey && k === "t") return "reopenTab";
    return { t: "newTab", w: "closeTab", l: "focusAddress", r: "reload", "[": "back", "]": "forward", ",": "settings", d: "pinPage",
      f: "find", "=": "zoomIn", "+": "zoomIn", "-": "zoomOut", "0": "zoomReset" }[k];
  }
  addEventListener("keydown", (e) => {
    const key = e.isTrusted && shortcut(e); // pages can dispatch fake key events; only real ones count
    if (!key) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    send({ type: "key", key, token: KEY_TOKEN });
  }, true);
  // Cmd/Ctrl+click and middle click on a link: new background tab (same checks as typing the URL).
  function openInTab(e) {
    const mod = mac ? e.metaKey : e.ctrlKey;
    if (!e.isTrusted || !(e.button === 1 || (e.button === 0 && mod))) return;
    const a = e.target.closest && e.target.closest("a[href]");
    if (!a || !/^https?:/.test(a.href)) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    send({ type: "openTab", url: a.href, token: KEY_TOKEN });
  }
  addEventListener("click", openInTab, true);
  addEventListener("auxclick", openInTab, true);
  // One stylesheet for all ad rules. Selectors go in chunks; a chunk with a selector this
  // engine doesn't understand is retried one by one, so one bad rule can't disable the rest.
  const adSheet = document.createElement("style");
  const adHidden = [], adAllowed = []; // for readable(): what to leave out for agents
  function addRules(selectors, body) {
    (body === AD_STYLE ? adHidden : adAllowed).push(...selectors);
    const sheet = adSheet.sheet;
    if (!sheet) return;
    for (let i = 0; i < selectors.length; i += 300) {
      const chunk = selectors.slice(i, i + 300);
      try {
        sheet.insertRule(chunk.join(",") + "{" + body + "}", sheet.cssRules.length);
      } catch {
        for (const sel of chunk) try { sheet.insertRule(sel + "{" + body + "}", sheet.cssRules.length); } catch {}
      }
    }
  }

  let adCount = 0, adTimer = 0;
  addEventListener("animationstart", (e) => {
    if (e.animationName !== "canbec-ad-seen") return;
    adCount++;
    clearTimeout(adTimer);
    adTimer = setTimeout(() => send({ type: "adsHidden", count: adCount }), 300);
  }, true);

  function setSlop(on) {
    document.getElementById("canbec-slop")?.remove();
    document.getElementById("canbec-slop-banner")?.remove();
    if (!on || !document.body) return;
    const frame = Object.assign(document.createElement("div"), { id: "canbec-slop" });
    const banner = Object.assign(document.createElement("div"), {
      id: "canbec-slop-banner",
      textContent: "⚠ You marked this site as AI slop",
      title: "Click to dismiss",
    });
    banner.onclick = () => banner.remove();
    document.body.append(frame, banner);
  }

  let flags = { slop: false, feedsLocked: true };
  window.__canbec = {
    // Called by the browser after every page load.
    flags(slop, feedsLocked) {
      flags = { slop, feedsLocked };
      document.documentElement.classList.toggle("canbec-feeds-locked", feedsLocked);
      setSlop(slop);
    },
    // Site rules for this page: more ads to cover, and generic rules this site is exempt from.
    ads(hide, allow) {
      addRules(hide, AD_STYLE);
      addRules(allow, AD_UNDO);
    },
    // Search result dates from the browser: { url: { year, obsolete, slop } }.
    dates(map) {
      for (const a of document.querySelectorAll(RESULT_LINKS)) if (map[a.href]) badge(a, map[a.href]);
    },
    // For AI agents (canbec --mcp): the page as plain text and links, without ads, locked
    // feeds and AI-labelled images. ponytail: --mcp runs this in the page world, where a page
    // can overwrite the hook; the browser itself uses an isolated world (#20).
    readable() {
      const root = document.body ? document.body.cloneNode(true) : document.createElement("body");
      const drop = (sel) => { try { root.querySelectorAll(sel).forEach((e) => e.remove()); } catch {} };
      const allowed = adAllowed.join(",");
      for (let i = 0; i < adHidden.length; i += 300) {
        const sel = adHidden.slice(i, i + 300).join(",");
        try { root.querySelectorAll(sel).forEach((e) => { if (!allowed || !e.matches(allowed)) e.remove(); }); } catch { adHidden.slice(i, i + 300).forEach(drop); }
      }
      if (flags.feedsLocked) feedSelectors.forEach(drop);
      drop("script,style,noscript,template,svg,iframe,#canbec-slop,#canbec-slop-banner");
      root.querySelectorAll("img.canbec-ai").forEach((e) => e.replaceWith("[AI image hidden]"));
      root.querySelectorAll("p,div,li,h1,h2,h3,h4,h5,h6,br,tr,section,article,header,footer,blockquote,pre").forEach((e) => e.append("\n"));
      const text = root.textContent.replace(/[ \t\r\f\v]+/g, " ").replace(/\n\s*\n+/g, "\n\n").trim().slice(0, 60000);
      const links = [...document.links].filter((a) => /^https?:/.test(a.href)).slice(0, 300)
        .map((a) => ({ text: a.textContent.trim().replace(/\s+/g, " ").slice(0, 120), href: a.href }));
      return { title: document.title, url: location.href, text, links };
    },
  };

  document.documentElement.classList.add("canbec-feeds-locked"); // locked until told otherwise
  const style = document.createElement("style");
  style.textContent = css;
  document.documentElement.append(style, adSheet);
  addRules(GENERIC_ADS, AD_STYLE);

  addEventListener("DOMContentLoaded", () => setSlop(flags.slop));

  // AI images: pictures whose metadata says "made by a generative model" (IPTC digital source
  // type, also used inside C2PA manifests) get a spoiler blur; one click shows them.
  // Only images the page lets scripts read (same origin or CORS); first 256 KB, 40 per page.
  // ponytail: metadata only, most AI images carry none; content-based hints are #25.
  const AI_MARK = "trainedAlgorithmicMedia";
  const checked = new WeakSet();
  let aiChecks = 0, aiCount = 0;
  async function checkImage(img) {
    const src = img.currentSrc || img.src;
    if (checked.has(img) || !/^https?:/.test(src) || img.naturalWidth * img.naturalHeight < 150 * 150 || aiChecks >= 40) return;
    checked.add(img);
    aiChecks++;
    try {
      const reader = (await fetch(src, { mode: "cors", credentials: "omit" })).body.getReader();
      let head = "";
      while (head.length < 262144) {
        const { done, value } = await reader.read();
        if (done) break;
        head += new TextDecoder("latin1").decode(value);
      }
      reader.cancel().catch(() => {});
      if (!head.includes(AI_MARK)) return;
      img.classList.add("canbec-ai");
      img.title = "AI-generated, says the image itself. Click to show";
      send({ type: "aiImages", count: ++aiCount });
    } catch {} // not readable cross-origin: skipped
  }
  // Element load events skip window (spec), so listen on the document.
  document.addEventListener("load", (e) => { if (e.target.tagName === "IMG") checkImage(e.target); }, true);
  addEventListener("DOMContentLoaded", () => { for (const img of document.images) if (img.complete) checkImage(img); });
  addEventListener("click", (e) => {
    const img = e.target.closest && e.target.closest("img.canbec-ai");
    if (!img) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    img.classList.remove("canbec-ai");
    img.removeAttribute("title");
  }, true);

  // Search results (DuckDuckGo): the browser looks up when each result first appeared on the web
  // (Wayback Machine) and badges pages from the AI era (after 2022-11-30) as "obsolete".
  const RESULT_LINKS = "a[data-testid='result-title-a'], a.result__a";
  const asked = new Set();
  let askTimer = 0;
  function askDates() {
    const urls = [...document.querySelectorAll(RESULT_LINKS)].map((a) => a.href).filter((u) => /^https?:/.test(u) && !asked.has(u)).slice(0, 30);
    if (!urls.length) return;
    urls.forEach((u) => asked.add(u));
    send({ type: "dateResults", urls, token: KEY_TOKEN });
  }
  if (/(^|\.)duckduckgo\.com$/.test(location.hostname)) {
    new MutationObserver(() => { clearTimeout(askTimer); askTimer = setTimeout(askDates, 400); })
      .observe(document.documentElement, { childList: true, subtree: true });
  }
  function badge(a, info) {
    if (a.parentNode.querySelector(":scope > .canbec-age")) return;
    const b = document.createElement("span");
    b.className = "canbec-age";
    const tags = [];
    if (info.slop) tags.push(["⚠ slop", "You marked this site as AI slop", "#d93025", "#fff"]);
    if (info.year && info.obsolete) tags.push(["AI era · " + info.year, "First seen on the web in " + info.year + ", after ChatGPT came out", "#ff7a45", "#fff"]);
    else if (info.year) tags.push(["since " + info.year, "On the web since " + info.year + ", before the AI boom", "#e8e1d3", "#1f2328"]);
    else if (info.year === null) tags.push(["not archived", "The Wayback Machine has never seen this page", "#e8e1d3", "#6b6f76"]);
    for (const [text, title, bg, fg] of tags) {
      const t = Object.assign(document.createElement("span"), { textContent: text, title });
      t.style.cssText = "display:inline-block;margin-left:6px;padding:1px 7px;border-radius:9px;font:600 11px/16px system-ui,sans-serif;vertical-align:middle;background:" + bg + ";color:" + fg;
      b.append(t);
    }
    a.after(b);
  }

  // A page asks for your email: the sidebar suggests a throwaway one (once per page).
  let emailTip = false;
  addEventListener("focusin", (e) => {
    const el = e.target;
    if (emailTip || !e.isTrusted || el.tagName !== "INPUT") return;
    if (el.type === "email" || /email/i.test(el.autocomplete + " " + el.name + " " + el.id)) {
      emailTip = true;
      send({ type: "emailField" });
    }
  }, true);
})();
