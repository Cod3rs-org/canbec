"""Publish docs/ to the GitHub wiki: python3 tools/sync_wiki.py
The repo's docs/ stay the source of truth; the wiki is a generated mirror.
Needs the wiki to exist once (GitHub: Wiki tab -> "Create the first page")."""
import os, re, shutil, subprocess, tempfile

REPO = "Cod3rs-org/canbec"
RAW = f"https://raw.githubusercontent.com/{REPO}/main/"
TREE = f"https://github.com/{REPO}/tree/main/"

# source file -> wiki page name (page names become URLs: /wiki/<name>)
PAGES = {
    "docs/README.md": "Home",
    "docs/research/01-initial-findings.md": "01-Initial-Findings",
    "docs/research/02-search.md": "02-Search-Without-AI",
    "docs/research/03-inspiration.md": "03-Open-Source-Inspiration",
    "docs/research/04-ui-and-tabs.md": "04-UI-Tabs-and-Slop-Tagging",
    "docs/research/05-engine-bakeoff.md": "05-Engine-Bake-off",
    "docs/research/06-extensibility.md": "06-Extensibility-MCP-Profiles",
    "docs/research/07-roadmap-notes.md": "07-Roadmap-Notes",
    "docs/architecture/ui-contract.md": "UI-Contract",
    "docs/benchmarks/2026-10-25-macos.md": "Benchmarks",
    "CLAUDE.md": "Development-Guide",
}
SIDEBAR = """**[canbec](https://github.com/Cod3rs-org/canbec)** — Control AI, Not Be Controlled

* [Home](Home)
* [Development guide](Development-Guide)
* [UI contract](UI-Contract)
* [Benchmarks](Benchmarks)

**Research & decisions**
* [01 Initial findings](01-Initial-Findings)
* [02 Search without AI](02-Search-Without-AI)
* [03 Inspiration](03-Open-Source-Inspiration)
* [04 UI, tabs, slop tagging](04-UI-Tabs-and-Slop-Tagging)
* [05 Engine bake-off](05-Engine-Bake-off)
* [06 Extensibility, MCP, profiles](06-Extensibility-MCP-Profiles)
* [07 Roadmap notes](07-Roadmap-Notes)
"""
FOOTER = "\n\n---\n*Generated from [`{src}`](" + f"https://github.com/{REPO}/blob/main/" + "{src}). Edit it there, not here.*\n"


def rewrite(text: str, src: str) -> str:
    base = os.path.dirname(src)

    def fix(m):
        bang, label, target = m.group(1), m.group(2), m.group(3)
        if re.match(r"^[a-z]+:|^#", target):
            return m.group(0)
        path, _, anchor = target.partition("#")
        resolved = os.path.normpath(os.path.join(base, path))
        if resolved in PAGES:
            return f"{bang}[{label}]({PAGES[resolved]}{'#' + anchor if anchor else ''})"
        if bang or re.search(r"\.(png|svg|jpg|gif)$", resolved):
            return f"{bang}[{label}]({RAW}{resolved})"
        return f"{bang}[{label}]({TREE}{resolved})"

    text = re.sub(r"(!?)\[([^\]]*)\]\(([^)\s]+)\)", fix, text)
    # <img src="..."> in HTML blocks
    text = re.sub(r'src="(?!https?:)([^"]+)"', lambda m: f'src="{RAW}{os.path.normpath(os.path.join(base, m.group(1)))}"', text)
    return text


def main():
    with tempfile.TemporaryDirectory() as tmp:
        subprocess.run(["git", "clone", "-q", f"git@github.com:{REPO}.wiki.git", tmp], check=True)
        for f in os.listdir(tmp):
            if f.endswith(".md"):
                os.remove(os.path.join(tmp, f))
        for src, page in PAGES.items():
            with open(src) as fh:
                body = rewrite(fh.read(), src)
            with open(os.path.join(tmp, page + ".md"), "w") as fh:
                fh.write(body + FOOTER.format(src=src))
        with open(os.path.join(tmp, "_Sidebar.md"), "w") as fh:
            fh.write(SIDEBAR)
        git = lambda *a: subprocess.run(["git", "-C", tmp, *a], check=True)
        git("add", "-A")
        if subprocess.run(["git", "-C", tmp, "diff", "--cached", "--quiet"]).returncode == 0:
            print("wiki already up to date")
            return
        git("commit", "-qm", "Sync wiki from docs/")
        git("push", "-q")
        print(f"wiki updated: https://github.com/{REPO}/wiki")


if __name__ == "__main__":
    main()
