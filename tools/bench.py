"""Benchmark canbec against other installed browsers (macOS).

    swiftc -O tools/firstwindow.swift -o target/firstwindow
    cargo build --release -p canbec
    python3 tools/serve.py target/engine-probe/wwwroot 5080 &   # for the probe page
    python3 tools/bench.py [runs]

Per browser x page: time to first window, RSS of all processes it spawned (after settling),
process count. Each run uses a throwaway profile where the browser allows it. Never touches
already-running browser processes: only processes that appear during the run are measured/stopped.
"""
import shutil, json, os, statistics, subprocess, sys, tempfile, time

RUNS = int(sys.argv[1]) if len(sys.argv) > 1 else 3
ONLY = set(sys.argv[2:])  # optional browser names; results merge into latest.json
SETTLE = 15  # seconds after the window appears before measuring memory
PAGES = {"probe": "http://127.0.0.1:5080/", "wikipedia": "https://en.wikipedia.org/wiki/Web_browser"}
EDGE = "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"


def processes():
    out = subprocess.run(["ps", "-axo", "pid=,rss=,comm="], capture_output=True, text=True).stdout
    procs = {}
    for line in out.splitlines():
        pid, rss, comm = line.strip().split(None, 2)
        procs[int(pid)] = (int(rss), comm)
    return procs


def window_pids(owner):
    """Pids that already own a window called `owner` (so a running browser isn't mistaken for ours)."""
    out = subprocess.run(["./target/firstwindow", owner, "", "0.05"], capture_output=True, text=True).stdout
    return [] if out.startswith("timeout") else [out.split()[1]]


def canbec(url, prof):
    # Real users have the weekly EasyList; measure with it (fetch once into target/easylist.txt).
    if not os.path.exists("target/easylist.txt"):
        subprocess.run(["curl", "-sL", "-o", "target/easylist.txt", "https://easylist.to/easylist/easylist.txt"])
    os.makedirs(f"{prof}/blocklists", exist_ok=True)
    shutil.copy("target/easylist.txt", f"{prof}/blocklists/ads.txt")
    return subprocess.Popen(["./target/release/canbec", url], env={**os.environ, "CANBEC_CONFIG_DIR": prof}, stderr=subprocess.DEVNULL)


BROWSERS = {
    "canbec": {
        "owner": "canbec", "match": ("canbec", "com.apple.WebKit"),
        "size": "target/release/canbec",
        "launch": lambda url, prof: canbec(url, prof),
    },
    "Safari": {
        "owner": "Safari", "match": ("Safari", "com.apple.WebKit"),
        # /Applications/Safari.app is a stub; the real bundle lives in the system Cryptex.
        "size": "/System/Cryptexes/App/System/Applications/Safari.app",
        # ponytail: Safari has no throwaway-profile flag; uses the user's profile.
        "launch": lambda url, prof: subprocess.Popen(["open", "-n", "-a", "Safari", url]),
        "quit": lambda: subprocess.run(["osascript", "-e", 'quit app "Safari"']),
    },
    "Edge": {
        "owner": "Microsoft Edge", "match": ("Microsoft Edge",),
        "size": "/Applications/Microsoft Edge.app",
        "launch": lambda url, prof: subprocess.Popen([EDGE, f"--user-data-dir={prof}", "--no-first-run", "--no-default-browser-check", "--new-window", url], stderr=subprocess.DEVNULL),
    },
}


def run_once(name, b, url):
    if name == "Safari" and any(c.endswith("Safari.app/Contents/MacOS/Safari") for _, c in processes().values()):
        return None  # don't hijack a running Safari
    before = set(processes())
    excluded = ",".join(window_pids(b["owner"]))
    with tempfile.TemporaryDirectory() as prof:
        t0 = time.time()
        b["launch"](url, prof)
        out = subprocess.run(["./target/firstwindow", b["owner"], excluded, "30"], capture_output=True, text=True).stdout
        if out.startswith("timeout"):
            return None
        t_window = float(out.split()[0]) - t0
        time.sleep(SETTLE)
        new = {pid: v for pid, v in processes().items() if pid not in before and any(m in v[1] for m in b["match"])}
        rss_mb = sum(r for r, _ in new.values()) / 1024
        if "quit" in b:
            b["quit"]()
        for pid in new:
            try:
                os.kill(pid, 15)
            except ProcessLookupError:
                pass
        time.sleep(3)
    return {"window_s": round(t_window, 3), "rss_mb": round(rss_mb, 1), "procs": len(new)}


def size_mb(path):
    out = subprocess.run(["du", "-sk", path], capture_output=True, text=True).stdout
    return round(int(out.split()[0]) / 1024, 1) if out else None


path = "docs/benchmarks/latest.json"
results = json.load(open(path)) if ONLY and os.path.exists(path) else {}
for name, b in BROWSERS.items():
    if ONLY and name not in ONLY:
        continue
    results[name] = {"size_mb": size_mb(b["size"]), "pages": {}}
    for page, url in PAGES.items():
        runs = [r for r in (run_once(name, b, url) for _ in range(RUNS)) if r]
        print(name, page, runs, flush=True)
        if runs:
            results[name]["pages"][page] = {k: statistics.median(r[k] for r in runs) for k in runs[0]} | {"runs": len(runs)}

os.makedirs("docs/benchmarks", exist_ok=True)
json.dump(results, open(path, "w"), indent=2)
print(json.dumps(results, indent=2))
