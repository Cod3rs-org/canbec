"""Benchmark canbec against other installed browsers (macOS, Windows).

    swiftc -O tools/firstwindow.swift -o target/firstwindow
    cargo build --release -p canbec
    python3 tools/serve.py target/engine-probe/wwwroot 5080 &   # for the probe page
    python3 tools/bench.py [runs]

Windows: no Swift helper; windows are found with EnumWindows, and memory is the launched
process's whole tree (canbec's msedgewebview2 children included). Results go to latest-windows.json.

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
WIN = sys.platform == "win32"


def processes():
    if WIN:  # pid -> (working set KB, name, parent pid, private KB)
        ps = 'Get-CimInstance Win32_Process | % { "$($_.ProcessId) $($_.ParentProcessId) $([int64]$_.WorkingSetSize/1024) $([int64]$_.PrivatePageCount/1024) $($_.Name)" }'
        out = subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True, text=True).stdout
        procs = {}
        for line in out.splitlines():
            pid, ppid, ws, priv, name = line.split(None, 4)
            procs[int(pid)] = (int(ws), name, int(ppid), int(priv))
        return procs
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
    return subprocess.Popen(["./target/release/canbec.exe" if WIN else "./target/release/canbec", url], env={**os.environ, "CANBEC_CONFIG_DIR": prof}, stderr=subprocess.DEVNULL)


def first_window_win(pid, timeout):
    """Time when `pid` first owns a visible, titled top-level window."""
    import ctypes
    from ctypes import wintypes
    user32, found = ctypes.windll.user32, []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def check(hwnd, _):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(hwnd) and user32.GetWindowTextLengthW(hwnd):
            found.append(hwnd)
        return True

    end = time.time() + timeout
    while time.time() < end:
        user32.EnumWindows(check, 0)
        if found:
            return time.time()
        time.sleep(0.02)


def tree(procs, root):
    pids, grew = {root} & procs.keys(), True
    while grew:
        more = {p for p, v in procs.items() if v[2] in pids} - pids
        grew, pids = bool(more), pids | more
    return pids


def run_once_win(b, url):
    with tempfile.TemporaryDirectory() as prof:
        t0 = time.time()
        proc = b["launch"](url, prof)
        shown = first_window_win(proc.pid, 30)
        if not shown:
            subprocess.run(["taskkill", "/T", "/F", "/PID", str(proc.pid)], capture_output=True)
            return None
        time.sleep(SETTLE)
        procs = processes()
        mine = tree(procs, proc.pid)
        result = {"window_s": round(shown - t0, 3),
                  "ws_mb": round(sum(procs[p][0] for p in mine) / 1024, 1),
                  "private_mb": round(sum(procs[p][3] for p in mine) / 1024, 1),
                  "procs": len(mine)}
        subprocess.run(["taskkill", "/T", "/F", "/PID", str(proc.pid)], capture_output=True)
        time.sleep(3)  # let the profile folder unlock before it's deleted
    return result


def chromium_win(exe):
    return lambda url, prof: subprocess.Popen([exe, f"--user-data-dir={prof}", "--no-first-run", "--no-default-browser-check", "--new-window", url], stderr=subprocess.DEVNULL)


BROWSERS_WIN = {
    "canbec": {"size": "target/release/canbec.exe", "launch": lambda url, prof: canbec(url, prof)},
    "Chrome": {"size": r"C:\Program Files\Google\Chrome\Application",
               "launch": chromium_win(r"C:\Program Files\Google\Chrome\Application\chrome.exe")},
    "Edge": {"size": r"C:\Program Files (x86)\Microsoft\Edge\Application",
             "launch": chromium_win(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe")},
}

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
    if os.path.isfile(path):
        return round(os.path.getsize(path) / 2**20, 1)
    if not WIN:
        out = subprocess.run(["du", "-sk", path], capture_output=True, text=True).stdout
        return round(int(out.split()[0]) / 1024, 1) if out else None
    return round(sum(os.path.getsize(os.path.join(d, f)) for d, _, fs in os.walk(path) for f in fs) / 2**20, 1)


path = "docs/benchmarks/latest-windows.json" if WIN else "docs/benchmarks/latest.json"
results = json.load(open(path)) if ONLY and os.path.exists(path) else {}
for name, b in (BROWSERS_WIN if WIN else BROWSERS).items():
    if ONLY and name not in ONLY:
        continue
    results[name] = {"size_mb": size_mb(b["size"]), "pages": {}}
    for page, url in PAGES.items():
        runs = [r for r in ((run_once_win(b, url) if WIN else run_once(name, b, url)) for _ in range(RUNS)) if r]
        print(name, page, runs, flush=True)
        if runs:
            results[name]["pages"][page] = {k: statistics.median(r[k] for r in runs) for k in runs[0]} | {"runs": len(runs)}

os.makedirs("docs/benchmarks", exist_ok=True)
json.dump(results, open(path, "w"), indent=2)
print(json.dumps(results, indent=2))
