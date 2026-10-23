import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { fillTerrain, lanes, legRef, mix64, nl4Base, sha256, stepRef, u64le, le64 } from '../bigref.js';
import { SEAL_DOMAIN, S1, S2, makeProfile } from '../profile.js';
import { Terrain, instantiate, kernelModule, shutdownPool } from '../engine.js';
import { DEFAULT_CONFIG, SEAL_OFF, terrainPages } from '../kernels.js';

after(() => shutdownPool());

const pkA = Buffer.from('f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9', 'hex');
const beacon = sha256(Buffer.from('pcoc-pack-2.2/beacon/seed'));
const seedA = sha256(pkA, beacon);

test('SPEC §7.4 known answers', () => {
  const D = sha256(Buffer.alloc(32, 0x11), Buffer.alloc(32, 0x22));
  assert.equal(D.toString('hex'), '5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c');
  const P = lanes(D);
  assert.deepEqual(
    P.map((x) => x.toString(16)),
    ['545dfe297dc78951', '27858669c45e046a', '9cdac73ac1a5fe85', '7c81df6efbf55f11'],
  );
  assert.equal(nl4Base(P, 0, 0), 0xc7582082a632c2c8n);
  const E = sha256(SEAL_DOMAIN, le64(0n));
  assert.equal(E.toString('hex'), '4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533');
  assert.equal(u64le(E), 0x76042423afbd1648n);
  assert.equal(sha256(Buffer.from('abc')).toString('hex'), 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
  assert.equal(mix64(0n), 0xe220a8397b1dcdafn);
});

test('wasm module validates and software SHA-256 seal matches node:crypto', () => {
  const pages = terrainPages(S1.terrainBytes);
  const mod = kernelModule(pages, DEFAULT_CONFIG);
  const memory = new WebAssembly.Memory({ initial: pages, maximum: pages, shared: true });
  const ex = instantiate(mod, memory);
  const dv = new DataView(memory.buffer);
  for (const acc of [0n, 1n, 0xdeadbeefcafef00dn, 0xffffffffffffffffn, 0x0123456789abcdefn]) {
    dv.setBigUint64(SEAL_OFF + 12, acc, true);
    ex.seal_probe(0);
    const E = sha256(SEAL_DOMAIN, le64(acc));
    assert.equal(dv.getBigUint64(0, true), u64le(E));
  }
});

for (const [name, profile] of [
  ['S1', S1],
  ['S2', S2],
  ['slot 8 words/terrain 64 slots', makeProfile('conformance-small', 64, 16, 4096, 50)],
  ['odd 3 words', makeProfile('conformance-small', 24, 8, 24 * 100, 10)],
] as const) {
  test(`fill, step and leg equal the BigInt oracle (${name})`, async () => {
    const ref = fillTerrain(profile, lanes(seedA));
    const t = await Terrain.build(profile, seedA, { threads: 1 });
    assert.deepEqual(Buffer.from(t.bytes), Buffer.from(ref));
    let S = u64le(seedA);
    for (let k = 0; k < 20; k++) {
      const a = stepRef(profile, ref, S);
      const b = t.stepDetailed(S);
      assert.deepEqual(a, b);
      S = a.next;
    }
    const want = legRef(profile, ref, u64le(seedA), Math.min(profile.stepsPerLeg, 200));
    assert.equal(t.runLeg(u64le(seedA), Math.min(profile.stepsPerLeg, 200), 'node'), want);
    assert.equal(t.runLeg(u64le(seedA), Math.min(profile.stepsPerLeg, 200), 'wasm'), want);
  });
}

test('parallel fill and parallel legs equal the single-thread result', async () => {
  const profile = makeProfile('conformance-small', 2048, 64, 2048 * 2048, 100);
  const one = await Terrain.build(profile, seedA, { threads: 1 });
  const many = await Terrain.build(profile, seedA, { threads: 4 });
  assert.deepEqual(Buffer.from(many.bytes), Buffer.from(one.bytes));
  const starts = [1n, 2n, 0xffffffffffffffffn, u64le(seedA), 77n, 1n << 40n];
  const par = await many.runLegs(starts, 300);
  const seq = starts.map((s) => one.runLeg(s, 300));
  assert.deepEqual(par, seq);
  const parSw = await many.runLegs(starts, 300, 'wasm');
  assert.deepEqual(parSw, seq);
});
