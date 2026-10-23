// Naive BigInt transcription of SPEC §10–§12. It is the correctness oracle for the
// fast engines (tests) and the arithmetic base of the generator-only negatives module.
// It is deliberately written the way the SPEC reads and is far too slow for production sizes.

import { createHash } from 'node:crypto';
import {
  MIX_ADD,
  MIX_MUL1,
  MIX_MUL2,
  PARENT_MUL,
  SEAL_DOMAIN,
  SLOT_MUL,
  U64,
  WORD_XOR,
  type Profile,
} from './profile.js';

export const sha256 = (...parts: Uint8Array[]): Buffer => {
  const h = createHash('sha256');
  for (const p of parts) h.update(p as unknown as NodeJS.ArrayBufferView);
  return h.digest();
};

export const le64 = (x: bigint): Buffer => {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(x & U64);
  return b;
};
export const u64le = (b: Uint8Array, off = 0): bigint => Buffer.from(b.buffer, b.byteOffset + off, 8).readBigUInt64LE();

export function mix64(x: bigint): bigint {
  let y = (x + MIX_ADD) & U64;
  y = ((y ^ (y >> 30n)) * MIX_MUL1) & U64;
  y = ((y ^ (y >> 27n)) * MIX_MUL2) & U64;
  return y ^ (y >> 31n);
}
/** `mix64` without its last line (the value y before `return y XOR (y >> 31)`). */
export function mix64Pre(x: bigint): bigint {
  let y = (x + MIX_ADD) & U64;
  y = ((y ^ (y >> 30n)) * MIX_MUL1) & U64;
  y = ((y ^ (y >> 27n)) * MIX_MUL2) & U64;
  return y;
}

export const rotl = (w: bigint, r: number): bigint => {
  const s = BigInt(r & 63);
  return ((w << s) | (w >> (64n - s))) & U64;
};

export function lanes(seed: Uint8Array): bigint[] {
  return [0, 1, 2, 3].map((k) => u64le(seed, 8 * k));
}

export const seedOf = (publicKey: Uint8Array, seedBeacon: Uint8Array): Buffer => sha256(publicKey, seedBeacon);

export const floorLog2 = (n: number): number => 31 - Math.clz32(n);
export function reverse32(v: number): number {
  let r = 0;
  for (let k = 0; k < 32; k++) if ((v >>> k) & 1) r |= 1 << (31 - k);
  return r >>> 0;
}
export function bitrevParent(i: number): number {
  const bits = Math.max(floorLog2(i), 1);
  return (reverse32(i) >>> (32 - bits)) % i;
}
export function mulParent(i: number): number {
  const h = (BigInt(i) * MIX_ADD) & U64;
  return Number((h >> 32n) % BigInt(i));
}

export type BaseWord = (P: bigint[], i: number, j: number) => bigint;

export const nl4Base: BaseWord = (P, i, j) => {
  const c = ((BigInt(i) << 32n) ^ BigInt(j) ^ WORD_XOR) & U64;
  let t = mix64(P[j % 4]! ^ c);
  t = mix64(t ^ rotl(P[(j + 1) % 4]!, 17));
  t = mix64(t ^ rotl(P[(j + 2) % 4]!, 31));
  t = mix64(t ^ rotl(P[(j + 3) % 4]!, 47));
  return t;
};

/** §10.4 fill. `base` is replaceable only so the generator-only negatives can reuse the surrounding fill. */
export function fillTerrain(profile: Profile, P: bigint[], base: BaseWord = nl4Base): Uint8Array {
  const W = profile.wordsPerSlot;
  const words = new BigUint64Array(profile.nSlots * W);
  for (let i = 0; i < profile.nSlots; i++) {
    const w: bigint[] = [];
    for (let j = 0; j < W; j++) w.push(base(P, i, j));
    if (i > 0) {
      const parents = [i - 1, bitrevParent(i), mulParent(i)];
      for (let k = 1; k <= 3; k++) {
        const p = parents[k - 1]!;
        const kc = (BigInt(k) * PARENT_MUL) & U64;
        for (let j = 0; j < W; j++) {
          const pw = words[p * W + j]!;
          w[j] = mix64((w[j]! ^ rotl(pw, (j + k) % 64)) ^ kc);
        }
      }
      const slotMix = (BigInt(i) * SLOT_MUL) & U64;
      for (let j = 0; j < W; j++) w[j] = mix64(w[j]! ^ slotMix);
    }
    for (let j = 0; j < W; j++) words[i * W + j] = w[j]!;
  }
  return new Uint8Array(words.buffer);
}

export function stepRef(profile: Profile, terrain: Uint8Array, S: bigint): { slot: number; acc: bigint; next: bigint } {
  const slot = Number(S % BigInt(profile.nSlots));
  const dv = new DataView(terrain.buffer, terrain.byteOffset + slot * profile.slotBytes, profile.slotBytes);
  let acc = 0n;
  for (let w = 0; w < profile.wordsPerSlot; w++) acc = (acc + mix64(dv.getBigUint64(8 * w, true) ^ S)) & U64;
  const E = sha256(SEAL_DOMAIN, le64(acc));
  return { slot, acc, next: u64le(E) };
}

export function legRef(profile: Profile, terrain: Uint8Array, start: bigint, steps = profile.stepsPerLeg): bigint {
  let S = start;
  for (let t = 0; t < steps; t++) S = stepRef(profile, terrain, S).next;
  return S;
}

export function sampleRef(profile: Profile, terrain: Uint8Array, prev: Uint8Array): Uint8Array {
  const off = Number(u64le(prev) % BigInt(profile.offsetModulus));
  return terrain.subarray(off, off + profile.sampleBytes);
}

export function checkpointRef(
  profile: Profile,
  terrain: Uint8Array,
  prev: Uint8Array,
  limb: Uint8Array,
  legResult: bigint,
): Buffer {
  return sha256(prev, limb, sampleRef(profile, terrain, prev), le64(legResult));
}

export function waypointRef(
  profile: Profile,
  terrain: Uint8Array,
  interrupt: Uint8Array,
  stageWaypoint: Uint8Array,
  checkpoints: Uint8Array[],
): Buffer {
  let F: Buffer = Buffer.alloc(32);
  for (const c of checkpoints) F = sha256(F, c);
  const slot = Number(u64le(interrupt) % BigInt(profile.nSlots));
  const sample = terrain.subarray(slot * profile.slotBytes, (slot + 1) * profile.slotBytes);
  return sha256(interrupt, sample, stageWaypoint, F);
}
