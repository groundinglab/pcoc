import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, test } from 'node:test';
import { sha256 } from '../bigref.js';
import { anchorComparison, hashcashChallenge, headerHash, leadingZeroBits, rateComparison, solveHashcash } from '../compare.js';
import { ChainSource, FixedBytes, OsRandom, locateLeg, sixLegDraw, thoroughDraw, unbiasedDraw } from '../draw.js';
import { shutdownPool } from '../engine.js';
import { JsonNumber, parseIndexBody, parseStrictJson } from '../jsonstrict.js';
import { PRODUCTION, S1, makeProfile } from '../profile.js';
import { computeNegatives } from '../negatives.js';
import { PK_A, PK_B, SEED_BEACON, produceStderr } from '../construction.js';
import { canonicalSiblingCount, decodePathBody, encodePathBody, validateFor } from '../pathbody.js';
import { ROOT, candidateMap } from './helpers.js';

after(() => shutdownPool());

test('Rule B2 and the anchor comparison at their boundaries', () => {
  assert.equal(rateComparison(5n, 20n, 1n, 4n), 'meets'); // 4*5 >= 1*20
  assert.equal(rateComparison(5n, 21n, 1n, 4n), 'refuse');
  assert.equal(rateComparison(0n, 1n, 1n, 5n), 'refuse');
  assert.equal(rateComparison(10n, 0n, 1n, 5n), 'refuse'); // span 0: undefined rate
  assert.equal(rateComparison(10n, -5n, 1n, 5n), 'refuse');
  assert.equal(rateComparison(6n, 3600n, 7n, 20n), 'refuse');
  assert.equal(rateComparison(1000n, 3600n, 1n, 5n), 'meets');
  assert.equal(rateComparison(1000n, 3600n, 7n, 20n), 'refuse');
  for (const [h, want] of [[99, 'not anchored'], [100, 'not anchored'], [101, 'not anchored'], [102, 'anchored'], [103, 'anchored'], [104, 'anchored'], [105, 'not anchored']] as const) {
    assert.equal(anchorComparison(100, h), want);
  }
});

test('draws: rejection sampling, distinctness, the 6-leg thorough case, short sources', () => {
  // u >= limit is consumed: with total 3, limit = 2^64 - (2^64 mod 3) = 2^64 - 1, so u = 2^64-1 is discarded
  const src = new FixedBytes(Buffer.concat([Buffer.alloc(8, 0xff), Buffer.from('0500000000000000', 'hex')]));
  assert.equal(unbiasedDraw(src, 3), 5 % 3);
  assert.equal(src.next8(), undefined);
  assert.equal(unbiasedDraw(new FixedBytes(Buffer.alloc(8, 0xff)), 4), 3); // power of two: first draw is used
  assert.equal(unbiasedDraw(new FixedBytes(Buffer.alloc(7)), 4), undefined);
  assert.equal(unbiasedDraw(new FixedBytes(Buffer.alloc(8, 0xff)), 3), undefined);
  // repeats are discarded; a short source is not read
  const rep = new FixedBytes(Buffer.concat([0, 0, 1, 1, 2, 3, 4].map((x) => Buffer.from([x, 0, 0, 0, 0, 0, 0, 0]))));
  assert.equal(sixLegDraw(rep, 10), undefined);
  const ok = sixLegDraw(new FixedBytes(Buffer.concat([0, 0, 1, 1, 2, 3, 4, 5].map((x) => Buffer.from([x, 0, 0, 0, 0, 0, 0, 0])))), 10);
  assert.deepEqual(ok, [0, 1, 2, 3, 4, 5]);
  assert.throws(() => sixLegDraw(new OsRandom(), 5));
  const six = sixLegDraw(new OsRandom(), 6)!;
  assert.deepEqual([...six].sort(), [0, 1, 2, 3, 4, 5]);
  // thorough: at 6 legs an index repeats only after all six are used, never within a group of six
  const t = thoroughDraw(new ChainSource('pcoc-pack-2.2/draw'), 6, 3);
  assert.equal(t.length, 18);
  for (let r = 0; r < 3; r++) assert.equal(new Set(t.slice(6 * r, 6 * r + 6)).size, 6);
  const t18 = thoroughDraw(new ChainSource('pcoc-pack-2.2/draw'), 18, 3);
  assert.equal(new Set(t18).size, 18);
  const short = thoroughDraw(new ChainSource('x', 8 * 10), 100, 3);
  assert.ok(short.length <= 10 && short.length < 18);
  // the draw is one sequence: the six-leg draw is the first six of the thorough draw
  assert.deepEqual(thoroughDraw(new ChainSource('pcoc-pack-2.2/draw'), 54, 3).slice(0, 6), sixLegDraw(new ChainSource('pcoc-pack-2.2/draw'), 54));
  assert.deepEqual(locateLeg([1, 3, 2], 0), { g: 0, j: 0 });
  assert.deepEqual(locateLeg([1, 3, 2], 3), { g: 1, j: 2 });
  assert.deepEqual(locateLeg([1, 3, 2], 4), { g: 2, j: 0 });
});

test('hashcash: challenge, solve, leading zero bits', () => {
  assert.equal(leadingZeroBits(Buffer.from('00000fff', 'hex')), 20);
  assert.equal(leadingZeroBits(Buffer.from('0000080f', 'hex')), 20);
  assert.equal(leadingZeroBits(Buffer.from('00000400', 'hex')), 21);
  assert.equal(leadingZeroBits(Buffer.from('80', 'hex')), 0);
  assert.equal(leadingZeroBits(Buffer.alloc(4)), 32);
  const hc = candidateMap('hashcash.txt');
  const challenge = hashcashChallenge(hc.get('hashcash.bari_origin')!, headerHash(Buffer.from(hc.get('hashcash.fee_header')!, 'hex')));
  assert.equal(challenge.toString('hex'), hc.get('hashcash.challenge'));
  const wp = Buffer.from(hc.get('hashcash.waypoint')!, 'hex');
  const nonce = BigInt(hc.get('hashcash.nonce')!);
  const le = Buffer.alloc(8);
  le.writeBigUInt64LE(nonce);
  const d = sha256(challenge, wp, le);
  assert.equal(d.toString('hex'), hc.get('hashcash.digest'));
  assert.ok(leadingZeroBits(d) >= 21);
  assert.equal(String(leadingZeroBits(d)), hc.get('hashcash.leading_zero_bits'));
  // first nonce counting up: every smaller nonce has fewer than 21 leading zero bits
  const buf = Buffer.concat([challenge, wp, Buffer.alloc(8)]);
  for (let n = 0n; n < nonce; n++) {
    buf.writeBigUInt64LE(n, 64);
    assert.ok(leadingZeroBits(sha256(buf)) < 21);
    if (n > 3000000n) break;
  }
  assert.equal(solveHashcash(challenge, wp, 8).digest[0], 0);
});

test('strict JSON', () => {
  assert.ok(parseStrictJson('{"a":[1,2,{"b":null}],"c":"x\\u00e9\\n"}') instanceof Map);
  for (const bad of ['', '{', '{"a":1,}', '{"a":1}{', '{"a":1,"a":2}', '{"x":{"a":1,"a":2}}', '[1,]', '01', '1.', '"\u0001"', "{'a':1}", '{"a":tru}', '-', '"\\x"', '{"a":1} 2']) {
    assert.throws(() => parseStrictJson(bad));
  }
  assert.equal(parseIndexBody(Buffer.from('[]')), undefined);
  assert.equal(parseIndexBody(Buffer.from('{"a":1} ')) instanceof Map, true);
  assert.equal(parseIndexBody(Buffer.from([0xef, 0xbb, 0xbf, 0x7b, 0x7d])), undefined);
  assert.equal(parseIndexBody(Buffer.from([0x7b, 0x22, 0xc3, 0x28, 0x22, 0x3a, 0x31, 0x7d])), undefined);
  const n = (s: string) => new JsonNumber(s);
  assert.equal(n('12').intIn(0n, 100n), 12n);
  assert.equal(n('12.0').intIn(0n, 100n), undefined);
  assert.equal(n('1e1').intIn(0n, 100n), undefined);
  assert.equal(n('-1').intIn(0n, 100n), undefined);
  assert.equal(n('9007199254740992').intIn(0n, 9007199254740991n), undefined);
});

test('canonical path body and Merkle validation', () => {
  assert.equal(canonicalSiblingCount(1), 0);
  assert.equal(canonicalSiblingCount(2), 1);
  assert.equal(canonicalSiblingCount(3), 2);
  assert.equal(canonicalSiblingCount(5), 3);
  assert.equal(canonicalSiblingCount(1024), 10);
  const leaf = sha256(Buffer.from('leaf'));
  const node = sha256(Buffer.from([0x1a]), leaf);
  const body = { leaf, leafIndex: 0, treeSize: 1, siblings: [], root: node, txid: Buffer.alloc(32, 1), vout: 0, blockHash: Buffer.alloc(32, 2), blockHeight: 7 };
  const bytes = encodePathBody(body);
  assert.equal(bytes.length, 146);
  const dec = decodePathBody(bytes)!;
  assert.ok(validateFor(dec, leaf));
  assert.ok(!validateFor(dec, sha256(Buffer.from('other'))));
  assert.equal(decodePathBody(Buffer.concat([bytes, Buffer.alloc(1)])), undefined);
  const v2 = Buffer.from(bytes);
  v2[0] = 2;
  assert.equal(decodePathBody(v2), undefined);
  const badIdx = Buffer.from(bytes);
  badIdx.writeUInt32BE(1, 33);
  assert.equal(decodePathBody(badIdx), undefined);
  const zeroSize = Buffer.from(bytes);
  zeroSize.writeUInt32BE(0, 37);
  assert.equal(decodePathBody(zeroSize), undefined);
  // two leaves
  const l1 = sha256(Buffer.from('leaf1'));
  const n0 = sha256(Buffer.from([0x1a]), leaf);
  const n1 = sha256(Buffer.from([0x1a]), l1);
  const root = sha256(Buffer.from([0x1b]), n0, n1);
  for (const [idx, lf, sib] of [[0, leaf, n1], [1, l1, n0]] as const) {
    const b = decodePathBody(encodePathBody({ ...body, leaf: lf, leafIndex: idx, treeSize: 2, siblings: [sib], root }))!;
    assert.ok(validateFor(b, lf));
  }
  const swapped = decodePathBody(encodePathBody({ ...body, leaf, leafIndex: 1, treeSize: 2, siblings: [n1], root }))!;
  assert.ok(!validateFor(swapped, leaf));
});

test('profiles obey §16.1 ranges', () => {
  assert.equal(S1.wordsPerSlot, 8);
  assert.equal(S1.nSlots, 1024);
  assert.equal(S1.offsetModulus, 65536 - 32 + 1);
  assert.equal(PRODUCTION.offsetModulus, 268434433);
  assert.equal(PRODUCTION.wordsPerSlot, 2048);
  assert.equal(PRODUCTION.nSlots, 16384);
  for (const args of [[7, 8, 64, 1], [12, 8, 120, 1], [16384 + 8, 8, 16384 + 8, 1], [8, 0, 64, 1], [8, 8, 60, 1], [8, 8, 64, 0], [8, 8, 64, 2000001], [8, 1025, 2048, 1], [8, 8, 4, 1], [8, 8, 268435464, 1]] as const) {
    assert.throws(() => makeProfile('conformance-small', args[0], args[1], args[2], args[3]), RangeError, args.join());
  }
  assert.doesNotThrow(() => makeProfile('conformance-small', 8, 1, 8, 1));
});

test('produce stderr is the Rule A3.4 lines in one ascending step', () => {
  const lines = produceStderr().split('\n');
  assert.equal(lines.pop(), '');
  assert.deepEqual(lines, [...Array.from({ length: 16 }, (_, k) => `path_skip ${k + 1}`), 'not fetched by design 17']);
});

test('negatives: each is computed, none equals honest, cached_leg revisits a slot', () => {
  const n = computeNegatives(S1, PK_A, PK_B, SEED_BEACON);
  assert.equal(n.legs.length, 10);
  assert.equal(n.terrains.length, 3);
  for (const l of n.legs) assert.equal(l.matchesHonest, false);
  for (const t of n.terrains) assert.equal(t.matchesHonest, false);
  assert.equal(new Set([...n.legs.map((l) => l.legsLeHex), n.honestLegsLeHex]).size, 11);
});

const cli = (args: string[], input?: Buffer) => spawnSync('node', [join(ROOT, 'dist', 'cli.js'), ...args], { encoding: 'utf8', input });
const WORDS = /\b(pass(ed)?|fail(ed|ure)?|accept(ed)?|refuse[ds]?|reject(ed)?|deny|denied|admit(ted)?)\b/i;

test('CLI exit codes and output shape', () => {
  const O1 = join(ROOT, 'objects', 'O1-S1.cbor');
  const small = ['--profile', 'conformance-small', '--sample-bytes', '32', '--slot-bytes', '64', '--terrain-bytes', '65536', '--steps-per-leg', '100'];
  const ok = cli(['verify', O1, ...small, '--index-fixture', '--draw-label', 'pcoc-pack-2.2/draw']);
  assert.equal(ok.status, 0);
  assert.ok(ok.stdout.endsWith('}\n'));
  assert.equal(ok.stdout.split('\n').length, 2);
  assert.equal(JSON.parse(ok.stdout).shape, 'match');
  assert.ok(!WORDS.test(ok.stdout), 'no pass/fail/accept/refuse wording');
  assert.equal(ok.stderr, '');

  const dir = mkdtempSync(join(tmpdir(), 'pcoc-'));
  const f = (name: string, bytes: Buffer) => (writeFileSync(join(dir, name), bytes), join(dir, name));
  const missing = cli(['verify', join(dir, 'nope')]);
  assert.equal(missing.status, 1);
  assert.ok(!WORDS.test(missing.stderr + missing.stdout));
  const notMap = cli(['verify', f('int.cbor', Buffer.from([1]))]);
  assert.equal(notMap.status, 1);
  assert.equal(notMap.stdout, '');
  assert.ok(!WORDS.test(notMap.stderr));
  const empty = cli(['verify', f('empty.cbor', Buffer.alloc(0))]);
  assert.equal(empty.status, 0);
  assert.equal(JSON.parse(empty.stdout).shape_item[0], 'malformed cbor');
  const unk = cli(['verify', f('unk.cbor', Buffer.from('a164 6b696e64 61 78'.replace(/ /g, ''), 'hex'))]);
  assert.equal(unk.status, 0);
  assert.equal(unk.stdout, '{"kind":"unknown kind","kind_text":"x"}\n');

  for (const args of [
    ['verify'], ['verify', O1, '--profile', 'conformance-small'], ['verify', O1, '--profile', 'conformance-small', '--sample-bytes', '32'],
    ['verify', O1, '--profile', 'bogus'], ['verify', O1, '--index-url', 'ftp://x'], ['verify', O1, '--index-url', 'http://x/'],
    ['verify', O1, '--index-url', 'http://127.0.0.1:1', '--index-fixture'], ['verify', O1, ...small.slice(0, 8), '--terrain-bytes', '65537', '--steps-per-leg', '100'],
    ['verify', O1, '--profile', 'production', '--slot-bytes', '64'], ['bogus'], [], ['produce'], ['produce', 'O9', dir],
  ]) {
    const r = cli(args);
    assert.equal(r.status, 2, args.join(' '));
    assert.equal(r.stdout, '');
    assert.ok(!WORDS.test(r.stderr), args.join(' '));
  }
  assert.equal(cli(['verify', O1, '--profile', 'test', '--index-fixture']).status, 0);
});

test('CLI produce writes O1 with the Rule A3.4 stderr', () => {
  const dir = mkdtempSync(join(tmpdir(), 'pcoc-'));
  const r = cli(['produce', 'O1', dir]);
  assert.equal(r.status, 0);
  assert.equal(r.stderr, produceStderr());
});
