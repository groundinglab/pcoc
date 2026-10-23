#!/usr/bin/env python3
"""Cross-verify harness for the four Satoshi Hike 2.2 SDKs.

Serves the CASES synthetic index on 127.0.0.1, runs each verifier, and writes
the markdown report. No timings. No network except that loopback listener.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import posixpath
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.request import urlopen

LANGS = ("rust", "c", "go", "java")
CANDIDATE_FILES = (
    "anchor.txt",
    "br1.txt",
    "draw.txt",
    "fold.txt",
    "hashcash.txt",
    "kat-small.txt",
    "negatives.txt",
    "nl4.txt",
    "prod.txt",
    "rate.txt",
    "seed.txt",
    "terrain.txt",
    "test-3leg.txt",
    "unknown-kind.txt",
    "wes.txt",
)
PINS = {
    "O1-S1.cbor": "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246",
    "O2-S2.cbor": "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0",
    "O3-PROD.cbor": "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7",
}
PROD_LEG = "88a04ac5beacb8a7"
DRAW = "pcoc-pack-2.2/draw"
PROFILES = {
    "O1-S1.cbor": [
        "--profile",
        "conformance-small",
        "--sample-bytes",
        "32",
        "--slot-bytes",
        "64",
        "--terrain-bytes",
        "65536",
        "--steps-per-leg",
        "100",
    ],
    "O2-S2.cbor": [
        "--profile",
        "conformance-small",
        "--sample-bytes",
        "64",
        "--slot-bytes",
        "256",
        "--terrain-bytes",
        "1048576",
        "--steps-per-leg",
        "1000",
    ],
    "O3-PROD.cbor": ["--profile", "production"],
}
BOOKENDS = ("a", "b", "N", "S", "delta_h")
TRUNCATED = bytes((0x58, 0x05, 0x01))
BAD_UTF8 = bytes((0xA1, 0x64, 0x6B, 0x69, 0x6E, 0x64, 0x61, 0xFF))


class CborError(Exception):
    pass


def head(buf, i):
    if i >= len(buf):
        raise CborError("truncated header")
    b = buf[i]
    i += 1
    major = b >> 5
    add = b & 0x1F
    if add < 24:
        n = add
    elif add == 24:
        if i >= len(buf):
            raise CborError("truncated header")
        n = buf[i]
        i += 1
    elif add == 25:
        if i + 2 > len(buf):
            raise CborError("truncated header")
        n = int.from_bytes(buf[i : i + 2], "big")
        i += 2
    elif add == 26:
        if i + 4 > len(buf):
            raise CborError("truncated header")
        n = int.from_bytes(buf[i : i + 4], "big")
        i += 4
    elif add == 27:
        if i + 8 > len(buf):
            raise CborError("truncated header")
        n = int.from_bytes(buf[i : i + 8], "big")
        i += 8
    else:
        raise CborError("unsupported cbor length")
    return major, n, i


def skip(buf, i):
    major, n, i = head(buf, i)
    if major in (0, 1):
        return i
    if major in (2, 3):
        if i + n > len(buf):
            raise CborError("truncated string")
        return i + n
    if major == 4:
        for _ in range(n):
            i = skip(buf, i)
        return i
    if major == 5:
        for _ in range(n):
            i = skip(buf, i)
            i = skip(buf, i)
        return i
    raise CborError("unsupported cbor major")


def read_text(buf, i):
    major, n, j = head(buf, i)
    if major != 3:
        return None, skip(buf, i)
    if j + n > len(buf):
        raise CborError("truncated text")
    try:
        text = bytes(buf[j : j + n]).decode("utf-8")
    except UnicodeDecodeError:
        text = None
    return text, j + n


def encode_tstr(text: str) -> bytes:
    raw = text.encode("utf-8")
    n = len(raw)
    if n < 24:
        return bytes((0x60 | n,)) + raw
    if n < 256:
        return bytes((0x78, n)) + raw
    raise CborError("kind text too long")


def replace_kind(buf: bytes, text: str) -> bytes:
    major, n, i = head(buf, 0)
    if major != 5:
        raise CborError("object is not a map")
    for _ in range(n):
        key, i = read_text(buf, i)
        start = i
        i = skip(buf, i)
        if key == "kind":
            return buf[:start] + encode_tstr(text) + buf[i:]
    raise CborError("object has no kind")


def replace_leg_results(buf: bytearray, leg: bytes) -> int:
    if len(leg) != 8:
        raise CborError("leg result is not 8 bytes")
    count = 0

    def walk(i):
        nonlocal count
        major, n, j = head(buf, i)
        if major in (0, 1):
            return j
        if major in (2, 3):
            if j + n > len(buf):
                raise CborError("truncated string")
            return j + n
        if major == 4:
            for _ in range(n):
                j = walk(j)
            return j
        if major == 5:
            for _ in range(n):
                key, j = read_text(buf, j)
                if key == "leg_results":
                    m, ln, j = head(buf, j)
                    if m != 4:
                        raise CborError("leg_results is not an array")
                    for _k in range(ln):
                        mb, nb, content = head(buf, j)
                        if mb != 2 or nb != 8:
                            raise CborError("leg result is not an 8-byte string")
                        buf[content : content + 8] = leg
                        count += 1
                        j = content + 8
                else:
                    j = walk(j)
            return j
        raise CborError("unsupported cbor major")

    end = walk(0)
    if end != len(buf):
        raise CborError("trailing bytes after object")
    return count


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def cases_document(height: int) -> bytes:
    if height == 1000:
        natural = hashlib.sha256(b"pcoc-pack-2.2/beacon/seed").digest()
    elif 1001 <= height <= 1018:
        natural = hashlib.sha256(f"pcoc-pack-2.2/beacon/{height - 1001}".encode("ascii")).digest()
    else:
        natural = hashlib.sha256(
            f"pcoc-pack-2.2/index/filler/{height}".encode("ascii")
        ).digest()
    disp = natural[::-1].hex()
    when = 1700000000 + 600 * height
    body = (
        '{"network":"synthetic","header":{"hash":"'
        + disp
        + '","height":'
        + str(height)
        + ',"time":'
        + str(when)
        + "}}"
    )
    return body.encode("ascii")


def load_tsv(path: str) -> dict[str, str]:
    raw = open(path, "rb").read()
    try:
        text = raw.decode("ascii")
    except UnicodeError as exc:
        raise ValueError(f"{path} is not ASCII") from exc
    rows: dict[str, str] = {}
    if text == "":
        return rows
    if not text.endswith("\n"):
        raise ValueError(f"{path} does not end with LF")
    for line in text.split("\n")[:-1]:
        if line == "" or "\t" not in line or line.endswith("\t ") or " " in line.split("\t", 1)[0]:
            raise ValueError(f"{path} has a line that is not key<TAB>value")
        key, val = line.split("\t", 1)
        if key in rows:
            raise ValueError(f"{path} repeats {key}")
        rows[key] = val
    return rows


def first_key(left: dict[str, str] | None, right: dict[str, str] | None, left_err: str, right_err: str) -> str:
    if left is None or right is None:
        return left_err or right_err or "unreadable"
    for key in sorted(set(left) | set(right)):
        if left.get(key) != right.get(key):
            return key
    return "(no key differs)"


def parse_erratum(path: str) -> dict[str, str]:
    lines = open(path, encoding="utf-8").read().splitlines()
    start = None
    for i, line in enumerate(lines):
        if line.startswith("honest.legs_le_hex\t"):
            start = i
            break
    if start is None:
        raise SystemExit(f"cross-verify: erratum reference table not found in {path}")
    table: dict[str, str] = {}
    for line in lines[start:]:
        if line.startswith("#") or line.strip() == "" or line.startswith("```"):
            break
        if "\t" not in line:
            break
        key, val = line.split("\t", 1)
        if not key or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789_." for c in key):
            break
        table[key] = val.strip()
    if "honest.legs_le_hex" not in table or "squeezed_seed_terrain.terrain_sha256" not in table:
        raise SystemExit("cross-verify: erratum reference table is incomplete")
    return table


def find_named(root: str, name: str) -> str | None:
    direct = os.path.join(root, name)
    nested = os.path.join(root, "impl", name)
    if os.path.isdir(direct):
        return direct
    if os.path.isdir(nested):
        return nested
    return None


def run_cmd(cmd: list[str], env: dict[str, str] | None = None, timeout: int = 180) -> tuple[int, bytes, bytes]:
    try:
        proc = subprocess.run(
            ["nice", "-n", "10", *cmd],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as exc:
        out = exc.stdout or b""
        err = exc.stderr or b""
        err += b"\nverifier did not finish\n"
        return 9, out if isinstance(out, bytes) else b"", err if isinstance(err, bytes) else b""
    return proc.returncode, proc.stdout, proc.stderr


def one_record(stdout: bytes) -> bytes | None:
    if not stdout.endswith(b"\n"):
        return None
    body = stdout[:-1]
    if b"\n" in body or body == b"":
        return None
    return body


def walk_not_read(value, path: list[str], hits: list[str]) -> None:
    if isinstance(value, dict):
        for key, item in value.items():
            walk_not_read(item, path + [str(key)], hits)
    elif isinstance(value, list):
        for i, item in enumerate(value):
            walk_not_read(item, path + [str(i)], hits)
    elif value == "not read":
        hits.append(".".join(path))


def structural(rec: dict) -> list[str]:
    reasons = []
    if rec.get("shape") != "match":
        reasons.append(f"shape {rec.get('shape')!r} item {rec.get('shape_item')!r}")
    headers = rec.get("headers")
    if not isinstance(headers, list) or len(headers) != 19:
        reasons.append("headers are not 19 elements")
    else:
        for i, header in enumerate(headers):
            if not isinstance(header, dict):
                reasons.append(f"header {i} is not an object")
                continue
            if header.get("index_comparison") != "match":
                reasons.append(
                    f"header {i} index_comparison {header.get('index_comparison')!r}"
                )
            tval = header.get("T")
            if isinstance(tval, bool) or not isinstance(tval, int):
                reasons.append(f"header {i} T {tval!r}")
    six = rec.get("six_leg")
    if not isinstance(six, dict) or six.get("results") != ["match"] * 6:
        shown = six.get("results") if isinstance(six, dict) else six
        reasons.append(f"six_leg results {shown!r}")
    hits: list[str] = []
    walk_not_read(rec, [], hits)
    if hits:
        reasons.append("not read at " + ", ".join(hits))
    for key in BOOKENDS:
        if rec.get(key) != "not defined":
            reasons.append(f"bookend {key} is {rec.get(key)!r}")
    return reasons


def snippet(text: bytes, limit: int = 240) -> str:
    raw = text.decode("utf-8", "replace").replace("\r", "")
    raw = raw.strip()
    if len(raw) > limit:
        raw = raw[:limit] + "…"
    return raw


def diff_at(left: bytes, right: bytes) -> str:
    n = min(len(left), len(right))
    for i in range(n):
        if left[i] != right[i]:
            a = left[max(0, i - 24) : i + 24]
            b = right[max(0, i - 24) : i + 24]
            return f"first difference at byte {i}: {a!r} versus {b!r}"
    return f"lengths {len(left)} and {len(right)}"


# PENDING_ERRATUM_2 was the one mask in this gate. SPEC 2.2 erratum 2 (E01,
# E02, option A) settled the fields it named: on a malformed CBOR value,
# paths[g].path_state and paths[g].plant for g in 1..16 are `not defined`,
# skip_list is [], and every per_stage checkpoint_count, whole_hike_legs and
# hike_legs are `not defined`; any text string (map key or `kind`) that is not
# valid UTF-8 is a malformed CBOR value. The tuple is empty, so the empty input, the truncated bytes and
# the invalid-UTF-8 `kind` are compared byte for byte, and each record must
# also equal MALFORMED_RECORD_SHA256.
PENDING_ERRATUM_2: tuple = ()

# SHA-256 of the one-line record (with its newline) that `verify <file>` prints
# for every malformed CBOR value under the section 6 constants (erratum 2, E02).
MALFORMED_RECORD_SHA256 = "eb7bff4d55ddcc47e47c600276693db8a406a0083a47c2526462651ffa5bdb8c"


def _json_skip_ws(text: str, i: int) -> int:
    n = len(text)
    while i < n and text[i] in " \t\r\n":
        i += 1
    return i


def _json_string_end(text: str, i: int) -> int:
    if i >= len(text) or text[i] != '"':
        raise ValueError("expected a JSON string")
    i += 1
    n = len(text)
    while i < n:
        if text[i] == "\\":
            if i + 1 >= n:
                raise ValueError("truncated JSON escape")
            i += 2
            continue
        if text[i] == '"':
            return i + 1
        i += 1
    raise ValueError("unterminated JSON string")


def _json_atomic_end(text: str, i: int) -> int:
    if text[i] == '"':
        return _json_string_end(text, i)
    if text.startswith("true", i):
        return i + 4
    if text.startswith("false", i):
        return i + 5
    if text.startswith("null", i):
        return i + 4
    j = i
    n = len(text)
    if j < n and text[j] == "-":
        j += 1
    digits = j
    while j < n and text[j].isdigit():
        j += 1
    if j == digits:
        raise ValueError("expected a JSON value")
    if j < n and text[j] == ".":
        j += 1
        digits = j
        while j < n and text[j].isdigit():
            j += 1
        if j == digits:
            raise ValueError("bad JSON fraction")
    if j < n and text[j] in "eE":
        j += 1
        if j < n and text[j] in "+-":
            j += 1
        digits = j
        while j < n and text[j].isdigit():
            j += 1
        if j == digits:
            raise ValueError("bad JSON exponent")
    return j


def _json_value_spans(text: str) -> list[tuple[tuple, int, int]]:
    """Byte spans of every atomic value and every array in `text`."""
    found: list[tuple[tuple, int, int]] = []

    def walk(i: int, path: tuple) -> int:
        i = _json_skip_ws(text, i)
        if i >= len(text):
            raise ValueError("truncated JSON")
        c = text[i]
        if c == "{":
            return walk_object(i, path)
        if c == "[":
            return walk_array(i, path)
        end = _json_atomic_end(text, i)
        found.append((path, i, end))
        return end

    def walk_object(i: int, path: tuple) -> int:
        i += 1
        first = True
        while True:
            i = _json_skip_ws(text, i)
            if i >= len(text):
                raise ValueError("truncated JSON object")
            if text[i] == "}":
                return i + 1
            if not first:
                if text[i] != ",":
                    raise ValueError("expected a comma in a JSON object")
                i = _json_skip_ws(text, i + 1)
            first = False
            if i >= len(text) or text[i] != '"':
                raise ValueError("expected a JSON object key")
            key_end = _json_string_end(text, i)
            key = json.loads(text[i:key_end])
            if not isinstance(key, str):
                raise ValueError("JSON object key is not a string")
            i = _json_skip_ws(text, key_end)
            if i >= len(text) or text[i] != ":":
                raise ValueError("expected a colon")
            i = walk(i + 1, path + (key,))

    def walk_array(i: int, path: tuple) -> int:
        start = i
        i += 1
        first = True
        index = 0
        while True:
            i = _json_skip_ws(text, i)
            if i >= len(text):
                raise ValueError("truncated JSON array")
            if text[i] == "]":
                found.append((path, start, i + 1))
                return i + 1
            if not first:
                if text[i] != ",":
                    raise ValueError("expected a comma in a JSON array")
                i = _json_skip_ws(text, i + 1)
            first = False
            i = walk(i, path + (index,))
            index += 1

    end = walk(0, ())
    end = _json_skip_ws(text, end)
    if end != len(text):
        raise ValueError("trailing bytes after the JSON value")
    return found


def _erratum2_masks(path: tuple, g_of: dict[int, object]) -> bool:
    if not PENDING_ERRATUM_2:
        return False
    if path == ("skip_list",) and ("skip_list",) in PENDING_ERRATUM_2:
        return True
    if path == ("analysis", "whole_hike_legs") and ("analysis", "whole_hike_legs") in PENDING_ERRATUM_2:
        return True
    if path == ("analysis", "hike_legs") and ("analysis", "hike_legs") in PENDING_ERRATUM_2:
        return True
    if (
        len(path) == 4
        and path[0] == "analysis"
        and path[1] == "per_stage"
        and isinstance(path[2], int)
        and path[3] == "checkpoint_count"
        and ("analysis", "per_stage", "checkpoint_count") in PENDING_ERRATUM_2
    ):
        return True
    if (
        len(path) == 3
        and path[0] == "paths"
        and isinstance(path[1], int)
        and path[2] in ("path_state", "plant")
        and ("paths", path[2]) in PENDING_ERRATUM_2
    ):
        g = g_of.get(path[1])
        return isinstance(g, int) and not isinstance(g, bool) and 1 <= g <= 16
    return False


def mask_pending_erratum_2(record: bytes) -> bytes:
    """Drop PENDING_ERRATUM_2 value bytes. An empty tuple returns `record`."""
    if not PENDING_ERRATUM_2:
        return record
    text = record.decode("utf-8")
    found = _json_value_spans(text)
    g_of: dict[int, object] = {}
    for path, start, end in found:
        if (
            len(path) == 3
            and path[0] == "paths"
            and isinstance(path[1], int)
            and path[2] == "g"
        ):
            g_of[path[1]] = json.loads(text[start:end])
    spans = [
        (start, end)
        for path, start, end in found
        if _erratum2_masks(path, g_of)
    ]
    spans.sort()
    chosen: list[tuple[int, int]] = []
    for start, end in spans:
        if start < 0 or end < start or end > len(text):
            raise ValueError("mask span is outside the record")
        if chosen and start < chosen[-1][1]:
            if end <= chosen[-1][1]:
                continue
            raise ValueError("mask spans overlap")
        chosen.append((start, end))
    for start, end in reversed(chosen):
        text = text[:start] + text[end:]
    return text.encode("utf-8")


def _json_token(value: object) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def _uniform(values: list) -> str:
    if values and all(item == values[0] for item in values):
        return _json_token(values[0])
    return _json_token(values)


def erratum2_field_table(label: str, parsed: dict[str, dict]) -> str:
    """Markdown table of the masked fields, one column per verifier."""

    def path_values(obj: dict, key: str) -> str:
        vals = []
        for el in obj.get("paths") or []:
            if isinstance(el, dict) and isinstance(el.get("g"), int) and 1 <= el["g"] <= 16:
                vals.append(el.get(key))
        return _uniform(vals)

    def analysis_value(obj: dict, key: str) -> str:
        analysis = obj.get("analysis")
        if not isinstance(analysis, dict):
            return "missing"
        return _json_token(analysis.get(key))

    def counts(obj: dict) -> str:
        analysis = obj.get("analysis")
        stages = analysis.get("per_stage") if isinstance(analysis, dict) else None
        if not isinstance(stages, list):
            return "missing"
        vals = []
        for el in stages:
            vals.append(el.get("checkpoint_count") if isinstance(el, dict) else None)
        return _uniform(vals)

    rows = (
        ("`paths[g].path_state` for g in 1…16", lambda obj: path_values(obj, "path_state")),
        ("`paths[g].plant` for g in 1…16", lambda obj: path_values(obj, "plant")),
        ("`skip_list`", lambda obj: _json_token(obj.get("skip_list"))),
        ("`analysis.per_stage[g].checkpoint_count` for g in 0…17", counts),
        ("`analysis.whole_hike_legs`", lambda obj: analysis_value(obj, "whole_hike_legs")),
        ("`analysis.hike_legs`", lambda obj: analysis_value(obj, "hike_legs")),
    )
    lines = [
        f"### `{label}`",
        "",
        "| field | rust | c | go | java |",
        "|---|---|---|---|---|",
    ]
    for name, getter in rows:
        cells = [getter(parsed[lang]) for lang in LANGS]
        lines.append("| " + " | ".join([name, *cells]) + " |")
    lines.append("")
    return "\n".join(lines)


def _pending_erratum2_section(tables: list[str], names: list[str]) -> str:
    lines = [
        "## pending erratum 2",
        "",
        "These cells are pending. They are not matches. "
        "`PENDING_ERRATUM_2` in `sdk/conformance/harness.py` is the only mask. "
        "Set that tuple to `()` and the empty and truncated records are compared byte for byte.",
        "",
        "SPEC.md line 1320 limits `path_state` to `present` or `skip`, and `plant` is `n/a` only when `path_state` is `skip`. "
        "SPEC.md line 1346 (Rule A4.1) says a value left without a usable object value is `not defined`. "
        "Malformed CBOR leaves no object, so those sentences do not pick one record.",
        "",
        "Three analysis counts differ for the same reason, and no other field differs. "
        "SPEC.md line 961 leaves `whole_hike_legs` `not defined` when a stage is missing. "
        "SPEC.md lines 1338 and 1591 define `checkpoint_count` and `hike_legs` as checkpoint counts, including zero. "
        "A missing stage has no usable count, so line 1346 records `not defined` rather than 0. "
        "Rust and Go record 0 for every `checkpoint_count`. "
        "Go records 0 for `whole_hike_legs` and `hike_legs`. "
        "The others record `not defined`.",
        "",
        "`shape`, `shape_item`, and the exit code are not masked. "
        "A malformed cell is pending only when every verifier exits 0, `shape` is `mismatch`, "
        '`shape_item` is `["malformed cbor"]`, and every other byte of the records matches.',
        "",
    ]
    if tables:
        lines.extend(tables)
    else:
        lines.append("No malformed cell is pending. A difference outside the mask is a failed cell.")
        lines.append("")
    listed = ", ".join(f"`{name}`" for name in names) if names else "(none)"
    lines.append(f"Pending cells: {listed}.")
    lines.append("")
    return "\n".join(lines)


class Gate:
    def __init__(self, args: argparse.Namespace):
        self.args = args
        self.failures: list[str] = []
        self.notes: list[str] = []
        self.matrix_lines: list[str] = []
        self.part: dict[str, str] = {}
        self.cells_run = 0
        self.cells_match = 0
        self.pending_count = 0
        self.pending_names: list[str] = []
        self.pending_section = ""
        self.invocations = 0
        self.docs = {h: cases_document(h) for h in range(989, 1019)}
        self.env = os.environ.copy()
        self.env["GOMAXPROCS"] = "1"

    def fail(self, cell: str) -> None:
        self.failures.append(cell)

    def sdk_object(self, lang: str, name: str) -> str:
        return os.path.join(self.args.input_root, lang, "objects", name)

    def sdk_candidate(self, lang: str, name: str) -> str:
        return os.path.join(self.args.input_root, lang, "candidates", name)

    def matrix_cmd(self, lang: str, obj: str, profile: list[str]) -> list[str]:
        draw = ["--draw-label", DRAW]
        if lang == "java":
            return [
                self.args.java_cli,
                "verify",
                *profile,
                "--index",
                self.index_dir,
                *draw,
                obj,
            ]
        binary = {"rust": self.args.rust_drv, "c": self.args.c_drv, "go": self.args.go_drv}[lang]
        return [binary, "--index-url", self.index_url, *profile, *draw, obj]

    def cli_cmd(self, lang: str, extra: list[str], obj: str) -> list[str]:
        binary = {
            "rust": self.args.rust_cli,
            "c": self.args.c_cli,
            "go": self.args.go_cli,
            "java": self.args.java_cli,
        }[lang]
        return [binary, "verify", *extra, obj]

    def verify_matrix(self, lang: str, obj: str, obj_name: str) -> tuple[int, bytes | None, bytes]:
        timeout = 2700 if obj_name == "O3-PROD.cbor" else 300
        code, out, err = run_cmd(
            self.matrix_cmd(lang, obj, PROFILES[obj_name]),
            env=self.env,
            timeout=timeout,
        )
        self.invocations += 1
        return code, one_record(out), err

    def check_pins_and_sources(self) -> dict[str, list[tuple[str, str, str]]]:
        """Return object name -> list of (source, path, sha)."""
        groups: dict[str, list[tuple[str, str, str]]] = {name: [] for name in PINS}
        origins: list[tuple[str, str]] = [(lang, self.args.input_root) for lang in LANGS]
        origins.extend(self.ref_roots)
        for name in PINS:
            for source, root in origins:
                if source in LANGS and root == self.args.input_root:
                    path = self.sdk_object(source, name)
                    label = f"{source}/{name[:-5]}"
                else:
                    obj_dir = find_named(root, "objects")
                    path = os.path.join(obj_dir, name) if obj_dir else ""
                    label = f"{source}/{name[:-5]}"
                if not path or not os.path.isfile(path):
                    self.fail(f"cell pin:{label}: object file missing")
                    continue
                data = open(path, "rb").read()
                digest = sha256(data)
                if digest != PINS[name]:
                    self.fail(f"cell pin:{label}: sha256 {digest} != {PINS[name]}")
                groups[name].append((label, path, digest))
        return groups

    def check_prod_leg(self) -> None:
        origins = [(lang, self.sdk_candidate(lang, "prod.txt")) for lang in LANGS]
        for source, root in self.ref_roots:
            cand = find_named(root, "candidates")
            if cand:
                origins.append((source, os.path.join(cand, "prod.txt")))
        for source, path in origins:
            cell = f"cell pin:{source}/prod.PROD.stage.0.leg.0.legs_le_hex"
            if not os.path.isfile(path):
                self.fail(f"{cell}: prod.txt missing")
                continue
            try:
                rows = load_tsv(path)
            except ValueError as exc:
                self.fail(f"{cell}: {exc}")
                continue
            got = rows.get("prod.PROD.stage.0.leg.0.legs_le_hex")
            if got != PROD_LEG:
                self.fail(f"{cell}: {got}")

    def check_candidates(self) -> None:
        lines = ["## Candidates", ""]
        for name in CANDIDATE_FILES:
            sdk_rows: dict[str, dict[str, str] | None] = {}
            sdk_bytes: dict[str, bytes | None] = {}
            for lang in LANGS:
                path = self.sdk_candidate(lang, name)
                try:
                    sdk_bytes[lang] = open(path, "rb").read()
                    sdk_rows[lang] = load_tsv(path)
                except (OSError, ValueError) as exc:
                    sdk_bytes[lang] = None
                    sdk_rows[lang] = None
                    self.fail(f"cell candidates:{lang}/{name}: {exc}")
            present = [b for b in sdk_bytes.values() if b is not None]
            if present and any(b != present[0] for b in present):
                # Name the first pair that differs and the first key.
                base_lang = next(lang for lang in LANGS if sdk_bytes[lang] is not None)
                for lang in LANGS:
                    if sdk_bytes[lang] is None or sdk_bytes[lang] == sdk_bytes[base_lang]:
                        continue
                    key = first_key(
                        sdk_rows[base_lang],
                        sdk_rows[lang],
                        "unreadable",
                        "unreadable",
                    )
                    self.fail(
                        f"cell candidates:{lang}/{name}: first differing key {key} (against {base_lang})"
                    )
                    break
            if name == "br1.txt" and all(sdk_bytes[lang] is not None for lang in LANGS):
                if all(sdk_bytes[lang] == sdk_bytes["rust"] for lang in LANGS):
                    self.check_br1(sdk_rows["rust"] or {})
            if not self.args.ref_dir:
                if present and all(b == present[0] for b in sdk_bytes.values()):
                    lines.append(f"- `{name}`: the four SDKs are byte-identical")
                continue
            ref_bytes: dict[str, bytes] = {}
            ref_rows: dict[str, dict[str, str] | None] = {}
            for source, root in self.ref_roots:
                path = os.path.join(find_named(root, "candidates") or "", name)
                if not os.path.isfile(path):
                    self.fail(f"cell candidates:{source}/{name}: missing")
                    continue
                data = open(path, "rb").read()
                ref_bytes[source] = data
                try:
                    ref_rows[source] = load_tsv(path)
                except ValueError:
                    ref_rows[source] = None
            groups: dict[bytes, list[str]] = {}
            for source, data in ref_bytes.items():
                groups.setdefault(data, []).append(source)
            if not groups:
                self.fail(f"cell candidates:ref/{name}: no reference file")
                continue
            ranked = sorted(groups.items(), key=lambda item: (-len(item[1]), item[1][0]))
            if len(ranked) > 1 and len(ranked[0][1]) == len(ranked[1][1]):
                self.fail(f"cell candidates:ref/{name}: no single agreed byte string")
                continue
            agreed, holders = ranked[0]
            for data, sources in ranked[1:]:
                key = first_key(ref_rows[holders[0]], ref_rows[sources[0]], "unreadable", "unreadable")
                self.notes.append(
                    f"reference exception `{name}`: {', '.join(sources)} differ from the agreed bytes held by {', '.join(holders)}; first key `{key}`"
                )
            sdk_ok = [b for b in sdk_bytes.values() if b is not None]
            if any(b != agreed for b in sdk_ok):
                for lang in LANGS:
                    if sdk_bytes[lang] is None or sdk_bytes[lang] == agreed:
                        continue
                    key = first_key(ref_rows[holders[0]], sdk_rows[lang], "unreadable", "unreadable")
                    self.fail(
                        f"cell candidates:{lang}/{name}: first differing key {key} (against agreed {holders[0]})"
                    )
            else:
                extra = ""
                if len(ranked) > 1:
                    extra = "; a reference copy differs and is not the agreed bytes"
                lines.append(
                    f"- `{name}`: four SDKs match the agreed reference bytes ({', '.join(holders)}){extra}"
                )
        if not self.args.ref_dir:
            pass
        lines.append("")
        self.part["candidates"] = "\n".join(lines)

    def check_br1(self, rows: dict[str, str]) -> None:
        """SPEC §13.6 rows the four SDKs must agree on. Status is 204 or 402."""
        expect = {
            "current": "204",
            "before": "204",
            "after_test": "204",
            "after_live": "204",
            "after_live_copy": "204",
            "outside": "402",
            "wrong_origin": "402",
            "display_current": "402",
            "display_before": "402",
            "display_after": "402",
            "wrong_nbits_test": "402",
            "wrong_nbits_live": "402",
        }
        planes = {
            "after_test": "test",
            "after_live": "live",
            "after_live_copy": "live",
            "wrong_nbits_test": "test",
            "wrong_nbits_live": "live",
        }
        for case, status in expect.items():
            got = rows.get(f"br1.{case}.status")
            if got != status:
                self.fail(f"cell br1:{case}: status {got} != {status}")
            if case in planes:
                plane = rows.get(f"br1.{case}.plane")
                if plane != planes[case]:
                    self.fail(f"cell br1:{case}: plane {plane} != {planes[case]}")

    def check_negatives(self, table: dict[str, str]) -> tuple[list[str], dict[str, str]]:
        lines = ["## Negatives", ""]
        leg_names = []
        terrain_names = []
        for key in table:
            if key == "honest.legs_le_hex":
                continue
            if key.endswith(".legs_le_hex"):
                leg_names.append(key[: -len(".legs_le_hex")])
            elif key.endswith(".terrain_sha256") and key != "honest.terrain_sha256":
                terrain_names.append(key[: -len(".terrain_sha256")])
        if len(leg_names) != 10 or len(terrain_names) != 3:
            self.fail(
                f"cell negatives:erratum: table has {len(leg_names)} leg values and {len(terrain_names)} terrain digests"
            )
        lines.append(
            f"Erratum table: {len(leg_names)} leg values, {len(terrain_names)} terrain digests, plus the honest pair."
        )
        leg_values = [table[name + ".legs_le_hex"] for name in leg_names]
        terrain_values = [table[name + ".terrain_sha256"] for name in terrain_names]
        if len(set(leg_values)) != len(leg_values):
            self.fail("cell negatives:erratum: leg values are not pairwise distinct")
        if len(set(terrain_values)) != len(terrain_values):
            self.fail("cell negatives:erratum: terrain digests are not pairwise distinct")
        if table["honest.legs_le_hex"] in leg_values:
            self.fail("cell negatives:erratum: a leg value equals honest")
        if table["honest.terrain_sha256"] in terrain_values:
            self.fail("cell negatives:erratum: a terrain digest equals honest")
        for lang in LANGS:
            path = self.sdk_candidate(lang, "negatives.txt")
            try:
                rows = load_tsv(path)
            except (OSError, ValueError) as exc:
                self.fail(f"cell negatives:{lang}/negatives.txt: {exc}")
                continue
            for key, val in table.items():
                got = rows.get("negatives." + key)
                if got != val:
                    self.fail(
                        f"cell negatives:{lang}/negatives.{key}: {got} != erratum {val}"
                    )
            for name in leg_names + terrain_names:
                flag = f"negatives.{name}.matches_honest"
                if rows.get(flag) != "false":
                    self.fail(f"cell negatives:{lang}/{flag}: {rows.get(flag)!r}")
            got_legs = [rows.get(f"negatives.{name}.legs_le_hex") for name in leg_names]
            got_terrain = [rows.get(f"negatives.{name}.terrain_sha256") for name in terrain_names]
            if len(set(got_legs)) != len(leg_names):
                self.fail(f"cell negatives:{lang}: leg values are not pairwise distinct")
            if len(set(got_terrain)) != len(terrain_names):
                self.fail(f"cell negatives:{lang}: terrain digests are not pairwise distinct")
        if not any(item.startswith("cell negatives:") for item in self.failures):
            lines.append(
                "Each SDK `negatives.txt` matches the erratum table, every `matches_honest` is false, the 10 leg values are pairwise distinct, and the 3 terrain digests are pairwise distinct."
            )
        lines.append("")
        self.part["negatives"] = "\n".join(lines)
        return leg_names, {name: table[name + ".legs_le_hex"] for name in leg_names}

    def pinned_o1(self) -> bytes | None:
        for lang in LANGS:
            path = self.sdk_object(lang, "O1-S1.cbor")
            if not os.path.isfile(path):
                continue
            data = open(path, "rb").read()
            if sha256(data) == PINS["O1-S1.cbor"]:
                return data
        return None

    def six_leg_negatives(self, leg_hex: dict[str, str], base: bytes) -> None:
        lines = [
            "### Six-leg negative cells",
            "",
            "Each object is the pinned O1-S1 with every `leg_results` byte string replaced by that negative's 8 wire bytes. Checkpoints are unchanged. The harness does not call an SDK negative module.",
            "",
        ]
        try:
            probe = bytearray(base)
            n = replace_leg_results(probe, bytes.fromhex(next(iter(leg_hex.values()))))
        except CborError as exc:
            self.fail(f"cell negatives:splice: {exc}")
            self.part["six"] = "\n".join(lines)
            return
        if n != 18:
            self.fail(f"cell negatives:splice: replaced {n} leg results, want 18")
        for name, hx in leg_hex.items():
            obj = bytearray(base)
            try:
                replace_leg_results(obj, bytes.fromhex(hx))
            except CborError as exc:
                self.fail(f"cell negatives:{name}: {exc}")
                continue
            fd, path = tempfile.mkstemp(prefix=f"xverify-{name}-", suffix=".cbor")
            os.write(fd, obj)
            os.close(fd)
            try:
                for lang in LANGS:
                    self.cells_run += 1
                    code, rec, err = self.verify_matrix(lang, path, "O1-S1.cbor")
                    cell = f"cell negatives:{name} × {lang}"
                    if code != 0 or rec is None:
                        self.fail(f"{cell}: exit {code} stderr {snippet(err)}")
                        continue
                    try:
                        parsed = json.loads(rec)
                    except json.JSONDecodeError:
                        self.fail(f"{cell}: record is not JSON")
                        continue
                    results = parsed.get("six_leg", {}).get("results") if isinstance(parsed.get("six_leg"), dict) else None
                    if results == ["mismatch"] * 6:
                        self.cells_match += 1
                    else:
                        self.fail(f"{cell}: six_leg results {results!r}")
            finally:
                os.unlink(path)
        if not any(item.startswith("cell negatives:") and "×" in item for item in self.failures):
            lines.append(
                f"{len(leg_hex) * len(LANGS)} cells, each six-leg result is mismatch on all six drawn legs."
            )
        lines.append("")
        self.part["six"] = "\n".join(lines)

    def _compare_malformed_erratum2(
        self,
        label: str,
        outputs: dict[str, tuple[int, bytes, bytes]],
        lines: list[str],
    ) -> str:
        """Compare one malformed-CBOR input. Mask only PENDING_ERRATUM_2."""
        cell = f"cell malformed:{label}"
        if any(outputs[lang][0] != 0 for lang in LANGS):
            shown = ", ".join(f"{lang} {outputs[lang][0]}" for lang in LANGS)
            self.fail(f"{cell}: exit {shown}")
            return ""
        parsed: dict[str, dict] = {}
        records: dict[str, bytes] = {}
        for lang in LANGS:
            rec = one_record(outputs[lang][1])
            if rec is None:
                self.fail(
                    f"{cell}: {lang} did not write one record; stdout {snippet(outputs[lang][1])}"
                )
                return ""
            try:
                obj = json.loads(rec)
            except json.JSONDecodeError:
                self.fail(f"{cell}: {lang} record is not JSON")
                return ""
            if obj.get("shape") != "mismatch" or obj.get("shape_item") != ["malformed cbor"]:
                self.fail(
                    f"{cell}: {lang} shape {obj.get('shape')!r} "
                    f"shape_item {obj.get('shape_item')!r}"
                )
                return ""
            parsed[lang] = obj
            records[lang] = rec
        masked: dict[str, bytes] = {}
        for lang in LANGS:
            try:
                masked[lang] = mask_pending_erratum_2(records[lang])
            except ValueError as exc:
                self.fail(f"{cell}: {lang} record could not be masked ({exc})")
                return ""
        parts = []
        base = masked["rust"]
        for lang in LANGS:
            if lang != "rust" and masked[lang] != base:
                parts.append(f"rust vs {lang}: {diff_at(base, masked[lang])}")
        if masked["go"] != masked["java"]:
            parts.append(f"go vs java: {diff_at(masked['go'], masked['java'])}")
        if parts:
            self.fail(f"{cell}: outside PENDING_ERRATUM_2; " + "; ".join(parts))
            return ""
        raw_same = all(records[lang] == records["rust"] for lang in LANGS)
        if raw_same:
            digest = sha256(records["rust"] + b"\n")
            if digest != MALFORMED_RECORD_SHA256:
                self.fail(f"{cell}: record sha256 {digest} != {MALFORMED_RECORD_SHA256}")
                return ""
            lines.append(
                f"- `{label}`: exit 0, identical record, `malformed cbor`, "
                f"sha256 `{MALFORMED_RECORD_SHA256}` (erratum 2 E02)"
            )
            return ""
        self.pending_count += 1
        self.pending_names.append(f"malformed {label}")
        lines.append(
            f"- `{label}`: pending erratum 2; exit 0; shape `mismatch`; "
            'shape_item `["malformed cbor"]`'
        )
        return erratum2_field_table(label, parsed)

    def check_malformed(self, base_o1: bytes | None) -> None:
        lines = ["## Malformed and unknown kind", ""]
        work = tempfile.mkdtemp(prefix="xverify-malformed-")
        try:
            cases: list[tuple[str, bytes, bytes | None]] = []
            empty = os.path.join(work, "empty.cbor")
            open(empty, "wb").close()
            cases.append(("empty", b"", b"malformed"))
            trunc = os.path.join(work, "trunc.cbor")
            open(trunc, "wb").write(TRUNCATED)
            cases.append(("truncated", TRUNCATED, b"malformed"))
            unknown_rows = None
            cand = self.sdk_candidate("rust", "unknown-kind.txt")
            try:
                unknown_rows = load_tsv(cand)
            except (OSError, ValueError) as exc:
                self.fail(f"cell malformed:unknown-kind.txt: {exc}")
            if base_o1 is not None and unknown_rows is not None:
                n = 0
                while f"unknown-kind.{n}.kind_text_hex" in unknown_rows:
                    text = bytes.fromhex(unknown_rows[f"unknown-kind.{n}.kind_text_hex"]).decode("utf-8")
                    expect = bytes.fromhex(unknown_rows[f"unknown-kind.{n}.record_hex"])
                    blob = replace_kind(base_o1, text)
                    path = os.path.join(work, f"kind-{n}.cbor")
                    open(path, "wb").write(blob)
                    cases.append((f"unknown-kind.{n}", blob, expect))
                    n += 1
                if n != 4:
                    self.fail(f"cell malformed:unknown-kind: found {n} labels, want 4")
            outputs: dict[str, dict[str, tuple[int, bytes, bytes]]] = {}
            for label, _blob, _expect in cases:
                outputs[label] = {}
            # Re-open paths. empty and trunc already written. unknown written above.
            paths = {"empty": empty, "truncated": trunc}
            for label, _blob, _expect in cases:
                if label.startswith("unknown-kind."):
                    n = label.rsplit(".", 1)[1]
                    paths[label] = os.path.join(work, f"kind-{n}.cbor")
            pending_tables: list[str] = []
            for label, _blob, expect in cases:
                for lang in LANGS:
                    code, out, err = run_cmd(self.cli_cmd(lang, [], paths[label]), env=self.env, timeout=60)
                    outputs[label][lang] = (code, out, err)
                if label in ("empty", "truncated"):
                    table = self._compare_malformed_erratum2(label, outputs[label], lines)
                    if table:
                        pending_tables.append(table)
                    continue
                codes = {outputs[label][lang][0] for lang in LANGS}
                stdouts = {outputs[label][lang][1] for lang in LANGS}
                cell = f"cell malformed:{label}"
                if len(codes) != 1 or len(stdouts) != 1:
                    parts = []
                    if len(codes) != 1:
                        parts.append(
                            "exit "
                            + ", ".join(f"{lang} {outputs[label][lang][0]}" for lang in LANGS)
                        )
                    base = outputs[label]["rust"][1]
                    for lang in LANGS:
                        if lang == "rust":
                            continue
                        other = outputs[label][lang][1]
                        if other != base:
                            parts.append(f"rust vs {lang}: {diff_at(base, other)}")
                    if outputs[label]["go"][1] != outputs[label]["java"][1]:
                        parts.append(
                            "go vs java: "
                            + diff_at(outputs[label]["go"][1], outputs[label]["java"][1])
                        )
                    self.fail(cell + ": " + "; ".join(parts))
                    continue
                code = next(iter(codes))
                stdout = next(iter(stdouts))
                rec = one_record(stdout)
                if code != 0 or rec != expect:
                    shown = diff_at(rec or b"", expect or b"") if rec is not None else snippet(stdout)
                    self.fail(f"{cell}: exit {code}; {shown}")
                else:
                    lines.append(f"- `{label}`: exit 0, identical Rule A5 record")
            lines.append("")
            utf_path = os.path.join(work, "utf8.cbor")
            open(utf_path, "wb").write(BAD_UTF8)
            # Erratum 2 E01: any text string (map key or `kind`) that is not valid UTF-8 is malformed CBOR.
            # The input is a one-pair map whose `kind` text is the byte 0xFF.
            utf_out: dict[str, tuple[int, bytes, bytes]] = {}
            for lang in LANGS:
                utf_out[lang] = run_cmd(self.cli_cmd(lang, [], utf_path), env=self.env, timeout=60)
            table = self._compare_malformed_erratum2("invalid-utf8-kind", utf_out, lines)
            if table:
                pending_tables.append(table)
            lines.append("")
            if self.pending_names:
                self.pending_section = _pending_erratum2_section(pending_tables, self.pending_names)
        finally:
            for dirpath, _dirnames, filenames in os.walk(work, topdown=False):
                for filename in filenames:
                    os.unlink(os.path.join(dirpath, filename))
            os.rmdir(work)
        self.part["malformed"] = "\n".join(lines)

    def cli_surface(self) -> None:
        lines = ["## CLI surface", ""]
        lines.append(
            "Matrix cells for Rust, C, and Go call the verify entry the CLI calls, because those CLIs do not take an index. Java's CLI takes `--index` as a directory of `{height}.json` files. The directory holds the same bytes as `GET /index/{height}` on the loopback listener. Every matrix command passes `--draw-label pcoc-pack-2.2/draw`."
        )
        lines.append("")
        empty = os.path.join(self.index_dir, "..", "empty-probe.cbor")
        # index_dir is the temp index directory; write the probe beside it.
        probe = os.path.join(os.path.dirname(self.index_dir), "empty-probe.cbor")
        open(probe, "wb").close()
        for lang in LANGS:
            cmd = self.cli_cmd(lang, ["--index", self.index_dir, "--draw-label", DRAW], probe)
            code, out, err = run_cmd(cmd, env=self.env, timeout=60)
            rendered = " ".join(cmd)
            lines.append(f"- `{lang}` command `{rendered}`")
            lines.append(f"  exit {code}; stdout `{snippet(out, 160)}`; stderr `{snippet(err, 200)}`")
        lines.append("")
        self.part["cli"] = "\n".join(lines)

    def run_matrix(self, groups: dict[str, list[tuple[str, str, str]]]) -> None:
        lines = ["## Matrix", ""]
        lines.append(
            "A cell is match when that verifier's section 14 record is byte-identical to every other verifier's record for the same object bytes, `shape` is `match`, every header `index_comparison` is `match` and `T` is an integer, every six-leg result is `match`, and the record contains no `not read`. On these objects the paths are empty, so `a`, `b`, `N`, `S`, and `delta_h` are `not defined`."
        )
        lines.append("")
        lines.append("| source | sha256 | rust | c | go | java |")
        lines.append("|---|---|---|---|---|---|")
        names = ["O1-S1.cbor", "O2-S2.cbor"]
        if self.args.tier == "full":
            names.append("O3-PROD.cbor")
        for name in names:
            by_hash: dict[str, list[tuple[str, str]]] = {}
            for label, path, digest in groups.get(name, []):
                by_hash.setdefault(digest, []).append((label, path))
            for digest, sources in by_hash.items():
                path = sources[0][1]
                per_lang: dict[str, tuple[int, bytes | None, bytes]] = {}
                sys.stderr.write(f"verify {name} {digest[:12]} via {sources[0][0]}\n")
                sys.stderr.flush()
                for lang in LANGS:
                    per_lang[lang] = self.verify_matrix(lang, path, name)
                records = {lang: per_lang[lang][1] for lang in LANGS}
                unique = {rec for rec in records.values() if rec is not None}
                identical = len(unique) == 1 and all(rec is not None for rec in records.values()) and all(
                    per_lang[lang][0] == 0 for lang in LANGS
                )
                reasons_common: list[str] = []
                parsed = None
                if identical:
                    try:
                        parsed = json.loads(next(iter(unique)))
                    except json.JSONDecodeError:
                        reasons_common.append("record is not JSON")
                    else:
                        reasons_common = structural(parsed)
                holders = ", ".join(label for label, _path in sources)
                self.notes.append(
                    f"`{name}` sha256 `{digest}` verified once per verifier; the bytes are identical for {holders}"
                )
                status = {}
                for label, _path in sources:
                    for lang in LANGS:
                        self.cells_run += 1
                        code, rec, err = per_lang[lang]
                        cell = f"cell {lang} × {label}"
                        if code != 0 or rec is None:
                            self.fail(f"{cell}: exit {code} stderr {snippet(err)}")
                            status[lang] = "FAIL"
                            continue
                        if not identical:
                            other = next(
                                (other for other in LANGS if records[other] not in (None, rec)),
                                None,
                            )
                            detail = diff_at(rec, records[other]) if other and records[other] else "record missing"
                            self.fail(f"{cell}: record is not byte-identical ({detail})")
                            status[lang] = "FAIL"
                            continue
                        if reasons_common:
                            self.fail(f"{cell}: " + "; ".join(reasons_common))
                            status[lang] = "FAIL"
                            continue
                        self.cells_match += 1
                        status[lang] = "match"
                lines.append(
                    "| {src} | `{sha}` | {rust} | {c} | {go} | {java} |".format(
                        src=", ".join(label for label, _p in sources),
                        sha=digest,
                        rust=status.get("rust", "FAIL"),
                        c=status.get("c", "FAIL"),
                        go=status.get("go", "FAIL"),
                        java=status.get("java", "FAIL"),
                    )
                )
        lines.append("")
        lines.append(
            "Bookends: on a match cell, `a`, `b`, `N`, `S`, and `delta_h` are `not defined`. Stages 1–17 store the empty path, so those bookends are not defined. Header reads are `match`, so nothing the CASES index can supply is `not read`."
        )
        lines.append("")
        self.part["matrix"] = "\n".join(lines)

    def pins_section(self) -> None:
        fails = [item for item in self.failures if item.startswith("cell pin:")]
        lines = ["## Pins", ""]
        lines.append(
            "O1-S1 `800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246`, "
            "O2-S2 `6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0`, "
            "O3-PROD `b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7`. "
            "`prod.PROD.stage.0.leg.0.legs_le_hex` is `88a04ac5beacb8a7`."
        )
        if fails:
            lines.append("")
            lines.append(f"{len(fails)} pin cells failed. They are listed at the top.")
        else:
            lines.append("")
            lines.append("Every object file matches its pin, and every PROD leg hex matches.")
        lines.append("")
        self.part["pins"] = "\n".join(lines)

    def report(self) -> str:
        result = "pass" if not self.failures else "FAIL"
        pending_note = ""
        if self.pending_count:
            pending_note = f" (pending: {self.pending_count}, not matches)"
        head = [
            f"# Cross-verify ({self.args.tier})",
            "",
            f"result: **{result}**{pending_note}",
            "",
            f"- cells run: {self.cells_run}",
            f"- cells matching: {self.cells_match}",
            f"- pending: {self.pending_count}",
            f"- verifier invocations: {self.invocations}",
            "",
        ]
        if self.failures:
            head.append("## Failed cells")
            head.append("")
            for item in self.failures:
                head.append(f"- {item}")
            head.append("")
        if self.notes:
            head.append("## Identical bytes and reference exceptions")
            head.append("")
            for item in self.notes:
                head.append(f"- {item}")
            head.append("")
        if self.pending_section:
            head.append(self.pending_section)
        head.append(
            "The CASES synthetic index is `GET /index/{height}` for heights 989 through 1018, served on 127.0.0.1 only. Java reads those bodies from `{height}.json`. Rust, C, and Go read them from the listener."
        )
        head.append("")
        order = ("matrix", "pins", "candidates", "negatives", "six", "malformed", "cli")
        body = "\n".join(self.part[name] for name in order if name in self.part)
        return "\n".join(head) + "\n" + body

    def execute(self) -> int:
        self.ref_roots = []
        if self.args.ref_dir:
            for name in sorted(os.listdir(self.args.ref_dir)):
                path = os.path.join(self.args.ref_dir, name)
                if os.path.isdir(path) and (
                    find_named(path, "objects") or find_named(path, "candidates")
                ):
                    self.ref_roots.append((name, path))
            if len(self.ref_roots) < 1:
                self.fail("cell ref-dir: no reference builds")
        with tempfile.TemporaryDirectory(prefix="xverify-index-") as tmp:
            self.index_dir = os.path.join(tmp, "index")
            os.mkdir(self.index_dir)
            for height, body in self.docs.items():
                with open(os.path.join(self.index_dir, f"{height}.json"), "wb") as handle:
                    handle.write(body)
            server = ThreadingHTTPServer(("127.0.0.1", 0), self._handler())
            server.daemon_threads = True
            port = server.server_address[1]
            self.index_url = f"http://127.0.0.1:{port}"
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                with urlopen(self.index_url + "/index/1000", timeout=5) as resp:
                    got = resp.read()
                if got != self.docs[1000]:
                    raise SystemExit("cross-verify: loopback index body is not the CASES document")
                file_body = open(os.path.join(self.index_dir, "1000.json"), "rb").read()
                if file_body != got:
                    raise SystemExit("cross-verify: Java index file is not the loopback body")
                groups = self.check_pins_and_sources()
                self.check_prod_leg()
                self.check_candidates()
                table = parse_erratum(self.args.erratum)
                _names, leg_hex = self.check_negatives(table)
                base = self.pinned_o1()
                if base is None:
                    self.fail("cell negatives:splice: no pinned O1-S1 to build negative objects from")
                elif leg_hex:
                    sys.stderr.write("six-leg negatives\n")
                    sys.stderr.flush()
                    self.six_leg_negatives(leg_hex, base)
                self.run_matrix(groups)
                sys.stderr.write("malformed\n")
                sys.stderr.flush()
                self.check_malformed(base)
                self.cli_surface()
                self.pins_section()
            finally:
                server.shutdown()
        text = self.report()
        sys.stdout.write(text)
        if self.args.out:
            parent = os.path.dirname(os.path.abspath(self.args.out))
            if parent:
                os.makedirs(parent, exist_ok=True)
            with open(self.args.out, "w", encoding="utf-8") as handle:
                handle.write(text)
        return 1 if self.failures else 0

    def _handler(self):
        docs = self.docs

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):  # noqa: N802
                path = posixpath.normpath(self.path.split("?", 1)[0])
                parts = path.split("/")
                if len(parts) == 3 and parts[1] == "index" and parts[2].isdigit() and parts[2] == str(int(parts[2])):
                    body = docs.get(int(parts[2]))
                    if body is not None:
                        self.send_response(200)
                        self.send_header("Content-Type", "application/json")
                        self.send_header("Content-Length", str(len(body)))
                        self.end_headers()
                        self.wfile.write(body)
                        return
                self.send_response(404)
                self.send_header("Content-Length", "0")
                self.end_headers()

            def log_message(self, fmt, *args):
                return

        return Handler


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--tier", required=True, choices=("pr", "full"))
    parser.add_argument("--ref-dir")
    parser.add_argument("--out")
    parser.add_argument("--input-root", required=True)
    parser.add_argument("--erratum", required=True)
    parser.add_argument("--rust-cli", required=True)
    parser.add_argument("--c-cli", required=True)
    parser.add_argument("--go-cli", required=True)
    parser.add_argument("--java-cli", required=True)
    parser.add_argument("--rust-drv", required=True)
    parser.add_argument("--c-drv", required=True)
    parser.add_argument("--go-drv", required=True)
    args = parser.parse_args()
    if args.ref_dir:
        args.ref_dir = os.path.abspath(args.ref_dir)
    return Gate(args).execute()


if __name__ == "__main__":
    sys.exit(main())
