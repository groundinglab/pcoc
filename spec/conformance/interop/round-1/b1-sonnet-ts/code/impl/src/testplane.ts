import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { Terrain, seedOf, buildHike, sha256, type Profile } from './core.ts';
import { PK_A, SEED_BEACON, SEED_HEIGHT, beaconSet, fixtureHash, hex, FIX_S1, FIX_S2, FIX_PROD, type Fixture } from './fixtures.ts';
import { encodeHike } from './object.ts';

const BASE_TIME = 1_700_000_000;
const SPACING = 600;

/** Synthetic offline index: one document per height around the stored headers, with Bitcoin display-order hashes. */
export function writeSyntheticIndex(dir: string): void {
  mkdirSync(dir, { recursive: true });
  const bs = beaconSet();
  const known = new Map<number, Buffer>([[bs.seed.height, bs.seed.hash], ...bs.interrupts.map((i) => [i.height, i.hash] as [number, Buffer])]);
  const lastHeight = bs.interrupts[17].height;
  for (let h = SEED_HEIGHT - 11; h <= lastHeight; h++) {
    const natural = known.get(h) ?? sha256(Buffer.from('pcoc-pack-2.2/index/filler/' + h, 'ascii'));
    const doc = { network: 'synthetic', header: { hash: Buffer.from(natural).reverse().toString('hex'), height: h, time: BASE_TIME + SPACING * h } };
    writeFileSync(join(dir, `${h}.json`), JSON.stringify(doc));
  }
}

export interface Produced {
  name: string;
  legs: number;
  seconds: number;
  file: string;
}

export function produceObject(outDir: string, name: string, fx: Fixture): Produced {
  mkdirSync(outDir, { recursive: true });
  const { P } = seedOf(PK_A, SEED_BEACON);
  const t0 = Date.now();
  const terrain = new Terrain(fx.profile as Profile, P);
  const hike = buildHike(terrain, PK_A, beaconSet(), fx.legsPerStage);
  const bytes = encodeHike(hike, fx.profile);
  const file = join(outDir, `${name}.cbor`);
  writeFileSync(file, bytes);
  const seconds = (Date.now() - t0) / 1000;
  writeSyntheticIndex(join(outDir, 'index'));
  void hex; void fixtureHash;
  return { name, legs: fx.legsPerStage.reduce((a, b) => a + b, 0), seconds, file };
}

/** O1: S1/pk_A, O2: S2/pk_A, O3: production profile/pk_A with one leg per stage. */
export function produceAll(outDir: string): Produced[] {
  return [produceObject(outDir, 'O1', FIX_S1), produceObject(outDir, 'O2', FIX_S2), produceObject(outDir, 'O3', FIX_PROD)];
}
