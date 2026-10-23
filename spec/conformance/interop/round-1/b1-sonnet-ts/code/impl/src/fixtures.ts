import { sha256, S1, S2, PROD, STAGES, type BeaconSet, type Profile } from './core.ts';

export const PK_A = Buffer.from('f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9', 'hex');
export const PK_B = Buffer.from('dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659', 'hex');

export const fixtureHash = (name: string): Buffer => sha256(Buffer.from('pcoc-pack-2.2/' + name, 'ascii'));

export const SEED_BEACON = fixtureHash('beacon/seed');
export const interruptBeacon = (k: number): Buffer => fixtureHash('beacon/' + k);

/** Synthetic test-plane heights: seed at 1000, I_k at 1001 + k. */
export const SEED_HEIGHT = 1000;
export function beaconSet(): BeaconSet {
  return {
    seed: { hash: SEED_BEACON, height: SEED_HEIGHT },
    interrupts: Array.from({ length: STAGES }, (_, k) => ({ hash: interruptBeacon(k), height: SEED_HEIGHT + 1 + k })),
  };
}

export interface Fixture {
  name: string;
  profile: Profile;
  legsPerStage: number[];
}
const same = (n: number) => Array.from({ length: STAGES }, () => n);
export const FIX_S1: Fixture = { name: 'S1', profile: S1, legsPerStage: same(1) };
export const FIX_S2: Fixture = { name: 'S2', profile: S2, legsPerStage: same(3) };
export const FIX_PROD: Fixture = { name: 'PROD', profile: PROD, legsPerStage: same(1) };

export const hex = (b: Uint8Array): string => Buffer.from(b).toString('hex');
export const u64hex = (x: bigint): string => BigInt.asUintN(64, x).toString(16).padStart(16, '0');
export const display = (natural: Uint8Array): Buffer => Buffer.from(natural).reverse();
