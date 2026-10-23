// Strict JSON reader for SPEC 9 bodies: one value, no BOM, no duplicate member names.
// Integers without fraction or exponent become bigint; other numbers become NonInteger.
export class NonInteger {
  raw: string;
  constructor(raw: string) {
    this.raw = raw;
  }
}
export type Json = null | boolean | string | bigint | NonInteger | Json[] | { [k: string]: Json };

export function parseStrictJson(buf: Uint8Array): Json | undefined {
  if (buf.length >= 3 && buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf) return undefined;
  let text: string;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(buf);
  } catch {
    return undefined;
  }
  let pos = 0;
  const ws = () => {
    while (pos < text.length && ' \t\n\r'.includes(text[pos])) pos++;
  };
  class Bad extends Error {}
  const bad = (): never => {
    throw new Bad();
  };
  function value(depth: number): Json {
    if (depth > 64) bad();
    ws();
    const c = text[pos];
    if (c === '{') {
      pos++;
      const o: { [k: string]: Json } = Object.create(null);
      ws();
      if (text[pos] === '}') {
        pos++;
        return o;
      }
      for (;;) {
        ws();
        if (text[pos] !== '"') bad();
        const k = str();
        if (k in o) bad();
        ws();
        if (text[pos++] !== ':') bad();
        o[k] = value(depth + 1);
        ws();
        const d = text[pos++];
        if (d === '}') return o;
        if (d !== ',') bad();
      }
    }
    if (c === '[') {
      pos++;
      const a: Json[] = [];
      ws();
      if (text[pos] === ']') {
        pos++;
        return a;
      }
      for (;;) {
        a.push(value(depth + 1));
        ws();
        const d = text[pos++];
        if (d === ']') return a;
        if (d !== ',') bad();
      }
    }
    if (c === '"') return str();
    for (const [lit, v] of [['true', true], ['false', false], ['null', null]] as const) {
      if (text.startsWith(lit, pos)) {
        pos += lit.length;
        return v;
      }
    }
    const m = /^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?/.exec(text.slice(pos, pos + 400));
    if (!m) return bad();
    pos += m[0].length;
    return m[2] === undefined && m[3] === undefined ? BigInt(m[0]) : new NonInteger(m[0]);
  }
  function str(): string {
    pos++;
    let out = '';
    for (;;) {
      if (pos >= text.length) bad();
      const c = text[pos++];
      if (c === '"') return out;
      if (c < ' ') bad();
      if (c === '\\') {
        const e = text[pos++];
        const simple: Record<string, string> = { '"': '"', '\\': '\\', '/': '/', b: '\b', f: '\f', n: '\n', r: '\r', t: '\t' };
        if (e in simple) out += simple[e];
        else if (e === 'u') {
          const h = text.slice(pos, pos + 4);
          if (!/^[0-9a-fA-F]{4}$/.test(h)) bad();
          out += String.fromCharCode(parseInt(h, 16));
          pos += 4;
        } else bad();
      } else out += c;
    }
  }
  try {
    const v = value(0);
    ws();
    if (pos !== text.length) return undefined;
    return v;
  } catch (e) {
    if (e instanceof Bad) return undefined;
    throw e;
  }
}
