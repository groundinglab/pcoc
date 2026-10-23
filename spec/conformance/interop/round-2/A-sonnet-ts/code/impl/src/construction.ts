// SPEC §10–§13 construction on top of the engine: seed, checkpoints, waypoints, and the offline drive
// that fills a hike object from fixed beacons (CASES.md "Fixed inputs").

import { checkpointRef, le64, sha256, u64le, waypointRef } from './bigref.js';
import { Terrain, type EngineOptions } from './engine.js';
import { constantsFor, encodeObject, type HikeObject, type StageRecord } from './object.js';
import { STAGES, type Profile } from './profile.js';

export const hex = (b: Uint8Array): string => Buffer.from(b.buffer, b.byteOffset, b.byteLength).toString('hex');
export const unhex = (s: string): Buffer => Buffer.from(s, 'hex');

// ---- CASES.md fixed inputs --------------------------------------------------------------------

export const PK_A = unhex('f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9');
export const PK_B = unhex('dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659');
export const SEED_BEACON = sha256(Buffer.from('pcoc-pack-2.2/beacon/seed', 'ascii'));
export const SEED_HEIGHT = 1000;
export const interruptBeacon = (k: number): Buffer => sha256(Buffer.from(`pcoc-pack-2.2/beacon/${k}`, 'ascii'));
export const interruptHeight = (k: number): number => SEED_HEIGHT + 1 + k;

/** `legs_per_stage` fixture metadata: the same count on every stage. */
export const uniformLegs = (n: number): number[] => Array.from({ length: STAGES }, () => n);

export const seedPreimage = (publicKey: Uint8Array, seedBeacon: Uint8Array): Buffer => Buffer.concat([publicKey, seedBeacon]);
/** §10: D = SHA-256(public_key ‖ seed_beacon). */
export const seedDigest = (publicKey: Uint8Array, seedBeacon: Uint8Array): Buffer => sha256(publicKey, seedBeacon);

/** §11.4 checkpoint = SHA-256(previous ‖ stage limb ‖ sample(previous) ‖ le64(leg result)). */
export const checkpointOf = checkpointRef;
/** §12.2 W_{g+1} = SHA-256(B ‖ slot_sample(B) ‖ W ‖ fold). */
export const waypointOf = waypointRef;

export interface LegRecord {
  legResult: bigint;
  checkpoint: Buffer;
}
export interface StageRun {
  /** the stage's second limb: the seed on stage 0, W_g afterwards */
  waypoint: Buffer;
  legs: LegRecord[];
}
export interface HikeRun {
  object: HikeObject;
  bytes: Uint8Array;
  stages: StageRun[];
  terrainDigest: Buffer;
}

export interface ProduceInput {
  profile: Profile;
  publicKey: Uint8Array;
  seedBeacon: Uint8Array;
  seedHeight: number;
  /** I_0 … I_17 */
  interrupts: Uint8Array[];
  interruptHeights: number[];
  legsPerStage: number[];
  engine?: EngineOptions;
  log?: (msg: string) => void;
}

/** Offline test-plane drive: the tip schedule is fixed by `legsPerStage`, so no clock is read. */
export async function produceHike(inp: ProduceInput): Promise<HikeRun> {
  const { profile } = inp;
  if (inp.interrupts.length !== STAGES || inp.interruptHeights.length !== STAGES || inp.legsPerStage.length !== STAGES) {
    throw new RangeError('18 interrupts, heights and leg counts are required');
  }
  const D = seedDigest(inp.publicKey, inp.seedBeacon);
  const terrain = await Terrain.build(profile, D, inp.engine);
  const t = terrain.bytes;
  const stages: StageRun[] = [];
  const records: StageRecord[] = [];
  let limb: Buffer = D;
  for (let g = 0; g < STAGES; g++) {
    let prev: Buffer = limb;
    const legs: LegRecord[] = [];
    for (let j = 0; j < inp.legsPerStage[g]!; j++) {
      const legResult = terrain.runLeg(u64le(prev));
      const checkpoint = checkpointRef(profile, t, prev, limb, legResult);
      legs.push({ legResult, checkpoint });
      inp.log?.(`stage ${g} leg ${j}`);
      prev = checkpoint;
    }
    stages.push({ waypoint: limb, legs });
    records.push({
      beacon: g === 0 ? inp.seedBeacon : inp.interrupts[g - 1]!,
      beaconHeight: g === 0 ? inp.seedHeight : inp.interruptHeights[g - 1]!,
      waypoint: limb,
      ...(g > 0 ? { path: new Uint8Array(0) } : {}),
      checkpoints: legs.map((l) => l.checkpoint),
      legResults: legs.map((l) => le64(l.legResult)),
    });
    if (g < STAGES - 1) {
      limb = waypointRef(profile, t, inp.interrupts[g]!, limb, legs.map((l) => l.checkpoint));
    }
  }
  const object: HikeObject = {
    ...constantsFor(profile),
    publicKey: inp.publicKey,
    seedBeacon: inp.seedBeacon,
    stages: records,
    lastInterrupt: inp.interrupts[STAGES - 1]!,
    lastInterruptHeight: inp.interruptHeights[STAGES - 1]!,
  };
  return { object, bytes: encodeObject(object), stages, terrainDigest: terrain.digest() };
}

/** The CASES.md objects: fixed beacons and heights, empty paths on stages 1–17. */
export function produceFixture(profile: Profile, legsPerStage: number[], engine?: EngineOptions, log?: (m: string) => void): Promise<HikeRun> {
  return produceHike({
    profile,
    publicKey: PK_A,
    seedBeacon: SEED_BEACON,
    seedHeight: SEED_HEIGHT,
    interrupts: Array.from({ length: STAGES }, (_, k) => interruptBeacon(k)),
    interruptHeights: Array.from({ length: STAGES }, (_, k) => interruptHeight(k)),
    legsPerStage,
    ...(engine ? { engine } : {}),
    ...(log ? { log } : {}),
  });
}

/** Produce stderr, written in one step at the close (Rule A3.4). */
export function produceStderr(): string {
  let s = '';
  for (let g = 1; g <= 16; g++) s += `path_skip ${g}\n`;
  return `${s}not fetched by design 17\n`;
}
