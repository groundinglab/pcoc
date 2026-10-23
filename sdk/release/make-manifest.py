#!/usr/bin/env python3
"""Write sdk/release/release-manifest.json from files in this tree.

Hashes and the negative count are read from the files. No leg is timed.
The commit field is the parent this file was generated against; the commit
that adds the file cannot name itself.
"""
import hashlib
import json
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "sdk" / "release" / "release-manifest.json"
COPY = ROOT / "sdk" / "release" / "manifests" / "2.3.0.json"
LEGACY = ROOT / "sdk" / "release" / "manifests" / "2.2.0.json"


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pin() -> dict:
    out = {}
    for line in (ROOT / "sdk" / "SPEC_PIN").read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        out[k] = v
    return out


def negatives() -> int:
    text = (ROOT / "sdk" / "rust" / "candidates" / "negatives.txt").read_text()
    return sum(1 for line in text.splitlines() if "matches_honest\tfalse" in line)


def prod_leg() -> str:
    for line in (ROOT / "sdk" / "rust" / "candidates" / "prod.txt").read_text().splitlines():
        if line.startswith("prod.PROD.stage.0.leg.0.legs_le_hex\t"):
            return line.split("\t", 1)[1]
    raise SystemExit("prod leg missing")


def parent() -> str:
    return subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip()


def main() -> None:
    pins = pin()
    manifest_sha = sha256(ROOT / "spec" / "conformance" / "pack" / "manifest.json")
    if pins.get("vector-sha256") != manifest_sha:
        raise SystemExit("SPEC_PIN does not match the pack manifest")
    sdk_version = (ROOT / "sdk" / "VERSION").read_text().strip()
    # 2.3 folds errata 1 and 2 into the SPEC. The pin names the tag and the
    # pack hash. It has no erratum key.
    if sdk_version != "2.3.0" or pins.get("tag") != "spec-2.3.0" or "erratum" in pins:
        raise SystemExit("SPEC_PIN is not the 2.3 form (tag spec-2.3.0, no erratum key) or VERSION is not 2.3.0")
    if not LEGACY.is_file():
        raise SystemExit("manifests/2.2.0.json is the 2.2 record and must stay")
    fixed = "\n".join([
        "O1-S1 object sha256 800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246",
        "O1-S1 six_leg match, checkpoints match, waypoints match",
        "negatives 13 non-match",
        f"PROD leg 0 legs_le_hex {prod_leg()}",
    ]) + "\n"
    doc = {
        "schema": "pcoc-sdk-release/1",
        "release": {
            "version": sdk_version,
            "date": "2026-10-23",
            "commit": parent(),
            "notes": "Parent of the commit that adds this file. Tags are not cut.",
        },
        "spec": {
            "version": "2.3",
            "tag": "spec-2.3.0",
            # Blob and tree links use the release tag. Errata are folded into SPEC.md.
            "url": "https://github.com/groundinglab/pcoc/blob/spec-2.3.0/spec/satoshi-hike-v2.3/SPEC.md",
            "pack_url": "https://github.com/groundinglab/pcoc/tree/spec-2.3.0/spec/conformance/pack",
            "pack_manifest_sha256": manifest_sha,
            "pack_status": json.loads((ROOT / "spec/conformance/pack/manifest.json").read_text())["status"],
        },
        "links": {
            "interop_evidence": "https://github.com/groundinglab/pcoc/tree/interop/evidence-2.2/spec/conformance/interop",
            "policy_page": "https://groundinglab.io/admission-gate/policy/",
            "policy_file": "https://github.com/groundinglab/pcoc/blob/spec-2.3.0/policy/groundinglab-admission/gl-admit-v10.toml",
        },
        "conformance": {
            "objects": {
                "O1-S1": sha256(ROOT / "sdk/rust/objects/O1-S1.cbor"),
                "O2-S2": sha256(ROOT / "sdk/rust/objects/O2-S2.cbor"),
                "O3-PROD": sha256(ROOT / "sdk/rust/objects/O3-PROD.cbor"),
            },
            "prod_leg_legs_le_hex": prod_leg(),
            "negatives": {"count": negatives(), "all_non_match": True},
            "quickstart_fixed_lines_sha256": hashlib.sha256(fixed.encode()).hexdigest(),
        },
        "sdks": [],
        "performance": [],
        "performance_note": "No timing run was taken for this manifest. The production-leg line in the quickstart is the pinned legs_le_hex, not a measured duration.",
        "site": {
            "v22_pending_closed": [15, 16, 17],
            "pages": ["technical/sdk", "technical/index", "make", "admission-gate/policy"],
        },
    }
    commands = {
        "rust": "cd sdk/rust\ncargo run -p pcoc-cli --release --quiet -- quickstart",
        "c": "cd sdk/c\nmake -j4 build/pcoc\n./build/pcoc quickstart",
        "go": "cd sdk/go\ngo run ./cmd/pcoc quickstart",
        "java": "cd sdk/java\n./gradlew run --args=quickstart",
    }
    toolchains = {
        "rust": "Rust 1.99.0",
        "c": "gcc, C11, -march=x86-64",
        "go": "Go 1.22 or newer",
        "java": "JDK 21",
    }
    prefix = manifest_sha[:16]
    for lang in ("rust", "c", "go", "java"):
        doc["sdks"].append({
            "language": lang,
            "version": sdk_version,
            "tag": f"sdk-{lang}-v{sdk_version}",
            "tag_status": "not cut",
            "toolchain_min": toolchains[lang],
            "quickstart": {
                "commands": commands[lang],
                "expected_stdout_example": "\n".join([
                    f"pcoc {sdk_version} ({lang}) spec-2.3.0 pack {prefix}: pin match",
                    "kernel scalar, sha256 sha-ni",
                    "O1-S1 object sha256 800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246",
                    "O1-S1 six_leg match, checkpoints match, waypoints match",
                    "negatives 13 non-match",
                    f"PROD leg 0 legs_le_hex {prod_leg()}",
                ]) + "\n",
                "machine_specific_lines": [2],
                "expected_stdout_sha256_fixed_lines": doc["conformance"]["quickstart_fixed_lines_sha256"],
            },
            "tested_on": [{
                "machine": "reference-system-3",
                "config": "AMD Ryzen 5 7430U, 12 hardware threads",
                "os": "Linux Mint 22.3",
                "date": "2026-10-23",
                "quickstart": "match",
                "cold_start": "not run",
                "kernel": "scalar",
                "sha256": "sha-ni",
            }],
            "performance": [],
        })
    if doc["conformance"]["negatives"]["count"] != 13:
        raise SystemExit("negative count")
    text = json.dumps(doc, indent=2) + "\n"
    OUT.write_text(text)
    COPY.parent.mkdir(parents=True, exist_ok=True)
    COPY.write_text(text)
    print(OUT)
    print("fixed", doc["conformance"]["quickstart_fixed_lines_sha256"])


if __name__ == "__main__":
    main()
