// SPEC §14.6 unbiased draw, the six-leg draw (Rule A4.4) and the §15.5 thorough draw,
// each from a supplied byte source. §16.1 "Draw bytes": a SHA-256 chain from an ASCII label.

import { randomBytes } from 'node:crypto';
import { sha256 } from './bigref.js';

/** Supplies the next 8 bytes, or undefined when the source cannot (the draw is then `not read`). */
export interface ByteSource {
  next8(): Buffer | undefined;
}

/** Operating-system CSPRNG (`getrandom`). */
export class OsRandom implements ByteSource {
  next8(): Buffer {
    return randomBytes(8);
  }
}

/** b_0 = SHA-256(label), b_(n+1) = SHA-256(b_n), concatenated; read from the first byte, 8 bytes at a time. */
export class ChainSource implements ByteSource {
  private block: Buffer;
  private off = 0;
  constructor(label: string, private readonly maxBytes = Infinity) {
    this.block = sha256(Buffer.from(label, 'ascii'));
  }
  private consumed = 0;
  next8(): Buffer | undefined {
    if (this.consumed + 8 > this.maxBytes) return undefined;
    const out = Buffer.alloc(8);
    for (let k = 0; k < 8; k++) {
      if (this.off === 32) {
        this.block = sha256(this.block);
        this.off = 0;
      }
      out[k] = this.block[this.off++]!;
    }
    this.consumed += 8;
    return out;
  }
}

export class FixedBytes implements ByteSource {
  private off = 0;
  constructor(private readonly bytes: Uint8Array) {}
  next8(): Buffer | undefined {
    if (this.off + 8 > this.bytes.length) return undefined;
    const b = Buffer.from(this.bytes.subarray(this.off, this.off + 8));
    this.off += 8;
    return b;
  }
}

const M = 1n << 64n;

/** One unbiased draw from 0 … total−1 (§14.6). A candidate with u ≥ limit is consumed and discarded. */
export function unbiasedDraw(src: ByteSource, total: number): number | undefined {
  const n = BigInt(total);
  const limit = M - (M % n);
  for (;;) {
    const b = src.next8();
    if (!b) return undefined;
    const u = b.readBigUInt64LE(0);
    if (u < limit) return Number(u % n);
  }
}

/** Rule A4.4: 6 distinct indices, discarding repeats. `undefined`: the byte source ran out (`not read`). */
export function sixLegDraw(src: ByteSource, legTotal: number): number[] | undefined {
  if (legTotal < 6) throw new RangeError('fewer than six legs: the draw is not made');
  const out: number[] = [];
  while (out.length < 6) {
    const q = unbiasedDraw(src, legTotal);
    if (q === undefined) return undefined;
    if (!out.includes(q)) out.push(q);
  }
  return out;
}

/**
 * §15.5 pseudo-code: one sequence of D = 6·rounds indices. Distinct within the current group of 6, and not
 * repeated while an unused index remains. Returns the indices drawn (shorter than D if the source ran out).
 */
export function thoroughDraw(src: ByteSource, legTotal: number, rounds: number): number[] {
  if (legTotal < 6) throw new RangeError('fewer than six legs: the draw is not defined');
  const D = 6 * rounds;
  const phase: number[] = [];
  const seen = new Set<number>();
  while (phase.length < D) {
    const q = unbiasedDraw(src, legTotal);
    if (q === undefined) break;
    const groupStart = 6 * Math.floor(phase.length / 6);
    if (phase.indexOf(q, groupStart) !== -1) continue;
    if (seen.has(q) && seen.size < legTotal) continue;
    phase.push(q);
    seen.add(q);
  }
  return phase;
}

/** Maps a global leg index to (stage g, leg j) from the per-stage checkpoint counts (Rule A4.4). */
export function locateLeg(counts: number[], q: number): { g: number; j: number } {
  let start = 0;
  for (let g = 0; g < counts.length; g++) {
    if (q < start + counts[g]!) return { g, j: q - start };
    start += counts[g]!;
  }
  throw new RangeError('leg index out of range');
}
