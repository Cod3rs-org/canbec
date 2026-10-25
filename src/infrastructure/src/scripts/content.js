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
  };

  const host = location.hostname.replace(/^www\./, "");
  const feedSelectors = Object.entries(FEEDS)
    .filter(([domain]) => host === domain || host.endsWith("." + domain))
    .flatMap(([, sel]) => sel);

  const css = `
    @keyframes canbec-ad-seen { from { outline-color: transparent } to { outline-color: transparent } }
    html.canbec-feeds-locked :is(${feedSelectors.join(",") || "#canbec-none"}) { display: none !important; }
    #canbec-slop { position: fixed; inset: 0; z-index: 2147483647; pointer-events: none;
      box-shadow: inset 0 0 0 6px #ff7a45, inset 0 0 0 8px #1f2328, inset 0 0 40px rgba(255,122,69,.35); }
    #canbec-slop-banner { position: fixed; top: 10px; left: 50%; transform: translateX(-50%); z-index: 2147483647;
      background: #1f2328; color: #f6f1e7; font: 600 13px system-ui, sans-serif; padding: 6px 12px; border-radius: 999px;
      box-shadow: 0 2px 10px rgba(0,0,0,.25); cursor: pointer; }`;

  // Captured before any page script runs, so a page can't wrap it to steal the shortcut token.
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
    return { t: "newTab", w: "closeTab", l: "focusAddress", r: "reload", "[": "back", "]": "forward", ",": "settings", d: "pinPage" }[k];
  }
  addEventListener("keydown", (e) => {
    const key = e.isTrusted && shortcut(e); // pages can dispatch fake key events; only real ones count
    if (!key) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    send({ type: "key", key, token: KEY_TOKEN });
  }, true);
  // One stylesheet for all ad rules. Selectors go in chunks; a chunk with a selector this
  // engine doesn't understand is retried one by one, so one bad rule can't disable the rest.
  const adSheet = document.createElement("style");
  function addRules(selectors, body) {
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
  };

  document.documentElement.classList.add("canbec-feeds-locked"); // locked until told otherwise
  const style = document.createElement("style");
  style.textContent = css;
  document.documentElement.append(style, adSheet);
  addRules(GENERIC_ADS, AD_STYLE);

  addEventListener("DOMContentLoaded", () => setSlop(flags.slop));
})();
