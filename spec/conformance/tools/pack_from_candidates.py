#!/usr/bin/env python3
"""Write pack JSON from the four SDK candidate files.

A file is written only when sdk/{rust,c,go,java}/candidates/<name> are
byte-identical. The script stops if they differ, or if a key is object_hex
or draw_bytes_hex. It does not invent a value.
"""
import json
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
LANGS = ("rust", "c", "go", "java")
PACK = ROOT / "spec" / "conformance" / "pack"
NAMES = (
    "seed.txt",
    "nl4.txt",
    "terrain.txt",
    "fold.txt",
    "kat-small.txt",
    "test-3leg.txt",
    "prod.txt",
    "wes.txt",
    "rate.txt",
    "anchor.txt",
    "br1.txt",
    "draw.txt",
    "hashcash.txt",
    "negatives.txt",
    "unknown-kind.txt",
)
FORBIDDEN = ("object_hex", "draw_bytes_hex")


def parse(text: str, name: str) -> dict:
    values = {}
    for line in text.splitlines():
        if not line:
            continue
        key, value = line.split("\t", 1)
        for bad in FORBIDDEN:
            if bad in key:
                raise SystemExit(f"{name} carries {bad}")
        if key in values:
            raise SystemExit(f"{name} repeats {key}")
        values[key] = value
    return values


def main() -> None:
    written = []
    for name in NAMES:
        texts = []
        for lang in LANGS:
            path = ROOT / "sdk" / lang / "candidates" / name
            texts.append(path.read_bytes())
        if any(item != texts[0] for item in texts[1:]):
            raise SystemExit(f"{name} differs across the four SDKs")
        values = parse(texts[0].decode(), name)
        doc = {
            "agreed_by": list(LANGS),
            "candidate": name,
            "values": values,
        }
        out = PACK / (name[:-4] + ".json")
        out.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")
        written.append(out.name)
    present = sorted(p.name for p in PACK.iterdir() if p.name != "manifest.json")
    # The generator does not delete blockhash-display.json, README.md, or the
    # Rust sign-in files. Those must already be in the directory.
    missing = [name for name in written if name not in present]
    if missing:
        raise SystemExit(f"not written: {missing}")
    for name in ("br1.json", "sign-in.json", "sign-in-origins.json"):
        if name not in present:
            raise SystemExit(f"pack cannot be complete, missing {name}")
    manifest = {
        "files": present,
        "pack": "pack-2.2.0",
        "status": "complete",
    }
    (PACK / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"wrote {len(written)} candidate files; manifest lists {len(present)}")


if __name__ == "__main__":
    main()
    sys.exit(0)
