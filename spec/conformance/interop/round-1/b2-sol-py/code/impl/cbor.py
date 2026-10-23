"""Minimal strict RFC 8949 codec for the SPEC §§7–8 object."""
from __future__ import annotations

from typing import Any


class DecodeError(ValueError):
    pass


def _head(major: int, n: int) -> bytes:
    if n < 24: return bytes([(major << 5) | n])
    if n < 256: return bytes([(major << 5) | 24, n])
    if n < 65536: return bytes([(major << 5) | 25]) + n.to_bytes(2, "big")
    if n < 2**32: return bytes([(major << 5) | 26]) + n.to_bytes(4, "big")
    if n < 2**64: return bytes([(major << 5) | 27]) + n.to_bytes(8, "big")
    raise ValueError("integer too large")


def encode(value: Any) -> bytes:
    if type(value) is int and value >= 0:
        return _head(0, value)
    if type(value) is bytes:
        return _head(2, len(value)) + value
    if type(value) is str:
        b = value.encode("utf-8")
        return _head(3, len(b)) + b
    if type(value) is list:
        return _head(4, len(value)) + b"".join(encode(x) for x in value)
    if type(value) is dict:
        pairs = sorted(((encode(k), encode(v)) for k, v in value.items()), key=lambda p: p[0])
        return _head(5, len(pairs)) + b"".join(k + v for k, v in pairs)
    raise ValueError(f"unsupported CBOR type {type(value)}")


def decode(data: bytes) -> Any:
    def item(p: int):
        if p >= len(data): raise DecodeError("truncated")
        initial = data[p]; p += 1
        major, ai = initial >> 5, initial & 31
        if ai < 24: n = ai
        elif ai in (24, 25, 26, 27):
            size = 1 << (ai - 24)
            if p + size > len(data): raise DecodeError("truncated")
            n = int.from_bytes(data[p:p+size], "big"); p += size
        else: raise DecodeError("indefinite/reserved")
        if major == 0: return n, p
        if major in (2, 3):
            if p + n > len(data): raise DecodeError("truncated")
            b = data[p:p+n]; p += n
            if major == 2: return b, p
            try: return b.decode("utf-8"), p
            except UnicodeDecodeError as e: raise DecodeError("utf8") from e
        if major == 4:
            a = []
            for _ in range(n):
                v, p = item(p); a.append(v)
            return a, p
        if major == 5:
            d = {}
            for _ in range(n):
                k, p = item(p)
                if type(k) is not str or k in d: raise DecodeError("map key")
                d[k], p = item(p)
            return d, p
        raise DecodeError("forbidden CBOR type")
    value, end = item(0)
    if end != len(data): raise DecodeError("trailing bytes")
    return value
