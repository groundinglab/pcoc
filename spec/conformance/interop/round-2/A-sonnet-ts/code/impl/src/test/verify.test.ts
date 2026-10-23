import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { after, test } from 'node:test';
import { decodeCbor, encodeCbor, type CborMap } from '../cbor.js';
import { sha256 } from '../bigref.js';
import { ChainSource } from '../draw.js';
import { shutdownPool } from '../engine.js';
import { FixtureIndex, HttpIndex, fixtureBody, serveFixtureIndex, type IndexDoc, type IndexSource } from '../indexsource.js';
import { JsonNumber } from '../jsonstrict.js';
import { encodePathBody, type PathBody } from '../pathbody.js';
import { S1, S2 } from '../profile.js';
import { verifyBytes } from '../verify.js';
import { readObject } from './helpers.js';

after(() => shutdownPool());

const O1 = readObject('O1-S1.cbor');
const O2 = readObject('O2-S2.cbor');
const draw = () => new ChainSource('pcoc-pack-2.2/draw');
type Rec = Record<string, any>;

async function run(bytes: Uint8Array, index: IndexSource = new FixtureIndex(), profile = S1): Promise<Rec> {
  const out = await verifyBytes(bytes, { profile, index, drawBytes: draw() });
  assert.equal(out.kind, 'record');
  if (out.kind !== 'record') throw new Error();
  assert.equal(JSON.stringify(JSON.parse(out.line)), out.line, 'compact one-line JSON');
  return out.record;
}

const stagesOf = (m: CborMap) => m.get('stages') as CborMap[];
function mutate(base: Uint8Array, f: (m: CborMap) => void): Uint8Array {
  const m = decodeCbor(base) as CborMap;
  f(m);
  return encodeCbor(m as never);
}

class Wrap implements IndexSource {
  readonly log: string[] = [];
  constructor(private readonly fn: (h: string, doc: IndexDoc | undefined) => IndexDoc | undefined = (_, d) => d) {}
  async read(h: string): Promise<IndexDoc | undefined> {
    this.log.push(h);
    return this.fn(h, await new FixtureIndex().read(h));
  }
}
const hdrOf = (d: IndexDoc) => d.get('header') as Map<string, any>;

test('honest O1: every measurement is match and the record has the §14 key order', async () => {
  const r = await run(O1);
  assert.deepEqual(Object.keys(r), [
    'profile', 'sample_bytes', 'slot_bytes', 'terrain_bytes', 'steps_per_leg', 'shape', 'shape_item', 'headers',
    'seed_value', 'checkpoints', 'waypoints', 'paths', 'anchors', 'anchored_total', 'skip_list', 'a', 'b', 'N', 'S',
    'delta_h', 'fixed_start', 'P_b', 'height_P_b', 'index_comparison_P_b', 'T_P_b', 'height_I_a_minus_1',
    'T_I_a_minus_1', 'six_leg', 'six_leg_runs', 'analysis',
  ]);
  assert.equal(r.shape, 'match');
  assert.deepEqual(r.shape_item, []);
  assert.equal(r.headers.length, 19);
  assert.deepEqual(Object.keys(r.headers[0]), ['stored_hash', 'stored_height', 'index_comparison', 'T']);
  assert.ok(r.headers.every((h: Rec) => h.index_comparison === 'match' && typeof h.T === 'number'));
  assert.equal(r.seed_value, 'match');
  assert.equal(r.checkpoints.length, 18);
  assert.ok(r.checkpoints.every((c: Rec, g: number) => c.g === g && c.legs.length === 1 && c.legs[0].result === 'match'));
  assert.equal(r.waypoints.length, 17);
  assert.ok(r.waypoints.every((w: Rec, k: number) => w.g === k + 1 && w.result === 'match'));
  assert.equal(r.paths.length, 17);
  assert.deepEqual(r.paths[0], { g: 1, path_state: 'skip', plant: 'n/a', P_g: 'not defined', height_P_g: 'not defined', d_g: 'not defined', claimed_block: 'not defined' });
  assert.deepEqual(r.paths[16], { g: 17, w17_path: 'not fetched by design', claimed_block: 'not defined' });
  assert.equal(r.anchors.length, 16);
  assert.equal(r.anchored_total, 0);
  assert.deepEqual(r.skip_list, Array.from({ length: 16 }, (_, k) => k + 1));
  for (const k of ['a', 'b', 'N', 'S', 'delta_h', 'P_b', 'height_P_b', 'T_P_b', 'height_I_a_minus_1', 'T_I_a_minus_1']) assert.equal(r[k], 'not defined', k);
  assert.deepEqual(r.fixed_start, { N: 'not defined', S: 'not defined', delta_h: 'not defined' });
  assert.deepEqual(r.six_leg, { indices: [7, 2, 1, 15, 10, 16], results: Array(6).fill('match') });
  assert.deepEqual(r.six_leg_runs, [r.six_leg]);
  assert.equal(r.analysis.whole_hike_legs, 17);
  assert.equal(r.analysis.hike_legs, 18);
  assert.equal(r.analysis.whole_hike_span, 10200);
  assert.equal(r.analysis.hike_span, 10800);
  assert.equal(r.analysis.per_stage.length, 18);
  assert.deepEqual(r.analysis.per_stage[17], { g: 17, checkpoint_count: 1, span: 600 });
  assert.equal(r.analysis.legs_outside, 'not defined');
});

test('honest O2 (three legs per stage, 54 legs)', async () => {
  const r = await run(O2, new FixtureIndex(), S2);
  assert.equal(r.shape, 'match');
  assert.deepEqual(r.six_leg.indices, [7, 20, 1, 51, 10, 16]);
  assert.deepEqual(r.six_leg.results, Array(6).fill('match'));
  assert.ok(r.checkpoints.every((c: Rec) => c.legs.length === 3 && c.legs.every((l: Rec) => l.result === 'match')));
  assert.equal(r.analysis.hike_legs, 54);
});

test('tampering is recorded at the leg, the checkpoint, the waypoint and the six-leg draw', async () => {
  const badLeg = mutate(O1, (m) => {
    const lr = stagesOf(m)[7]!.get('leg_results') as Uint8Array[];
    lr[0] = Buffer.from('0000000000000001', 'hex');
  });
  let r = await run(badLeg);
  assert.equal(r.checkpoints[7].legs[0].result, 'mismatch');
  assert.equal(r.checkpoints.filter((c: Rec) => c.legs[0].result === 'mismatch').length, 1);
  assert.equal(r.six_leg.results[0], 'mismatch'); // index 7 is drawn first
  assert.equal(r.six_leg.results.filter((x: string) => x === 'mismatch').length, 1);

  const badCp = mutate(O1, (m) => {
    (stagesOf(m)[3]!.get('checkpoints') as Uint8Array[])[0] = new Uint8Array(32);
  });
  r = await run(badCp);
  assert.equal(r.checkpoints[3].legs[0].result, 'mismatch');
  assert.equal(r.waypoints[3].result, 'mismatch'); // W_4 folds stage 3's checkpoint
  assert.equal(r.waypoints[2].result, 'match');
  assert.equal(r.shape, 'match');

  const badW = mutate(O1, (m) => {
    stagesOf(m)[5]!.set('waypoint', new Uint8Array(32));
  });
  r = await run(badW);
  assert.equal(r.waypoints[4].result, 'mismatch');
  assert.equal(r.waypoints[5].result, 'mismatch');
  assert.equal(r.checkpoints[5].legs[0].result, 'mismatch');

  const badSeed = mutate(O1, (m) => {
    stagesOf(m)[0]!.set('waypoint', new Uint8Array(32));
  });
  r = await run(badSeed);
  assert.equal(r.seed_value, 'mismatch');
  assert.equal(r.waypoints[0].result, 'mismatch'); // W_1 uses the stored stage-0 field
  assert.equal(r.checkpoints[0].legs[0].result, 'match'); // stage 0 checkpoints use the computed seed
});

test('shape items, in table order, every violated condition', async () => {
  let r = await run(mutate(O1, (m) => stagesOf(m)[0]!.set('path', new Uint8Array(0))));
  assert.deepEqual(r.shape_item, ['stage-0 path key']);
  assert.equal(r.shape, 'mismatch');

  r = await run(mutate(O1, (m) => stagesOf(m)[17]!.set('path', Buffer.from([1]))));
  assert.deepEqual(r.shape_item, ['W_17 path']);
  assert.deepEqual(r.paths[16], { g: 17, w17_path: 'not defined', claimed_block: 'not defined' });

  r = await run(mutate(O1, (m) => (m.set('n_slots', 7n), m.set('fill', 'x'))));
  assert.deepEqual(r.shape_item, ['constant']);

  r = await run(mutate(O1, (m) => (m.delete('last_interrupt'), m.delete('last_interrupt_height'))));
  assert.deepEqual(r.shape_item, ['field set', 'last_interrupt', 'header height order', 'header hash differ']);
  assert.deepEqual(r.headers[18], { stored_hash: 'not defined', stored_height: 'not defined', index_comparison: 'not read', T: 'not read' });
  assert.equal(r.headers.length, 19);

  r = await run(mutate(O1, (m) => m.delete('last_interrupt')));
  assert.deepEqual(r.shape_item, ['field set', 'last_interrupt', 'header hash differ']);

  r = await run(mutate(O1, (m) => m.set('last_interrupt', new Uint8Array(31))));
  assert.ok(r.shape_item.includes('last_interrupt'));
  assert.ok(!r.shape_item.includes('field set'));

  r = await run(mutate(O1, (m) => ((stagesOf(m)[4]!.get('checkpoints') as Uint8Array[])[0] = new Uint8Array(31))));
  assert.deepEqual(r.shape_item, ['checkpoint width']);
  assert.equal(r.checkpoints[4].legs[0].result, 'mismatch');

  r = await run(mutate(O1, (m) => ((stagesOf(m)[4]!.get('leg_results') as Uint8Array[])[0] = new Uint8Array(7))));
  assert.deepEqual(r.shape_item, ['leg result width']);

  r = await run(mutate(O1, (m) => stagesOf(m)[2]!.set('leg_count', 2n)));
  assert.deepEqual(r.shape_item, ['leg_count']);
  assert.equal(r.checkpoints[2].legs.length, 1, 'legs runs over the checkpoints, not leg_count');

  r = await run(mutate(O1, (m) => stagesOf(m)[2]!.set('leg_count', 0n)));
  assert.deepEqual(r.shape_item, ['leg_count']);

  r = await run(mutate(O1, (m) => stagesOf(m)[0]!.set('beacon', new Uint8Array(32))));
  assert.ok(r.shape_item.includes('seed beacon'));

  r = await run(mutate(O1, (m) => stagesOf(m)[9]!.set('beacon_height', 1000n)));
  assert.deepEqual(r.shape_item, ['header height order']);

  r = await run(mutate(O1, (m) => stagesOf(m)[9]!.set('beacon', stagesOf(m)[8]!.get('beacon')!)));
  assert.ok(r.shape_item.includes('header hash differ'));

  r = await run(mutate(O1, (m) => stagesOf(m)[5]!.set('beacon_height', 2n ** 60n)));
  assert.ok(r.shape_item.includes('height_range'));
  assert.ok(r.shape_item.includes('header height order'));
  assert.deepEqual(r.headers[5], { stored_hash: r.headers[5].stored_hash, stored_height: 'not defined', index_comparison: 'not read', T: 'not read' });

  r = await run(mutate(O1, (m) => m.set('last_interrupt_height', 2n ** 53n)));
  assert.ok(r.shape_item.includes('height_range'));
  assert.ok(r.shape_item.includes('last_interrupt'));
  r = await run(mutate(O1, (m) => m.set('last_interrupt_height', 2n ** 53n - 1n)));
  assert.ok(!r.shape_item.includes('height_range'));
  assert.equal(r.headers[18].index_comparison, 'not read'); // the index has no document at that height

  r = await run(mutate(O1, (m) => m.set('stages', (m.get('stages') as CborMap[]).slice(0, 17))));
  assert.deepEqual(r.shape_item.slice(0, 3), ['W_17 path', 'stages length', 'leg_count']);
  assert.equal(r.headers.length, 19);
  assert.equal(r.checkpoints.length, 18);
  assert.equal(r.checkpoints[17].legs, 'not defined');
  assert.equal(r.waypoints[16].result, 'not defined');
  assert.equal(r.analysis.per_stage.length, 18);
  assert.equal(r.analysis.whole_hike_legs, 'not defined');

  r = await run(mutate(O1, (m) => m.set('extra', 1n)));
  assert.deepEqual(r.shape_item, ['field set']);
});

test('a non-shortest integer and unsorted keys still measure as match', async () => {
  const needle = Buffer.concat([Buffer.from([0x6d]), Buffer.from('steps_per_leg'), Buffer.from([0x18, 0x64])]);
  const at = Buffer.from(O1).indexOf(needle);
  assert.ok(at > 0);
  const longer = Buffer.concat([
    O1.subarray(0, at + 14),
    Buffer.from([0x1b, 0, 0, 0, 0, 0, 0, 0, 0x64]),
    O1.subarray(at + 16),
  ]);
  assert.equal((await run(longer)).shape, 'match');
});

test('malformed CBOR is a record with exactly ["malformed cbor"] and fixed array lengths', async () => {
  for (const bad of [Buffer.alloc(0), Buffer.from('ff', 'hex'), O1.subarray(0, 100), Buffer.concat([O1, Buffer.from([0])])]) {
    const r = await run(bad);
    assert.equal(r.shape, 'mismatch');
    assert.deepEqual(r.shape_item, ['malformed cbor']);
    assert.equal(r.headers.length, 19);
    assert.ok(r.headers.every((h: Rec) => h.stored_hash === 'not defined' && h.index_comparison === 'not read' && h.T === 'not read'));
    assert.equal(r.seed_value, 'not defined');
    assert.equal(r.checkpoints.length, 18);
    assert.ok(r.checkpoints.every((c: Rec) => c.legs === 'not defined'));
    assert.equal(r.waypoints.length, 17);
    assert.ok(r.waypoints.every((w: Rec) => w.result === 'not defined'));
    assert.equal(r.paths.length, 17);
    assert.equal(r.six_leg, 'not defined');
    assert.deepEqual(r.six_leg_runs, []);
    assert.equal(r.analysis.per_stage.length, 18);
    assert.equal(r.analysis.hike_span, 'not read');
  }
});

test('decode failure has no record; unknown kind is the Rule A5 record', async () => {
  for (const b of ['01', '80', 'a0', 'a164 6b696e64 01'.replace(/ /g, '')]) {
    const out = await verifyBytes(Buffer.from(b, 'hex'), { profile: S1, index: new FixtureIndex() });
    assert.equal(out.kind, 'decode-failure');
  }
  for (const [label, rec] of [
    ['v2.1-pcoc-object', '{"kind":"unknown kind","kind_text":"v2.1-pcoc-object"}'],
    ['', '{"kind":"unknown kind","kind_text":""}'],
    ['v2.2-pcoc-objecT', '{"kind":"unknown kind","kind_text":"v2.2-pcoc-objecT"}'],
    ['a"b\\c\n\u0001é\u2028', '{"kind":"unknown kind","kind_text":"a\\"b\\\\c\\n\\u0001é\u2028"}'],
  ] as const) {
    const out = await verifyBytes(mutate(O1, (m) => m.set('kind', label)), { profile: S1, index: new FixtureIndex() });
    assert.deepEqual(out, { kind: 'unknown-kind', line: rec });
  }
  // a malformed item that carries a foreign kind is not Rule A5
  const bad = Buffer.concat([mutate(O1, (m) => m.set('kind', 'x')), Buffer.from([0])]);
  assert.equal((await run(bad)).shape_item[0], 'malformed cbor');
});

test('index comparison, T, and median-time-past', async () => {
  let r = await run(O1, new Wrap(() => undefined));
  assert.ok(r.headers.every((h: Rec) => h.index_comparison === 'not read' && h.T === 'not read'));
  assert.equal(r.analysis.hike_span, 'not read');
  assert.equal(r.analysis.whole_hike_span, 'not read');
  assert.equal(r.six_leg.results.length, 6);

  r = await run(O1, new Wrap((h, d) => (h === '1005' ? new Map([...d!, ['header', new Map([...hdrOf(d!), ['hash', '00'.repeat(32)]])]]) : d)));
  assert.equal(r.headers[5].index_comparison, 'mismatch');
  assert.equal(r.headers[5].T, 'not read');
  assert.equal(r.analysis.per_stage[4].span, 'not read');
  assert.equal(r.analysis.per_stage[5].span, 'not read');
  assert.equal(r.analysis.per_stage[6].span, 600);

  r = await run(O1, new Wrap((h, d) => (h === '1005' ? new Map([...d!, ['header', new Map([...hdrOf(d!), ['height', new JsonNumber('1006')]])]]) : d)));
  assert.equal(r.headers[5].index_comparison, 'mismatch');

  r = await run(O1, new Wrap((h, d) => (h === '1005' ? new Map([...d!, ['header', new Map([...hdrOf(d!), ['height', new JsonNumber('1005.0')]])]]) : d)));
  assert.equal(r.headers[5].index_comparison, 'not read');

  r = await run(O1, new Wrap((h, d) => (h === '1005' ? new Map([['network', 'synthetic']]) : d)));
  assert.equal(r.headers[5].index_comparison, 'not read');

  r = await run(O1, new Wrap((h, d) => {
    if (h !== '1005') return d;
    const hd = new Map(hdrOf(d!));
    hd.delete('time');
    return new Map([...d!, ['header', hd]]);
  }));
  assert.equal(r.headers[5].index_comparison, 'match');
  assert.equal(r.headers[5].T, 'not read');

  // A producer cannot shorten hike_span by choosing a beacon whose nTime runs ahead: MTP, not raw time.
  r = await run(O1, new Wrap((h, d) => (h === '1018' ? new Map([...d!, ['header', new Map([...hdrOf(d!), ['time', new JsonNumber('1800000000')]])]]) : d)));
  assert.equal(r.headers[18].T, 1800000000);
  assert.equal(r.analysis.hike_span, 10800);
  assert.equal(r.analysis.whole_hike_span, 1800000000 - 1700600600);

  // a missing time in the MTP window of the first beacon makes hike_span not read
  r = await run(O1, new Wrap((h, d) => {
    if (h !== '993') return d;
    const hd = new Map(hdrOf(d!));
    hd.delete('time');
    return new Map([...d!, ['header', hd]]);
  }));
  assert.equal(r.analysis.hike_span, 'not read');
  assert.equal(r.analysis.whole_hike_span, 10200);
});

test('verification over real HTTP, and the §9 body rules', async () => {
  const good = await serveFixtureIndex();
  try {
    const r = await run(O1, new HttpIndex(good.url));
    assert.ok(r.headers.every((h: Rec) => h.index_comparison === 'match'));
    assert.equal(r.analysis.hike_span, 10800);
  } finally {
    good.server.close();
  }
  const b1005 = fixtureBody(1005)!;
  const variants: Record<string, (res: import('node:http').ServerResponse) => void> = {
    redirect: (res) => res.writeHead(302, { location: '/index/1006' }).end(),
    status500: (res) => res.writeHead(500).end(b1005),
    status201: (res) => res.writeHead(201).end(b1005),
    bom: (res) => res.writeHead(200).end(Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from(b1005)])),
    badUtf8: (res) => res.writeHead(200).end(Buffer.concat([Buffer.from(b1005.slice(0, -1)), Buffer.from([0xff, 0x7d])])),
    duplicate: (res) => res.writeHead(200).end(b1005.replace('"network":"synthetic"', '"network":"synthetic","network":"main"')),
    nestedDuplicate: (res) => res.writeHead(200).end(b1005.replace('"time":', '"time":1,"time":')),
    twoValues: (res) => res.writeHead(200).end(`${b1005} {}`),
    array: (res) => res.writeHead(200).end('[1]'),
    notJson: (res) => res.writeHead(200).end('nope'),
    fractionHeight: (res) => res.writeHead(200).end(b1005.replace('"height":1005', '"height":1005.0')),
    exponentHeight: (res) => res.writeHead(200).end(b1005.replace('"height":1005', '"height":1.005e3')),
    stringHeight: (res) => res.writeHead(200).end(b1005.replace('"height":1005', '"height":"1005"')),
    shortHash: (res) => res.writeHead(200).end(b1005.replace(/"hash":"[0-9a-f]/, '"hash":"')),
    noHeader: (res) => res.writeHead(200).end('{"network":"synthetic"}'),
    headerNotObject: (res) => res.writeHead(200).end('{"header":[]}'),
  };
  let which = 'redirect';
  const server = createServer((req, res) => {
    const m = /^\/index\/(\d+)$/.exec(req.url ?? '');
    if (m && m[1] === '1005') return variants[which]!(res);
    const body = m ? fixtureBody(Number(m[1])) : undefined;
    if (body === undefined) return void res.writeHead(404).end();
    res.writeHead(200).end(body);
  });
  await new Promise<void>((ok) => server.listen(0, '127.0.0.1', ok));
  const url = `http://127.0.0.1:${(server.address() as { port: number }).port}`;
  try {
    for (const name of Object.keys(variants)) {
      which = name;
      const r = await run(O1, new HttpIndex(url));
      assert.equal(r.headers[5].index_comparison, 'not read', name);
      assert.equal(r.headers[5].T, 'not read', name);
      assert.equal(r.headers[4].index_comparison, 'match', name);
    }
    which = 'noHeader';
    assert.equal((await run(O1, new HttpIndex(url))).analysis.per_stage[5].span, 'not read');
  } finally {
    server.close();
  }
  const dead = await serveFixtureIndex();
  dead.server.close();
  assert.equal((await run(O1, new HttpIndex(dead.url))).headers[0].index_comparison, 'not read');
});

// ---- paths and plants ---------------------------------------------------------------------------

function pathFor(leaf: Buffer, blockHash: Buffer, blockHeight: number, tag: string, tamperRoot = false): { bytes: Buffer; body: PathBody } {
  const par = (l: Buffer, r: Buffer) => sha256(Buffer.from([0x1b]), l, r);
  const n0 = sha256(Buffer.from([0x1a]), leaf);
  const n1 = par(n0, n0);
  const s2 = sha256(Buffer.from(`sibling ${tag}`));
  const root = tamperRoot ? sha256(Buffer.from('bad')) : par(s2, n1);
  const body: PathBody = {
    leaf, leafIndex: 2, treeSize: 3, siblings: [n0, s2], root,
    txid: sha256(Buffer.from(`txid ${tag}`)), vout: 1, blockHash, blockHeight,
  };
  return { bytes: encodePathBody(body), body };
}

function plantDoc(d: IndexDoc, body: PathBody, network = 'synthetic', edit: (p: Map<string, any>) => void = () => {}): IndexDoc {
  const p = new Map<string, any>([
    ['root', body.root.toString('hex')],
    ['txid', Buffer.from(body.txid).reverse().toString('hex')],
    ['vout', new JsonNumber(String(body.vout))],
    ['block_hash', Buffer.from(body.blockHash).reverse().toString('hex')],
    ['block_height', new JsonNumber(String(body.blockHeight))],
    ['tree_size', new JsonNumber('3')],
  ]);
  edit(p);
  return new Map<string, any>([...d, ['network', network], ['plants', [new Map([['root', 'zz']]), p]]]);
}

function withPaths(spec: { g: number; height: number; tamper?: boolean; hashOverride?: Buffer }[]): { bytes: Uint8Array; bodies: Map<number, PathBody> } {
  const bodies = new Map<number, PathBody>();
  const bytes = mutate(O1, (m) => {
    const st = stagesOf(m);
    for (const s of spec) {
      const leaf = Buffer.from(st[s.g]!.get('waypoint') as Uint8Array);
      const k = s.height - 1000;
      const hash = s.hashOverride ?? Buffer.from(st[k]!.get('beacon') as Uint8Array);
      const p = pathFor(leaf, hash, s.height, String(s.g), s.tamper);
      bodies.set(s.g, p.body);
      st[s.g]!.set('path', p.bytes);
    }
  });
  return { bytes, bodies };
}

const plantFn =
  (bodies: Map<number, PathBody>, network = 'synthetic', edit?: (p: Map<string, any>) => void) =>
  (h: string, d: IndexDoc | undefined): IndexDoc | undefined => {
    for (const b of bodies.values()) if (String(b.blockHeight) === h && d) return plantDoc(d, b, network, edit);
    return d;
  };
const plantsAt = (bodies: Map<number, PathBody>, network = 'synthetic', edit?: (p: Map<string, any>) => void) =>
  new Wrap(plantFn(bodies, network, edit));

test('present paths: anchored plants, bookends, fixed_start and d_g', async () => {
  const { bytes, bodies } = withPaths([{ g: 2, height: 1004 }, { g: 5, height: 1007 }]);
  const r = await run(bytes, plantsAt(bodies));
  assert.equal(r.shape, 'match');
  assert.equal(r.paths[1].path_state, 'present');
  assert.equal(r.paths[1].plant, 'match');
  const disp = (b: Buffer) => Buffer.from(b).reverse().toString('hex');
  assert.deepEqual(r.paths[1].P_g, { block_hash: disp(bodies.get(2)!.blockHash), block_height: 1004 });
  assert.equal(r.paths[1].height_P_g, 1004);
  assert.equal(r.paths[1].d_g, 2); // 1004 - height(I_1) = 1004 - 1002
  assert.deepEqual(r.paths[1].claimed_block, r.paths[1].P_g);
  assert.equal(r.paths[4].d_g, 2);
  assert.deepEqual(r.anchors.map((a: string, k: number) => (a === 'anchored' ? k + 1 : 0)).filter(Boolean), [2, 5]);
  assert.equal(r.anchored_total, 2);
  assert.deepEqual(r.skip_list, [1, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
  assert.equal(r.a, 2);
  assert.equal(r.b, 5);
  assert.equal(r.N, 3);
  assert.equal(r.S, 3000);
  assert.equal(r.delta_h, 5);
  assert.deepEqual(r.P_b, r.paths[4].P_g);
  assert.equal(r.height_P_b, 1007);
  assert.equal(r.index_comparison_P_b, 'match');
  assert.equal(r.T_P_b, 1700000000 + 600 * 1007);
  assert.equal(r.height_I_a_minus_1, 1002);
  assert.equal(r.T_I_a_minus_1, 1700000000 + 600 * 1002);
  assert.deepEqual(r.fixed_start, { N: 4, S: 3600, delta_h: 6 });
  assert.equal(r.analysis.legs_outside, 15);
  assert.equal(r.analysis.hike_span, 10800);
});

test('present paths: every non-match and non-read case', async () => {
  const two = withPaths([{ g: 2, height: 1004 }, { g: 5, height: 1007 }]);

  // the plant document for g=2 does not come back: bookends are not read
  let r = await run(two.bytes, new Wrap((h, d) => (h === '1004' ? undefined : plantFn(two.bodies)(h, d))));
  assert.equal(r.paths[1].plant, 'not read');
  assert.equal(r.paths[1].claimed_block.block_height, 1004);
  assert.equal(r.anchored_total, 1);
  for (const k of ['a', 'b', 'N', 'S', 'delta_h', 'P_b', 'height_P_b', 'index_comparison_P_b', 'T_P_b', 'height_I_a_minus_1', 'T_I_a_minus_1', 'analysis.legs_outside']) {
    assert.equal(k.startsWith('analysis') ? r.analysis.legs_outside : r[k], 'not read', k);
  }
  assert.deepEqual(r.fixed_start, { N: 'not read', S: 'not read', delta_h: 'not read' });

  // network neither main nor synthetic: plant mismatch, one anchored at most: bookends not defined
  r = await run(two.bytes, plantsAt(two.bodies, 'regtest'));
  assert.equal(r.paths[1].plant, 'mismatch');
  assert.equal(r.paths[1].P_g, 'not defined');
  assert.equal(r.paths[1].d_g, 'not defined');
  assert.equal(r.anchored_total, 0);
  assert.equal(r.a, 'not defined');
  r = await run(two.bytes, plantsAt(two.bodies, 'main'));
  assert.equal(r.anchored_total, 2);

  // each plant field is compared
  for (const edit of [
    (p: Map<string, any>) => p.set('vout', new JsonNumber('2')),
    (p: Map<string, any>) => p.set('vout', new JsonNumber('1.0')),
    (p: Map<string, any>) => p.set('vout', '1'),
    (p: Map<string, any>) => p.set('root', '00'.repeat(32)),
    (p: Map<string, any>) => p.set('txid', two.bodies.get(2)!.txid.toString('hex')), // natural, not display
    (p: Map<string, any>) => p.set('block_hash', two.bodies.get(2)!.blockHash.toString('hex')), // natural, not display
    (p: Map<string, any>) => p.set('block_height', new JsonNumber('1005')),
    (p: Map<string, any>) => p.set('block_height', new JsonNumber('4294967296')),
  ]) {
    r = await run(two.bytes, new Wrap((h, d) => (h === '1004' ? plantDoc(d!, two.bodies.get(2)!, 'synthetic', edit) : plantFn(two.bodies)(h, d))));
    assert.equal(r.paths[1].plant, 'mismatch');
    assert.equal(r.anchored_total, 1);
    assert.equal(r.a, 'not defined');
  }

  // a document with no plant for the block
  r = await run(two.bytes, new Wrap());
  assert.equal(r.paths[1].plant, 'mismatch');
  assert.equal(r.paths[1].claimed_block.block_height, 1004);

  // a plant document with no usable header: plant match, header not read, bookends are read
  const noHeader = new Wrap((h, d) => {
    if (h === '1007') {
      const x = plantDoc(d!, two.bodies.get(5)!);
      x.delete('header');
      return x;
    }
    return plantFn(two.bodies)(h, d);
  });
  r = await run(two.bytes, noHeader);
  assert.equal(r.paths[4].plant, 'match');
  assert.equal(r.headers[7].index_comparison, 'not read');
  assert.equal(r.index_comparison_P_b, 'not read');
  assert.equal(r.a, 'not read');

  // outside the window: not anchored, plant not read, and the plant height is never fetched
  const out = withPaths([{ g: 2, height: 700, hashOverride: sha256(Buffer.from('elsewhere')) }]);
  const w = new Wrap();
  r = await run(out.bytes, w);
  assert.equal(r.paths[1].plant, 'not read');
  assert.equal(r.anchors[1], 'not anchored');
  assert.equal(r.paths[1].claimed_block.block_height, 700);
  assert.ok(!w.log.includes('700'));
  assert.equal(r.skip_list.includes(2), false);
  assert.equal(r.a, 'not defined');

  // in the window but not a stored interrupt: not anchored, plant not read
  const notStored = withPaths([{ g: 2, height: 1004, hashOverride: sha256(Buffer.from('not stored')) }]);
  r = await run(notStored.bytes, plantsAt(notStored.bodies));
  assert.equal(r.paths[1].plant, 'not read');
  assert.equal(r.anchors[1], 'not anchored');

  // a path that does not validate: plant mismatch, claimed_block not defined
  const bad = withPaths([{ g: 2, height: 1004, tamper: true }]);
  r = await run(bad.bytes, plantsAt(bad.bodies));
  assert.equal(r.paths[1].plant, 'mismatch');
  assert.equal(r.paths[1].claimed_block, 'not defined');
  r = await run(mutate(O1, (m) => stagesOf(m)[2]!.set('path', Buffer.from([1, 2, 3]))));
  assert.equal(r.paths[1].path_state, 'present');
  assert.equal(r.paths[1].plant, 'mismatch');

  // W_17: claimed_block read from the bytes, no plant, shape item
  const w17 = mutate(O1, (m) => stagesOf(m)[17]!.set('path', pathFor(Buffer.alloc(32), Buffer.alloc(32, 7), 1018, 'w17').bytes));
  r = await run(w17);
  assert.deepEqual(r.shape_item, ['W_17 path']);
  assert.equal(r.paths[16].w17_path, 'not defined');
  assert.equal(r.paths[16].claimed_block.block_height, 1018);
});
