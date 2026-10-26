// canbec extension runtime: the small MV3 subset, content scripts + chrome.storage.
// infrastructure::extensions appends one __canbecExt(...) call per content_scripts entry.
// ponytail: runs in the page's world like content.js (isolated world is #20), so a page can see
// an extension's storage. No background pages or runtime messaging yet.
(() => {
  if (window.__canbecExt) return;
  const STORAGE = "__CANBEC_EXT_STORAGE__"; // { id: items } as of browser start
  // The top page asks the core for the current storage (other pages may have changed it since).
  let fresh;
  const current = new Promise((resolve) => (fresh = resolve));
  window.__canbecExtStorage = (all) => fresh(all);
  if (window.top === window) {
    try {
      window.ipc.postMessage(JSON.stringify({ type: "extStorageGet" }));
    } catch {}
  }
  setTimeout(() => fresh(STORAGE), window.top === window ? 1000 : 0);

  const copy = (v) => (v === undefined ? v : JSON.parse(JSON.stringify(v)));
  // Callback style and promise style, like Chrome.
  const done = (cb, promise) => (typeof cb === "function" ? void promise.then(cb) : promise);
  const apis = {};
  function api(id) {
    if (apis[id]) return apis[id];
    let data;
    const load = async () => (data = data || copy(((await current) || STORAGE)[id] || {}));
    const listeners = new Set();
    const save = (changes) => {
      window.ipc.postMessage(JSON.stringify({ type: "extStorageSet", id, items: data }));
      for (const f of listeners) f(changes, "local");
    };
    const select = (keys) => {
      if (keys == null) return copy(data);
      if (typeof keys === "string") keys = [keys];
      const defaults = Array.isArray(keys) ? {} : keys;
      const out = {};
      for (const k of Array.isArray(keys) ? keys : Object.keys(keys)) {
        const v = k in data ? data[k] : defaults[k];
        if (v !== undefined) out[k] = copy(v);
      }
      return out;
    };
    const change = (update) => (cb) => done(cb, load().then(() => {
      const before = copy(data);
      update();
      const changes = {};
      for (const k of new Set([...Object.keys(before), ...Object.keys(data)])) {
        if (JSON.stringify(before[k]) !== JSON.stringify(data[k])) changes[k] = { oldValue: before[k], newValue: copy(data[k]) };
      }
      save(changes);
    }));
    const area = {
      get: (keys, cb) => (typeof keys === "function" ? done(keys, load().then(() => select(null))) : done(cb, load().then(() => select(keys)))),
      set: (items, cb) => change(() => Object.assign(data, copy(items)))(cb),
      remove: (keys, cb) => change(() => [].concat(keys).forEach((k) => delete data[k]))(cb),
      clear: (cb) => change(() => Object.keys(data).forEach((k) => delete data[k]))(cb),
    };
    const onChanged = { addListener: (f) => listeners.add(f), removeListener: (f) => listeners.delete(f), hasListener: (f) => listeners.has(f) };
    return (apis[id] = { storage: { local: area, sync: area, onChanged }, runtime: { id, getManifest: () => ({}), getURL: (p) => p } });
  }

  const href = location.href.split("#")[0];
  const any = (regexes) => regexes.some((r) => new RegExp(r).test(href));
  const when = (runAt, f) => {
    if (runAt === "document_start") return f();
    if (document.readyState === "loading") addEventListener("DOMContentLoaded", f, { once: true });
    else f();
  };
  window.__canbecExt = (id, s, run) => {
    if ((!s.all_frames && window.top !== window) || !any(s.matches) || any(s.exclude)) return;
    if (s.css) {
      const style = document.createElement("style");
      style.textContent = s.css;
      const add = () => (document.head || document.documentElement).append(style);
      document.documentElement ? add() : when("document_end", add);
    }
    when(s.run_at, () => {
      try {
        run(api(id));
      } catch (e) {
        console.error(`[extension ${id}]`, e);
      }
    });
  };
})();
