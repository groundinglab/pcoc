// WebAssembly kernels (generated, see wasm.ts): terrain fill, slot fold, one-block SHA-256 seal.
//
// Memory map (bytes) of the shared memory every Terrain owns:
//   0 .. 65535        256 leg/fill contexts of 256 bytes
//   65536 .. 262143   parent tables (two i32 per slot: bitrev_parent, mul_parent)
//   262144 ..         terrain
// Context layout: +0 state S (u64), +64 seal buffer (SEAL_DOMAIN ‖ le64(acc), 20 bytes), +128 lanes P0..P3.

import {
  MIX_ADD,
  MIX_MUL1,
  MIX_MUL2,
  PARENT_MUL,
  SLOT_MUL,
  WORD_XOR,
} from './profile.js';
import {
  type C,
  Fn,
  ModuleBuilder,
  T,
  block,
  brIf,
  br,
  call,
  get,
  i32,
  i64,
  loop,
  set,
  v128,
} from './wasm.js';

export const CTX_BYTES = 256;
export const MAX_CTX = 256;
export const PARENTS_BASE = 65536;
export const TERRAIN_BASE = 262144;
export const SEAL_OFF = 64;
export const LANES_OFF = 128;

/** Fold kernel shape: `vec` SIMD vectors (2 words each) plus `scalar` scalar words per loop iteration. */
export interface FoldShape {
  vec: number;
  scalar: number;
}
export interface KernelConfig {
  fold: FoldShape;
  /** columns per fill unroll (words); must divide into multiples of 4 */
  fillUnroll: 4 | 8;
}

export const DEFAULT_CONFIG: KernelConfig = { fold: { vec: 0, scalar: 8 }, fillUnroll: 4 };

export function terrainPages(terrainBytes: number): number {
  return Math.ceil((TERRAIN_BASE + terrainBytes) / 65536);
}

// ---- scalar / vector mix64 statement emitters -------------------------------------------------

function mixScalar(ts: number[]): C {
  const out: C = [];
  const stage = (f: (t: number) => C) => {
    for (const t of ts) out.push(...f(t));
  };
  stage((t) => set(t, i64.add(get(t), i64.const(MIX_ADD))));
  stage((t) =>
    set(t, i64.mul(i64.xor(get(t), i64.shrU(get(t), i64.const(30))), i64.const(MIX_MUL1))),
  );
  stage((t) =>
    set(t, i64.mul(i64.xor(get(t), i64.shrU(get(t), i64.const(27))), i64.const(MIX_MUL2))),
  );
  stage((t) => set(t, i64.xor(get(t), i64.shrU(get(t), i64.const(31)))));
  return out;
}

function mixVector(ts: number[]): C {
  const out: C = [];
  const stage = (f: (t: number) => C) => {
    for (const t of ts) out.push(...f(t));
  };
  const addC = v128.const64x2(MIX_ADD, MIX_ADD);
  const m1 = v128.const64x2(MIX_MUL1, MIX_MUL1);
  const m2 = v128.const64x2(MIX_MUL2, MIX_MUL2);
  stage((t) => set(t, v128.add64(get(t), addC)));
  stage((t) => set(t, v128.mul64(v128.xor(get(t), v128.shrU64(get(t), i32.const(30))), m1)));
  stage((t) => set(t, v128.mul64(v128.xor(get(t), v128.shrU64(get(t), i32.const(27))), m2)));
  stage((t) => set(t, v128.xor(get(t), v128.shrU64(get(t), i32.const(31)))));
  return out;
}

// ---- fold -------------------------------------------------------------------------------------

function emitFold(f: Fn, shape: FoldShape): void {
  const [addr, words, S] = [0, 1, 2];
  const p = f.local(T.i32);
  const end = f.local(T.i32);
  const blockEnd = f.local(T.i32);
  const accS = f.local(T.i64);
  const accV = f.local(T.v128);
  const vS = f.local(T.v128);
  const vt = Array.from({ length: shape.vec }, () => f.local(T.v128));
  const st = Array.from({ length: shape.scalar }, () => f.local(T.i64));
  const tail = f.local(T.i64);
  const B = 2 * shape.vec + shape.scalar;

  f.emit(
    set(p, get(addr)),
    set(end, i32.add(get(addr), i32.shl(get(words), i32.const(3)))),
    set(vS, v128.splat64(get(S))),
    set(accS, i64.const(0)),
    set(accV, v128.const64x2(0n, 0n)),
  );
  if (B > 0) {
    f.emit(
      set(blockEnd, i32.sub(get(end), i32.shl(i32.remU(get(words), i32.const(B)), i32.const(3)))),
      block(
        loop(
          brIf(1, i32.geU(get(p), get(blockEnd))),
          ...vt.flatMap((t, v) => set(t, v128.xor(v128.load(get(p), 16 * v), get(vS)))),
          ...st.flatMap((t, s) => set(t, i64.xor(i64.load(get(p), 8 * (2 * shape.vec + s)), get(S)))),
          mixVector(vt),
          mixScalar(st),
          ...vt.flatMap((t) => set(accV, v128.add64(get(accV), get(t)))),
          ...st.flatMap((t) => set(accS, i64.add(get(accS), get(t)))),
          set(p, i32.add(get(p), i32.const(8 * B))),
          br(0),
        ),
      ),
    );
  }
  f.emit(
    block(
      loop(
        brIf(1, i32.geU(get(p), get(end))),
        set(tail, i64.xor(i64.load(get(p)), get(S))),
        mixScalar([tail]),
        set(accS, i64.add(get(accS), get(tail))),
        set(p, i32.add(get(p), i32.const(8))),
        br(0),
      ),
    ),
    ret64(
      i64.add(
        i64.add(get(accS), v128.extract64(get(accV), 0)),
        v128.extract64(get(accV), 1),
      ),
    ),
  );
}
const ret64 = (v: C): C => [...v, 0x0f];

// ---- one-block SHA-256 of SEAL_DOMAIN ‖ le64(acc) -----------------------------------------------

const K256 = [
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

function emitSeal(f: Fn): void {
  const acc = 0;
  const W = Array.from({ length: 16 }, () => f.local(T.i32));
  const V = Array.from({ length: 8 }, () => f.local(T.i32));
  const t1 = f.local(T.i32);
  const t2 = f.local(T.i32);
  const rotr = (x: C, n: number): C => i32.rotl(x, i32.const(32 - n));
  const shr = (x: C, n: number): C => i32.shrU(x, i32.const(n));
  const bswap = (x: C): C =>
    i32.or(
      i32.and(i32.rotl(x, i32.const(8)), i32.const(0x00ff00ff)),
      i32.and(i32.rotl(x, i32.const(24)), i32.const(0xff00ff00)),
    );
  const accLo = f.local(T.i32);
  const accHi = f.local(T.i32);
  f.emit(
    set(accLo, i32.wrapI64(get(acc))),
    set(accHi, i32.wrapI64(i64.shrU(get(acc), i64.const(32)))),
    set(W[0]!, i32.const(0x70636f63)),
    set(W[1]!, i32.const(0x2d77616c)),
    set(W[2]!, i32.const(0x6b2d7631)),
    set(W[3]!, bswap(get(accLo))),
    set(W[4]!, bswap(get(accHi))),
    set(W[5]!, i32.const(0x80000000)),
    ...W.slice(6, 15).flatMap((w) => set(w, i32.const(0))),
    set(W[15]!, i32.const(160)),
  );
  const IV = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
  ];
  V.forEach((v, k) => f.emit(set(v, i32.const(IV[k]!))));
  let v = [...V];
  for (let t = 0; t < 64; t++) {
    const wt = W[t % 16]!;
    if (t >= 16) {
      const w2 = get(W[(t - 2) % 16]!);
      const w15 = get(W[(t - 15) % 16]!);
      f.emit(
        set(
          wt,
          i32.add(
            i32.add(
              i32.xor(i32.xor(rotr(w2, 17), rotr(w2, 19)), shr(w2, 10)),
              get(W[(t - 7) % 16]!),
            ),
            i32.add(i32.xor(i32.xor(rotr(w15, 7), rotr(w15, 18)), shr(w15, 3)), get(wt)),
          ),
        ),
      );
    }
    const [a, b, c, d, e, ff, g, h] = v as [number, number, number, number, number, number, number, number];
    f.emit(
      set(
        t1,
        i32.add(
          i32.add(
            i32.add(get(h), i32.xor(i32.xor(rotr(get(e), 6), rotr(get(e), 11)), rotr(get(e), 25))),
            i32.add(get(wt), i32.const(K256[t]!)),
          ),
          i32.xor(get(g), i32.and(get(e), i32.xor(get(ff), get(g)))),
        ),
      ),
      set(
        t2,
        i32.add(
          i32.xor(i32.xor(rotr(get(a), 2), rotr(get(a), 13)), rotr(get(a), 22)),
          i32.or(i32.and(get(a), get(b)), i32.and(get(c), i32.or(get(a), get(b)))),
        ),
      ),
      set(d, i32.add(get(d), get(t1))),
      set(h, i32.add(get(t1), get(t2))),
    );
    v = [h, a, b, c, d, e, ff, g];
  }
  f.emit(
    ret64(
      i64.or(
        i64.extendU(bswap(i32.add(get(v[0]!), i32.const(IV[0]!)))),
        i64.shl(i64.extendU(bswap(i32.add(get(v[1]!), i32.const(IV[1]!)))), i64.const(32)),
      ),
    ),
  );
}

// ---- terrain fill -----------------------------------------------------------------------------

function emitFillSlot(f: Fn, withParents: boolean, unroll: number): void {
  // params: i, j0, j1, tbase, pbase, ctx, words
  const [I, J0, J1, TBASE, PBASE, CTX, WORDS] = [0, 1, 2, 3, 4, 5, 6];
  const L = [0, 1, 2, 3].map(() => f.local(T.i64));
  const R1 = [0, 1, 2, 3].map(() => f.local(T.i64));
  const R2 = [0, 1, 2, 3].map(() => f.local(T.i64));
  const R3 = [0, 1, 2, 3].map(() => f.local(T.i64));
  const cbase = f.local(T.i64);
  const slotMix = f.local(T.i64);
  const jj = f.local(T.i64);
  const j = f.local(T.i32);
  const outAddr = f.local(T.i32);
  const par = [f.local(T.i32), f.local(T.i32), f.local(T.i32)];
  const t = Array.from({ length: unroll }, () => f.local(T.i64));
  const pw = f.local(T.i64);
  const slotBytes = f.local(T.i32);

  f.emit(
    ...L.flatMap((l, k) => set(l, i64.load(get(CTX), LANES_OFF + 8 * k))),
    ...[0, 1, 2, 3].flatMap((u) => [
      ...set(R1[u]!, i64.rotl(get(L[(u + 1) % 4]!), i64.const(17))),
      ...set(R2[u]!, i64.rotl(get(L[(u + 2) % 4]!), i64.const(31))),
      ...set(R3[u]!, i64.rotl(get(L[(u + 3) % 4]!), i64.const(47))),
    ]),
    set(cbase, i64.xor(i64.shl(i64.extendU(get(I)), i64.const(32)), i64.const(WORD_XOR))),
    set(slotMix, i64.mul(i64.extendU(get(I)), i64.const(SLOT_MUL))),
    set(slotBytes, i32.shl(get(WORDS), i32.const(3))),
    set(j, get(J0)),
    set(
      outAddr,
      i32.add(i32.add(get(TBASE), i32.mul(get(I), get(slotBytes))), i32.shl(get(J0), i32.const(3))),
    ),
  );
  if (withParents) {
    f.emit(
      set(
        par[0]!,
        i32.add(
          i32.add(get(TBASE), i32.mul(i32.sub(get(I), i32.const(1)), get(slotBytes))),
          i32.shl(get(J0), i32.const(3)),
        ),
      ),
      set(
        par[1]!,
        i32.add(
          i32.add(get(TBASE), i32.mul(i32.load(i32.add(get(PBASE), i32.shl(get(I), i32.const(3)))), get(slotBytes))),
          i32.shl(get(J0), i32.const(3)),
        ),
      ),
      set(
        par[2]!,
        i32.add(
          i32.add(
            get(TBASE),
            i32.mul(i32.load(i32.add(get(PBASE), i32.shl(get(I), i32.const(3))), 4), get(slotBytes)),
          ),
          i32.shl(get(J0), i32.const(3)),
        ),
      ),
    );
  }
  const body: C[] = [];
  body.push(set(jj, i64.extendU(get(j))));
  // base word: NL4
  for (let w = 0; w < unroll; w++) {
    const u = w % 4;
    body.push(
      set(t[w]!, i64.xor(get(L[u]!), i64.xor(get(cbase), i64.add(get(jj), i64.const(w))))),
    );
  }
  body.push(mixScalar(t));
  for (let w = 0; w < unroll; w++) body.push(set(t[w]!, i64.xor(get(t[w]!), get(R1[w % 4]!))));
  body.push(mixScalar(t));
  for (let w = 0; w < unroll; w++) body.push(set(t[w]!, i64.xor(get(t[w]!), get(R2[w % 4]!))));
  body.push(mixScalar(t));
  for (let w = 0; w < unroll; w++) body.push(set(t[w]!, i64.xor(get(t[w]!), get(R3[w % 4]!))));
  body.push(mixScalar(t));
  if (withParents) {
    for (let k = 1; k <= 3; k++) {
      const kc = (BigInt(k) * PARENT_MUL) & 0xffffffffffffffffn;
      for (let w = 0; w < unroll; w++) {
        body.push(
          set(pw, i64.load(get(par[k - 1]!), 8 * w)),
          set(
            t[w]!,
            i64.xor(
              i64.xor(get(t[w]!), i64.rotl(get(pw), i64.add(get(jj), i64.const(w + k)))),
              i64.const(kc),
            ),
          ),
        );
      }
      body.push(mixScalar(t));
    }
    for (let w = 0; w < unroll; w++) body.push(set(t[w]!, i64.xor(get(t[w]!), get(slotMix))));
    body.push(mixScalar(t));
  }
  for (let w = 0; w < unroll; w++) body.push(i64.store(get(outAddr), get(t[w]!), 8 * w));
  body.push(set(outAddr, i32.add(get(outAddr), i32.const(8 * unroll))));
  if (withParents) {
    for (const pr of par) body.push(set(pr, i32.add(get(pr), i32.const(8 * unroll))));
  }
  body.push(set(j, i32.add(get(j), i32.const(unroll))));
  f.emit(block(loop(brIf(1, i32.geU(get(j), get(J1))), ...body, br(0))));
}

// ---- steps / legs -----------------------------------------------------------------------------

export interface KernelExports {
  fill_slot0(i: number, j0: number, j1: number, tbase: number, pbase: number, ctx: number, words: number): void;
  fill_slot(i: number, j0: number, j1: number, tbase: number, pbase: number, ctx: number, words: number): void;
  step_hw(ctx: number, tbase: number, nSlots: number, words: number): void;
  leg_sw(ctx: number, tbase: number, nSlots: number, words: number, steps: number): void;
  fold_probe(addr: number, words: number, ctx: number): void;
  seal_probe(ctx: number): void;
}

export function buildKernelBinary(memPages: number, cfg: KernelConfig = DEFAULT_CONFIG): Uint8Array {
  const m = new ModuleBuilder(memPages, true);
  const fold = m.fn('fold', [T.i32, T.i32, T.i64], [T.i64], false);
  emitFold(fold, cfg.fold);
  const seal = m.fn('seal', [T.i64], [T.i64], false);
  emitSeal(seal);

  const P7 = [T.i32, T.i32, T.i32, T.i32, T.i32, T.i32, T.i32];
  emitFillSlot(m.fn('fill_slot0', P7, [], true), false, cfg.fillUnroll);
  emitFillSlot(m.fn('fill_slot', P7, [], true), true, cfg.fillUnroll);

  const foldIdx = m.index(fold);
  const sealIdx = m.index(seal);

  // step_hw(ctx, tbase, nSlots, words): fold one slot, store acc into the seal buffer.
  const sh = m.fn('step_hw', [T.i32, T.i32, T.i32, T.i32], [], true);
  {
    const [ctx, tbase, nSlots, words] = [0, 1, 2, 3];
    const S = sh.local(T.i64);
    const slot = sh.local(T.i32);
    sh.emit(
      set(S, i64.load(get(ctx))),
      set(slot, i32.wrapI64(i64.remU(get(S), i64.extendU(get(nSlots))))),
      i64.store(
        i32.add(get(ctx), i32.const(SEAL_OFF + 12)),
        call(
          foldIdx,
          i32.add(get(tbase), i32.mul(get(slot), i32.shl(get(words), i32.const(3)))),
          get(words),
          get(S),
        ),
      ),
    );
  }
  // leg_sw(ctx, tbase, nSlots, words, steps): whole steps inside WebAssembly, software SHA-256.
  const ls = m.fn('leg_sw', [T.i32, T.i32, T.i32, T.i32, T.i32], [], true);
  {
    const [ctx, tbase, nSlots, words, steps] = [0, 1, 2, 3, 4];
    const S = ls.local(T.i64);
    const n = ls.local(T.i32);
    const ns = ls.local(T.i64);
    ls.emit(
      set(S, i64.load(get(ctx))),
      set(n, get(steps)),
      set(ns, i64.extendU(get(nSlots))),
      block(
        loop(
          brIf(1, i32.eqz(get(n))),
          set(
            S,
            call(
              sealIdx,
              call(
                foldIdx,
                i32.add(
                  get(tbase),
                  i32.mul(i32.wrapI64(i64.remU(get(S), get(ns))), i32.shl(get(words), i32.const(3))),
                ),
                get(words),
                get(S),
              ),
            ),
          ),
          set(n, i32.sub(get(n), i32.const(1))),
          br(0),
        ),
      ),
      i64.store(get(ctx), get(S)),
    );
  }
  // probes for tests: fold a slot / seal an accumulator through the context
  const fp = m.fn('fold_probe', [T.i32, T.i32, T.i32], [], true);
  fp.emit(
    i64.store(
      i32.add(get(2), i32.const(SEAL_OFF + 12)),
      call(foldIdx, get(0), get(1), i64.load(get(2))),
    ),
  );
  const sp = m.fn('seal_probe', [T.i32], [], true);
  sp.emit(i64.store(get(0), call(sealIdx, i64.load(get(0), SEAL_OFF + 12))));
  return m.build();
}
