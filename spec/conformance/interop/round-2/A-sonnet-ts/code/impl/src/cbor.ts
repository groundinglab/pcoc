// CBOR for the hike object (SPEC §7.3): a strict decoder and the core deterministic encoder.

export type CborValue = bigint | Uint8Array | string | CborValue[] | CborMap;
export type CborMap = Map<string, CborValue>;

export class CborError extends Error {}

/** Nesting deeper than this is treated as malformed (implementation limit, see RESULTS.md). */
const MAX_DEPTH = 512;

const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

class Reader {
  pos = 0;
  constructor(readonly b: Uint8Array) {}
  need(n: number): void {
    if (this.b.length - this.pos < n) throw new CborError('truncated');
  }
}

function readArg(r: Reader, ai: number): bigint {
  if (ai < 24) return BigInt(ai);
  const n = ai === 24 ? 1 : ai === 25 ? 2 : ai === 26 ? 4 : ai === 27 ? 8 : 0;
  if (n === 0) throw new CborError(ai === 31 ? 'indefinite length' : 'reserved additional information');
  r.need(n);
  let v = 0n;
  for (let k = 0; k < n; k++) v = (v << 8n) | BigInt(r.b[r.pos + k]!);
  r.pos += n;
  return v;
}

function readLen(r: Reader, ai: number): number {
  const v = readArg(r, ai);
  if (v > BigInt(r.b.length - r.pos)) throw new CborError('truncated');
  return Number(v);
}

function readItem(r: Reader, depth: number): CborValue {
  if (depth > MAX_DEPTH) throw new CborError('nesting too deep');
  r.need(1);
  const ib = r.b[r.pos++]!;
  const major = ib >> 5;
  const ai = ib & 31;
  switch (major) {
    case 0:
      return readArg(r, ai);
    case 1:
      throw new CborError('negative integer');
    case 2: {
      const n = readLen(r, ai);
      const v = r.b.slice(r.pos, r.pos + n);
      r.pos += n;
      return v;
    }
    case 3: {
      const n = readLen(r, ai);
      let s: string;
      try {
        s = utf8.decode(r.b.subarray(r.pos, r.pos + n));
      } catch {
        throw new CborError('text is not UTF-8');
      }
      r.pos += n;
      return s;
    }
    case 4: {
      const n = readLen(r, ai);
      const out: CborValue[] = [];
      for (let k = 0; k < n; k++) out.push(readItem(r, depth + 1));
      return out;
    }
    case 5: {
      const n = readLen(r, ai);
      const m: CborMap = new Map();
      for (let k = 0; k < n; k++) {
        r.need(1);
        if (r.b[r.pos]! >> 5 !== 3) throw new CborError('map key is not a text string');
        const key = readItem(r, depth + 1) as string;
        if (m.has(key)) throw new CborError('duplicate key');
        m.set(key, readItem(r, depth + 1));
      }
      return m;
    }
    case 6:
      throw new CborError('tag');
    default:
      throw new CborError('float or simple value');
  }
}

/** Decodes exactly one definite-length data item under §7.3. Throws CborError for a malformed CBOR value. */
export function decodeCbor(bytes: Uint8Array): CborValue {
  const r = new Reader(bytes);
  const v = readItem(r, 0);
  if (r.pos !== bytes.length) throw new CborError('bytes after the data item');
  return v;
}

// ---- encoder ----------------------------------------------------------------------------------

function head(major: number, n: bigint | number): Uint8Array {
  const v = BigInt(n);
  const m = major << 5;
  if (v < 24n) return Uint8Array.of(m | Number(v));
  if (v < 0x100n) return Uint8Array.of(m | 24, Number(v));
  if (v < 0x10000n) return Uint8Array.of(m | 25, Number(v >> 8n), Number(v & 255n));
  if (v < 0x100000000n) {
    return Uint8Array.of(m | 26, Number(v >> 24n), Number((v >> 16n) & 255n), Number((v >> 8n) & 255n), Number(v & 255n));
  }
  if (v >= 1n << 64n || v < 0n) throw new RangeError('unsigned integer out of range');
  const o = new Uint8Array(9);
  o[0] = m | 27;
  for (let k = 0; k < 8; k++) o[1 + k] = Number((v >> BigInt(56 - 8 * k)) & 255n);
  return o;
}

const textEnc = new TextEncoder();

export type Encodable = number | bigint | Uint8Array | string | Encodable[] | Map<string, Encodable>;

function concat(parts: Uint8Array[]): Uint8Array {
  let n = 0;
  for (const p of parts) n += p.length;
  const out = new Uint8Array(n);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

/** Core deterministic encoding (RFC 8949 §4.2.1): shortest heads, definite lengths, bytewise-sorted map keys. */
export function encodeCbor(v: Encodable): Uint8Array {
  if (typeof v === 'number') {
    if (!Number.isSafeInteger(v) || v < 0) throw new RangeError('only unsigned integers are encoded');
    return head(0, v);
  }
  if (typeof v === 'bigint') return head(0, v);
  if (typeof v === 'string') {
    const t = textEnc.encode(v);
    return concat([head(3, t.length), t]);
  }
  if (v instanceof Uint8Array) return concat([head(2, v.length), v]);
  if (Array.isArray(v)) return concat([head(4, v.length), ...v.map(encodeCbor)]);
  const entries = [...v.entries()].map(([k, val]) => ({ k: encodeCbor(k), v: encodeCbor(val) }));
  entries.sort((a, b) => Buffer.compare(a.k, b.k));
  return concat([head(5, entries.length), ...entries.flatMap((e) => [e.k, e.v])]);
}
