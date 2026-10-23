"""Strict section 7.3 deterministic CBOR encoder and definite-length decoder."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


class CBORError(ValueError):
    pass


def _head(major: int, value: int) -> bytes:
    if value < 0:
        raise CBORError("negative values are not supported")
    prefix = major << 5
    if value < 24:
        return bytes((prefix | value,))
    if value <= 0xFF:
        return bytes((prefix | 24, value))
    if value <= 0xFFFF:
        return bytes((prefix | 25,)) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes((prefix | 26,)) + value.to_bytes(4, "big")
    if value <= 0xFFFFFFFFFFFFFFFF:
        return bytes((prefix | 27,)) + value.to_bytes(8, "big")
    raise CBORError("integer exceeds u64")


def encode(value: Any) -> bytes:
    if isinstance(value, bool) or value is None:
        raise CBORError("simple values are not permitted")
    if isinstance(value, int):
        return _head(0, value)
    if isinstance(value, bytes):
        return _head(2, len(value)) + value
    if isinstance(value, str):
        raw = value.encode("utf-8")
        return _head(3, len(raw)) + raw
    if isinstance(value, (list, tuple)):
        return _head(4, len(value)) + b"".join(encode(item) for item in value)
    if isinstance(value, dict):
        pairs = [(encode(key), encode(item)) for key, item in value.items()]
        pairs.sort(key=lambda pair: pair[0])
        return _head(5, len(pairs)) + b"".join(k + v for k, v in pairs)
    raise CBORError(f"unsupported CBOR value: {type(value).__name__}")


@dataclass(slots=True)
class _Decoder:
    data: bytes
    pos: int = 0

    def take(self, size: int) -> bytes:
        end = self.pos + size
        if end > len(self.data):
            raise CBORError("truncated CBOR")
        result = self.data[self.pos:end]
        self.pos = end
        return result

    def argument(self, ai: int) -> int:
        if ai < 24:
            return ai
        if ai == 24:
            return self.take(1)[0]
        if ai == 25:
            return int.from_bytes(self.take(2), "big")
        if ai == 26:
            return int.from_bytes(self.take(4), "big")
        if ai == 27:
            return int.from_bytes(self.take(8), "big")
        raise CBORError("indefinite or reserved additional information")

    def item(self) -> Any:
        initial = self.take(1)[0]
        major, ai = initial >> 5, initial & 31
        if major in {6, 7} or major == 1:
            raise CBORError("tags, simple, float, and negative values are forbidden")
        length = self.argument(ai)
        if major == 0:
            return length
        if major == 2:
            return self.take(length)
        if major == 3:
            try:
                return self.take(length).decode("utf-8")
            except UnicodeDecodeError as exc:
                raise CBORError("invalid UTF-8") from exc
        if major == 4:
            return [self.item() for _ in range(length)]
        if major == 5:
            result: dict[str, Any] = {}
            for _ in range(length):
                key = self.item()
                if not isinstance(key, str):
                    raise CBORError("map key is not text")
                if key in result:
                    raise CBORError("duplicate map key")
                result[key] = self.item()
            return result
        raise CBORError("unsupported major type")


def decode(data: bytes) -> Any:
    decoder = _Decoder(data)
    value = decoder.item()
    if decoder.pos != len(data):
        raise CBORError("trailing bytes")
    return value
