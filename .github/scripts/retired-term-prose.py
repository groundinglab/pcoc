#!/usr/bin/env python3
"""Walk-prose check: the word "walk" (whole word, any case) must not appear in prose files
(*.md, *.html, *.xml; **/import/** excluded, as in the release plan's G1-prose rule).
Allowed: occurrences of the SEAL_DOMAIN wire constant `pcoc-walk-v1`, and lines listed exactly
(path + full line content) in .github/retired-term-allowlist.txt. Code identifiers are out of scope.
Run from the repository root. Exit 1 on any hit."""
import re, subprocess, sys
allow = set()
for raw in open(".github/retired-term-allowlist.txt", encoding="utf-8"):
    raw = raw.rstrip("\n")
    if raw and not raw.startswith("#"):
        path, content = raw.split("\t", 1)
        allow.add((path, content))
out = subprocess.run(["rg", "-nwi", "--no-heading", "--with-filename", "walk", "--glob", "*.md", "--glob", "*.html",
                      "--glob", "*.xml", "--glob", "!**/import/**", "."], capture_output=True, text=True,
                     stdin=subprocess.DEVNULL)
if out.returncode > 1:
    sys.exit(out.stderr)
word = re.compile(r"\bwalk\b", re.I)
hits = used = 0
for line in out.stdout.splitlines():
    path, num, content = line.split(":", 2)
    path = path.removeprefix("./")
    if (path, content) in allow:
        used += 1; continue
    if not word.search(content.replace("pcoc-walk-v1", "")):
        continue
    print(f"{path}:{num}: {content}"); hits += 1
print(f"walk-prose hits: {hits} (allowlisted lines matched: {used}/{len(allow)})")
sys.exit(1 if hits else 0)
