import { createHash } from 'node:crypto';
import { buildKernelModule } from './wasm.ts';

export const M64 = 0xffffffffffffffffn;
export const MIX_ADD = 0x9e3779b97f4a7c15n;
export const MIX_MUL1 = 0xbf58476d1ce4e5b9n;
export const MIX_MUL2 = 0x94d049bb133111ebn;
export const WORD_XOR = 0xc0ffeen;
export const PARENT_MUL = 0xd1b54a32d192ed03n;
export const SLOT_MUL = 0xa24baed4963ee407n;
export const SEAL_DOMAIN = Buffer.from('pcoc-walk-v1', 'ascii');
export const KIND = 'v2.2-pcoc-object';
export const FILL = 'dag2';
export const STAGES = 18;

export interface Profile {
  name: 'production' | 'conformance-small';
  slotBytes: number;
  sampleBytes: number;
  terrainBytes: number;
  stepsPerLeg: number;
}

export const PROD: Profile = {
  name: 'production', slotBytes: 16384, sampleBytes: 1024, terrainBytes: 268435456, stepsPerLeg: 2000000,
};
export const S1: Profile = { name: 'conformance-small', slotBytes: 64, sampleBytes: 32, terrainBytes: 65536, stepsPerLeg: 100 };
export const S2: Profile = { name: 'conformance-small', slotBytes: 256, sampleBytes: 64, terrainBytes: 1048576, stepsPerLeg: 1000 };

export const wordsPerSlot = (p: Profile) => p.slotBytes / 8;
export const nSlots = (p: Profile) => p.terrainBytes / p.slotBytes;
export const offsetModulus = (p: Profile) => p.terrainBytes - p.sampleBytes + 1;

export function sha256(...parts: (Uint8Array | Buffer)[]): Buffer {
  const h = createHash('sha256');
  for (const p of parts) h.update(p);
  return h.digest();
}

export const le64 = (x: bigint): Buffer => {
  const b = Buffer.alloc(8);
  b.writeBigUInt64LE(x & M64);
  return b;
};
export const u64le = (b: Uint8Array, off = 0): bigint => Buffer.from(b.buffer, b.byteOffset + off, 8).readBigUInt64LE(0);

export function mix64(x: bigint): bigint {
  let y = (x + MIX_ADD) & M64;
  y = ((y ^ (y >> 30n)) * MIX_MUL1) & M64;
  y = ((y ^ (y >> 27n)) * MIX_MUL2) & M64;
  return y ^ (y >> 31n);
}

export const rotl = (w: bigint, r: bigint): bigint => {
  const k = r & 63n;
  return k === 0n ? w : ((w << k) | (w >> (64n - k))) & M64;
};

export function seedOf(publicKey: Uint8Array, seedBeacon: Uint8Array) {
  const preimage = Buffer.concat([publicKey, seedBeacon]);
  const D = sha256(preimage);
  const P = [0, 1, 2, 3].map((k) => u64le(D, 8 * k));
  return { preimage, D, P };
}

export function nl4Base(P: bigint[], i: bigint, j: number): bigint {
  const jb = BigInt(j);
  const c = ((i << 32n) & M64) ^ jb ^ WORD_XOR;
  let t = mix64(P[j % 4] ^ c);
  t = mix64(t ^ rotl(P[(j + 1) % 4], 17n));
  t = mix64(t ^ rotl(P[(j + 2) % 4], 31n));
  t = mix64(t ^ rotl(P[(j + 3) % 4], 47n));
  return t;
}

function floorLog2(n: number): number {
  return 31 - Math.clz32(n);
}
function reverse32(v: number): number {
  let r = 0;
  for (let k = 0; k < 32; k++) if ((v >>> k) & 1) r |= 1 << (31 - k);
  return r >>> 0;
}
export function parents(i: number): [number, number, number] {
  const bits = Math.max(floorLog2(i), 1);
  const bitrev = (reverse32(i) >>> (32 - bits)) % i;
  const h = (BigInt(i) * MIX_ADD) & M64;
  const mulp = Number((h >> 32n) % BigInt(i));
  return [i - 1, bitrev, mulp];
}

let kernel: WebAssembly.Module | undefined;
let sealBuf = Buffer.alloc(20);
sealBuf.write('pcoc-walk-v1', 0, 'ascii');
function seal(acc: bigint): bigint {
  sealBuf.writeBigUInt64LE(BigInt.asUintN(64, acc), 12);
  return createHash('sha256').update(sealBuf).digest().readBigUInt64LE(0);
}

export class Terrain {
  readonly profile: Profile;
  readonly mem: WebAssembly.Memory;
  readonly bytes: Buffer;
  private legFn: (S: bigint, steps: number, slotBytes: number, nSlots: bigint) => bigint;

  constructor(profile: Profile, P: bigint[], variant: FillVariant = {}) {
    this.profile = profile;
    const total = profile.terrainBytes + 64;
    const pages = Math.ceil(total / 65536);
    this.mem = new WebAssembly.Memory({ initial: pages });
    kernel ??= new WebAssembly.Module(buildKernelModule());
    const inst = new WebAssembly.Instance(kernel, { env: { seal, mem: this.mem } });
    const fillSlot = inst.exports.fillSlot as (...a: unknown[]) => void;
    this.legFn = inst.exports.leg as Terrain['legFn'];
    this.bytes = Buffer.from(this.mem.buffer, 0, profile.terrainBytes);
    const lanes = new BigUint64Array(this.mem.buffer, profile.terrainBytes, 8);
    for (let k = 0; k < 8; k++) lanes[k] = P[k % 4];
    const wps = wordsPerSlot(profile);
    const n = nSlots(profile);
    if (variant.slowFill) {
      variant.slowFill(this, P);
      return;
    }
    for (let i = 0; i < n; i++) {
      const cur = i * profile.slotBytes;
      if (i === 0) {
        fillSlot(0n, wps, 0, 0, 0, 0, cur, profile.terrainBytes, 0n);
      } else {
        const [p1, p2, p3] = parents(i);
        fillSlot(BigInt(i), wps, p1 * profile.slotBytes, p2 * profile.slotBytes, p3 * profile.slotBytes, 1, cur,
          profile.terrainBytes, (BigInt(i) * SLOT_MUL) & M64);
      }
    }
  }

  slot(i: number): Buffer {
    return this.bytes.subarray(i * this.profile.slotBytes, (i + 1) * this.profile.slotBytes);
  }

  sample(previousCheckpoint: Uint8Array): Buffer {
    const off = Number(u64le(previousCheckpoint) % BigInt(offsetModulus(this.profile)));
    return this.bytes.subarray(off, off + this.profile.sampleBytes);
  }

  digest(): Buffer {
    return sha256(this.bytes);
  }

  /** Section 11.2: STEPS_PER_LEG full-slot steps. */
  runLeg(S0: bigint, steps = this.profile.stepsPerLeg): bigint {
    return BigInt.asUintN(64, this.legFn(BigInt.asIntN(64, S0), steps, this.profile.slotBytes, BigInt(nSlots(this.profile))));
  }
}

export interface FillVariant {
  slowFill?: (t: Terrain, P: bigint[]) => void;
}

/** Section 11.1 reference step in BigInt, with the full slot. Used for traces. */
export function stepRef(t: Terrain, S: bigint) {
  const i = Number(S % BigInt(nSlots(t.profile)));
  const slot = t.slot(i);
  let acc = 0n;
  for (let w = 0; w < slot.length / 8; w++) acc = (acc + mix64(slot.readBigUInt64LE(8 * w) ^ S)) & M64;
  const next = seal(acc);
  return { slotIndex: i, acc, next };
}

export function checkpointOf(prev: Uint8Array, limb: Uint8Array, sample: Uint8Array, legResult: bigint): Buffer {
  return sha256(prev, limb, sample, le64(legResult));
}

export function foldOf(checkpoints: Uint8Array[]): Buffer {
  let F: Buffer = Buffer.alloc(32);
  for (const c of checkpoints) F = sha256(F, c);
  return F;
}

export function waypointOf(t: Terrain, B: Uint8Array, stageLimb: Uint8Array, checkpoints: Uint8Array[]): Buffer {
  const slotIndex = Number(u64le(B) % BigInt(nSlots(t.profile)));
  return sha256(B, t.slot(slotIndex), stageLimb, foldOf(checkpoints));
}

export interface StageData {
  beacon: Buffer;
  beaconHeight: number;
  waypoint: Buffer;
  checkpoints: Buffer[];
  legResults: Buffer[];
  path?: Buffer;
}

export interface Hike {
  publicKey: Buffer;
  seedBeacon: Buffer;
  seedBeaconHeight: number;
  stages: StageData[];
  lastInterrupt: Buffer;
  lastInterruptHeight: number;
}

export interface BeaconSet {
  seed: { hash: Buffer; height: number };
  interrupts: { hash: Buffer; height: number }[];
}

/** Sections 12 and 13.2 with the tip schedule fixed by legsPerStage. */
export function buildHike(t: Terrain, publicKey: Buffer, beacons: BeaconSet, legsPerStage: number[]): Hike {
  const { D } = seedOf(publicKey, beacons.seed.hash);
  const stages: StageData[] = [];
  let waypoint: Buffer = D;
  let beacon = beacons.seed;
  for (let g = 0; g < STAGES; g++) {
    let previous: Buffer = waypoint;
    const checkpoints: Buffer[] = [];
    const legResults: Buffer[] = [];
    for (let l = 0; l < legsPerStage[g]; l++) {
      const result = t.runLeg(u64le(previous));
      const cp = checkpointOf(previous, waypoint, t.sample(previous), result);
      checkpoints.push(cp);
      legResults.push(le64(result));
      previous = cp;
    }
    const stage: StageData = { beacon: beacon.hash, beaconHeight: beacon.height, waypoint, checkpoints, legResults };
    if (g >= 1) stage.path = Buffer.alloc(0);
    stages.push(stage);
    const interrupt = beacons.interrupts[g];
    if (g < STAGES - 1) {
      waypoint = waypointOf(t, interrupt.hash, waypoint, checkpoints);
      beacon = interrupt;
    }
  }
  const last = beacons.interrupts[STAGES - 1];
  return { publicKey, seedBeacon: beacons.seed.hash, seedBeaconHeight: beacons.seed.height, stages, lastInterrupt: last.hash, lastInterruptHeight: last.height };
}
