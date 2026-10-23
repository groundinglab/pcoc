// Deterministic candidate generator: writes every CASES.md case, with exactly its keys, into candidates/.
// Usage: node dist/gen-candidates.js [outdir]    (default: candidates)

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { encodeCbor } from './cbor.js';
import { checkpointRef, le64, lanes, nl4Base, sha256 } from './bigref.js';
import { hashcashChallenge, headerHash, leadingZeroBits, solveHashcash, anchorComparison, rateComparison } from './compare.js';
import {
  PK_A,
  PK_B,
  SEED_BEACON,
  hex,
  produceFixture,
  seedDigest,
  seedPreimage,
  uniformLegs,
  type HikeRun,
} from './construction.js';
import { ChainSource, sixLegDraw, thoroughDraw } from './draw.js';
import { Terrain, shutdownPool } from './engine.js';
import { NoIndex } from './indexsource.js';
import { objectToCbor } from './object.js';
import { computeNegatives } from './negatives.js';
import { PRODUCTION, S1, S2 } from './profile.js';
import { verifyBytes } from './verify.js';

const u64hex = (x: bigint): string => x.toString(16).padStart(16, '0');

class Lines {
  private m = new Map<string, string>();
  set(key: string, value: string | number | bigint | boolean): void {
    if (this.m.has(key)) throw new Error(`duplicate key ${key}`);
    const v = String(value);
    if (!/^[\x20-\x7e]*$/.test(v) || /\s$/.test(v) || v.includes('\t')) throw new Error(`bad value for ${key}`);
    this.m.set(key, v);
  }
  get size(): number {
    return this.m.size;
  }
  render(): string {
    const keys = [...this.m.keys()].sort((a, b) => Buffer.compare(Buffer.from(a, 'ascii'), Buffer.from(b, 'ascii')));
    return keys.map((k) => `${k}\t${this.m.get(k)}\n`).join('');
  }
}

function hikeLines(prefix: string, run: HikeRun, out: Lines): void {
  out.set(`${prefix}.terrain_sha256`, hex(run.terrainDigest));
  run.stages.forEach((st, g) => {
    out.set(`${prefix}.stage.${g}.waypoint`, hex(st.waypoint));
    st.legs.forEach((l, j) => {
      out.set(`${prefix}.stage.${g}.leg.${j}.legs_le_hex`, hex(le64(l.legResult)));
      out.set(`${prefix}.stage.${g}.leg.${j}.checkpoint`, hex(l.checkpoint));
    });
  });
}

export async function generateCandidates(log: (m: string) => void = () => {}): Promise<Map<string, string>> {
  const files = new Map<string, Lines>();
  const f = (name: string): Lines => {
    let l = files.get(name);
    if (!l) files.set(name, (l = new Lines()));
    return l;
  };

  // 1. seed
  const seed = f('seed.txt');
  for (const [name, pk] of [
    ['pk_A', PK_A],
    ['pk_B', PK_B],
  ] as const) {
    const D = seedDigest(pk, SEED_BEACON);
    seed.set(`seed.${name}.preimage`, hex(seedPreimage(pk, SEED_BEACON)));
    seed.set(`seed.${name}.D`, hex(D));
    lanes(D).forEach((p, k) => seed.set(`seed.${name}.P${k}_u64`, u64hex(p)));
  }

  // S1 and S2 hikes (cases 5, 6), reused for terrain, nl4, wes, unknown-kind
  log('S1 hike');
  const run1 = await produceFixture(S1, uniformLegs(1));
  log('S2 hike');
  const run2 = await produceFixture(S2, uniformLegs(3));
  const D_A = seedDigest(PK_A, SEED_BEACON);
  const P_A = lanes(D_A);

  // 2. nl4
  const nl4 = f('nl4.txt');
  const t1 = await Terrain.build(S1, D_A);
  for (const i of [0, 1, 1023]) {
    for (let j = 0; j < S1.wordsPerSlot; j++) {
      nl4.set(`nl4.S1.slot.${i}.word.${j}.base_u64`, u64hex(nl4Base(P_A, i, j)));
      const stored = Buffer.from(t1.slot(i)).readBigUInt64LE(8 * j);
      nl4.set(`nl4.S1.slot.${i}.word.${j}.stored_u64`, u64hex(stored));
    }
  }

  // 4. fold
  const fold = f('fold.txt');
  {
    let S = P_A[0]!;
    for (let n = 0; n < 4; n++) {
      const st = t1.stepDetailed(S);
      fold.set(`fold.S1.step.${n}.S_u64`, u64hex(S));
      fold.set(`fold.S1.step.${n}.slot_index`, st.slot);
      fold.set(`fold.S1.step.${n}.acc_u64`, u64hex(st.acc));
      fold.set(`fold.S1.step.${n}.S_next_u64`, u64hex(st.next));
      S = st.next;
    }
    fold.set('fold.S1.leg_result_u64', u64hex(t1.runLeg(P_A[0]!)));
  }

  // 5, 6. kat-small, test-3leg
  hikeLines('kat-small.S1', run1, f('kat-small.txt'));
  hikeLines('test-3leg.S2', run2, f('test-3leg.txt'));

  // 3, 7. production terrain and first leg
  log('PROD terrain');
  const tp = await Terrain.build(PRODUCTION, D_A);
  const prodDigest = hex(tp.digest());
  log('PROD leg');
  const prodLeg = tp.runLeg(P_A[0]!);
  const prodCp = checkpointRef(PRODUCTION, tp.bytes, D_A, D_A, prodLeg);
  const terrain = f('terrain.txt');
  terrain.set('terrain.S1.terrain_sha256', hex(run1.terrainDigest));
  terrain.set('terrain.S2.terrain_sha256', hex(run2.terrainDigest));
  terrain.set('terrain.PROD.terrain_sha256', prodDigest);
  const prod = f('prod.txt');
  prod.set('prod.PROD.terrain_sha256', prodDigest);
  prod.set('prod.PROD.stage.0.leg.0.legs_le_hex', hex(le64(prodLeg)));
  prod.set('prod.PROD.stage.0.leg.0.checkpoint', hex(prodCp));

  // 8. wes
  const wes = f('wes.txt');
  wes.set('wes.S1.D', hex(D_A));
  wes.set('wes.S1.stage.0.waypoint', hex(run1.stages[0]!.waypoint));
  wes.set('wes.S1.equal', Buffer.from(D_A).equals(run1.stages[0]!.waypoint));

  // 9. rate
  const rate = f('rate.txt');
  for (const L of [0, 1, 5, 6, 1000]) {
    for (const s of [1, 300, 3600]) {
      for (const [p, q] of [
        [1, 5],
        [1, 4],
        [7, 20],
      ] as const) {
        rate.set(`rate.legs.${L}.span.${s}.floor.${p}_${q}`, rateComparison(BigInt(L), BigInt(s), BigInt(p), BigInt(q)));
      }
    }
  }

  // 10. anchor
  const anchor = f('anchor.txt');
  for (let h = 100; h <= 106; h++) anchor.set(`anchor.H.100.height.${h}`, anchorComparison(100, h));

  // 11. draw
  const draw = f('draw.txt');
  const label = 'pcoc-pack-2.2/draw';
  draw.set('draw.b0_label', label);
  draw.set('draw.thorough_rounds', 3);
  for (const L of [6, 18, 54]) {
    draw.set(`draw.legs.${L}.six_leg.indices`, sixLegDraw(new ChainSource(label), L)!.join(','));
    const th = thoroughDraw(new ChainSource(label), L, 3);
    if (th.length !== 18) throw new Error('thorough draw short');
    draw.set(`draw.legs.${L}.thorough.indices`, th.join(','));
  }

  // 12. unknown-kind
  const uk = f('unknown-kind.txt');
  const labels = ['v2.1-pcoc-object', 'satoshi-hike-v2.0-object', '', 'v2.2-pcoc-objecT'];
  for (const [n, text] of labels.entries()) {
    const m = objectToCbor(run1.object);
    m.set('kind', text);
    const bytes = encodeCbor(m as never);
    const out = await verifyBytes(bytes, { profile: S1, index: new NoIndex() });
    if (out.kind !== 'unknown-kind') throw new Error('expected Rule A5');
    uk.set(`unknown-kind.${n}.kind_text_hex`, hex(Buffer.from(text, 'utf8')));
    uk.set(`unknown-kind.${n}.record_hex`, hex(Buffer.from(out.line, 'utf8')));
  }

  // 13. hashcash
  const hc = f('hashcash.txt');
  {
    const origin = 'https://example.com';
    const a = (s: string) => sha256(Buffer.from(s, 'ascii'));
    const feeHeader = Buffer.concat([
      a('pcoc-pack-2.2/hashcash/fee_header/0'),
      a('pcoc-pack-2.2/hashcash/fee_header/1'),
      a('pcoc-pack-2.2/hashcash/fee_header/2').subarray(0, 16),
    ]);
    const hh = headerHash(feeHeader);
    const waypoint = a('pcoc-pack-2.2/hashcash/waypoint');
    const challenge = hashcashChallenge(origin, hh);
    const { nonce, digest } = solveHashcash(challenge, waypoint, 21);
    hc.set('hashcash.bari_origin', origin);
    hc.set('hashcash.fee_header', hex(feeHeader));
    hc.set('hashcash.header_hash', hex(hh));
    hc.set('hashcash.waypoint', hex(waypoint));
    hc.set('hashcash.difficulty_bits', 21);
    hc.set('hashcash.challenge', hex(challenge));
    hc.set('hashcash.nonce', nonce);
    hc.set('hashcash.digest', hex(digest));
    hc.set('hashcash.leading_zero_bits', leadingZeroBits(digest));
  }

  // 14. negatives
  const neg = f('negatives.txt');
  {
    const n = computeNegatives(S1, PK_A, PK_B, SEED_BEACON);
    neg.set('negatives.honest.legs_le_hex', n.honestLegsLeHex);
    neg.set('negatives.honest.terrain_sha256', n.honestTerrainSha256);
    for (const l of n.legs) {
      neg.set(`negatives.${l.name}.legs_le_hex`, l.legsLeHex);
      neg.set(`negatives.${l.name}.matches_honest`, l.matchesHonest);
    }
    for (const t of n.terrains) {
      neg.set(`negatives.${t.name}.terrain_sha256`, t.terrainSha256);
      neg.set(`negatives.${t.name}.matches_honest`, t.matchesHonest);
    }
    if (neg.size !== 28) throw new Error('negatives must have 28 keys');
  }

  const out = new Map<string, string>();
  for (const name of [
    'seed.txt',
    'nl4.txt',
    'terrain.txt',
    'fold.txt',
    'kat-small.txt',
    'test-3leg.txt',
    'prod.txt',
    'wes.txt',
    'rate.txt',
    'anchor.txt',
    'draw.txt',
    'unknown-kind.txt',
    'hashcash.txt',
    'negatives.txt',
  ]) {
    out.set(name, files.get(name)!.render());
  }
  return out;
}

async function main(): Promise<void> {
  const dir = process.argv[2] ?? 'candidates';
  const files = await generateCandidates((m) => process.stderr.write(`gen: ${m}\n`));
  mkdirSync(dir, { recursive: true });
  for (const [name, text] of files) writeFileSync(join(dir, name), text);
  await shutdownPool();
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((e) => {
    process.stderr.write(`${e instanceof Error ? e.stack : e}\n`);
    process.exit(1);
  });
}
