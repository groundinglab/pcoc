#!/usr/bin/env python3
"""Fail if a tracked file still names a withdrawn Satoshi Hike v2.0 term.

Patterns (any tracked file):
  fat[-_ ]?slot          case-insensitive
  fill_pin
  PSEED64
  slotXOR | slotXor | slot_xor
  memory-hard            case-insensitive

Allowlist, and only this allowlist:
  .github/scripts/withdrawn-step.py
  the SPEC changelog:
    spec/CHANGELOG.md, or any spec/**/CHANGELOG.md
    a Markdown heading in spec/**/*.md whose text contains "changelog" or "changes"
    the bold block "**Changes…**" and its following bullet list (the v2.0 changes list)

The SPEC body, the SDKs, and the gate are not allowlisted. This check stays red
until spec/v2.1-correction and the SDK branches delete the terms from those trees.
Name a withdrawn term only in the changelog.

Run from the repository root. Exit 1 on any hit.
"""
import re
import subprocess
import sys

ALLOW_EXACT = {
    ".github/scripts/withdrawn-step.py",
}

PATTERNS = (
    ("fatslot", re.compile(r"fat[-_ ]?slot", re.IGNORECASE)),
    ("fill_pin", re.compile(r"fill_pin")),
    ("PSEED64", re.compile(r"PSEED64")),
    ("slot_xor", re.compile(r"slotXOR|slotXor|slot_xor")),
    ("memory-hard", re.compile(r"memory-hard", re.IGNORECASE)),
)

HEADING = re.compile(r"^(#{1,6})\s+.*\b(?:changelog|changes)\b", re.IGNORECASE)
BOLD_CHANGES = re.compile(r"^\*\*Changes\b", re.IGNORECASE)


def allowed_file(path):
    if path in ALLOW_EXACT:
        return True
    if path.startswith("spec/") and path.rsplit("/", 1)[-1].lower() == "changelog.md":
        return True
    return False


def changelog_lines(path, lines):
    """0-based indexes of the SPEC changelog. Other files return an empty set."""
    if not path.startswith("spec/") or not path.endswith(".md"):
        return set()
    skip = set()
    i = 0
    n = len(lines)
    while i < n:
        match = HEADING.match(lines[i])
        if match:
            level = len(match.group(1))
            skip.add(i)
            i += 1
            while i < n:
                heading = re.match(r"^(#{1,6})\s+", lines[i])
                if heading and len(heading.group(1)) <= level:
                    break
                skip.add(i)
                i += 1
            continue
        if BOLD_CHANGES.match(lines[i]):
            skip.add(i)
            i += 1
            while i < n:
                if lines[i].startswith("#"):
                    break
                if lines[i].startswith("**") and not lines[i].startswith("**Changes"):
                    break
                if lines[i].strip() == "" or lines[i].lstrip().startswith(("-", "*", "+")):
                    skip.add(i)
                    i += 1
                    continue
                break
            continue
        i += 1
    return skip


def tracked_files():
    out = subprocess.run(
        ["git", "ls-files", "-z"],
        check=True,
        capture_output=True,
    )
    return [p.decode("utf-8", "surrogateescape") for p in out.stdout.split(b"\0") if p]


def main():
    hits = 0
    by_file = {}
    for path in tracked_files():
        if allowed_file(path):
            continue
        try:
            raw = open(path, "rb").read()
        except OSError as err:
            print(f"{path}: unreadable: {err}", file=sys.stderr)
            return 2
        text = raw.decode("utf-8", "replace")
        lines = text.splitlines()
        skip = changelog_lines(path, lines)
        for number, line in enumerate(lines, 1):
            if (number - 1) in skip:
                continue
            for name, pattern in PATTERNS:
                if pattern.search(line):
                    print(f"{path}:{number}:{name}: {line.strip()}")
                    hits += 1
                    by_file.setdefault(path, set()).add(name)
                    break
    print(f"withdrawn-step hits: {hits} across {len(by_file)} files")
    for path in sorted(by_file):
        print(f"  {path}: {', '.join(sorted(by_file[path]))}")
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
