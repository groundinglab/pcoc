import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { after, test } from 'node:test';
import { generateCandidates } from '../gen-candidates.js';
import { shutdownPool } from '../engine.js';
import { ROOT, candidateMap, readCandidate } from './helpers.js';

after(() => shutdownPool());

const EXPECTED_FILES = [
  'anchor.txt', 'draw.txt', 'fold.txt', 'hashcash.txt', 'kat-small.txt', 'negatives.txt', 'nl4.txt',
  'prod.txt', 'rate.txt', 'seed.txt', 'terrain.txt', 'test-3leg.txt', 'unknown-kind.txt', 'wes.txt',
];

test('re-running the generator reproduces the committed candidates byte for byte', async () => {
  const files = await generateCandidates();
  assert.deepEqual([...files.keys()].sort(), EXPECTED_FILES);
  assert.deepEqual(readdirSync(join(ROOT, 'candidates')).sort(), EXPECTED_FILES);
  for (const [name, text] of files) assert.equal(readCandidate(name), text, name);
});

test('candidate files obey the CASES.md format and key counts', () => {
  for (const name of EXPECTED_FILES) {
    const text = readCandidate(name);
    assert.ok(text.endsWith('\n'), name);
    assert.ok(/^[\x20-\x7e\n\t]*$/.test(text), `${name} is ASCII`);
    const lines = text.slice(0, -1).split('\n');
    const keys = lines.map((l) => {
      assert.equal(l.split('\t').length, 2, `${name}: ${l}`);
      return l.split('\t')[0]!;
    });
    const sorted = [...keys].sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
    assert.deepEqual(keys, sorted, `${name} is sorted by key bytes`);
    assert.equal(new Set(keys).size, keys.length);
  }
  const count = (f: string) => candidateMap(f).size;
  assert.equal(count('seed.txt'), 12);
  assert.equal(count('nl4.txt'), 48);
  assert.equal(count('terrain.txt'), 3);
  assert.equal(count('fold.txt'), 17);
  assert.equal(count('kat-small.txt'), 1 + 18 * 3);
  assert.equal(count('test-3leg.txt'), 1 + 18 + 18 * 3 * 2);
  assert.equal(count('prod.txt'), 3);
  assert.equal(count('wes.txt'), 3);
  assert.equal(count('rate.txt'), 45);
  assert.equal(count('anchor.txt'), 7);
  assert.equal(count('draw.txt'), 2 + 6);
  assert.equal(count('unknown-kind.txt'), 8);
  assert.equal(count('hashcash.txt'), 9);
  assert.equal(count('negatives.txt'), 28);
});

test('every negative is a non-match', () => {
  const m = candidateMap('negatives.txt');
  const names = [...m.keys()].filter((k) => k.endsWith('.matches_honest'));
  assert.equal(names.length, 13);
  for (const k of names) assert.equal(m.get(k), 'false', k);
  const honestLeg = m.get('negatives.honest.legs_le_hex');
  const honestTerrain = m.get('negatives.honest.terrain_sha256');
  for (const [k, v] of m) {
    if (k.endsWith('.legs_le_hex') && !k.includes('honest')) assert.notEqual(v, honestLeg, k);
    if (k.endsWith('.terrain_sha256') && !k.includes('honest')) assert.notEqual(v, honestTerrain, k);
  }
  assert.equal(honestLeg, candidateMap('kat-small.txt').get('kat-small.S1.stage.0.leg.0.legs_le_hex'));
  assert.equal(honestTerrain, candidateMap('terrain.txt').get('terrain.S1.terrain_sha256'));
});
