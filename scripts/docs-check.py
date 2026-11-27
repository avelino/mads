#!/usr/bin/env python3
"""Checks the docs tree: SUMMARY coverage, links, H1, fence languages, em dashes."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
SKIP = ("superpowers", "assets")
LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)\s]+)\)")
errors = []


def err(path, msg):
    errors.append(f"{path.relative_to(ROOT)}: {msg}")


def pages():
    for p in sorted(DOCS.rglob("*.md")):
        if p.relative_to(DOCS).parts[0] not in SKIP:
            yield p


def summary_targets():
    text = (DOCS / "SUMMARY.md").read_text()
    return {t for t in LINK.findall(text)}


def check_links(path, text):
    in_fence = False
    for line in text.splitlines():
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        for target in LINK.findall(line):
            if re.match(r"^(https?:|mailto:|#)", target):
                continue
            file = target.split("#")[0]
            if file and not (path.parent / file).resolve().exists():
                err(path, f"broken link {target}")


def check_page(path):
    text = path.read_text()
    if path.name != "SUMMARY.md" and not text.lstrip().startswith("# "):
        err(path, "missing H1")
    if "—" in text:
        err(path, "em dash")
    fences = [l.strip() for l in text.splitlines() if l.strip().startswith("```")]
    for i, f in enumerate(fences):
        if i % 2 == 0 and f == "```":
            err(path, "code fence without a language")
    check_links(path, text)


def main():
    targets = summary_targets()
    listed = {(DOCS / t).resolve() for t in targets if not t.startswith("http")}
    for t in listed:
        if not t.exists():
            errors.append(f"docs/SUMMARY.md: target missing {t.relative_to(ROOT)}")
    for p in pages():
        if p.name != "SUMMARY.md" and p.resolve() not in listed:
            err(p, "not in SUMMARY.md")
        check_page(p)
    for extra in ("README.md", "CONTRIBUTING.md"):
        f = ROOT / extra
        if f.exists():
            check_links(f, f.read_text())
    readme = (DOCS / "README.md").read_text()
    in_readme = {(DOCS / t.split("#")[0]).resolve() for t in LINK.findall(readme) if not t.startswith(("http", "#"))}
    for t in listed:
        if t not in in_readme and t.name != "README.md":
            errors.append(f"docs/README.md: does not link {t.relative_to(DOCS)}")
    for e in errors:
        print(e)
    sys.exit(1 if errors else 0)


main()
