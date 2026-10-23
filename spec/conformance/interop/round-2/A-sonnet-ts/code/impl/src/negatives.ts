// GENERATOR-ONLY. SPEC §16 "Permanent negatives": constructions the SPEC forbids, computed so the pack
// can pin them as non-matches. Nothing outside gen-candidates.ts and the tests imports this module.
// It is deliberately BigInt-based and shares no code path with the producer, verifier, or engine.

import {
  fillTerrain,
  lanes,
  legRef,
  le64,
  mix64,
  mix64Pre,
  nl4Base,
  rotl,
  sha256,
  u64le,
  type BaseWord,
} from './bigref.js';
import { SEAL_DOMAIN, U64, WORD_XOR, type Profile } from './profile.js';

export interface LegNegative {
  name: string;
  legsLeHex: string;
  matchesHonest: boolean;
}
export interface TerrainNegative {
  name: string;
  terrainSha256: string;
  matchesHonest: boolean;
}
export interface Negatives {
  honestLegsLeHex: string;
  honestTerrainSha256: string;
  legs: LegNegative[];
  terrains: TerrainNegative[];
}

type Fold = (S: bigint, words: bigint[], slot: number, cache: Map<number, bigint>) => bigint;

function negLeg(profile: Profile, terrain: Uint8Array, start: bigint, fold: Fold): bigint {
  const dv = new DataView(terrain.buffer, terrain.byteOffset, terrain.byteLength);
  const cache = new Map<number, bigint>();
  let S = start;
  for (let t = 0; t < profile.stepsPerLeg; t++) {
    const slot = Number(S % BigInt(profile.nSlots));
    const words: bigint[] = [];
    for (let w = 0; w < profile.wordsPerSlot; w++) words.push(dv.getBigUint64(slot * profile.slotBytes + 8 * w, true));
    const folded = fold(S, words, slot, cache) & U64;
    S = u64le(sha256(SEAL_DOMAIN, le64(folded)));
  }
  return S;
}

const honestAcc = (S: bigint, words: bigint[], upTo = words.length): bigint => {
  let acc = 0n;
  for (let w = 0; w < upTo; w++) acc = (acc + mix64(words[w]! ^ S)) & U64;
  return acc;
};

export function computeNegatives(profile: Profile, pkA: Uint8Array, pkB: Uint8Array, seedBeacon: Uint8Array): Negatives {
  const seedA = sha256(pkA, seedBeacon);
  const seedB = sha256(pkB, seedBeacon);
  const PA = lanes(seedA);
  const terrainA = fillTerrain(profile, PA);
  const terrainB = fillTerrain(profile, lanes(seedB));
  const start = PA[0]!;
  const honestLeg = legRef(profile, terrainA, start);
  const honestLegHex = le64(honestLeg).toString('hex');
  const honestTerrain = sha256(terrainA).toString('hex');

  let reused = false;
  const legFolds: [string, Fold, Uint8Array?][] = [
    [
      'slot_summary_fold',
      (S, words) => {
        let x = 0n;
        words.forEach((w, k) => (x ^= w ^ BigInt(k)));
        return S ^ x;
      },
    ],
    [
      'xor_accumulator_fold',
      (S, words) => {
        let acc = 0n;
        words.forEach((w, k) => (acc ^= mix64(w ^ BigInt(k) ^ S)));
        return acc;
      },
    ],
    [
      'xor_w_fold',
      (S, words) => {
        let acc = 0n;
        words.forEach((w, k) => (acc = (acc + mix64(w ^ BigInt(k) ^ S)) & U64));
        return acc;
      },
    ],
    [
      's_plus_w_fold',
      (S, words) => {
        let acc = 0n;
        words.forEach((w, k) => (acc = (acc + mix64(w ^ ((S + BigInt(k)) & U64))) & U64));
        return acc;
      },
    ],
    [
      'prefold_hoist',
      (S, words) => {
        let pre = 0n;
        words.forEach((w, k) => (pre = (pre + mix64(w ^ BigInt(k))) & U64));
        return (pre + S) & U64;
      },
    ],
    [
      'final_stage_hoist',
      (S, words) => {
        let x = 0n;
        for (const w of words) x ^= mix64Pre(w ^ S);
        return x ^ (x >> 31n);
      },
    ],
    ['partial_slot_read', (S, words) => honestAcc(S, words, 1)],
    ['partial_slot_read_2047', (S, words) => honestAcc(S, words, words.length - 1)],
    ['f3_shared_terrain', (S, words) => honestAcc(S, words), terrainB],
    [
      'cached_leg',
      (S, words, slot, cache) => {
        const hit = cache.get(slot);
        if (hit !== undefined) {
          reused = true;
          return hit;
        }
        const v = honestAcc(S, words);
        cache.set(slot, v);
        return v;
      },
    ],
  ];

  const legs: LegNegative[] = legFolds.map(([name, fold, terrainOverride]) => {
    reused = false;
    const hex = le64(negLeg(profile, terrainOverride ?? terrainA, start, fold)).toString('hex');
    if (name === 'cached_leg' && !reused) throw new Error('cached_leg: the leg does not read any slot twice; choose other fixture inputs');
    const matchesHonest = hex === honestLegHex;
    if (matchesHonest) throw new Error(`negative ${name} equals the honest leg; choose other fixture inputs`);
    return { name, legsLeHex: hex, matchesHonest };
  });

  const cOf = (i: number, j: number): bigint => ((BigInt(i) << 32n) ^ BigInt(j) ^ WORD_XOR) & U64;
  const oneLane: BaseWord = (P, i, j) => mix64(P[j % 4]! ^ cOf(i, j));
  const all4: BaseWord = (P, i, j) =>
    mix64(P[j % 4]! ^ rotl(P[(j + 1) % 4]!, 17) ^ rotl(P[(j + 2) % 4]!, 31) ^ rotl(P[(j + 3) % 4]!, 47) ^ cOf(i, j));
  const squeezed = PA[0]! ^ PA[1]! ^ PA[2]! ^ PA[3]!;
  const terrainNegs: [string, Uint8Array][] = [
    ['one_lane_flip', fillTerrain(profile, PA, oneLane)],
    ['all4_kernel', fillTerrain(profile, PA, all4)],
    ['squeezed_seed_terrain', fillTerrain(profile, [squeezed, squeezed, squeezed, squeezed], nl4Base)],
  ];
  const terrains: TerrainNegative[] = terrainNegs.map(([name, t]) => {
    const h = sha256(t).toString('hex');
    if (h === honestTerrain) throw new Error(`negative ${name} equals the honest terrain; choose other fixture inputs`);
    return { name, terrainSha256: h, matchesHonest: h === honestTerrain };
  });

  return { honestLegsLeHex: honestLegHex, honestTerrainSha256: honestTerrain, legs, terrains };
}
