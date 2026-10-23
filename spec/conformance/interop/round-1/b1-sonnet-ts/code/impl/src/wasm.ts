// Hand-assembled WebAssembly kernel. It exists only so that the full-slot fold of
// SPEC 11.1 and the dag2 fill of SPEC 10.4 run on native i64 arithmetic.
// Nothing here caches, tabulates or shortens any read.

const I32 = 0x7f;
const I64 = 0x7e;

function uleb(n: number): number[] {
  const out: number[] = [];
  do {
    let b = n & 0x7f;
    n = Math.floor(n / 128);
    if (n > 0) b |= 0x80;
    out.push(b);
  } while (n > 0);
  return out;
}

function sleb(v: bigint): number[] {
  const out: number[] = [];
  for (;;) {
    const b = Number(v & 0x7fn);
    v >>= 7n;
    if ((v === 0n && (b & 0x40) === 0) || (v === -1n && (b & 0x40) !== 0)) {
      out.push(b);
      return out;
    }
    out.push(b | 0x80);
  }
}

const i64c = (v: bigint) => [0x42, ...sleb(BigInt.asIntN(64, v))];
const i32c = (v: number) => [0x41, ...sleb(BigInt(v))];
const get = (i: number) => [0x20, ...uleb(i)];
const set = (i: number) => [0x21, ...uleb(i)];
const load64 = (off = 0) => [0x29, 3, ...uleb(off)];
const store64 = () => [0x37, 3, 0];

const OP = {
  i64add: 0x7c, i64mul: 0x7e, i64xor: 0x85, i64shl: 0x86, i64shru: 0x88, i64rotl: 0x89, i64remu: 0x82,
  i32add: 0x6a, i32mul: 0x6c, i32and: 0x71, i32shl: 0x74, i32ltu: 0x49, i32wrap: 0xa7, i64extu: 0xad,
};

const MIX_ADD = 0x9e3779b97f4a7c15n;
const MIX_MUL1 = 0xbf58476d1ce4e5b9n;
const MIX_MUL2 = 0x94d049bb133111ebn;
const WORD_XOR = 0xc0ffeen;
const PARENT_MUL = 0xd1b54a32d192ed03n;

// value x on the stack -> mix64(x) on the stack; y is a scratch i64 local
function mix(y: number): number[] {
  return [
    ...i64c(MIX_ADD), OP.i64add, ...set(y),
    ...get(y), ...get(y), ...i64c(30n), OP.i64shru, OP.i64xor, ...i64c(MIX_MUL1), OP.i64mul, ...set(y),
    ...get(y), ...get(y), ...i64c(27n), OP.i64shru, OP.i64xor, ...i64c(MIX_MUL2), OP.i64mul, ...set(y),
    ...get(y), ...get(y), ...i64c(31n), OP.i64shru, OP.i64xor,
  ];
}

function fillSlotBody(): number[] {
  // params: 0 i(i64) 1 wps 2 par1 3 par2 4 par3 5 hasParents 6 cur 7 lane 8 slotmix(i64)
  // locals: 9 j(i32) 10 jj(i64) 11 t(i64) 12 y(i64) 13 c(i64) 14 pw(i64) 15 a(i32)
  const [P_I, WPS, PAR1, , , HASP, CUR, LANE, SLOTMIX, J, JJ, T, Y, C, PW, A] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
  const parIdx = [PAR1, PAR1 + 1, PAR1 + 2];
  const code: number[] = [];
  code.push(...i32c(0), ...set(J));
  code.push(0x02, 0x40, 0x03, 0x40);
  code.push(...get(J), OP.i64extu, ...set(JJ));
  code.push(...get(P_I), ...i64c(32n), OP.i64shl, ...get(JJ), OP.i64xor, ...i64c(WORD_XOR), OP.i64xor, ...set(C));
  code.push(...get(LANE), ...get(J), ...i32c(3), OP.i32and, ...i32c(3), OP.i32shl, OP.i32add, ...set(A));
  code.push(...get(A), ...load64(0), ...get(C), OP.i64xor, ...mix(Y), ...set(T));
  const rots = [17n, 31n, 47n];
  for (let k = 0; k < 3; k++) {
    code.push(...get(T), ...get(A), ...load64(8 * (k + 1)), ...i64c(rots[k]), OP.i64rotl, OP.i64xor, ...mix(Y), ...set(T));
  }
  code.push(...get(HASP), 0x04, 0x40);
  for (let k = 1; k <= 3; k++) {
    code.push(...get(parIdx[k - 1]), ...get(J), ...i32c(3), OP.i32shl, OP.i32add, ...load64(0), ...set(PW));
    code.push(...get(T), ...get(PW), ...get(JJ), ...i64c(BigInt(k)), OP.i64add, OP.i64rotl, OP.i64xor,
      ...i64c((BigInt(k) * PARENT_MUL) & 0xffffffffffffffffn), OP.i64xor, ...mix(Y), ...set(T));
  }
  code.push(...get(T), ...get(SLOTMIX), OP.i64xor, ...mix(Y), ...set(T));
  code.push(0x0b);
  code.push(...get(CUR), ...get(J), ...i32c(3), OP.i32shl, OP.i32add, ...get(T), ...store64());
  code.push(...get(J), ...i32c(1), OP.i32add, ...set(J));
  code.push(...get(J), ...get(WPS), OP.i32ltu, 0x0d, 0);
  code.push(0x0b, 0x0b, 0x0b);
  return code;
}

function legBody(): number[] {
  // params: 0 S(i64) 1 steps(i32) 2 slotBytes(i32) 3 nSlots(i64)
  // locals: 4 n(i32) 5 p(i32) 6 end(i32) 7 acc(i64) 8 y(i64)
  const [S, STEPS, SB, NS, N, P, END, ACC, Y] = [0, 1, 2, 3, 4, 5, 6, 7, 8];
  const code: number[] = [];
  code.push(...i32c(0), ...set(N));
  code.push(0x02, 0x40, 0x03, 0x40);
  code.push(...get(S), ...get(NS), OP.i64remu, OP.i32wrap, ...get(SB), OP.i32mul, ...set(P));
  code.push(...get(P), ...get(SB), OP.i32add, ...set(END));
  code.push(...i64c(0n), ...set(ACC));
  code.push(0x02, 0x40, 0x03, 0x40);
  code.push(...get(ACC), ...get(P), ...load64(0), ...get(S), OP.i64xor, ...mix(Y), OP.i64add, ...set(ACC));
  code.push(...get(P), ...i32c(8), OP.i32add, ...set(P));
  code.push(...get(P), ...get(END), OP.i32ltu, 0x0d, 0);
  code.push(0x0b, 0x0b);
  code.push(...get(ACC), 0x10, 0, ...set(S));
  code.push(...get(N), ...i32c(1), OP.i32add, ...set(N));
  code.push(...get(N), ...get(STEPS), OP.i32ltu, 0x0d, 0);
  code.push(0x0b, 0x0b);
  code.push(...get(S));
  code.push(0x0b);
  return code;
}

const vec = (items: number[][]) => [...uleb(items.length), ...items.flat()];
const section = (id: number, body: number[]) => [id, ...uleb(body.length), ...body];
const str = (s: string) => [...uleb(s.length), ...Buffer.from(s, 'ascii')];
const funcType = (params: number[], results: number[]) => [0x60, ...uleb(params.length), ...params, ...uleb(results.length), ...results];

function fn(locals: [number, number][], body: number[]): number[] {
  const decl = [...uleb(locals.length), ...locals.flatMap(([n, t]) => [...uleb(n), t])];
  const inner = [...decl, ...body];
  return [...uleb(inner.length), ...inner];
}

export function buildKernelModule(): Uint8Array {
  const types = vec([
    funcType([I64], [I64]), // 0 seal
    funcType([I64, I32, I32, I32, I32, I32, I32, I32, I64], []), // 1 fillSlot
    funcType([I64, I32, I32, I64], [I64]), // 2 leg
  ]);
  const imports = vec([
    [...str('env'), ...str('seal'), 0x00, ...uleb(0)],
    [...str('env'), ...str('mem'), 0x02, 0x00, ...uleb(1)],
  ]);
  const funcs = vec([uleb(1), uleb(2)]);
  const exports = vec([
    [...str('fillSlot'), 0x00, ...uleb(1)],
    [...str('leg'), 0x00, ...uleb(2)],
  ]);
  // fillSlot locals from index 9: j(i32), jj t y c pw (i64 x5), a(i32)
  const fillFn = fn([[1, I32], [5, I64], [1, I32]], fillSlotBody());
  // leg locals from index 4: n p end (i32 x3), acc y (i64 x2)
  const legFn = fn([[3, I32], [2, I64]], legBody());
  const codeSec = vec([fillFn, legFn]);
  return Uint8Array.from([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    ...section(1, types), ...section(2, imports), ...section(3, funcs), ...section(7, exports), ...section(10, codeSec),
  ]);
}
