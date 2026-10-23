import assert from 'node:assert/strict';
import { test } from 'node:test';
import { CborError, decodeCbor, encodeCbor } from '../cbor.js';
import { classify } from '../object.js';

const h = (s: string): Buffer => Buffer.from(s.replace(/\s+/g, ''), 'hex');
const malformed = (s: string) => assert.throws(() => decodeCbor(h(s)), CborError, s);

test('decoder rejects every §7.3 malformed form', () => {
  malformed(''); // empty
  malformed('1c'); // reserved additional info
  malformed('5f 41 00 ff'); // indefinite byte string
  malformed('7f 61 61 ff'); // indefinite text
  malformed('9f 01 ff'); // indefinite array
  malformed('bf 61 61 01 ff'); // indefinite map
  malformed('c0 01'); // tag
  malformed('c2 41 00'); // bignum tag
  malformed('f9 3c 00'); // float16
  malformed('fb 3f f0 00 00 00 00 00 00'); // float64
  malformed('f4'); // simple false
  malformed('f6'); // null
  malformed('f7'); // undefined
  malformed('20'); // negative integer
  malformed('a1 01 01'); // non-text key
  malformed('a1 41 61 01'); // byte-string key
  malformed('a2 61 61 01 61 61 02'); // duplicate key
  malformed('a2 61 61 01 78 01 61 02'); // duplicate key via non-shortest length
  malformed('01 00'); // trailing byte
  malformed('18'); // truncated head
  malformed('42 00'); // truncated byte string
  malformed('62 61'); // truncated text
  malformed('82 01'); // truncated array
  malformed('61 ff'); // text that is not UTF-8
  malformed('5b ff ff ff ff ff ff ff ff'); // absurd length
});

test('decoder accepts non-shortest integers and unsorted keys', () => {
  assert.equal(decodeCbor(h('18 05')), 5n);
  assert.equal(decodeCbor(h('1b 00 00 00 00 00 00 00 05')), 5n);
  assert.equal(decodeCbor(h('19 00 05')), 5n);
  const m = decodeCbor(h('a2 61 62 01 61 61 02')) as Map<string, bigint>;
  assert.equal(m.get('a'), 2n);
  assert.equal(decodeCbor(h('1b ff ff ff ff ff ff ff ff')), 2n ** 64n - 1n);
  assert.equal(decodeCbor(h('78 01 61')), 'a');
});

test('encoder: shortest heads, sorted keys, definite lengths', () => {
  const hx = (v: Parameters<typeof encodeCbor>[0]) => Buffer.from(encodeCbor(v)).toString('hex');
  assert.equal(hx(23), '17');
  assert.equal(hx(24), '1818');
  assert.equal(hx(255), '18ff');
  assert.equal(hx(256), '190100');
  assert.equal(hx(65535), '19ffff');
  assert.equal(hx(65536), '1a00010000');
  assert.equal(hx(4294967295), '1affffffff');
  assert.equal(hx(4294967296), '1b0000000100000000');
  assert.equal(hx(2n ** 64n - 1n), '1bffffffffffffffff');
  assert.throws(() => encodeCbor(2n ** 64n));
  assert.throws(() => encodeCbor(-1));
  assert.equal(hx('a'), '6161');
  assert.equal(hx('x'.repeat(24)).slice(0, 4), '7818');
  assert.equal(hx(new Uint8Array(0)), '40');
  assert.equal(hx([]), '80');
  // key order: length first, then bytes
  assert.equal(
    hx(new Map<string, number>([['bb', 1], ['a', 2], ['ab', 3], ['b', 4]])),
    'a4' + '6161' + '02' + '6162' + '04' + '626162' + '03' + '626262' + '01',
  );
});

test('§7.3 decode order', () => {
  assert.equal(classify(h('')).class, 'malformed');
  assert.equal(classify(h('ff')).class, 'malformed');
  assert.equal(classify(h('01 02')).class, 'malformed');
  assert.equal(classify(h('01')).class, 'decode-failure'); // well-formed non-map
  assert.equal(classify(h('80')).class, 'decode-failure');
  assert.equal(classify(h('a0')).class, 'decode-failure'); // no kind
  assert.equal(classify(h('a1 64 6b696e64 01')).class, 'decode-failure'); // kind not text
  assert.equal(classify(h('a1 64 6b696e64 41 00')).class, 'decode-failure'); // kind bytes
  // a map with a foreign kind but indefinite length is malformed, not Rule A5
  assert.equal(classify(h('bf 64 6b696e64 61 78 ff')).class, 'malformed');
  // trailing byte after a map with a foreign kind: malformed, not Rule A5
  assert.equal(classify(h('a1 64 6b696e64 61 78 00')).class, 'malformed');
  const u = classify(h('a1 64 6b696e64 61 78'));
  assert.deepEqual(u, { class: 'unknown-kind', kindText: 'x' });
  const ok = classify(h('a1 64 6b696e64 70 76322e322d70636f632d6f626a656374'));
  assert.equal(ok.class, 'object');
});
