import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  Terrain, seedOf, nl4Base, nSlots, wordsPerSlot, stepRef, buildHike, le64, u64le, sha256, S1, S2, PROD, M64, STAGES, type Profile, type Hike,
} from '../core.ts';
import { PK_A, PK_B, SEED_BEACON, beaconSet, hex, u64hex, fixtureHash, FIX_S1, FIX_S2, FIX_PROD, type Fixture } from '../fixtures.ts';
import { computeNegatives } from '../negatives.ts';

export type Files = Record<string, Map<string, string>>;

const pad = (n: number, w = 2) => String(n).padStart(w, '0');

function seedCase(): Map<string, string> {
  const m = new Map<string, string>();
  for (const [name, pk] of [['pk_A', PK_A], ['pk_B', PK_B]] as const) {
    const { preimage, D, P } = seedOf(pk, SEED_BEACON);
    m.set(`seed.${name}.preimage`, hex(preimage));
    m.set(`seed.${name}.D`, hex(D));
    P.forEach((p, k) => m.set(`seed.${name}.P${k}_u64`, u64hex(p)));
  }
  return m;
}

function nl4Case(t: Terrain, P: bigint[]): Map<string, string> {
  const m = new Map<string, string>();
  const n = nSlots(S1);
  const wps = wordsPerSlot(S1);
  for (const i of [0, 1, n - 1]) {
    for (let j = 0; j < 16; j++) {
      m.set(`nl4.S1.pk_A.slot${i}.j${pad(j)}.base_u64`, u64hex(nl4Base(P, BigInt(i), j)));
      if (j < wps) m.set(`nl4.S1.pk_A.slot${i}.j${pad(j)}.final_u64`, u64hex(t.slot(i).readBigUInt64LE(8 * j)));
    }
  }
  return m;
}

function foldCase(t: Terrain, P: bigint[]): Map<string, string> {
  const m = new Map<string, string>();
  let S = P[0];
  for (let s = 0; s < 4; s++) {
    const r = stepRef(t, S);
    const k = `fold.S1.pk_A.step${s}`;
    m.set(`${k}.S_in_u64`, u64hex(S));
    m.set(`${k}.slot_index`, String(r.slotIndex));
    m.set(`${k}.acc_u64`, u64hex(r.acc));
    m.set(`${k}.S_out_u64`, u64hex(r.next));
    S = r.next;
  }
  m.set('fold.S1.pk_A.leg_result_u64', u64hex(t.runLeg(P[0])));
  return m;
}

function katCase(name: string, fx: Fixture, hike: Hike, onlyFirstLeg = false): Map<string, string> {
  const m = new Map<string, string>();
  hike.stages.forEach((st, g) => {
    if (onlyFirstLeg && g > 0) return;
    const sk = `${name}.${fx.name}.pk_A.stage${pad(g)}`;
    st.checkpoints.forEach((cp, l) => {
      m.set(`${sk}.leg${pad(l)}.legs_le_hex`, hex(st.legResults[l]));
      m.set(`${sk}.leg${pad(l)}.checkpoint`, hex(cp));
    });
    if (!onlyFirstLeg) m.set(`${sk}.waypoint`, hex(st.waypoint));
  });
  return m;
}

function rateCase(): Map<string, string> {
  const m = new Map<string, string>();
  for (const legs of [0, 1, 5, 6, 1000])
    for (const span of [1, 300, 3600])
      for (const [p, q] of [[1, 5], [1, 4], [7, 20]]) {
        const meets = span > 0 && q * legs >= p * span;
        m.set(`rate.hike_legs${legs}.hike_span${span}.floor${p}_${q}`, meets ? 'meets' : 'refuse');
      }
  return m;
}

function anchorCase(): Map<string, string> {
  const m = new Map<string, string>();
  const H = 100;
  for (let h = 100; h <= 106; h++) m.set(`anchor.H${H}.height${h}`, [2, 3, 4].includes(h - H) ? 'anchored' : 'not anchored');
  return m;
}

class ByteSource {
  private buf = Buffer.alloc(0);
  private next = sha256(Buffer.from('pcoc-pack-2.2/draw', 'ascii'));
  take8(): bigint {
    while (this.buf.length < 8) {
      this.buf = Buffer.concat([this.buf, this.next]);
      this.next = sha256(this.next);
    }
    const v = this.buf.readBigUInt64LE(0);
    this.buf = this.buf.subarray(8);
    return v;
  }
}

/** SPEC 14.6 unbiased draw. */
function drawOne(src: ByteSource, total: number): number {
  const M = 1n << 64n;
  const limit = M - (M % BigInt(total));
  for (;;) {
    const u = src.take8();
    if (u < limit) return Number(u % BigInt(total));
  }
}

function sixLegDraw(total: number): number[] {
  const src = new ByteSource();
  const out: number[] = [];
  while (out.length < 6) {
    const q = drawOne(src, total);
    if (!out.includes(q)) out.push(q);
  }
  return out;
}

/** SPEC 15.5 thorough draw, thorough_rounds = 3 (policy gl-admit-v9), D = 18. */
function thoroughDraw(total: number, D = 18): number[] {
  const src = new ByteSource();
  const phase: number[] = [];
  while (phase.length < D) {
    const q = drawOne(src, total);
    const groupStart = 6 * Math.floor(phase.length / 6);
    if (phase.slice(groupStart).includes(q)) continue;
    if (phase.includes(q) && new Set(phase).size < total) continue;
    phase.push(q);
  }
  return phase;
}

function drawCase(): Map<string, string> {
  const m = new Map<string, string>();
  m.set('draw.b0_label', 'pcoc-pack-2.2/draw');
  m.set('draw.thorough_rounds', '3');
  for (const total of [6, 18, 54]) {
    m.set(`draw.legs${pad(total)}.six_leg.indices`, sixLegDraw(total).join(','));
    m.set(`draw.legs${pad(total)}.thorough.indices`, thoroughDraw(total).join(','));
  }
  return m;
}

function unknownKindCase(): Map<string, string> {
  const m = new Map<string, string>();
  const rec = (label: string) => hex(Buffer.from(JSON.stringify({ kind: 'unknown kind', kind_text: label }), 'utf8'));
  m.set('unknown-kind.v2.1-pcoc-object.record_hex', rec('v2.1-pcoc-object'));
  m.set('unknown-kind.satoshi-hike-v2.0-object.record_hex', rec('satoshi-hike-v2.0-object'));
  m.set('unknown-kind.empty.record_hex', rec(''));
  m.set('unknown-kind.v2.2-pcoc-objecT.record_hex', rec('v2.2-pcoc-objecT'));
  return m;
}

export function leadingZeroBits(d: Uint8Array): number {
  let n = 0;
  for (const b of d) {
    if (b === 0) {
      n += 8;
      continue;
    }
    return n + Math.clz32(b) - 24;
  }
  return n;
}

function hashcashCase(): Map<string, string> {
  const m = new Map<string, string>();
  const originLabel = 'bari_origin';
  const originHost = hex(fixtureHash(originLabel)).slice(0, 32);
  const origin = `https://${originHost}.invalid`;
  const feeHeader = Buffer.concat([fixtureHash('hashcash/fee_header/0'), fixtureHash('hashcash/fee_header/1'), fixtureHash('hashcash/fee_header/2').subarray(0, 16)]);
  const headerHash = sha256(sha256(feeHeader));
  const waypoint = fixtureHash('hashcash/waypoint');
  const challenge = sha256(Buffer.from('bari-hc-v2', 'ascii'), Buffer.from(origin, 'ascii'), headerHash);
  const pre = Buffer.concat([challenge, waypoint, Buffer.alloc(8)]);
  let nonce = 0n;
  let digest: Buffer;
  for (;;) {
    pre.writeBigUInt64LE(nonce, 64);
    digest = sha256(pre);
    if (leadingZeroBits(digest) >= 21) break;
    nonce++;
  }
  m.set('hashcash.origin_label', originLabel);
  m.set('hashcash.origin', origin);
  m.set('hashcash.fee_header_labels', 'hashcash/fee_header/0,hashcash/fee_header/1,hashcash/fee_header/2[0:16]');
  m.set('hashcash.fee_header', hex(feeHeader));
  m.set('hashcash.header_hash', hex(headerHash));
  m.set('hashcash.waypoint_label', 'hashcash/waypoint');
  m.set('hashcash.waypoint', hex(waypoint));
  m.set('hashcash.challenge', hex(challenge));
  m.set('hashcash.difficulty', '21');
  m.set('hashcash.nonce', nonce.toString());
  m.set('hashcash.digest', hex(digest));
  return m;
}

export function generate(log: (s: string) => void = () => {}): Files {
  const files: Files = {};
  const timed = <T>(label: string, f: () => T): T => {
    const t0 = Date.now();
    const r = f();
    log(`${label}: ${((Date.now() - t0) / 1000).toFixed(1)}s`);
    return r;
  };
  files['seed.txt'] = seedCase();
  const { P: PA } = seedOf(PK_A, SEED_BEACON);
  const { P: PB } = seedOf(PK_B, SEED_BEACON);
  const tS1 = new Terrain(S1, PA);
  const tS1B = new Terrain(S1, PB);
  files['nl4.txt'] = nl4Case(tS1, PA);
  files['fold.txt'] = foldCase(tS1, PA);
  const beacons = beaconSet();
  const hikeS1 = buildHike(tS1, PK_A, beacons, FIX_S1.legsPerStage);
  files['kat-small.txt'] = katCase('kat-small', FIX_S1, hikeS1);
  const tS2 = new Terrain(S2, PA);
  const hikeS2 = buildHike(tS2, PK_A, beacons, FIX_S2.legsPerStage);
  files['test-3leg.txt'] = katCase('test-3leg', FIX_S2, hikeS2);
  const terrain = new Map<string, string>();
  terrain.set('terrain.S1.pk_A.sha256', hex(tS1.digest()));
  terrain.set('terrain.S2.pk_A.sha256', hex(tS2.digest()));
  const tProd = timed('PROD terrain fill', () => new Terrain(PROD, PA));
  terrain.set('terrain.PROD.pk_A.sha256', hex(tProd.digest()));
  files['terrain.txt'] = terrain;
  const prod = new Map<string, string>();
  prod.set('prod.PROD.pk_A.terrain_sha256', hex(tProd.digest()));
  const D = seedOf(PK_A, SEED_BEACON).D;
  const result = timed('PROD first leg', () => tProd.runLeg(u64le(D)));
  const cp = sha256(D, D, tProd.sample(D), le64(result));
  prod.set('prod.PROD.pk_A.stage00.leg00.legs_le_hex', hex(le64(result)));
  prod.set('prod.PROD.pk_A.stage00.leg00.checkpoint', hex(cp));
  files['prod.txt'] = prod;
  const wes = new Map<string, string>();
  wes.set('wes.S1.pk_A.stage0_waypoint', hex(hikeS1.stages[0].waypoint));
  wes.set('wes.S1.pk_A.D', hex(D));
  wes.set('wes.S1.pk_A.equal', String(hikeS1.stages[0].waypoint.equals(D)));
  files['wes.txt'] = wes;
  files['rate.txt'] = rateCase();
  files['anchor.txt'] = anchorCase();
  files['draw.txt'] = drawCase();
  files['unknown-kind.txt'] = unknownKindCase();
  files['hashcash.txt'] = timed('hashcash solve', hashcashCase);
  const neg = new Map<string, string>();
  const results = computeNegatives(S1, PA, tS1, tS1B);
  for (const [name, r] of Object.entries(results)) {
    neg.set(`negatives.S1.pk_A.${name}.${r.field}`, r.value);
    neg.set(`negatives.S1.pk_A.${name}.matches_honest`, String(r.value === r.honest));
  }
  files['negatives.txt'] = neg;
  void STAGES; void M64; void FIX_PROD;
  return files;
}

export function render(m: Map<string, string>): string {
  const keys = [...m.keys()].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  return keys.map((k) => `${k}\t${m.get(k)}\n`).join('');
}

export function writeAll(dir: string, files: Files): void {
  mkdirSync(dir, { recursive: true });
  for (const [name, m] of Object.entries(files)) writeFileSync(join(dir, name), render(m));
}

export type { Profile };
