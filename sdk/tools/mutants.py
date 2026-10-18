#!/usr/bin/env python3
"""One exact-string edit per mutant, then that mutant's tests.

Run from anywhere:

    python3 sdk/tools/mutants.py

A mutant is killed when its command exits non-zero. The file is restored
either way. Paths are relative to the repository root.
"""
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# Each old string is the current source. new is the edit that brings the bug back.
MUTANTS = [
    {
        "id": "F5-Rust",
        "file": "sdk/rust/hike/src/object.rs",
        "old": "fn height_in_order(height: Option<u64>) -> Option<u64> {\n    height\n}",
        "new": "fn height_in_order(height: Option<u64>) -> Option<u64> {\n    height.filter(|height| *height <= crate::consts::JSON_INT_MAX)\n}",
        "cmd": "cargo test --locked --manifest-path sdk/rust/Cargo.toml --test break_cases -- an_out_of_range_interrupt_height_keeps_exact_order -- --exact",
    },
    {
        "id": "F5-Go",
        "file": "sdk/go/hike/verify.go",
        "old": "h, okH, _ := uintField(st, \"beacon_height\")\n\t\t\t// A height above 2^53−1 still takes part, by its exact value.\n\t\t\t// The record writes it `not defined` (`height_range`).\n\t\t\theaders[g] = pair{d: d, h: h, okD: okD && len(d) == 32, okH: okH}",
        "new": "h, okH, over := uintField(st, \"beacon_height\")\n\t\t\theaders[g] = pair{d: d, h: h, okD: okD && len(d) == 32, okH: okH && !over}",
        "cmd": "go test ./hike/ -count=1 -timeout 120s -run TestF5OutOfRangeInterruptHeightKeepsExactOrder",
        "cwd": "sdk/go",
    },
    {
        "id": "F5-Java",
        "file": "sdk/java/src/pcoc/hike/Measure.java",
        "old": "    private static Long orderedHeight(Long height) {\n        return height;\n    }",
        "new": "    private static Long orderedHeight(Long height) {\n        if (height == null || Hike.aboveJson(height)) {\n            return null;\n        }\n        return height;\n    }",
        "cmd": "bash sdk/java/v2.sh",
    },
    {
        "id": "Q-j-Java",
        "file": "sdk/java/src/pcoc/hike/Measure.java",
        "old": "        Long last = uint(top, \"last_interrupt_height\");\n        if (last != null && Hike.aboveJson(last)) {\n            return true;\n        }",
        "new": "        Long slots = uint(top, \"n_slots\");\n        if (slots != null && Hike.aboveJson(slots)) {\n            return true;\n        }\n        Long last = uint(top, \"last_interrupt_height\");\n        if (last != null && Hike.aboveJson(last)) {\n            return true;\n        }",
        "cmd": "bash sdk/java/v2.sh",
    },
    {
        "id": "plant-Go",
        "file": "sdk/go/hike/verify.go",
        "old": "\t\t} else {\n\t\t\tout.plant = \"not read\"\n\t\t}",
        "new": "\t\t} else {\n\t\t\tout.plant = \"match\"\n\t\t\tout.anchor = \"anchored\"\n\t\t}",
        "cmd": "go test ./hike/ -count=1 -timeout 120s -run TestPlantNotReadWhenIndexReturnsNoDocument",
        "cwd": "sdk/go",
    },
    {
        "id": "plant-Java",
        "file": "sdk/java/src/pcoc/hike/Measure.java",
        "old": "                    plant = \"not read\";\n                    anchor = \"not anchored\";\n                    tally.plantReadNotRead = true;",
        "new": "                    plant = \"match\";\n                    anchor = \"anchored\";",
        "cmd": "bash sdk/java/v2.sh",
    },
    {
        "id": "F10-Rust",
        "file": "sdk/rust/hike/src/cbor.rs",
        "old": "n.min(64)",
        "new": "n",
        "all": True,
        "cmd": "cargo test --locked --manifest-path sdk/rust/Cargo.toml --lib -- a_count_equal_to_the_bytes_left_does_not_reserve_the_count -- --exact",
    },
    {
        "id": "F10-Go",
        "file": "sdk/go/hike/cbor.go",
        "old": "capCount(*arg)",
        "new": "int(*arg)",
        "all": True,
        "cmd": "go test ./hike/ -count=1 -timeout 180s -run TestF10CountEqualToBytesLeftDoesNotPresize",
        "cwd": "sdk/go",
    },
    {
        "id": "F10-Java",
        "file": "sdk/java/src/pcoc/hike/Cbor.java",
        "old": "argument > 64 ? 64 : (int) argument",
        "new": "(int) argument",
        "cmd": "bash sdk/java/v2.sh",
    },
    {
        "id": "F11-Rust",
        "file": "sdk/rust/hike/src/object.rs",
        "old": "            if header {\n                notes.push(\"beacon\");\n            }",
        "new": "            notes.push(\"beacon\");",
        "cmd": "cargo test --locked --manifest-path sdk/rust/Cargo.toml --test break_cases -- a_broken_stage_past_17_adds_no_header_condition -- --exact",
    },
    {
        "id": "Q-b-Go",
        "file": "sdk/go/hike/verify.go",
        "old": "\tif st.kind != 5 {\n\t\t// A non-map element is a stage with every field absent.\n\t\treturn cborVal{kind: 5}, true\n\t}",
        "new": "\tif st.kind != 5 {\n\t\treturn cborVal{}, false\n\t}",
        "cmd": "go test ./hike/ -count=1 -timeout 120s -run TestQBNonMapStageIsEveryFieldAbsent",
        "cwd": "sdk/go",
    },
    {
        "id": "gate-version",
        "file": "services/admission-gate/tests/gate.rs",
        "old": 'assert_eq!(body["version"], "0.5.6");',
        "new": 'assert_eq!(body["version"], "0.5.5");',
        "cmd": "cargo test --locked --manifest-path services/admission-gate/Cargo.toml --test gate -- health_reports_version_commit_spec_and_config -- --exact",
    },
]


def run(mutant):
    path = os.path.join(ROOT, mutant["file"])
    with open(path, encoding="utf-8") as fh:
        original = fh.read()
    old, new = mutant["old"], mutant["new"]
    found = original.count(old)
    if mutant.get("all"):
        if found < 1:
            return "error", f"old string missing ({found})"
        edited = original.replace(old, new)
    else:
        if found != 1:
            return "error", f"old string count {found}"
        edited = original.replace(old, new, 1)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(edited)
    try:
        cwd = os.path.join(ROOT, mutant["cwd"]) if mutant.get("cwd") else ROOT
        proc = subprocess.run(
            ["bash", "-c", mutant["cmd"]],
            cwd=cwd,
            env=os.environ.copy(),
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
    finally:
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(original)
    if proc.returncode != 0:
        return "killed", f"exit {proc.returncode}"
    return "survived", "tests passed"


def main():
    rows = []
    for mutant in MUTANTS:
        status, detail = run(mutant)
        rows.append((mutant["id"], status, detail))
        print(f"{mutant['id']}: {status} ({detail})", flush=True)
    killed = sum(1 for _, status, _ in rows if status == "killed")
    survived = sum(1 for _, status, _ in rows if status == "survived")
    print(f"mutants killed={killed} survived={survived} error={len(rows) - killed - survived}")
    return 0 if survived == 0 and all(status != "error" for _, status, _ in rows) else 1


if __name__ == "__main__":
    sys.exit(main())
