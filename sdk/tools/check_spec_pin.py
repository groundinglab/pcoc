#!/usr/bin/env python3
"""Fail unless sdk/SPEC_PIN holds for this checkout and for its spec tag.

1. spec/conformance in the working tree hashes to vector-sha256.
2. The spec tag named by tag= exists in this clone, and spec/conformance in
   that tag's tree hashes to the same value. So the tag line is checked too.

Same hash as spec/conformance/tools/ring1_checks.py: sha256 over every file
under spec/conformance/ except spec/conformance/tools/, sorted by directory
and then file name, each as path, NUL, sha256(bytes). Run from the repo root.
Needs a git clone that has the tag (a full clone, or `git fetch --tags`)."""
import hashlib, os, subprocess, sys

pin = dict(l.strip().split("=", 1) for l in open("sdk/SPEC_PIN") if "=" in l)


def digest(entries):
    """entries: (directory, file name) -> bytes reader."""
    h = hashlib.sha256()
    for root, name in sorted(entries, key=lambda e: (e[0], e[1])):
        p = f"{root}/{name}"
        h.update(p.encode() + b"\0" + hashlib.sha256(entries[(root, name)]()).digest())
    return h.hexdigest()


def skip(root):
    return root == "spec/conformance/tools" or root.startswith("spec/conformance/tools/")


tree = {}
for root, _, fs in os.walk("spec/conformance"):
    if skip(root):
        continue
    for f in fs:
        tree[(root, f)] = (lambda p=os.path.join(root, f): open(p, "rb").read())
got = digest(tree)
ok = got == pin["vector-sha256"]
print(f"tag {pin['tag']} pinned {pin['vector-sha256']} got {got}")

tag = pin["tag"]
ls = subprocess.run(["git", "ls-tree", "-r", "-z", f"refs/tags/{tag}^{{tree}}", "--", "spec/conformance"],
                    capture_output=True)
if ls.returncode != 0 or not ls.stdout:
    print(f"tag {tag}: not found in this clone (needs a full clone or `git fetch --tags`)")
    sys.exit(1)
tagged = {}
for item in ls.stdout.split(b"\0"):
    if not item:
        continue
    meta, path = item.split(b"\t", 1)
    path = path.decode()
    root, name = path.rsplit("/", 1)
    if skip(root):
        continue
    blob = meta.split()[2].decode()
    tagged[(root, name)] = (lambda b=blob: subprocess.run(["git", "cat-file", "blob", b],
                                                          capture_output=True, check=True).stdout)
at_tag = digest(tagged)
print(f"tag {tag} tree hashes to {at_tag}")
ok = ok and at_tag == pin["vector-sha256"]
sys.exit(0 if ok else 1)
