export type CborValue = bigint | Uint8Array | string | CborValue[] | Map<string, CborValue>;

export class CborError extends Error {}

function head(major: number, n: bigint | number): number[] {
  const v = BigInt(n);
  if (v < 24n) return [(major << 5) | Number(v)];
  if (v < 0x100n) return [(major << 5) | 24, Number(v)];
  if (v < 0x10000n) return [(major << 5) | 25, Number(v >> 8n), Number(v & 0xffn)];
  if (v < 0x100000000n) return [(major << 5) | 26, ...[24n, 16n, 8n, 0n].map((s) => Number((v >> s) & 0xffn))];
  return [(major << 5) | 27, ...[56n, 48n, 40n, 32n, 24n, 16n, 8n, 0n].map((s) => Number((v >> s) & 0xffn))];
}

/** Core deterministic encoding (RFC 8949 4.2.1) of the value kinds the object uses. */
export function encode(v: CborValue): Buffer {
  if (typeof v === 'bigint') return Buffer.from(head(0, v));
  if (typeof v === 'string') {
    const b = Buffer.from(v, 'utf8');
    return Buffer.concat([Buffer.from(head(3, b.length)), b]);
  }
  if (v instanceof Uint8Array) return Buffer.concat([Buffer.from(head(2, v.length)), v]);
  if (Array.isArray(v)) return Buffer.concat([Buffer.from(head(4, v.length)), ...v.map(encode)]);
  const entries = [...v.entries()].map(([k, val]) => [encode(k), encode(val)] as const);
  entries.sort((a, b) => Buffer.compare(a[0], b[0]));
  return Buffer.concat([Buffer.from(head(5, entries.length)), ...entries.flatMap(([k, val]) => [k, val])]);
}

/** Decodes exactly one definite-length data item (SPEC 7.3). Anything else is malformed. */
export function decode(buf: Uint8Array): CborValue {
  let pos = 0;
  const utf8 = new TextDecoder('utf-8', { fatal: true });
  const fail = (m: string): never => {
    throw new CborError(m);
  };
  function arg(info: number): bigint {
    if (info < 24) return BigInt(info);
    const n = info === 24 ? 1 : info === 25 ? 2 : info === 26 ? 4 : info === 27 ? 8 : fail('indefinite or reserved');
    if (pos + n > buf.length) fail('truncated');
    let v = 0n;
    for (let i = 0; i < n; i++) v = (v << 8n) | BigInt(buf[pos++]);
    return v;
  }
  function item(depth: number): CborValue {
    if (depth > 64) fail('too deep');
    if (pos >= buf.length) fail('truncated');
    const ib = buf[pos++];
    const major = ib >> 5;
    const info = ib & 31;
    switch (major) {
      case 0:
        return arg(info);
      case 2:
      case 3: {
        const len = arg(info);
        if (len > BigInt(buf.length - pos)) fail('truncated');
        const slice = buf.subarray(pos, pos + Number(len));
        pos += Number(len);
        if (major === 2) return Uint8Array.from(slice);
        try {
          return utf8.decode(slice);
        } catch {
          return fail('bad utf8');
        }
      }
      case 4: {
        const n = arg(info);
        if (n > BigInt(buf.length - pos)) fail('truncated');
        const out: CborValue[] = [];
        for (let i = 0; i < Number(n); i++) out.push(item(depth + 1));
        return out;
      }
      case 5: {
        const n = arg(info);
        if (n > BigInt(buf.length - pos)) fail('truncated');
        const m = new Map<string, CborValue>();
        for (let i = 0; i < Number(n); i++) {
          const k = item(depth + 1);
          if (typeof k !== 'string') fail('non-text key');
          if (m.has(k as string)) fail('duplicate key');
          m.set(k as string, item(depth + 1));
        }
        return m;
      }
      default:
        return fail('negative integer, tag, float or simple value');
    }
  }
  const v = item(0);
  if (pos !== buf.length) fail('trailing bytes');
  return v;
}
