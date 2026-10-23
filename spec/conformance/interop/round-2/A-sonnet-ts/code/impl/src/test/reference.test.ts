// Independent cross-checks: the candidates against a second, naive BigInt computation, and the plain
// scalar tier against the WebAssembly engine.
import assert from 'node:assert/strict';
import { after, test } from 'node:test';
import { checkpointRef, fillTerrain, lanes, legRef, le64, sha256, u64le, waypointRef } from '../bigref.js';
import { PK_A, SEED_BEACON, hex, interruptBeacon, produceFixture, uniformLegs } from '../construction.js';
import { Terrain, shutdownPool } from '../engine.js';
import { S1, S2 } from '../profile.js';
import { fillScalar, legScalar } from '../scalar.js';
import { encodeObject } from '../object.js';
import { candidateMap, readObject } from './helpers.js';

after(() => shutdownPool());

test('kat-small candidates equal a naive BigInt hike', () => {
  const kat = candidateMap('kat-small.txt');
  const D = sha256(PK_A, SEED_BEACON);
  const terrain = fillTerrain(S1, lanes(D));
  assert.equal(kat.get('kat-small.S1.terrain_sha256'), hex(sha256(terrain)));
  let limb: Buffer = D;
  for (let g = 0; g < 18; g++) {
    assert.equal(kat.get(`kat-small.S1.stage.${g}.waypoint`), hex(limb));
    const prev = limb;
    const leg = legRef(S1, terrain, u64le(prev));
    const cp = checkpointRef(S1, terrain, prev, limb, leg);
    assert.equal(kat.get(`kat-small.S1.stage.${g}.leg.0.legs_le_hex`), hex(le64(leg)));
    assert.equal(kat.get(`kat-small.S1.stage.${g}.leg.0.checkpoint`), hex(cp));
    if (g < 17) limb = waypointRef(S1, terrain, interruptBeacon(g), limb, [cp]);
  }
});

test('plain scalar tier equals the engine (fill and legs) at S1 and S2', async () => {
  for (const profile of [S1, S2]) {
    const D = sha256(PK_A, SEED_BEACON);
    const t = await Terrain.build(profile, D);
    const plain = fillScalar(profile, D);
    assert.equal(Buffer.compare(plain, t.bytes), 0);
    const start = u64le(D);
    assert.equal(legScalar(profile, plain, start), t.runLeg(start, profile.stepsPerLeg, 'node'));
    assert.equal(t.runLeg(start, profile.stepsPerLeg, 'wasm'), t.runLeg(start, profile.stepsPerLeg, 'node'));
    assert.deepEqual(await t.runLegs([start, start + 1n]), [t.runLeg(start), t.runLeg(start + 1n)]);
  }
});

test('committed objects O1 and O2 are reproduced byte for byte', async () => {
  const o1 = await produceFixture(S1, uniformLegs(1));
  assert.equal(Buffer.compare(o1.bytes, readObject('O1-S1.cbor')), 0);
  const o2 = await produceFixture(S2, uniformLegs(3));
  assert.equal(Buffer.compare(o2.bytes, readObject('O2-S2.cbor')), 0);
  assert.equal(Buffer.compare(encodeObject(o1.object), o1.bytes), 0);
});
