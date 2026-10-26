// canbec privacy script, injected at document start into every page and iframe.
// 1. Fingerprint noise: canvas, WebGL and audio readouts get tiny, invisible changes, different
//    for every site and new on every start, so trackers can't recognise the same browser.
//    Hardware values (cores, memory, screen, GPU) come from a fake but common device.
// 2. Tracking parameters (utm_*, fbclid, …) are removed from links before you follow them,
//    and from the address of the page you landed on.
// 3. Global Privacy Control: tells sites you opt out of selling or sharing your data.
(() => {
  if (window.__canbecPrivacy) return;
  window.__canbecPrivacy = true;

  const FP_SEED = "__CANBEC_FP_SEED__"; // random per run, filled in by the browser
  const TRACKING_PARAMS = "__CANBEC_TRACKING_PARAMS__";

  // Deterministic per site + run: reading the same canvas twice gives the same answer
  // (random-per-call noise is easy for trackers to detect).
  let seed = 2166136261;
  for (const c of FP_SEED + location.hostname) seed = Math.imul(seed ^ c.charCodeAt(0), 16777619);
  const bit = (i) => {
    let x = Math.imul(seed ^ i, 2654435761);
    x ^= x >>> 15;
    return Math.imul(x, 2246822507) >>> 31;
  };

  // Wrapped functions still look native to Function.prototype.toString.
  const originals = new WeakMap();
  const toString = Function.prototype.toString;
  Function.prototype.toString = new Proxy(toString, {
    apply: (target, self, args) => Reflect.apply(target, originals.get(self) || self, args),
  });
  function wrap(proto, name, make) {
    const original = proto && proto[name];
    if (typeof original !== "function") return;
    const wrapped = make(original);
    originals.set(wrapped, original);
    Object.defineProperty(proto, name, { value: wrapped, writable: true, configurable: true });
  }

  // Canvas: flip the lowest bit of a few pixels in what the page reads back.
  const farble = (data) => {
    for (let i = 0; i < data.length; i += 4 * 61) data[i] ^= bit(i);
  };
  const getImageData = CanvasRenderingContext2D.prototype.getImageData;
  function noisyCopy(canvas) {
    const { width: w, height: h } = canvas;
    if (!w || !h || w * h > 4e6) return canvas; // ponytail: huge canvases (editors) pass through untouched
    const copy = document.createElement("canvas");
    copy.width = w;
    copy.height = h;
    const ctx = copy.getContext("2d");
    ctx.drawImage(canvas, 0, 0);
    const img = getImageData.call(ctx, 0, 0, w, h);
    farble(img.data);
    ctx.putImageData(img, 0, 0);
    return copy;
  }
  wrap(HTMLCanvasElement.prototype, "toDataURL", (f) => function (...a) { return f.apply(noisyCopy(this), a); });
  wrap(HTMLCanvasElement.prototype, "toBlob", (f) => function (...a) { return f.apply(noisyCopy(this), a); });
  wrap(CanvasRenderingContext2D.prototype, "getImageData", (f) => function (...a) {
    const img = f.apply(this, a);
    farble(img.data);
    return img;
  });

  // Fake device ("persona"): common values near this computer's, never its own, the same on a
  // site until restart. The lists come from domain::persona; `pick` is persona::pick (keep in sync).
  const P = "__CANBEC_PERSONA__";
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const pick = (options, real, salt) => {
    const allowed = options.filter((o) => !same(o, real));
    let x = Math.imul(seed ^ salt, 2654435761);
    x ^= x >>> 15;
    x = Math.imul(x, 2246822507) >>> 0;
    return allowed.length ? allowed[x % allowed.length] : real;
  };
  // Replaces a getter; the new one prints like the native one.
  function fake(proto, name, value) {
    const desc = proto && Object.getOwnPropertyDescriptor(proto, name);
    if (!desc || !desc.get) return;
    const get = Object.getOwnPropertyDescriptor({ get [name]() { return value; } }, name).get;
    originals.set(get, desc.get);
    Object.defineProperty(proto, name, { ...desc, get });
  }
  fake(Navigator.prototype, "hardwareConcurrency", pick(P.cores, navigator.hardwareConcurrency, 1));
  if ("deviceMemory" in navigator) fake(Navigator.prototype, "deviceMemory", pick(P.memory, navigator.deviceMemory, 2));
  // Screen: picked on first read, when the window size is known (it's 0 at document start).
  // Never smaller than the window; the taskbar/menu bar keeps its real height.
  let size;
  const fakeScreen = () => {
    if (size) return size;
    const { width, height, availWidth, availHeight } = real;
    const fits = P.screens.filter(([w, h]) => w >= outerWidth && h >= outerHeight);
    const [w, h] = pick(fits, [width, height], 3);
    return (size = { width: w, height: h, availWidth: w - (width - availWidth), availHeight: h - (height - availHeight) });
  };
  const real = {};
  for (const name of ["width", "height", "availWidth", "availHeight"]) {
    const desc = Object.getOwnPropertyDescriptor(Screen.prototype, name);
    if (!desc || !desc.get) continue;
    Object.defineProperty(real, name, { get: () => desc.get.call(screen) });
    const get = Object.getOwnPropertyDescriptor({ get [name]() { return fakeScreen()[name]; } }, name).get;
    originals.set(get, desc.get);
    Object.defineProperty(Screen.prototype, name, { ...desc, get });
  }
  if (window.NavigatorUAData && P.platform_versions.length) {
    wrap(NavigatorUAData.prototype, "getHighEntropyValues", (f) => function (...a) {
      return f.apply(this, a).then((v) => {
        if (v.platformVersion) v.platformVersion = pick(P.platform_versions, v.platformVersion, 5);
        return v;
      });
    });
  }
  // WebGL: a common GPU for this OS instead of the exact model.
  let gpu;
  for (const gl of [window.WebGLRenderingContext, window.WebGL2RenderingContext]) {
    wrap(gl && gl.prototype, "getParameter", (f) => function (p) {
      if (p !== 0x9245 && p !== 0x9246) return f.call(this, p); // UNMASKED_VENDOR/RENDERER_WEBGL
      gpu = gpu || pick(P.gpus, [f.call(this, 0x9245), f.call(this, 0x9246)], 4);
      return p === 0x9245 ? gpu[0] : gpu[1];
    });
  }

  // Audio: inaudible noise (1e-7) in rendered samples that pages read back.
  const touched = new WeakSet();
  wrap(window.AudioBuffer && AudioBuffer.prototype, "getChannelData", (f) => function (...a) {
    const data = f.apply(this, a);
    if (!touched.has(data)) {
      touched.add(data);
      for (let i = 0; i < data.length; i += 97) data[i] += bit(i) ? 1e-7 : -1e-7;
    }
    return data;
  });

  try {
    Object.defineProperty(Navigator.prototype, "globalPrivacyControl", { get: () => true, configurable: true });
  } catch {}

  // Tracking parameters.
  const isTracking = (name) => name.startsWith("utm_") || TRACKING_PARAMS.includes(name);
  function clean(href) {
    try {
      const url = new URL(href, location.href);
      const names = [...url.searchParams.keys()].filter(isTracking);
      if (!names.length) return null;
      names.forEach((n) => url.searchParams.delete(n));
      return url.href;
    } catch {
      return null;
    }
  }
  // Rewrite the link right before it's followed (mouse, keyboard, middle click).
  const onLink = (e) => {
    const a = e.target.closest && e.target.closest("a[href]");
    const href = a && clean(a.href);
    if (href) a.href = href;
  };
  addEventListener("mousedown", onLink, true);
  addEventListener("keydown", (e) => e.key === "Enter" && onLink(e), true);
  if (window.top === window) {
    const landed = clean(location.href);
    if (landed) history.replaceState(history.state, "", landed);
  }
})();
