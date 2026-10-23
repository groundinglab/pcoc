// Plain scalar tier: pure TypeScript on 32-bit limbs, no WebAssembly, no workers, no SIMD.
// It exists as the baseline the optimised engine is measured against and as a second
// implementation the tests compare with. node:crypto supplies SHA-256 (SHA-NI where present).

import { hash as nodeHash } from 'node:crypto';
import { bitrevParent, mulParent } from './bigref.js';
import type { Profile } from './profile.js';

const ADD_H = 0x9e3779b9;
const ADD_L = 0x7f4a7c15;
const M1_H = 0xbf58476d;
const M1_L = 0x1ce4e5b9;
const M2_H = 0x94d049bb;
const M2_L = 0x133111eb;
const PMUL_H = 0xd1b54a32;
const PMUL_L = 0xd192ed03;
const SMUL_H = 0xa24baed4;
const SMUL_L = 0x963ee407;
const WORD_XOR = 0xc0ffee;

let rh = 0;
let rl = 0;

function mulhi(a: number, b: number): number {
  const a0 = a & 65535;
  const a1 = a >>> 16;
  const b0 = b & 65535;
  const b1 = b >>> 16;
  const t = a0 * b0;
  const m1 = a1 * b0 + (t >>> 16);
  const m2 = a0 * b1 + (m1 & 65535);
  return (a1 * b1 + Math.floor(m1 / 65536) + Math.floor(m2 / 65536)) >>> 0;
}

/** r = mix64(h:l), result in rh:rl. */
function mix(h: number, l: number): void {
  l >>>= 0;
  h >>>= 0;
  let lo = (l + ADD_L) >>> 0;
  let hi = (h + ADD_H + (lo < l ? 1 : 0)) >>> 0;
  lo = (lo ^ ((lo >>> 30) | (hi << 2))) >>> 0;
  hi = (hi ^ (hi >>> 30)) >>> 0;
  let nl = Math.imul(lo, M1_L) >>> 0;
  hi = (mulhi(lo, M1_L) + Math.imul(hi, M1_L) + Math.imul(lo, M1_H)) >>> 0;
  lo = nl;
  lo = (lo ^ ((lo >>> 27) | (hi << 5))) >>> 0;
  hi = (hi ^ (hi >>> 27)) >>> 0;
  nl = Math.imul(lo, M2_L) >>> 0;
  hi = (mulhi(lo, M2_L) + Math.imul(hi, M2_L) + Math.imul(lo, M2_H)) >>> 0;
  lo = nl;
  rl = (lo ^ ((lo >>> 31) | (hi << 1))) >>> 0;
  rh = (hi ^ (hi >>> 31)) >>> 0;
}

function rotlInto(h: number, l: number, r: number): void {
  if (r === 0) {
    rh = h;
    rl = l;
  } else if (r < 32) {
    rh = ((h << r) | (l >>> (32 - r))) >>> 0;
    rl = ((l << r) | (h >>> (32 - r))) >>> 0;
  } else if (r === 32) {
    rh = l;
    rl = h;
  } else {
    const s = r - 32;
    rh = ((l << s) | (h >>> (32 - s))) >>> 0;
    rl = ((h << s) | (l >>> (32 - s))) >>> 0;
  }
}

/** §10.4 fill on 32-bit limbs. Returns the terrain as little-endian bytes. */
export function fillScalar(profile: Profile, seed: Uint8Array): Uint8Array {
  const W = profile.wordsPerSlot;
  const nSlots = profile.nSlots;
  const buf = new Uint32Array(nSlots * W * 2);
  const sv = new DataView(seed.buffer, seed.byteOffset, 32);
  const PL = [0, 1, 2, 3].map((k) => sv.getUint32(8 * k, true));
  const PH = [0, 1, 2, 3].map((k) => sv.getUint32(8 * k + 4, true));
  const wh = new Uint32Array(W);
  const wl = new Uint32Array(W);
  // rotated lane constants per lane index and offset
  const RH: number[][] = [];
  const RL: number[][] = [];
  for (const [n, r] of [
    [1, 17],
    [2, 31],
    [3, 47],
  ] as const) {
    RH[n] = [];
    RL[n] = [];
    for (let k = 0; k < 4; k++) {
      rotlInto(PH[k]!, PL[k]!, r);
      RH[n]![k] = rh;
      RL[n]![k] = rl;
    }
  }
  for (let i = 0; i < nSlots; i++) {
    const ch = i >>> 0;
    for (let j = 0; j < W; j++) {
      const k0 = j & 3;
      mix(PH[k0]! ^ ch, (PL[k0]! ^ ((j ^ WORD_XOR) >>> 0)) >>> 0);
      for (let n = 1; n <= 3; n++) {
        const k = (j + n) & 3;
        mix(rh ^ RH[n]![k]!, rl ^ RL[n]![k]!);
      }
      wh[j] = rh;
      wl[j] = rl;
    }
    if (i > 0) {
      const parents = [i - 1, bitrevParent(i), mulParent(i)];
      for (let k = 1; k <= 3; k++) {
        const base = parents[k - 1]! * W * 2;
        const kh = k === 1 ? PMUL_H : k === 2 ? mulHi2(PMUL_H, PMUL_L, 2) : mulHi2(PMUL_H, PMUL_L, 3);
        const kl = k === 1 ? PMUL_L : k === 2 ? mulLo2(PMUL_L, 2) : mulLo2(PMUL_L, 3);
        for (let j = 0; j < W; j++) {
          rotlInto(buf[base + 2 * j + 1]!, buf[base + 2 * j]!, (j + k) & 63);
          mix(wh[j]! ^ rh ^ kh, (wl[j]! ^ rl ^ kl) >>> 0);
          wh[j] = rh;
          wl[j] = rl;
        }
      }
      const sh = ((Math.imul(i, SMUL_H) + mulhi(i, SMUL_L)) >>> 0) >>> 0;
      const sl = Math.imul(i, SMUL_L) >>> 0;
      for (let j = 0; j < W; j++) {
        mix(wh[j]! ^ sh, (wl[j]! ^ sl) >>> 0);
        wh[j] = rh;
        wl[j] = rl;
      }
    }
    const o = i * W * 2;
    for (let j = 0; j < W; j++) {
      buf[o + 2 * j] = wl[j]!;
      buf[o + 2 * j + 1] = wh[j]!;
    }
  }
  return new Uint8Array(buf.buffer);
}

function mulLo2(l: number, k: number): number {
  return Math.imul(l, k) >>> 0;
}
function mulHi2(h: number, l: number, k: number): number {
  return (Math.imul(h, k) + mulhi(l, k)) >>> 0;
}

const SEAL = Buffer.alloc(20);
SEAL.write('pcoc-walk-v1', 0, 'ascii');

/** One leg of `steps` steps from state `start`. Software mix64 on 32-bit limbs, node:crypto for the seal. */
export function legScalar(profile: Profile, terrain: Uint8Array, start: bigint, steps = profile.stepsPerLeg): bigint {
  const W = profile.wordsPerSlot;
  const words = new Uint32Array(terrain.buffer, terrain.byteOffset, terrain.byteLength / 4);
  const n = profile.nSlots;
  let sl = Number(start & 0xffffffffn) >>> 0;
  let sh = Number(start >> 32n) >>> 0;
  for (let t = 0; t < steps; t++) {
    const slot = (((sh % n) * 4294967296) + sl) % n;
    const base = slot * W * 2;
    let ah = 0;
    let al = 0;
    for (let w = 0; w < W; w++) {
      mix(words[base + 2 * w + 1]! ^ sh, (words[base + 2 * w]! ^ sl) >>> 0);
      const lo = al + rl;
      al = lo >>> 0;
      ah = (ah + rh + (lo > 0xffffffff ? 1 : 0)) >>> 0;
    }
    SEAL.writeUInt32LE(al, 12);
    SEAL.writeUInt32LE(ah, 16);
    const e = nodeHash('sha256', SEAL, 'buffer');
    sl = e.readUInt32LE(0);
    sh = e.readUInt32LE(4);
  }
  return (BigInt(sh) << 32n) | BigInt(sl);
}
