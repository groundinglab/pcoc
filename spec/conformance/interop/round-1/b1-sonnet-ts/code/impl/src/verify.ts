import { randomBytes } from 'node:crypto';
import { decode, CborError, type CborValue } from './cbor.ts';
import { parseStrictJson, NonInteger, type Json } from './json.ts';
import { KIND, FILL, STAGES, Terrain, seedOf, sha256, u64le, foldOf, nSlots, type Profile } from './core.ts';

const MAX_SAFE = (1n << 53n) - 1n;
const ND = 'not defined';
const NR = 'not read';
type Tok = string;
type Num = number | Tok;

export interface VerifyOptions {
  profile: Profile;
  /** Returns the bytes of the index document for a height, or null when no document is returned. */
  readIndex: (height: number) => Uint8Array | null;
  /** Rig-only draw bytes (SPEC 16.1); when absent the OS CSPRNG is read. */
  drawBytes?: Uint8Array;
}

export class DecodeFailure extends Error {}

const isBytes = (v: CborValue | undefined): v is Uint8Array => v instanceof Uint8Array;
const isUint = (v: CborValue | undefined): v is bigint => typeof v === 'bigint';
const isMap = (v: CborValue | undefined): v is Map<string, CborValue> => v instanceof Map;
const hexOf = (b: Uint8Array) => Buffer.from(b).toString('hex');
const sameBytes = (a: Uint8Array | undefined, b: Uint8Array | undefined) => !!a && !!b && Buffer.compare(a, b) === 0;

interface IndexHeader {
  hash: Buffer;
  height: number;
  time: number | null;
}

function jsonObj(j: Json | undefined): { [k: string]: Json } | undefined {
  return j !== null && typeof j === 'object' && !Array.isArray(j) && !(j instanceof NonInteger) ? (j as { [k: string]: Json }) : undefined;
}

function headerOf(doc: { [k: string]: Json } | undefined): IndexHeader | null {
  const h = jsonObj(doc?.header);
  if (!h) return null;
  const hash = h.hash;
  if (typeof hash !== 'string' || !/^[0-9a-fA-F]{64}$/.test(hash)) return null;
  const height = h.height;
  if (typeof height !== 'bigint' || height < 0n || height > MAX_SAFE) return null;
  const t = h.time;
  const time = typeof t === 'bigint' && t >= 0n && t <= 0xffffffffn ? Number(t) : null;
  return { hash: Buffer.from(hash, 'hex').reverse(), height: Number(height), time };
}

interface PathBody {
  leaf: Buffer; leafIndex: number; treeSize: number; siblings: Buffer[]; root: Buffer; txid: Buffer; vout: number; blockHash: Buffer; blockHeight: number;
}

function decodePath(b: Uint8Array): PathBody | null {
  const buf = Buffer.from(b);
  if (buf.length < 146 || buf[0] !== 1) return null;
  const sc = buf[41];
  if (sc > 32 || buf.length !== 146 + 32 * sc) return null;
  const treeSize = buf.readUInt32BE(37);
  const leafIndex = buf.readUInt32BE(33);
  if (treeSize < 1 || leafIndex >= treeSize) return null;
  let width = treeSize;
  let count = 0;
  while (width > 1) {
    count++;
    width = Math.floor(width / 2) + (width % 2);
  }
  if (count !== sc) return null;
  const siblings: Buffer[] = [];
  let o = 42;
  for (let i = 0; i < sc; i++, o += 32) siblings.push(buf.subarray(o, o + 32));
  return {
    leaf: buf.subarray(1, 33), leafIndex, treeSize, siblings, root: buf.subarray(o, o + 32), txid: buf.subarray(o + 32, o + 64),
    vout: buf.readUInt32BE(o + 64), blockHash: buf.subarray(o + 68, o + 100), blockHeight: buf.readUInt32BE(o + 100),
  };
}

function validateFor(p: PathBody, waypoint: Uint8Array | undefined): boolean {
  if (!sameBytes(p.leaf, waypoint)) return false;
  let node: Buffer = sha256(Buffer.from([0x1a]), p.leaf);
  let index = p.leafIndex;
  let width = p.treeSize;
  for (const sib of p.siblings) {
    if (width % 2 === 1 && index === width - 1) {
      if (!sib.equals(node)) return false;
      node = sha256(Buffer.from([0x1b]), node, node);
    } else if (index % 2 === 0) node = sha256(Buffer.from([0x1b]), node, sib);
    else node = sha256(Buffer.from([0x1b]), sib, node);
    index = Math.floor(index / 2);
    width = Math.floor(width / 2) + (width % 2);
  }
  return node.equals(p.root);
}

const claimedBlock = (p: PathBody) => ({ block_hash: Buffer.from(p.blockHash).reverse().toString('hex'), block_height: p.blockHeight });

function mtp(readHeader: (h: number) => IndexHeader | null, beaconHeader: { height: number; time: Num }): Num {
  const h = beaconHeader.height;
  const n = Math.min(11, h + 1);
  const times: number[] = [];
  for (let k = h - n + 1; k <= h; k++) {
    if (k === h) {
      if (typeof beaconHeader.time !== 'number') return NR;
      times.push(beaconHeader.time);
      continue;
    }
    const x = readHeader(k);
    if (!x || x.height !== k || x.time === null) return NR;
    times.push(x.time);
  }
  times.sort((a, b) => a - b);
  return times[Math.floor(n / 2)];
}

export function verify(bytes: Uint8Array, opts: VerifyOptions): Record<string, unknown> {
  const profile = opts.profile;
  let decoded: CborValue | undefined;
  let malformed = false;
  try {
    decoded = decode(bytes);
  } catch (e) {
    if (!(e instanceof CborError)) throw e;
    malformed = true;
  }
  if (!malformed) {
    if (!isMap(decoded)) throw new DecodeFailure('not a CBOR map');
    const kind = decoded.get('kind');
    if (typeof kind !== 'string') throw new DecodeFailure('no kind text');
    if (kind !== KIND) return { kind: 'unknown kind', kind_text: kind };
  }
  const top = malformed ? undefined : (decoded as Map<string, CborValue>);

  const readCache = new Map<number, Json | undefined>();
  const readDoc = (h: number): { [k: string]: Json } | undefined => {
    if (!readCache.has(h)) {
      const raw = opts.readIndex(h);
      readCache.set(h, raw ? parseStrictJson(raw) : undefined);
    }
    return jsonObj(readCache.get(h));
  };
  const readHeader = (h: number) => headerOf(readDoc(h));

  // ---- shape (14.1)
  const items = new Set<string>();
  const stagesV = top?.get('stages');
  const stageMaps: (Map<string, CborValue> | undefined)[] = [];
  for (let g = 0; g < STAGES; g++) {
    const s = Array.isArray(stagesV) ? stagesV[g] : undefined;
    stageMaps.push(isMap(s) ? s : undefined);
  }
  const heightRange = (v: CborValue | undefined) => isUint(v) && v > MAX_SAFE;
  const heightNum = (v: CborValue | undefined): number | undefined => (isUint(v) && v <= MAX_SAFE ? Number(v) : undefined);
  const bytesOf = (m: Map<string, CborValue> | undefined, k: string) => {
    const v = m?.get(k);
    return isBytes(v) ? v : undefined;
  };
  const publicKey = bytesOf(top, 'public_key');
  const seedBeacon = bytesOf(top, 'seed_beacon');

  if (malformed) items.add('malformed cbor');
  else {
    const t = top!;
    const topKeys = ['kind', 'fill', 'slot_bytes', 'n_slots', 'sample_bytes', 'steps_per_leg', 'public_key', 'seed_beacon', 'stages', 'last_interrupt', 'last_interrupt_height'];
    let fieldSet = t.size === topKeys.length && topKeys.every((k) => t.has(k));
    fieldSet &&= typeof t.get('kind') === 'string' && typeof t.get('fill') === 'string' && isBytes(t.get('last_interrupt')) && isUint(t.get('last_interrupt_height'))
      && ['slot_bytes', 'n_slots', 'sample_bytes', 'steps_per_leg'].every((k) => isUint(t.get(k))) && Array.isArray(stagesV)
      && publicKey?.length === 32 && seedBeacon?.length === 32;
    const stageKeys = (g: number) => (g === 0 ? ['beacon', 'waypoint', 'leg_count', 'checkpoints', 'leg_results', 'beacon_height']
      : ['path', 'beacon', 'waypoint', 'leg_count', 'checkpoints', 'leg_results', 'beacon_height']);
    if (Array.isArray(stagesV)) {
      stagesV.forEach((s, g) => {
        if (g >= STAGES) return;
        if (!isMap(s)) {
          fieldSet = false;
          return;
        }
        if (g === 0 && s.has('path')) items.add('stage-0 path key');
        const keys = stageKeys(g);
        const ok = keys.every((k) => s.has(k)) && s.size === keys.length + (g === 0 && s.has('path') ? 1 : 0)
          && isBytes(s.get('beacon')) && isBytes(s.get('waypoint')) && isUint(s.get('beacon_height')) && isUint(s.get('leg_count'))
          && (g === 0 || isBytes(s.get('path')))
          && Array.isArray(s.get('checkpoints')) && (s.get('checkpoints') as CborValue[]).every(isBytes)
          && Array.isArray(s.get('leg_results')) && (s.get('leg_results') as CborValue[]).every(isBytes);
        if (!ok) fieldSet = false;
      });
    }
    if (!fieldSet) items.add('field set');
    const s17 = stageMaps[17];
    if (!s17 || !isBytes(s17.get('path')) || (s17.get('path') as Uint8Array).length !== 0) items.add('W_17 path');
    const prof = { kind: KIND, fill: FILL, slot_bytes: BigInt(profile.slotBytes), n_slots: BigInt(nSlots(profile)), sample_bytes: BigInt(profile.sampleBytes), steps_per_leg: BigInt(profile.stepsPerLeg) };
    if (Object.entries(prof).some(([k, v]) => t.get(k) !== v)) items.add('constant');
    if (!Array.isArray(stagesV) || stagesV.length !== STAGES) items.add('stages length');
    for (const s of stageMaps) {
      if (!s) {
        items.add('leg_count');
        continue;
      }
      const cps = s.get('checkpoints');
      const lrs = s.get('leg_results');
      const lc = s.get('leg_count');
      if (!isUint(lc) || lc < 1n || !Array.isArray(cps) || !Array.isArray(lrs) || BigInt(cps.length) !== lc || BigInt(lrs.length) !== lc) items.add('leg_count');
      if (Array.isArray(cps) && cps.some((c) => !isBytes(c) || c.length !== 32)) items.add('checkpoint width');
      if (Array.isArray(lrs) && lrs.some((c) => !isBytes(c) || c.length !== 8)) items.add('leg result width');
    }
    if (!sameBytes(bytesOf(stageMaps[0], 'beacon'), seedBeacon)) items.add('seed beacon');
  }
  const last = bytesOf(top, 'last_interrupt');
  const lastHeightRaw = top?.get('last_interrupt_height');
  const storedHash: (Uint8Array | undefined)[] = [];
  const storedHeight: (number | undefined)[] = [];
  const rawHeights: (CborValue | undefined)[] = [];
  for (let g = 0; g < STAGES; g++) {
    storedHash.push(bytesOf(stageMaps[g], 'beacon'));
    rawHeights.push(stageMaps[g]?.get('beacon_height'));
    storedHeight.push(heightNum(rawHeights[g]));
  }
  storedHash.push(last);
  rawHeights.push(lastHeightRaw);
  storedHeight.push(heightNum(lastHeightRaw));
  if (!malformed) {
    if (!last || last.length !== 32 || sameBytes(last, storedHash[17]) || !(typeof storedHeight[18] === 'number' && typeof storedHeight[17] === 'number' && storedHeight[18] > storedHeight[17])) items.add('last_interrupt');
    for (let k = 1; k <= 18; k++) {
      if (!(typeof storedHeight[k] === 'number' && typeof storedHeight[k - 1] === 'number' && storedHeight[k]! > storedHeight[k - 1]!)) {
        items.add('header height order');
      }
      if (!storedHash[k] || !storedHash[k - 1] || sameBytes(storedHash[k], storedHash[k - 1])) items.add('header hash differ');
    }
    if (rawHeights.some(heightRange)) items.add('height_range');
  }
  const ORDER = ['malformed cbor', 'field set', 'stage-0 path key', 'W_17 path', 'constant', 'stages length', 'leg_count', 'checkpoint width', 'leg result width', 'seed beacon', 'last_interrupt', 'header height order', 'header hash differ', 'height_range'];
  const shapeItem = malformed ? ['malformed cbor'] : ORDER.filter((s) => items.has(s));

  // ---- headers (14.2)
  const headers = storedHash.map((h, k) => {
    const height = storedHeight[k];
    let cmp: Tok = NR;
    let T: Num = NR;
    if (h && height !== undefined) {
      const x = readHeader(height);
      if (x) {
        cmp = x.hash.equals(Buffer.from(h)) && x.height === height ? 'match' : 'mismatch';
        if (cmp === 'match' && x.time !== null) T = x.time;
      }
    }
    return { stored_hash: h ? hexOf(h) : ND, stored_height: height ?? ND, index_comparison: cmp, T };
  });
  const Tn = (k: number): number | undefined => (typeof headers[k].T === 'number' ? (headers[k].T as number) : undefined);

  // ---- seed, terrain
  let D: Buffer | undefined;
  let terrain: Terrain | undefined;
  if (publicKey?.length === 32 && seedBeacon?.length === 32) {
    const sd = seedOf(Buffer.from(publicKey), Buffer.from(seedBeacon));
    D = sd.D;
    terrain = new Terrain(profile, sd.P);
  }
  const wp = (g: number) => bytesOf(stageMaps[g], 'waypoint');
  const seedValue = D && sameBytes(wp(0), D) ? 'match' : 'mismatch';

  const cpsOf = (g: number): Uint8Array[] | undefined => {
    const v = stageMaps[g]?.get('checkpoints');
    return Array.isArray(v) && v.every(isBytes) ? (v as Uint8Array[]) : undefined;
  };
  const lrsOf = (g: number): CborValue[] => {
    const v = stageMaps[g]?.get('leg_results');
    return Array.isArray(v) ? v : [];
  };
  const startOf = (g: number): Uint8Array | undefined => (g === 0 ? D : wp(g));

  const checkpointsRec = Array.from({ length: STAGES }, (_, g) => {
    const cps = cpsOf(g);
    const start = startOf(g);
    if (!terrain || !cps || !start || start.length !== 32) return { g, legs: ND as Tok | unknown[] };
    const legs = cps.map((cp, j) => {
      const lr = lrsOf(g)[j];
      if (!isBytes(lr)) return { j, result: 'mismatch' };
      const prev = j === 0 ? start : cps[j - 1];
      if (prev.length < 8) return { j, result: 'mismatch' };
      const digest = sha256(prev, start, terrain!.sample(prev), lr);
      return { j, result: digest.equals(Buffer.from(cp)) ? 'match' : 'mismatch' };
    });
    return { g, legs: legs as Tok | unknown[] };
  });

  const waypointsRec = Array.from({ length: STAGES - 1 }, (_, i) => {
    const g = i;
    const cps = cpsOf(g);
    const B = bytesOf(stageMaps[g + 1], 'beacon');
    const W = wp(g);
    const next = wp(g + 1);
    if (!terrain || !cps || !B || B.length < 8 || !W || W.length !== 32 || !next) return { g: g + 1, result: ND };
    const slotIdx = Number(u64le(B) % BigInt(nSlots(profile)));
    const calc = sha256(B, terrain.slot(slotIdx), W, foldOf(cps));
    return { g: g + 1, result: calc.equals(Buffer.from(next)) ? 'match' : 'mismatch' };
  });

  // ---- paths (14.3)
  const interrupts: { hash: Uint8Array | undefined; height: number | undefined }[] = [];
  for (let k = 1; k <= 18; k++) interrupts.push({ hash: storedHash[k], height: storedHeight[k] });
  const H = (g: number) => storedHeight[g];
  const pathsRec: Record<string, unknown>[] = [];
  const anchors: Tok[] = [];
  const present: boolean[] = [];
  let issuedUnread = false;
  const plantInfo: ({ block: ReturnType<typeof claimedBlock>; header: IndexHeader | null; docRead: boolean } | undefined)[] = [];
  for (let g = 1; g <= 16; g++) {
    const pathBytes = bytesOf(stageMaps[g], 'path');
    if (!stageMaps[g]) {
      pathsRec.push({ g, path_state: ND, plant: ND, P_g: ND, height_P_g: ND, d_g: ND, claimed_block: ND });
      anchors.push('not anchored');
      present.push(false);
      continue;
    }
    if (!pathBytes || pathBytes.length === 0) {
      pathsRec.push({ g, path_state: 'skip', plant: 'n/a', P_g: ND, height_P_g: ND, d_g: ND, claimed_block: ND });
      anchors.push('not anchored');
      present.push(false);
      continue;
    }
    const body = decodePath(pathBytes);
    const valid = body !== null && validateFor(body, wp(g));
    if (!body || !valid) {
      pathsRec.push({ g, path_state: 'present', plant: 'mismatch', P_g: ND, height_P_g: ND, d_g: ND, claimed_block: ND });
      anchors.push('not anchored');
      present.push(false);
      continue;
    }
    present.push(true);
    const cb = claimedBlock(body);
    const base = H(g);
    const inWindow = base !== undefined && [2, 3, 4].includes(body.blockHeight - base);
    const isInterrupt = interrupts.some((x) => x.height === body.blockHeight && x.hash && sameBytes(x.hash, body.blockHash));
    if (!inWindow || !isInterrupt) {
      pathsRec.push({ g, path_state: 'present', plant: NR, P_g: ND, height_P_g: ND, d_g: ND, claimed_block: cb });
      anchors.push('not anchored');
      continue;
    }
    const doc = readDoc(body.blockHeight);
    if (!doc) {
      issuedUnread = true;
      pathsRec.push({ g, path_state: 'present', plant: NR, P_g: ND, height_P_g: ND, d_g: ND, claimed_block: cb });
      anchors.push('not anchored');
      continue;
    }
    const plants: { [k: string]: Json }[] = [];
    if (Array.isArray(doc.plants)) for (const p of doc.plants) if (jsonObj(p)) plants.push(jsonObj(p)!);
    if (jsonObj(doc.plant)) plants.push(jsonObj(doc.plant)!);
    const u32 = (v: Json | undefined) => (typeof v === 'bigint' && v >= 0n && v <= 0xffffffffn ? Number(v) : -1);
    const hexEq = (v: Json | undefined, natural: Uint8Array, reverse: boolean) =>
      typeof v === 'string' && /^[0-9a-fA-F]{64}$/.test(v) && (reverse ? Buffer.from(v, 'hex').reverse() : Buffer.from(v, 'hex')).equals(Buffer.from(natural));
    const netOk = doc.network === 'main' || doc.network === 'synthetic';
    const matched = netOk && plants.some((p) => hexEq(p.root, body.root, false) && hexEq(p.txid, body.txid, true) && u32(p.vout) === body.vout
      && hexEq(p.block_hash, body.blockHash, true) && u32(p.block_height) === body.blockHeight);
    if (!matched) {
      pathsRec.push({ g, path_state: 'present', plant: 'mismatch', P_g: ND, height_P_g: ND, d_g: ND, claimed_block: cb });
      anchors.push('not anchored');
      continue;
    }
    const prevHdr = headers[g];
    const d = prevHdr.index_comparison === 'match' ? body.blockHeight - (prevHdr.stored_height as number) : ND;
    pathsRec.push({ g, path_state: 'present', plant: 'match', P_g: cb, height_P_g: body.blockHeight, d_g: d, claimed_block: cb });
    anchors.push('anchored');
    plantInfo[g] = { block: cb, header: headerOf(doc), docRead: true };
  }
  const s17path = bytesOf(stageMaps[17], 'path');
  let c17: unknown = ND;
  if (s17path && s17path.length > 0 && s17path.length >= 146) {
    const sc = s17path[41];
    const buf = Buffer.from(s17path);
    const o = 42 + 32 * sc + 32 + 32 + 4;
    if (buf.length >= o + 36) c17 = { block_hash: Buffer.from(buf.subarray(o, o + 32)).reverse().toString('hex'), block_height: buf.readUInt32BE(o + 32) };
  }
  pathsRec.push(s17path && s17path.length === 0 ? { g: 17, w17_path: 'not fetched by design', claimed_block: ND } : { g: 17, w17_path: ND, claimed_block: c17 });
  const anchoredIdx = anchors.map((a, i) => (a === 'anchored' ? i + 1 : 0)).filter(Boolean);
  const skipList = pathsRec.slice(0, 16).filter((p) => p.path_state === 'skip').map((p) => p.g as number);

  // ---- counts, bookends (14.4)
  const counts = Array.from({ length: STAGES }, (_, g) => cpsOf(g)?.length ?? 0);
  const sumCounts = (from: number, to: number) => counts.slice(from, to).reduce((a, b) => a + b, 0);
  const presentCount = present.filter(Boolean).length;
  const allBook = (v: Tok) => ({
    a: v, b: v, N: v, S: v, delta_h: v, fixed_start: { N: v, S: v, delta_h: v }, P_b: v, height_P_b: v, index_comparison_P_b: v, T_P_b: v,
    height_I_a_minus_1: v, T_I_a_minus_1: v, legs_outside: v,
  });
  let book: Record<string, unknown> & { fixed_start: Record<string, unknown>; legs_outside: unknown };
  if (presentCount < 2) book = allBook(ND);
  else if (issuedUnread) book = allBook(NR);
  else if (anchoredIdx.length < 2) book = allBook(ND);
  else {
    const a = anchoredIdx[0];
    const b = anchoredIdx[anchoredIdx.length - 1];
    const N = sumCounts(a, b);
    const pb = plantInfo[b];
    if (headers[a].index_comparison === NR || !pb) book = allBook(NR);
    else {
      const pbHeader = pb.header;
      const pbHeight = pb.block.block_height;
      let cmpPb: Tok = NR;
      let heightPb: Num = ND;
      let TPb: Num = NR;
      if (pbHeader) {
        const eq = pbHeader.hash.equals(Buffer.from(pb.block.block_hash, 'hex').reverse()) && pbHeader.height === pbHeight;
        cmpPb = eq ? 'match' : 'mismatch';
        if (eq) {
          heightPb = pbHeader.height;
          if (pbHeader.time !== null) TPb = pbHeader.time;
        }
      }
      if (cmpPb === NR) book = allBook(NR);
      else {
        const hIa = headers[a].index_comparison === 'match' ? (headers[a].stored_height as number) : ND;
        const TIa = headers[a].T;
        const S = typeof TPb === 'number' && typeof TIa === 'number' ? TPb - TIa : NR;
        const delta = typeof heightPb === 'number' && typeof hIa === 'number' ? heightPb - hIa : ND;
        const fsN = sumCounts(1, b);
        const T1 = headers[1].T;
        const fsS = typeof TPb === 'number' && typeof T1 === 'number' ? TPb - T1 : NR;
        let fsDelta: Num;
        if (headers[1].index_comparison === NR) fsDelta = NR;
        else if (headers[1].index_comparison === 'mismatch' || typeof heightPb !== 'number') fsDelta = ND;
        else fsDelta = heightPb - (headers[1].stored_height as number);
        book = {
          a, b, N, S, delta_h: delta, fixed_start: { N: fsN, S: fsS, delta_h: fsDelta }, P_b: pb.block, height_P_b: heightPb,
          index_comparison_P_b: cmpPb, T_P_b: TPb, height_I_a_minus_1: hIa, T_I_a_minus_1: TIa, legs_outside: sumCounts(0, STAGES) - N,
        };
      }
    }
  }

  // ---- analysis (14.4, A4.10)
  const diff = (hi: number, lo: number): Num => {
    const x = Tn(hi);
    const y = Tn(lo);
    return x !== undefined && y !== undefined ? x - y : NR;
  };
  const mtpFirst = typeof headers[0].stored_height === 'number' ? mtp(readHeader, { height: headers[0].stored_height, time: headers[0].T }) : NR;
  const mtpLast = typeof headers[18].stored_height === 'number' ? mtp(readHeader, { height: headers[18].stored_height, time: headers[18].T }) : NR;
  const analysis = {
    whole_hike_legs: sumCounts(1, STAGES),
    whole_hike_span: diff(18, 1),
    per_stage: Array.from({ length: STAGES }, (_, g) => ({ g, checkpoint_count: counts[g], span: diff(g + 1, g) })),
    legs_outside: book.legs_outside,
    hike_legs: sumCounts(0, STAGES),
    hike_span: typeof mtpFirst === 'number' && typeof mtpLast === 'number' ? mtpLast - mtpFirst : NR,
  };

  // ---- six-leg (14.6)
  let sixLeg: unknown;
  let sixRuns: unknown;
  const total = sumCounts(0, STAGES);
  if (total < 6) {
    sixLeg = ND;
    sixRuns = [];
  } else {
    const src = opts.drawBytes;
    let off = 0;
    const take8 = (): bigint | undefined => {
      if (src) {
        if (off + 8 > src.length) return undefined;
        const v = u64le(src, off);
        off += 8;
        return v;
      }
      return randomBytes(8).readBigUInt64LE(0);
    };
    const M = 1n << 64n;
    const limit = M - (M % BigInt(total));
    const idx: number[] = [];
    let short = false;
    while (idx.length < 6) {
      const u = take8();
      if (u === undefined) {
        short = true;
        break;
      }
      if (u < limit) {
        const q = Number(u % BigInt(total));
        if (!idx.includes(q)) idx.push(q);
      }
    }
    if (short || !terrain) {
      sixLeg = NR;
      sixRuns = NR;
    } else {
      const results = idx.map((q) => {
        let g = 0;
        let rem = q;
        while (rem >= counts[g]) rem -= counts[g++];
        const cps = cpsOf(g)!;
        const start = startOf(g);
        const lr = lrsOf(g)[rem];
        if (!start || !isBytes(lr) || lr.length !== 8) return 'mismatch';
        const prev = rem === 0 ? start : cps[rem - 1];
        return terrain!.runLeg(u64le(prev)) === u64le(lr) ? 'match' : 'mismatch';
      });
      sixLeg = { indices: idx, results };
      sixRuns = [sixLeg];
    }
  }

  const rec: Record<string, unknown> = { profile: profile.name };
  if (profile.name === 'conformance-small') {
    rec.sample_bytes = profile.sampleBytes;
    rec.slot_bytes = profile.slotBytes;
    rec.terrain_bytes = profile.terrainBytes;
    rec.steps_per_leg = profile.stepsPerLeg;
  }
  Object.assign(rec, {
    shape: shapeItem.length === 0 ? 'match' : 'mismatch', shape_item: shapeItem, headers, seed_value: seedValue,
    checkpoints: checkpointsRec, waypoints: waypointsRec, paths: pathsRec, anchors, anchored_total: anchoredIdx.length, skip_list: skipList,
    a: book.a, b: book.b, N: book.N, S: book.S, delta_h: book.delta_h, fixed_start: book.fixed_start, P_b: book.P_b, height_P_b: book.height_P_b,
    index_comparison_P_b: book.index_comparison_P_b, T_P_b: book.T_P_b, height_I_a_minus_1: book.height_I_a_minus_1, T_I_a_minus_1: book.T_I_a_minus_1,
    six_leg: sixLeg, six_leg_runs: sixRuns, analysis,
  });
  return rec;
}
