// SPEC §14 verifier: one hike object in, one verify record out. Measurements only: no pass/fail wording.

import { checkpointRef, sha256, u64le, waypointRef } from './bigref.js';
import { OsRandom, locateLeg, sixLegDraw, type ByteSource } from './draw.js';
import { Terrain, type EngineOptions } from './engine.js';
import { type IndexDoc, type IndexSource, headerFacts } from './indexsource.js';
import { JsonNumber } from './jsonstrict.js';
import { classify, unknownKindRecord } from './object.js';
import { decodePathBody, validateFor, type PathBody } from './pathbody.js';
import { ANCHOR_WINDOW_OFFSETS, FILL, KIND, MAX_SAFE, STAGES, type Profile } from './profile.js';
import type { CborMap, CborValue } from './cbor.js';

const ND = 'not defined';
const NR = 'not read';

type Tok<T> = T | string;

export interface VerifyOptions {
  profile: Profile;
  index: IndexSource;
  /** rig-only: injected draw bytes. Production reads the OS CSPRNG. */
  drawBytes?: ByteSource;
  engine?: EngineOptions;
}

export type VerifyOutcome =
  | { kind: 'record'; line: string; record: Record<string, unknown> }
  | { kind: 'unknown-kind'; line: string }
  | { kind: 'decode-failure' };

// ---- lenient view of the decoded map ----------------------------------------------------------

interface StageView {
  exists: boolean;
  beacon?: Buffer;
  beaconHeight?: bigint;
  waypoint?: Buffer;
  /** path bytes when the key is a byte string */
  path?: Uint8Array;
  hasPathKey: boolean;
  legCount?: bigint;
  cps?: Uint8Array[];
  lrs?: Uint8Array[];
}

const isBytes = (v: CborValue | undefined): v is Uint8Array => v instanceof Uint8Array;
const isUint = (v: CborValue | undefined): v is bigint => typeof v === 'bigint';
const bytesArray = (v: CborValue | undefined): Uint8Array[] | undefined =>
  Array.isArray(v) && v.every(isBytes) ? (v as Uint8Array[]) : undefined;

function sameKeys(m: CborMap, keys: string[]): boolean {
  return m.size === keys.length && keys.every((k) => m.has(k));
}

const TOP_KEYS = [
  'fill',
  'kind',
  'stages',
  'n_slots',
  'public_key',
  'slot_bytes',
  'seed_beacon',
  'sample_bytes',
  'steps_per_leg',
  'last_interrupt',
  'last_interrupt_height',
];
const STAGE0_KEYS = ['beacon', 'waypoint', 'leg_count', 'checkpoints', 'leg_results', 'beacon_height'];
const STAGEN_KEYS = ['path', ...STAGE0_KEYS];

const heightOk = (h: bigint | undefined): h is bigint => h !== undefined && h <= MAX_SAFE;
const num = (v: bigint): number => Number(v);

interface View {
  pk?: Buffer;
  seedBeacon?: Buffer;
  stages: StageView[];
  lastInterrupt?: Buffer;
  lastInterruptHeight?: bigint;
  shapeItems: string[];
}

function buildView(map: CborMap, profile: Profile): View {
  const items = new Set<string>();
  let fieldSet = !sameKeys(map, TOP_KEYS);
  const text = (k: string) => (typeof map.get(k) === 'string' ? (map.get(k) as string) : undefined);
  const uint = (k: string) => (isUint(map.get(k)) ? (map.get(k) as bigint) : undefined);
  const b32 = (v: CborValue | undefined): Buffer | undefined => (isBytes(v) && v.length === 32 ? Buffer.from(v) : undefined);

  const kind = text('kind');
  const fill = text('fill');
  if (kind === undefined || fill === undefined) fieldSet = true;
  for (const k of ['slot_bytes', 'n_slots', 'sample_bytes', 'steps_per_leg', 'last_interrupt_height']) {
    if (map.has(k) && uint(k) === undefined) fieldSet = true;
  }
  const pkRaw = map.get('public_key');
  const sbRaw = map.get('seed_beacon');
  const pk = b32(pkRaw);
  const seedBeacon = b32(sbRaw);
  if (map.has('public_key') && pk === undefined) fieldSet = true;
  if (map.has('seed_beacon') && seedBeacon === undefined) fieldSet = true;
  const liRaw = map.get('last_interrupt');
  if (map.has('last_interrupt') && !isBytes(liRaw)) fieldSet = true;
  const lastInterrupt = b32(liRaw);
  const lastInterruptHeightRaw = uint('last_interrupt_height');

  // constant
  const expect: [string, bigint | string][] = [
    ['kind', KIND],
    ['fill', FILL],
    ['slot_bytes', BigInt(profile.slotBytes)],
    ['n_slots', BigInt(profile.nSlots)],
    ['sample_bytes', BigInt(profile.sampleBytes)],
    ['steps_per_leg', BigInt(profile.stepsPerLeg)],
  ];
  const constantViolated = expect.some(([k, v]) => map.get(k) !== v);

  // stages
  const stagesRaw = map.get('stages');
  const stagesArr = Array.isArray(stagesRaw) ? stagesRaw : undefined;
  if (!stagesArr) fieldSet = true;
  const stagesLength = stagesArr?.length === STAGES;
  const stages: StageView[] = [];
  let stage0Path = false;
  let legCountViolated = false;
  let cpWidth = false;
  let lrWidth = false;
  for (let g = 0; g < STAGES; g++) {
    const raw = stagesArr?.[g];
    if (!(raw instanceof Map)) {
      if (raw !== undefined) fieldSet = true;
      stages.push({ exists: false, hasPathKey: false });
      legCountViolated = true;
      continue;
    }
    const keys = g === 0 ? STAGE0_KEYS : STAGEN_KEYS;
    const hasPathKey = raw.has('path');
    if (g === 0) {
      if (hasPathKey) stage0Path = true;
      const rest = new Map(raw);
      rest.delete('path');
      if (!sameKeys(rest, keys)) fieldSet = true;
    } else if (!sameKeys(raw, keys)) fieldSet = true;
    const sv: StageView = { exists: true, hasPathKey };
    const beaconRaw = raw.get('beacon');
    if (raw.has('beacon') && !(isBytes(beaconRaw) && beaconRaw.length === 32)) fieldSet = true;
    if (isBytes(beaconRaw) && beaconRaw.length === 32) sv.beacon = Buffer.from(beaconRaw);
    const bh = raw.get('beacon_height');
    if (raw.has('beacon_height') && !isUint(bh)) fieldSet = true;
    if (isUint(bh) && heightOk(bh)) sv.beaconHeight = bh;
    const wpRaw = raw.get('waypoint');
    if (raw.has('waypoint') && !(isBytes(wpRaw) && wpRaw.length === 32)) fieldSet = true;
    if (isBytes(wpRaw) && wpRaw.length === 32) sv.waypoint = Buffer.from(wpRaw);
    const pathRaw = raw.get('path');
    if (g > 0) {
      if (raw.has('path') && !isBytes(pathRaw)) fieldSet = true;
      if (isBytes(pathRaw)) sv.path = pathRaw;
    }
    const lc = raw.get('leg_count');
    if (raw.has('leg_count') && !isUint(lc)) fieldSet = true;
    if (isUint(lc)) sv.legCount = lc;
    const cpsRaw = raw.get('checkpoints');
    const lrsRaw = raw.get('leg_results');
    if (raw.has('checkpoints') && !bytesArray(cpsRaw)) fieldSet = true;
    if (raw.has('leg_results') && !bytesArray(lrsRaw)) fieldSet = true;
    const cps = bytesArray(cpsRaw);
    const lrs = bytesArray(lrsRaw);
    if (cps) sv.cps = cps;
    if (lrs) sv.lrs = lrs;
    if (
      sv.legCount === undefined ||
      sv.legCount < 1n ||
      !cps ||
      !lrs ||
      BigInt(cps.length) !== sv.legCount ||
      lrs.length !== cps.length
    ) {
      legCountViolated = true;
    }
    if (cps?.some((c) => c.length !== 32)) cpWidth = true;
    if (lrs?.some((c) => c.length !== 8)) lrWidth = true;
    stages.push(sv);
  }

  // headers, in stored order
  const hdr: { hash?: Buffer; height?: bigint }[] = [];
  let heightRange = false;
  for (let g = 0; g < STAGES; g++) {
    const s = stages[g]!;
    if (s.exists) {
      const raw = (stagesArr![g] as CborMap).get('beacon_height');
      if (isUint(raw) && raw > MAX_SAFE) heightRange = true;
    }
    hdr.push({ ...(s.beacon ? { hash: s.beacon } : {}), ...(s.beaconHeight !== undefined ? { height: s.beaconHeight } : {}) });
  }
  if (lastInterruptHeightRaw !== undefined && lastInterruptHeightRaw > MAX_SAFE) heightRange = true;
  const liHeight = heightOk(lastInterruptHeightRaw) ? lastInterruptHeightRaw : undefined;
  hdr.push({ ...(lastInterrupt ? { hash: lastInterrupt } : {}), ...(liHeight !== undefined ? { height: liHeight } : {}) });

  let heightOrder = false;
  let hashDiffer = false;
  for (let k = 1; k <= STAGES; k++) {
    const a = hdr[k - 1]!;
    const b = hdr[k]!;
    if (a.height === undefined || b.height === undefined || !(b.height > a.height)) heightOrder = true;
    if (!a.hash || !b.hash || a.hash.equals(b.hash)) hashDiffer = true;
  }
  const lastInterruptItem =
    !(isBytes(liRaw) && liRaw.length === 32) ||
    !stages[17]!.beacon ||
    (lastInterrupt !== undefined && lastInterrupt.equals(stages[17]!.beacon)) ||
    liHeight === undefined ||
    stages[17]!.beaconHeight === undefined ||
    !(liHeight > stages[17]!.beaconHeight);
  const seedBeaconItem = !stages[0]!.beacon || !seedBeacon || !stages[0]!.beacon.equals(seedBeacon);
  const w17 = !stages[17]!.exists || stages[17]!.path === undefined || stages[17]!.path!.length !== 0;

  if (fieldSet) items.add('field set');
  if (stage0Path) items.add('stage-0 path key');
  if (w17) items.add('W_17 path');
  if (constantViolated) items.add('constant');
  if (!stagesLength) items.add('stages length');
  if (legCountViolated) items.add('leg_count');
  if (cpWidth) items.add('checkpoint width');
  if (lrWidth) items.add('leg result width');
  if (seedBeaconItem) items.add('seed beacon');
  if (lastInterruptItem) items.add('last_interrupt');
  if (heightOrder) items.add('header height order');
  if (hashDiffer) items.add('header hash differ');
  if (heightRange) items.add('height_range');
  const ORDER = [
    'malformed cbor',
    'field set',
    'stage-0 path key',
    'W_17 path',
    'constant',
    'stages length',
    'leg_count',
    'checkpoint width',
    'leg result width',
    'seed beacon',
    'last_interrupt',
    'header height order',
    'header hash differ',
    'height_range',
  ];
  const view: View = {
    stages,
    shapeItems: ORDER.filter((s) => items.has(s)),
  };
  if (pk) view.pk = pk;
  if (seedBeacon) view.seedBeacon = seedBeacon;
  if (lastInterrupt) view.lastInterrupt = lastInterrupt;
  if (liHeight !== undefined) view.lastInterruptHeight = liHeight;
  return view;
}

// ---- index reads ------------------------------------------------------------------------------

class Reads {
  private cache = new Map<string, Promise<IndexDoc | undefined>>();
  constructor(private readonly src: IndexSource) {}
  get(height: bigint | number): Promise<IndexDoc | undefined> {
    const k = height.toString();
    let p = this.cache.get(k);
    if (!p) {
      p = this.src.read(k);
      this.cache.set(k, p);
    }
    return p;
  }
}

interface HeaderMeasure {
  stored_hash: string;
  stored_height: Tok<number>;
  index_comparison: 'match' | 'mismatch' | 'not read';
  T: Tok<number>;
}

interface Stored {
  hash?: Buffer | undefined;
  height?: bigint | undefined;
}

async function measureHeader(reads: Reads, s: Stored): Promise<HeaderMeasure> {
  const out: HeaderMeasure = {
    stored_hash: s.hash ? s.hash.toString('hex') : ND,
    stored_height: s.height !== undefined ? num(s.height) : ND,
    index_comparison: 'not read',
    T: NR,
  };
  if (!s.hash || s.height === undefined) return out;
  const f = headerFacts(await reads.get(s.height));
  if (!f) return out;
  if (!f.natural.equals(s.hash) || f.height !== s.height) {
    out.index_comparison = 'mismatch';
    return out;
  }
  out.index_comparison = 'match';
  if (f.time !== undefined) out.T = num(f.time);
  return out;
}

/** A window header other than the beacon's own: a returned document whose header.height is the requested height and whose time follows §9. */
function windowTime(doc: IndexDoc | undefined, height: bigint): bigint | undefined {
  const h = doc?.get('header');
  if (!(h instanceof Map)) return undefined;
  const hh = h.get('height');
  const tt = h.get('time');
  if (!(hh instanceof JsonNumber) || hh.intIn(0n, MAX_SAFE) !== height) return undefined;
  return tt instanceof JsonNumber ? tt.intIn(0n, 0xffffffffn) : undefined;
}

/** Rule A4.10 median-time-past at a matched beacon header. */
async function mtp(reads: Reads, h: HeaderMeasure, height: bigint | undefined): Promise<bigint | undefined> {
  if (height === undefined || h.index_comparison !== 'match' || typeof h.T !== 'number') return undefined;
  const n = height + 1n < 11n ? Number(height + 1n) : 11;
  const times: bigint[] = [];
  const heights: bigint[] = [];
  for (let k = 0; k < n; k++) heights.push(height - BigInt(n - 1 - k));
  const docs = await Promise.all(heights.map((x) => (x === height ? Promise.resolve(undefined) : reads.get(x))));
  for (let k = 0; k < n; k++) {
    if (heights[k] === height) {
      times.push(BigInt(h.T));
      continue;
    }
    const t = windowTime(docs[k], heights[k]!);
    if (t === undefined) return undefined;
    times.push(t);
  }
  times.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  return times[Math.floor(n / 2)];
}

// ---- plants -----------------------------------------------------------------------------------

const hexNatural = (v: JsonNumber | string | undefined | unknown): Buffer | undefined =>
  typeof v === 'string' && /^[0-9a-fA-F]{64}$/.test(v) ? Buffer.from(v, 'hex') : undefined;

function plantMatches(doc: IndexDoc, body: PathBody): boolean {
  const network = doc.get('network');
  if (network !== 'main' && network !== 'synthetic') return false;
  const plants: Map<string, unknown>[] = [];
  const arr = doc.get('plants');
  if (Array.isArray(arr)) for (const p of arr) if (p instanceof Map) plants.push(p);
  const one = doc.get('plant');
  if (one instanceof Map) plants.push(one);
  const u32 = (v: unknown): bigint | undefined => (v instanceof JsonNumber ? v.intIn(0n, 0xffffffffn) : undefined);
  return plants.some((p) => {
    const root = hexNatural(p.get('root'));
    const txid = hexNatural(p.get('txid'));
    const bh = hexNatural(p.get('block_hash'));
    const vout = u32(p.get('vout'));
    const height = u32(p.get('block_height'));
    return (
      root?.equals(body.root) === true &&
      txid !== undefined &&
      Buffer.from(txid).reverse().equals(body.txid) &&
      bh !== undefined &&
      Buffer.from(bh).reverse().equals(body.blockHash) &&
      vout === BigInt(body.vout) &&
      height === BigInt(body.blockHeight)
    );
  });
}

const blockOf = (b: PathBody): { block_hash: string; block_height: number } => ({
  block_hash: Buffer.from(b.blockHash).reverse().toString('hex'),
  block_height: b.blockHeight,
});

// ---- the record -------------------------------------------------------------------------------

function provenance(profile: Profile): Record<string, unknown> {
  const r: Record<string, unknown> = { profile: profile.name };
  if (profile.name === 'conformance-small') {
    r.sample_bytes = profile.sampleBytes;
    r.slot_bytes = profile.slotBytes;
    r.terrain_bytes = profile.terrainBytes;
    r.steps_per_leg = profile.stepsPerLeg;
  }
  return r;
}

interface PathInfo {
  state: 'present' | 'skip' | 'absent';
  valid: boolean;
  body?: PathBody;
  issued: boolean;
  doc?: IndexDoc | undefined;
  plant?: 'match' | 'mismatch' | 'not read';
  anchored: boolean;
}

export async function verifyBytes(bytes: Uint8Array, opts: VerifyOptions): Promise<VerifyOutcome> {
  const c = classify(bytes);
  if (c.class === 'decode-failure') return { kind: 'decode-failure' };
  if (c.class === 'unknown-kind') return { kind: 'unknown-kind', line: unknownKindRecord(c.kindText) };
  const { profile } = opts;
  const reads = new Reads(opts.index);

  let view: View;
  let shapeItems: string[];
  if (c.class === 'malformed') {
    shapeItems = ['malformed cbor'];
    view = { stages: Array.from({ length: STAGES }, () => ({ exists: false, hasPathKey: false })), shapeItems };
  } else {
    view = buildView(c.map, profile);
    shapeItems = view.shapeItems;
  }
  const stages = view.stages;

  // §14.2 headers
  const stored: Stored[] = [];
  for (let g = 0; g < STAGES; g++) stored.push({ hash: stages[g]!.beacon, height: stages[g]!.beaconHeight });
  stored.push({ hash: view.lastInterrupt, height: view.lastInterruptHeight });
  const headers = await Promise.all(stored.map((s) => measureHeader(reads, s)));

  // terrain
  const haveSeed = view.pk !== undefined && view.seedBeacon !== undefined;
  const D = haveSeed ? sha256(view.pk!, view.seedBeacon!) : undefined;
  const terrain = D ? await Terrain.build(profile, D, opts.engine) : undefined;
  const tbytes = terrain?.bytes;

  // §14.3 chain
  const cpUsable = (g: number): boolean => {
    const s = stages[g]!;
    if (!s.cps || !tbytes) return false;
    return g === 0 ? D !== undefined : s.waypoint !== undefined;
  };
  const prevOf = (g: number, j: number): Uint8Array | undefined => {
    const s = stages[g]!;
    if (j > 0) return s.cps![j - 1];
    return g === 0 ? D : s.waypoint;
  };
  const limbOf = (g: number): Uint8Array => (g === 0 ? D! : stages[g]!.waypoint!);
  const checkpointsRec: unknown[] = [];
  for (let g = 0; g < STAGES; g++) {
    if (!cpUsable(g)) {
      checkpointsRec.push({ g, legs: ND });
      continue;
    }
    const s = stages[g]!;
    const legs = s.cps!.map((cp, j) => {
      const lr = s.lrs?.[j];
      const prev = prevOf(g, j)!;
      let result: 'match' | 'mismatch' = 'mismatch';
      if (lr !== undefined && prev.length >= 8) {
        const off = Number(u64le(prev) % BigInt(profile.offsetModulus));
        const d = sha256(prev, limbOf(g), tbytes!.subarray(off, off + profile.sampleBytes), lr);
        if (Buffer.from(cp).equals(d)) result = 'match';
      }
      return { j, result };
    });
    checkpointsRec.push({ g, legs });
  }
  const waypointsRec: unknown[] = [];
  for (let g = 0; g < STAGES - 1; g++) {
    const s = stages[g]!;
    const nx = stages[g + 1]!;
    if (!tbytes || !s.cps || !s.waypoint || !nx.beacon || !nx.waypoint) {
      waypointsRec.push({ g: g + 1, result: ND });
      continue;
    }
    const w = waypointRef(profile, tbytes, nx.beacon, s.waypoint, s.cps);
    waypointsRec.push({ g: g + 1, result: w.equals(nx.waypoint) ? 'match' : 'mismatch' });
  }
  // waypoint 17 has no recompute from stage 17: W_17 is recomputed from stage 16 (g = 16 above) -> index g+1 = 17.

  // seed_value
  let seedValue: string = ND;
  if (D && stages[0]!.waypoint) seedValue = stages[0]!.waypoint.equals(D) ? 'match' : 'mismatch';
  else if (D && stages[0]!.exists) seedValue = 'mismatch';

  // §14.3 paths
  const info: PathInfo[] = [];
  const storedInterrupts: { hash: Buffer; height: bigint }[] = [];
  for (let k = 1; k <= STAGES; k++) {
    const h = stored[k]!;
    if (h.hash && h.height !== undefined) storedInterrupts.push({ hash: h.hash, height: h.height });
  }
  for (let g = 1; g <= 16; g++) {
    const s = stages[g]!;
    const pi: PathInfo = { state: 'absent', valid: false, issued: false, anchored: false };
    if (s.path !== undefined) {
      if (s.path.length === 0) pi.state = 'skip';
      else {
        pi.state = 'present';
        const body = decodePathBody(s.path);
        if (body && s.waypoint && validateFor(body, s.waypoint)) {
          pi.valid = true;
          pi.body = body;
          const H = s.beaconHeight;
          const inWindow =
            H !== undefined && (ANCHOR_WINDOW_OFFSETS as readonly number[]).some((o) => BigInt(body.blockHeight) === H + BigInt(o));
          const isInterrupt = storedInterrupts.some((i) => i.height === BigInt(body.blockHeight) && i.hash.equals(body.blockHash));
          if (inWindow && isInterrupt) {
            pi.issued = true;
          }
        } else pi.plant = 'mismatch';
      }
    }
    info.push(pi);
  }
  await Promise.all(
    info.map(async (pi) => {
      if (!pi.issued) return;
      pi.doc = await reads.get(pi.body!.blockHeight);
      if (!pi.doc) pi.plant = 'not read';
      else if (plantMatches(pi.doc, pi.body!)) {
        pi.plant = 'match';
        pi.anchored = true;
      } else pi.plant = 'mismatch';
    }),
  );

  const pathsRec: unknown[] = [];
  const anchors: string[] = [];
  const skipList: number[] = [];
  for (let g = 1; g <= 16; g++) {
    const pi = info[g - 1]!;
    anchors.push(pi.anchored ? 'anchored' : 'not anchored');
    if (pi.state === 'skip') {
      skipList.push(g);
      pathsRec.push({ g, path_state: 'skip', plant: 'n/a', P_g: ND, height_P_g: ND, d_g: ND, claimed_block: ND });
      continue;
    }
    if (pi.state === 'absent') {
      pathsRec.push({ g, path_state: ND, plant: NR, P_g: ND, height_P_g: ND, d_g: ND, claimed_block: ND });
      continue;
    }
    let plant: string = pi.plant ?? NR;
    if (pi.valid && !pi.issued) plant = NR;
    const rec: Record<string, unknown> = { g, path_state: 'present', plant, P_g: ND, height_P_g: ND, d_g: ND, claimed_block: ND };
    if (pi.valid) rec.claimed_block = blockOf(pi.body!);
    if (pi.plant === 'match') {
      rec.P_g = blockOf(pi.body!);
      rec.height_P_g = pi.body!.blockHeight;
      const hm = headers[g]!;
      if (hm.index_comparison === 'match' && typeof hm.stored_height === 'number') rec.d_g = pi.body!.blockHeight - hm.stored_height;
    }
    pathsRec.push(rec);
  }
  // g = 17
  {
    const s = stages[17]!;
    const p = s.path;
    if (p !== undefined && p.length === 0) pathsRec.push({ g: 17, w17_path: 'not fetched by design', claimed_block: ND });
    else {
      const body = p ? decodePathBody(p) : undefined;
      pathsRec.push({ g: 17, w17_path: ND, claimed_block: body ? blockOf(body) : ND });
    }
  }
  const anchoredTotal = anchors.filter((a) => a === 'anchored').length;

  // A4.11 bookends
  const checkpointCount = (g: number): number => stages[g]!.cps?.length ?? 0;
  const sumCounts = (from: number, to: number): number => {
    let n = 0;
    for (let g = from; g <= to; g++) n += checkpointCount(g);
    return n;
  };
  const book: Record<string, unknown> = {};
  const setAll = (tok: string): void => {
    for (const k of ['a', 'b', 'N', 'S', 'delta_h', 'P_b', 'height_P_b', 'index_comparison_P_b', 'T_P_b', 'height_I_a_minus_1', 'T_I_a_minus_1']) {
      book[k] = tok;
    }
    book.index_comparison_P_b = tok === ND ? ND : NR;
    book.T_P_b = tok === ND ? ND : NR;
    book.T_I_a_minus_1 = tok === ND ? ND : NR;
    book.fixed_start = { N: tok, S: tok, delta_h: tok };
    book.legs_outside = tok;
  };
  const validCount = info.filter((p) => p.valid).length;
  const anyIssuedNoDoc = info.some((p) => p.issued && p.doc === undefined);
  if (validCount < 2) setAll(ND);
  else if (anyIssuedNoDoc) setAll(NR);
  else if (anchoredTotal < 2) setAll(ND);
  else {
    const idx = anchors.map((a, k) => (a === 'anchored' ? k + 1 : 0)).filter((x) => x > 0);
    const a = idx[0]!;
    const b = idx[idx.length - 1]!;
    const Pb = info[b - 1]!;
    const pbDoc = Pb.doc;
    const pbFacts = headerFacts(pbDoc);
    const ha = headers[a]!;
    if (ha.index_comparison === 'not read' || !pbFacts) setAll(NR);
    else {
      const N = sumCounts(a, b - 1);
      const body = Pb.body!;
      const disp = Buffer.from(body.blockHash).reverse();
      const pbMatch = Buffer.from(pbFacts.natural).reverse().equals(disp) && pbFacts.height === BigInt(body.blockHeight);
      // header.hash is display order; headerFacts reversed it once, so reversing again gives display bytes
      const pbCmp = pbMatch ? 'match' : 'mismatch';
      const heightPb: Tok<number> = pbMatch ? num(pbFacts.height) : ND;
      const tPb: Tok<number> = pbMatch && pbFacts.time !== undefined ? num(pbFacts.time) : NR;
      const heightIa: Tok<number> = ha.index_comparison === 'match' ? (ha.stored_height as number) : ND;
      const tIa: Tok<number> = ha.index_comparison === 'match' ? ha.T : NR;
      book.a = a;
      book.b = b;
      book.N = N;
      book.S = typeof tPb === 'number' && typeof tIa === 'number' ? tPb - tIa : NR;
      book.delta_h = typeof heightPb === 'number' && typeof heightIa === 'number' ? heightPb - heightIa : ND;
      book.P_b = blockOf(body);
      book.height_P_b = heightPb;
      book.index_comparison_P_b = pbCmp;
      book.T_P_b = tPb;
      book.height_I_a_minus_1 = heightIa;
      book.T_I_a_minus_1 = tIa;
      const h1 = headers[1]!;
      let fsS: Tok<number> = NR;
      if (typeof tPb === 'number' && typeof h1.T === 'number') fsS = tPb - h1.T;
      let fsD: Tok<number> = ND;
      if (h1.index_comparison === 'not read') fsD = NR;
      else if (h1.index_comparison === 'match' && pbMatch) fsD = num(pbFacts.height) - (h1.stored_height as number);
      book.fixed_start = { N: sumCounts(1, b - 1), S: fsS, delta_h: fsD };
      book.legs_outside = sumCounts(0, STAGES - 1) - N;
    }
  }

  // §14.6 six-leg draw
  const counts = stages.map((_, g) => checkpointCount(g));
  const legTotal = counts.reduce((x, y) => x + y, 0);
  let sixLeg: unknown = ND;
  let sixRuns: unknown = [];
  if (legTotal >= 6 && terrain) {
    const src = opts.drawBytes ?? new OsRandom();
    const idxs = sixLegDraw(src, legTotal);
    if (!idxs) {
      sixLeg = NR;
      sixRuns = NR;
    } else {
      const jobs = idxs.map((q) => locateLeg(counts, q));
      const starts: bigint[] = [];
      const ok: boolean[] = [];
      for (const { g, j } of jobs) {
        const prev = prevOf(g, j);
        if (!prev || prev.length < 8 || (g > 0 && !stages[g]!.waypoint) ) {
          starts.push(0n);
          ok.push(false);
        } else {
          starts.push(u64le(prev));
          ok.push(true);
        }
      }
      const results = await terrain.runLegs(starts);
      const tokens = jobs.map(({ g, j }, k) => {
        const lr = stages[g]!.lrs?.[j];
        if (!ok[k] || !lr || lr.length !== 8) return 'mismatch';
        return Buffer.from(lr).readBigUInt64LE(0) === results[k] ? 'match' : 'mismatch';
      });
      const run = { indices: idxs, results: tokens };
      sixLeg = run;
      sixRuns = [run];
    }
  }

  // §14.4 analysis
  const Ts = headers.map((h) => h.T);
  const span = (x: Tok<number>, y: Tok<number>): Tok<number> => (typeof x === 'number' && typeof y === 'number' ? x - y : NR);
  const allUsable = (from: number, to: number): boolean => {
    for (let g = from; g <= to; g++) if (!stages[g]!.cps) return false;
    return true;
  };
  const mtpFirst = await mtp(reads, headers[0]!, stored[0]!.height);
  const mtpLast = await mtp(reads, headers[18]!, stored[18]!.height);
  const analysis = {
    whole_hike_legs: allUsable(1, 17) ? sumCounts(1, 17) : ND,
    whole_hike_span: span(Ts[18]!, Ts[1]!),
    per_stage: stages.map((_, g) => ({
      g,
      checkpoint_count: stages[g]!.cps ? counts[g]! : ND,
      span: g <= 16 ? span(Ts[g + 1]!, Ts[g]!) : span(Ts[18]!, Ts[17]!),
    })),
    legs_outside: book.legs_outside,
    hike_legs: allUsable(0, 17) ? sumCounts(0, 17) : ND,
    hike_span: mtpFirst !== undefined && mtpLast !== undefined ? num(mtpLast - mtpFirst) : NR,
  };

  const record: Record<string, unknown> = {
    ...provenance(profile),
    shape: shapeItems.length === 0 ? 'match' : 'mismatch',
    shape_item: shapeItems,
    headers,
    seed_value: seedValue,
    checkpoints: checkpointsRec,
    waypoints: waypointsRec,
    paths: pathsRec,
    anchors,
    anchored_total: anchoredTotal,
    skip_list: skipList,
    a: book.a,
    b: book.b,
    N: book.N,
    S: book.S,
    delta_h: book.delta_h,
    fixed_start: book.fixed_start,
    P_b: book.P_b,
    height_P_b: book.height_P_b,
    index_comparison_P_b: book.index_comparison_P_b,
    T_P_b: book.T_P_b,
    height_I_a_minus_1: book.height_I_a_minus_1,
    T_I_a_minus_1: book.T_I_a_minus_1,
    six_leg: sixLeg,
    six_leg_runs: sixRuns,
    analysis,
  };
  return { kind: 'record', line: JSON.stringify(record), record };
}
