// canbec content script — injected at document start into every website.
// Three jobs: cover ads (blur + grey hatching), show the slop warning border, hide infinite feeds.
// Edit freely: selectors are plain lists at the top.
(() => {
  if (window.__canbec) return;

  // ponytail: a short cosmetic list; adblock-rust (EasyList cosmetic rules) replaces it later.
  const AD_SELECTORS = [
    "ins.adsbygoogle", "[id^='google_ads']", "[id^='div-gpt-ad']", "iframe[src*='doubleclick.net']",
    "iframe[src*='googlesyndication']", "[data-ad]", "[data-ad-slot]", "[aria-label='Advertisement']",
    ".ad-banner", ".advertisement", ".sponsored", "[data-testid='placementTracking']",
  ];

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
    .canbec-ad { position: relative !important; filter: blur(14px) grayscale(1) !important; pointer-events: none !important; }
    .canbec-ad-cover { position: absolute; z-index: 2147483646; pointer-events: none; border-radius: 6px;
      background: repeating-linear-gradient(45deg, rgba(220,220,220,.92) 0 6px, rgba(180,180,180,.92) 6px 12px); }
    html.canbec-feeds-locked :is(${feedSelectors.join(",") || "#canbec-none"}) { display: none !important; }
    #canbec-slop { position: fixed; inset: 0; z-index: 2147483647; pointer-events: none;
      border: 6px solid transparent;
      border-image: repeating-linear-gradient(45deg, #ff7a45 0 12px, #1f2328 12px 24px) 6; }
    #canbec-slop-banner { position: fixed; top: 10px; left: 50%; transform: translateX(-50%); z-index: 2147483647;
      background: #1f2328; color: #f6f1e7; font: 600 13px system-ui, sans-serif; padding: 6px 12px; border-radius: 999px;
      box-shadow: 0 2px 10px rgba(0,0,0,.25); cursor: pointer; }`;

  const send = (msg) => window.ipc && window.ipc.postMessage(JSON.stringify(msg));
  let adCount = -1;

  function coverAds() {
    const ads = document.querySelectorAll(AD_SELECTORS.join(","));
    ads.forEach((el) => {
      if (el.classList.contains("canbec-ad")) return;
      el.classList.add("canbec-ad");
      // Keep the ad's space (no layout jump) and draw hatching on top of it.
      const r = el.getBoundingClientRect();
      if (r.width && r.height) {
        const cover = document.createElement("div");
        cover.className = "canbec-ad-cover";
        Object.assign(cover.style, { left: r.left + scrollX + "px", top: r.top + scrollY + "px", width: r.width + "px", height: r.height + "px" });
        document.body.appendChild(cover);
      }
    });
    if (ads.length !== adCount) send({ type: "adsHidden", count: (adCount = ads.length) });
  }

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
  };

  document.documentElement.classList.add("canbec-feeds-locked"); // locked until told otherwise
  const style = document.createElement("style");
  style.textContent = css;
  document.documentElement.appendChild(style);

  addEventListener("DOMContentLoaded", () => {
    coverAds();
    setSlop(flags.slop);
    // ponytail: periodic rescan for lazily inserted ads; a MutationObserver if it ever costs too much.
    setInterval(coverAds, 1500);
  });
})();
