// Integer comparisons: Rule B2 (leg-rate floor) and the §16.1 anchor comparison; Hashcash (§13.1).

import { createHash } from 'node:crypto';
import { ANCHOR_WINDOW_OFFSETS } from './profile.js';
import { sha256 } from './bigref.js';

/** Rule B2: `meets` when hike_span is an integer > 0 and q · hike_legs ≥ p · hike_span, otherwise `refuse`. */
export function rateComparison(hikeLegs: bigint, hikeSpan: bigint, p: bigint, q: bigint): 'meets' | 'refuse' {
  return hikeSpan > 0n && q * hikeLegs >= p * hikeSpan ? 'meets' : 'refuse';
}

/** §16.1 anchor comparison: `anchored` when the inclusion height is H plus one of ANCHOR_WINDOW_OFFSETS. */
export function anchorComparison(H: number, inclusionHeight: number): 'anchored' | 'not anchored' {
  return (ANCHOR_WINDOW_OFFSETS as readonly number[]).includes(inclusionHeight - H) ? 'anchored' : 'not anchored';
}

export const HC_PREFIX = Buffer.from('bari-hc-v2', 'ascii');

export const headerHash = (header: Uint8Array): Buffer => sha256(sha256(header));
/** challenge = SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash) */
export const hashcashChallenge = (bariOrigin: string, headerHashNatural: Uint8Array): Buffer =>
  sha256(HC_PREFIX, Buffer.from(bariOrigin, 'ascii'), headerHashNatural);

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

/** The first nonce counting up from 0 whose digest SHA-256(challenge ‖ waypoint ‖ le64(nonce)) has `bits` leading zero bits. */
export function solveHashcash(challenge: Uint8Array, waypoint: Uint8Array, bits = 21): { nonce: bigint; digest: Buffer } {
  const buf = Buffer.alloc(challenge.length + waypoint.length + 8);
  buf.set(challenge, 0);
  buf.set(waypoint, challenge.length);
  const o = challenge.length + waypoint.length;
  const fullBytes = bits >> 3;
  const rem = bits & 7;
  const mask = rem === 0 ? 0 : (0xff << (8 - rem)) & 0xff;
  for (let nonce = 0n; ; nonce++) {
    buf.writeBigUInt64LE(nonce, o);
    const d = createHash('sha256').update(buf).digest();
    let ok = true;
    for (let k = 0; k < fullBytes; k++) if (d[k] !== 0) { ok = false; break; }
    if (ok && (rem === 0 || (d[fullBytes]! & mask) === 0)) return { nonce, digest: d };
  }
}
