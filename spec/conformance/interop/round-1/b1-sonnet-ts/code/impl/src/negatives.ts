// Permanent negatives (SPEC 16). Generator only: no SDK code path imports this module.
// Each function computes a deliberately non-conforming shortcut so the pack can show
// that its output differs from the honest one.
import {
  M64, mix64, rotl, nl4Base, parents, wordsPerSlot, nSlots, sha256, le64, u64le, SLOT_MUL, PARENT_MUL, MIX_ADD, MIX_MUL1, MIX_MUL2,
  SEAL_DOMAIN, Terrain, type Profile,
} from './core.ts';

const seal = (acc: bigint): bigint => u64le(sha256(SEAL_DOMAIN, le64(acc)));

type FoldFn = (words: bigint[], S: bigint) => bigint;

function legWith(t: Terrain, S0: bigint, fold: FoldFn, steps = t.profile.stepsPerLeg): bigint {
  let S = S0;
  const n = BigInt(nSlots(t.profile));
  for (let s = 0; s < steps; s++) {
    const slot = t.slot(Number(S % n));
    const words: bigint[] = [];
    for (let w = 0; w < slot.length / 8; w++) words.push(slot.readBigUInt64LE(8 * w));
    S = seal(fold(words, S));
  }
  return S;
}

const honestFold: FoldFn = (words, S) => words.reduce((a, w) => (a + mix64(w ^ S)) & M64, 0n);

function preFinal(x: bigint): bigint {
  let y = (x + MIX_ADD) & M64;
  y = ((y ^ (y >> 30n)) * MIX_MUL1) & M64;
  y = ((y ^ (y >> 27n)) * MIX_MUL2) & M64;
  return y;
}

export const foldNegatives: Record<string, FoldFn> = {
  slot_summary_fold: (words, S) => words.reduce((a, w, i) => a ^ (w ^ BigInt(i)), 0n) ^ S,
  xor_accumulator_fold: (words, S) => words.reduce((a, w, i) => a ^ mix64(w ^ BigInt(i) ^ S), 0n),
  xor_w_fold: (words, S) => words.reduce((a, w, i) => (a + mix64(w ^ BigInt(i) ^ S)) & M64, 0n),
  s_plus_w_fold: (words, S) => words.reduce((a, w, i) => (a + mix64(w ^ ((S + BigInt(i)) & M64))) & M64, 0n),
  prefold_hoist: (words, S) => {
    const x = words.reduce((a, w) => a ^ preFinal(w ^ S), 0n);
    return x ^ (x >> 31n);
  },
  partial_slot_read: (words, S) => honestFold(words.slice(0, words.length / 2), S),
  partial_slot_read_2047: (words, S) => honestFold(words.slice(0, words.length - 1), S),
};

function fillWith(profile: Profile, base: (i: bigint, j: number) => bigint): Buffer {
  const wps = wordsPerSlot(profile);
  const n = nSlots(profile);
  const bytes = Buffer.alloc(profile.terrainBytes);
  for (let i = 0; i < n; i++) {
    const w: bigint[] = [];
    for (let j = 0; j < wps; j++) w.push(base(BigInt(i), j));
    if (i > 0) {
      const ps = parents(i);
      for (let k = 1; k <= 3; k++) {
        const off = ps[k - 1] * profile.slotBytes;
        for (let j = 0; j < wps; j++) {
          const pw = bytes.readBigUInt64LE(off + 8 * j);
          w[j] = mix64((w[j] ^ rotl(pw, BigInt(j + k))) ^ ((BigInt(k) * PARENT_MUL) & M64));
        }
      }
      const sm = (BigInt(i) * SLOT_MUL) & M64;
      for (let j = 0; j < wps; j++) w[j] = mix64(w[j] ^ sm);
    }
    for (let j = 0; j < wps; j++) bytes.writeBigUInt64LE(w[j], i * profile.slotBytes + 8 * j);
  }
  return bytes;
}

const oneLane = (P: bigint[]) => (i: bigint, j: number) => mix64(P[j % 4] ^ (((i << 32n) & M64) ^ BigInt(j) ^ 0xc0ffeen));
const allFourLinear = (P: bigint[]) => (i: bigint, j: number) =>
  mix64(P[j % 4] ^ rotl(P[(j + 1) % 4], 17n) ^ rotl(P[(j + 2) % 4], 31n) ^ rotl(P[(j + 3) % 4], 47n) ^ (((i << 32n) & M64) ^ BigInt(j) ^ 0xc0ffeen));

export interface NegativeResult {
  field: 'legs_le_hex' | 'terrain_sha256';
  value: string;
  honest: string;
}

/** `honestTerrain`/`otherTerrain` are S1 terrains for pk_A and pk_B. */
export function computeNegatives(profile: Profile, P: bigint[], honestTerrain: Terrain, otherTerrain: Terrain): Record<string, NegativeResult> {
  const S0 = P[0];
  const honestLeg = le64(honestTerrain.runLeg(S0)).toString('hex');
  const honestDigest = honestTerrain.digest().toString('hex');
  const out: Record<string, NegativeResult> = {};
  const leg = (name: string, v: bigint) => {
    out[name] = { field: 'legs_le_hex', value: le64(v).toString('hex'), honest: honestLeg };
  };
  const terr = (name: string, bytes: Buffer) => {
    out[name] = { field: 'terrain_sha256', value: sha256(bytes).toString('hex'), honest: honestDigest };
  };
  for (const [name, fold] of Object.entries(foldNegatives)) leg(name, legWith(honestTerrain, S0, fold));
  terr('one_lane_flip', fillWith(profile, oneLane(P)));
  terr('all4_kernel', fillWith(profile, allFourLinear(P)));
  leg('f3_shared_terrain', legWith(otherTerrain, S0, honestFold));
  {
    const cache = new Map<number, bigint>();
    leg('cached_leg', legWith(honestTerrain, S0, (words, S) => {
      const key = Number(S % BigInt(nSlots(profile)));
      let v = cache.get(key);
      if (v === undefined) {
        v = honestFold(words, S);
        cache.set(key, v);
      }
      return v;
    }));
  }
  {
    const pin = P[0] ^ P[1] ^ P[2] ^ P[3];
    terr('squeezed_seed_terrain', fillWith(profile, (i, j) => nl4Base([pin, pin, pin, pin], i, j)));
  }
  for (const [name, r] of Object.entries(out)) {
    if (r.value === r.honest) throw new Error(`negative ${name} matches the honest value`);
  }
  return out;
}
