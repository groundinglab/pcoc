#!/usr/bin/env python3
"""Light link check: every relative Markdown link in docs/, apps/*.md, services/*.md and root *.md
points at a file or directory that exists. External links are not fetched. Run from the repo root."""
import glob, os, re, sys
files = sorted(set(glob.glob("*.md") + glob.glob("docs/**/*.md", recursive=True)
                   + glob.glob("services/*.md") + glob.glob("apps/*.md")))
bad = 0
for f in files:
    text = re.sub(r"```.*?```", "", open(f, encoding="utf-8").read(), flags=re.S)
    for target in re.findall(r"\]\(([^)\s]+)\)", text):
        if re.match(r"^[a-z][a-z0-9+.-]*:|^#", target):
            continue
        p = os.path.normpath(os.path.join(os.path.dirname(f), target.split("#")[0]))
        if not os.path.exists(p):
            print(f"FAIL {f}: {target}"); bad += 1
print(f"checked {len(files)} files, {bad} dead links")
sys.exit(1 if bad else 0)
