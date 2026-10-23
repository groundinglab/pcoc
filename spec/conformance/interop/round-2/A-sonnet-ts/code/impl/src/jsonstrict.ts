// Strict JSON for index documents (SPEC §9): one value, no BOM, no repeated member name at any depth,
// numbers kept as their written form so that "integer, not fraction or exponent" can be decided.

export class JsonNumber {
  constructor(readonly raw: string) {}
  /** An integer literal: no fractional part, no exponent. */
  get isInteger(): boolean {
    return /^-?(0|[1-9][0-9]*)$/.test(this.raw);
  }
  /** The value when the literal is an integer within `lo … hi`; otherwise undefined. */
  intIn(lo: bigint, hi: bigint): bigint | undefined {
    if (!this.isInteger) return undefined;
    const v = BigInt(this.raw);
    return v >= lo && v <= hi ? v : undefined;
  }
}

export type JsonValue = null | boolean | string | JsonNumber | JsonValue[] | Map<string, JsonValue>;

export class JsonError extends Error {}

const MAX_DEPTH = 512;

class P {
  i = 0;
  constructor(readonly s: string) {}
  ws(): void {
    while (this.i < this.s.length) {
      const c = this.s.charCodeAt(this.i);
      if (c === 0x20 || c === 0x09 || c === 0x0a || c === 0x0d) this.i++;
      else break;
    }
  }
  fail(m: string): never {
    throw new JsonError(`${m} at ${this.i}`);
  }
  value(depth: number): JsonValue {
    if (depth > MAX_DEPTH) this.fail('nesting too deep');
    this.ws();
    const c = this.s[this.i];
    if (c === '{') return this.object(depth);
    if (c === '[') return this.array(depth);
    if (c === '"') return this.string();
    if (c === '-' || (c !== undefined && c >= '0' && c <= '9')) return this.number();
    for (const [lit, v] of [
      ['true', true],
      ['false', false],
      ['null', null],
    ] as const) {
      if (this.s.startsWith(lit, this.i)) {
        this.i += lit.length;
        return v;
      }
    }
    return this.fail('unexpected token');
  }
  object(depth: number): JsonValue {
    this.i++;
    const m = new Map<string, JsonValue>();
    this.ws();
    if (this.s[this.i] === '}') {
      this.i++;
      return m;
    }
    for (;;) {
      this.ws();
      if (this.s[this.i] !== '"') this.fail('member name expected');
      const k = this.string();
      if (m.has(k)) this.fail('repeated member name');
      this.ws();
      if (this.s[this.i++] !== ':') this.fail('colon expected');
      m.set(k, this.value(depth + 1));
      this.ws();
      const c = this.s[this.i++];
      if (c === '}') return m;
      if (c !== ',') this.fail('comma or brace expected');
    }
  }
  array(depth: number): JsonValue {
    this.i++;
    const a: JsonValue[] = [];
    this.ws();
    if (this.s[this.i] === ']') {
      this.i++;
      return a;
    }
    for (;;) {
      a.push(this.value(depth + 1));
      this.ws();
      const c = this.s[this.i++];
      if (c === ']') return a;
      if (c !== ',') this.fail('comma or bracket expected');
    }
  }
  string(): string {
    this.i++;
    let out = '';
    for (;;) {
      if (this.i >= this.s.length) this.fail('unterminated string');
      const c = this.s.charCodeAt(this.i++);
      if (c === 0x22) return out;
      if (c < 0x20) this.fail('control character in string');
      if (c !== 0x5c) {
        out += String.fromCharCode(c);
        continue;
      }
      const e = this.s[this.i++];
      switch (e) {
        case '"':
        case '\\':
        case '/':
          out += e;
          break;
        case 'b':
          out += '\b';
          break;
        case 'f':
          out += '\f';
          break;
        case 'n':
          out += '\n';
          break;
        case 'r':
          out += '\r';
          break;
        case 't':
          out += '\t';
          break;
        case 'u': {
          const h = this.s.slice(this.i, this.i + 4);
          if (!/^[0-9a-fA-F]{4}$/.test(h)) this.fail('bad unicode escape');
          out += String.fromCharCode(parseInt(h, 16));
          this.i += 4;
          break;
        }
        default:
          this.fail('bad escape');
      }
    }
  }
  number(): JsonValue {
    const m = /^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?/.exec(this.s.slice(this.i, this.i + 400));
    if (!m) this.fail('bad number');
    this.i += m[0].length;
    return new JsonNumber(m[0]);
  }
}

export function parseStrictJson(text: string): JsonValue {
  const p = new P(text);
  const v = p.value(0);
  p.ws();
  if (p.i !== text.length) p.fail('second value or trailing characters');
  return v;
}

const utf8 = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

/** §9 successful body: UTF-8 (no BOM, not stripped), one JSON object, no repeated member name. Undefined otherwise. */
export function parseIndexBody(body: Uint8Array): Map<string, JsonValue> | undefined {
  if (body.length >= 3 && body[0] === 0xef && body[1] === 0xbb && body[2] === 0xbf) return undefined;
  let text: string;
  try {
    text = utf8.decode(body);
  } catch {
    return undefined;
  }
  try {
    const v = parseStrictJson(text);
    return v instanceof Map ? v : undefined;
  } catch (e) {
    if (e instanceof JsonError) return undefined;
    throw e;
  }
}
