// Minimal WebAssembly binary emitter (MVP + the fixed-width SIMD ops this project needs).
// Kernels are generated from TypeScript so the whole implementation stays in one language.

export type C = number[];
export const T = { i32: 0x7f, i64: 0x7e, v128: 0x7b } as const;
export type ValType = (typeof T)[keyof typeof T];

export function uleb(n: number | bigint): number[] {
  let v = BigInt(n);
  const out: number[] = [];
  do {
    let b = Number(v & 0x7fn);
    v >>= 7n;
    if (v !== 0n) b |= 0x80;
    out.push(b);
  } while (v !== 0n);
  return out;
}

export function sleb(n: number | bigint): number[] {
  let v = BigInt(n);
  const out: number[] = [];
  for (;;) {
    const b = Number(v & 0x7fn);
    v >>= 7n;
    const done = (v === 0n && (b & 0x40) === 0) || (v === -1n && (b & 0x40) !== 0);
    out.push(done ? b : b | 0x80);
    if (done) return out;
  }
}

const bin = (opc: number) => (a: C, b: C): C => [...a, ...b, opc];
const un = (opc: number) => (a: C): C => [...a, opc];
const simd = (opc: number): C => [0xfd, ...uleb(opc)];
const memarg = (align: number, offset: number): C => [align, ...uleb(offset)];

export const i32 = {
  const: (v: number): C => [0x41, ...sleb(v | 0)],
  add: bin(0x6a),
  sub: bin(0x6b),
  mul: bin(0x6c),
  and: bin(0x71),
  or: bin(0x72),
  xor: bin(0x73),
  shl: bin(0x74),
  shrU: bin(0x76),
  rotl: bin(0x77),
  ltU: bin(0x49),
  geU: bin(0x4f),
  eq: bin(0x46),
  ne: bin(0x47),
  eqz: un(0x45),
  remU: bin(0x70),
  load: (addr: C, offset = 0): C => [...addr, 0x28, ...memarg(2, offset)],
  store: (addr: C, val: C, offset = 0): C => [...addr, ...val, 0x36, ...memarg(2, offset)],
  wrapI64: un(0xa7),
};

export const i64 = {
  const: (v: bigint | number): C => [0x42, ...sleb(BigInt.asIntN(64, BigInt(v)))],
  add: bin(0x7c),
  sub: bin(0x7d),
  mul: bin(0x7e),
  remU: bin(0x82),
  and: bin(0x83),
  or: bin(0x84),
  xor: bin(0x85),
  shl: bin(0x86),
  shrU: bin(0x88),
  rotl: bin(0x89),
  eqz: un(0x50),
  ltU: bin(0x54),
  extendU: un(0xad),
  load: (addr: C, offset = 0): C => [...addr, 0x29, ...memarg(0, offset)],
  store: (addr: C, val: C, offset = 0): C => [...addr, ...val, 0x37, ...memarg(0, offset)],
};

export const v128 = {
  load: (addr: C, offset = 0): C => [...addr, ...simd(0x00), ...memarg(0, offset)],
  store: (addr: C, val: C, offset = 0): C => [...addr, ...val, ...simd(0x0b), ...memarg(0, offset)],
  xor: (a: C, b: C): C => [...a, ...b, ...simd(0x51)],
  or: (a: C, b: C): C => [...a, ...b, ...simd(0x50)],
  bitselect: (a: C, b: C, mask: C): C => [...a, ...b, ...mask, ...simd(0x52)],
  splat64: (x: C): C => [...x, ...simd(0x12)],
  extract64: (x: C, lane: number): C => [...x, ...simd(0x1d), lane],
  add64: (a: C, b: C): C => [...a, ...b, ...simd(0xce)],
  mul64: (a: C, b: C): C => [...a, ...b, ...simd(0xd5)],
  shl64: (a: C, n: C): C => [...a, ...n, ...simd(0xcb)],
  shrU64: (a: C, n: C): C => [...a, ...n, ...simd(0xcd)],
  const64x2: (lo: bigint, hi: bigint): C => {
    const b = Buffer.alloc(16);
    b.writeBigUInt64LE(BigInt.asUintN(64, lo), 0);
    b.writeBigUInt64LE(BigInt.asUintN(64, hi), 8);
    return [...simd(0x0c), ...b];
  },
};

export const get = (l: number): C => [0x20, ...uleb(l)];
export const set = (l: number, v: C): C => [...v, 0x21, ...uleb(l)];
export const tee = (l: number, v: C): C => [...v, 0x22, ...uleb(l)];
export const drop = (v: C): C => [...v, 0x1a];
type Part = C | number;
export const call = (f: number, ...args: Part[]): C => [...args.flat(), 0x10, ...uleb(f)];
export const loop = (...body: Part[]): C => [0x03, 0x40, ...body.flat(), 0x0b];
export const block = (...body: Part[]): C => [0x02, 0x40, ...body.flat(), 0x0b];
export const ifThen = (cond: C, ...body: Part[]): C => [...cond, 0x04, 0x40, ...body.flat(), 0x0b];
export const ifElse = (cond: C, thenB: C, elseB: C): C => [...cond, 0x04, 0x40, ...thenB, 0x05, ...elseB, 0x0b];
export const brIf = (depth: number, cond: C): C => [...cond, 0x0d, ...uleb(depth)];
export const br = (depth: number): C => [0x0c, ...uleb(depth)];
export const ret = (v: C = []): C => [...v, 0x0f];

export class Fn {
  readonly locals: ValType[] = [];
  body: C = [];
  constructor(
    readonly name: string,
    readonly params: ValType[],
    readonly results: ValType[],
    readonly exported: boolean,
  ) {}
  local(t: ValType): number {
    this.locals.push(t);
    return this.params.length + this.locals.length - 1;
  }
  emit(...c: Part[]): void {
    for (const x of c) {
      if (typeof x === 'number') this.body.push(x);
      else this.body.push(...x);
    }
  }
}

export class ModuleBuilder {
  readonly fns: Fn[] = [];
  constructor(
    private readonly memPages: number,
    private readonly shared: boolean,
  ) {}
  fn(name: string, params: ValType[], results: ValType[], exported = true): Fn {
    const f = new Fn(name, params, results, exported);
    this.fns.push(f);
    return f;
  }
  index(f: Fn): number {
    return this.fns.indexOf(f);
  }
  build(): Uint8Array {
    const section = (id: number, payload: number[]): number[] => [id, ...uleb(payload.length), ...payload];
    const vec = (items: number[][]): number[] => [...uleb(items.length), ...items.flat()];
    const str = (s: string): number[] => [...uleb(s.length), ...Buffer.from(s, 'ascii')];
    const types: number[][] = [];
    const typeIdx = (f: Fn): number => {
      const t = [0x60, ...uleb(f.params.length), ...f.params, ...uleb(f.results.length), ...f.results];
      const key = t.join(',');
      let k = types.findIndex((x) => x.join(',') === key);
      if (k < 0) {
        types.push(t);
        k = types.length - 1;
      }
      return k;
    };
    const fnTypes = this.fns.map(typeIdx);
    const imports = vec([
      [
        ...str('env'),
        ...str('memory'),
        0x02,
        this.shared ? 0x03 : 0x01,
        ...uleb(this.memPages),
        ...(this.shared ? uleb(this.memPages) : []),
      ],
    ]);
    const exports = vec(
      this.fns
        .map((f, i) => [f, i] as const)
        .filter(([f]) => f.exported)
        .map(([f, i]) => [...str(f.name), 0x00, ...uleb(i)]),
    );
    const codes = this.fns.map((f) => {
      const locals = vec(f.locals.map((t) => [1, t]));
      const body = [...locals, ...f.body, 0x0b];
      return [...uleb(body.length), ...body];
    });
    const out = [
      0x00,
      0x61,
      0x73,
      0x6d,
      0x01,
      0x00,
      0x00,
      0x00,
      ...section(1, vec(types)),
      ...section(2, imports),
      ...section(3, vec(fnTypes.map((t) => uleb(t)))),
      ...section(7, exports),
      ...section(10, vec(codes)),
    ];
    return Uint8Array.from(out);
  }
}
