import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, readdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { generate, writeAll } from '../src/gen/candidates.ts';
import { verify } from '../src/verify.ts';
import { decode, encode } from '../src/cbor.ts';
import { sha256, SEAL_DOMAIN, le64, seedOf, nl4Base, S1 } from '../src/core.ts';
import { encodeHike } from '../src/object.ts';
import { buildHike, Terrain } from '../src/core.ts';
import { PK_A, SEED_BEACON, beaconSet, FIX_S1 } from '../src/fixtures.ts';
import { writeSyntheticIndex } from '../src/testplane.ts';

const committed = new URL('../../candidates/', import.meta.url).pathname;

test('committed candidates are unchanged by a fresh generator run', () => {
  const dir = mkdtempSync(join(tmpdir(), 'cand-'));
  writeAll(dir, generate());
  const names = readdirSync(committed).sort();
  assert.deepEqual(readdirSync(dir).sort(), names);
  assert.equal(names.length, 14);
  for (const n of names) assert.equal(readFileSync(join(dir, n), 'utf8'), readFileSync(join(committed, n), 'utf8'), n);
});

test('candidate files obey the format rules', () => {
  for (const n of readdirSync(committed)) {
    const text = readFileSync(join(committed, n), 'latin1');
    assert.ok(text.endsWith('\n') && !text.includes('\n\n'));
    const keys = text.split('\n').slice(0, -1).map((l) => {
      assert.match(l, /^[\x21-\x7e]+\t[\x21-\x7e]+( [\x21-\x7e]+)*$/, l);
      return l.split('\t')[0];
    });
    assert.deepEqual(keys, [...keys].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b))));
  }
});

test('SPEC 7.4 checks', () => {
  assert.equal(sha256(SEAL_DOMAIN, le64(0n)).toString('hex'), '4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533');
  const pre = Buffer.concat([Buffer.alloc(32, 0x11), Buffer.alloc(32, 0x22)]);
  const D = sha256(pre);
  assert.equal(D.toString('hex'), '5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c');
  const P = [0, 1, 2, 3].map((k) => D.readBigUInt64LE(8 * k));
  assert.equal(nl4Base(P, 0n, 0), 0xc7582082a632c2c8n);
});

test('CBOR decode rejects what SPEC 7.3 calls malformed', () => {
  for (const hex of ['9f01ff', 'c101', 'f90000', '20', 'a1010a', 'a2616101616102', '0000', 'fb0000000000000000']) {
    assert.throws(() => decode(Buffer.from(hex, 'hex')), hex);
  }
  assert.deepEqual(decode(Buffer.from('1800', 'hex')), 0n);
  assert.deepEqual(encode(decode(encode(new Map([['a', 1n]])))), encode(new Map([['a', 1n]])));
});

test('verifier: honest S1 object matches; tampering is recorded; unknown kind is not a shape mismatch', () => {
  const idx = mkdtempSync(join(tmpdir(), 'idx-'));
  writeSyntheticIndex(idx);
  const readIndex = (h: number) => {
    try {
      return readFileSync(join(idx, `${h}.json`));
    } catch {
      return null;
    }
  };
  const t = new Terrain(S1, seedOf(PK_A, SEED_BEACON).P);
  const bytes = encodeHike(buildHike(t, PK_A, beaconSet(), FIX_S1.legsPerStage), S1);
  const rec = verify(bytes, { profile: S1, readIndex }) as Record<string, any>;
  assert.equal(rec.shape, 'match');
  assert.equal(rec.seed_value, 'match');
  assert.ok(rec.checkpoints.every((c: any) => c.legs.every((l: any) => l.result === 'match')));
  assert.ok(rec.waypoints.every((w: any) => w.result === 'match'));
  assert.equal(rec.six_leg.results.length, 6);
  assert.equal(rec.analysis.hike_span, 10800);
  const bad = Buffer.from(bytes);
  bad[bad.length - 1] ^= 1;
  assert.notEqual((verify(bad, { profile: S1, readIndex }) as Record<string, any>).headers[18].index_comparison, 'match');
  const unk = encode(new Map<string, any>([['kind', 'v2.1-pcoc-object']]));
  assert.deepEqual(verify(unk, { profile: S1, readIndex }), { kind: 'unknown kind', kind_text: 'v2.1-pcoc-object' });
  assert.deepEqual((verify(Buffer.from('9f', 'hex'), { profile: S1, readIndex }) as Record<string, any>).shape_item, ['malformed cbor']);
});
