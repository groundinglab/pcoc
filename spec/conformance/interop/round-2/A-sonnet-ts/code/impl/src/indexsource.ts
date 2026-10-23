// Portable index reads (SPEC §9): one GET per height, no redirect, status 200 only, strict body.
// Also the CASES.md synthetic test-plane index, in process and over HTTP.

import { createServer, type Server } from 'node:http';
import { sha256 } from './bigref.js';
import { SEED_BEACON, SEED_HEIGHT, interruptBeacon } from './construction.js';
import { JsonNumber, parseIndexBody, type JsonValue } from './jsonstrict.js';

export type IndexDoc = Map<string, JsonValue>;

export interface IndexSource {
  /** One clock read of `GET {index-url}/index/{height}`. Undefined: not a successful read. */
  read(height: string): Promise<IndexDoc | undefined>;
}

export class NoIndex implements IndexSource {
  async read(): Promise<undefined> {
    return undefined;
  }
}

export class HttpIndex implements IndexSource {
  constructor(
    readonly base: string,
    readonly timeoutMs = 60_000,
  ) {}
  async read(height: string): Promise<IndexDoc | undefined> {
    try {
      const res = await fetch(`${this.base}/index/${height}`, {
        redirect: 'manual',
        signal: AbortSignal.timeout(this.timeoutMs),
      });
      if (res.status !== 200) {
        await res.body?.cancel();
        return undefined;
      }
      return parseIndexBody(new Uint8Array(await res.arrayBuffer()));
    } catch {
      return undefined;
    }
  }
}

/** §9: an absolute http(s) URL with no trailing slash. */
export function validIndexUrl(u: string): boolean {
  if (!/^https?:\/\//i.test(u) || u.endsWith('/')) return false;
  try {
    new URL(u);
    return true;
  } catch {
    return false;
  }
}

// ---- CASES.md synthetic index -----------------------------------------------------------------

export const FIXTURE_LOW = SEED_HEIGHT - 11;
export const FIXTURE_HIGH = SEED_HEIGHT + 18;

const naturalAt = (h: number): Buffer => {
  if (h === SEED_HEIGHT) return SEED_BEACON;
  if (h >= SEED_HEIGHT + 1 && h <= SEED_HEIGHT + 18) return interruptBeacon(h - SEED_HEIGHT - 1);
  return sha256(Buffer.from(`pcoc-pack-2.2/index/filler/${h}`, 'ascii'));
};

/** The compact JSON body for one height, or undefined outside 989 … 1018. */
export function fixtureBody(h: number): string | undefined {
  if (!Number.isInteger(h) || h < FIXTURE_LOW || h > FIXTURE_HIGH) return undefined;
  const display = Buffer.from(naturalAt(h)).reverse().toString('hex');
  return `{"network":"synthetic","header":{"hash":"${display}","height":${h},"time":${1700000000 + 600 * h}}}`;
}

export class FixtureIndex implements IndexSource {
  async read(height: string): Promise<IndexDoc | undefined> {
    if (!/^(0|[1-9][0-9]*)$/.test(height)) return undefined;
    const body = fixtureBody(Number(height));
    return body === undefined ? undefined : parseIndexBody(Buffer.from(body, 'utf8'));
  }
}

export function serveFixtureIndex(port = 0): Promise<{ server: Server; url: string }> {
  return new Promise((resolve, reject) => {
    const server = createServer((req, res) => {
      const m = /^\/index\/(0|[1-9][0-9]*)$/.exec(req.url ?? '');
      const body = m ? fixtureBody(Number(m[1])) : undefined;
      if (body === undefined) {
        res.writeHead(404).end();
        return;
      }
      res.writeHead(200, { 'content-type': 'application/json' }).end(body);
    });
    server.once('error', reject);
    server.listen(port, '127.0.0.1', () => {
      const addr = server.address();
      if (addr && typeof addr === 'object') resolve({ server, url: `http://127.0.0.1:${addr.port}` });
      else reject(new Error('no address'));
    });
  });
}

// ---- header facts (§9) ------------------------------------------------------------------------

export interface HeaderFacts {
  /** natural order (the display hex reversed once) */
  natural: Buffer;
  height: bigint;
  /** header.time when it follows §9, otherwise undefined */
  time: bigint | undefined;
}

const MAX_SAFE = (1n << 53n) - 1n;

/** §9: a successful header read, or undefined (header missing, not an object, or hash/height unusable). */
export function headerFacts(doc: IndexDoc | undefined): HeaderFacts | undefined {
  const h = doc?.get('header');
  if (!(h instanceof Map)) return undefined;
  const hash = h.get('hash');
  const height = h.get('height');
  if (typeof hash !== 'string' || !/^[0-9a-fA-F]{64}$/.test(hash)) return undefined;
  if (!(height instanceof JsonNumber)) return undefined;
  const hv = height.intIn(0n, MAX_SAFE);
  if (hv === undefined) return undefined;
  const t = h.get('time');
  const tv = t instanceof JsonNumber ? t.intIn(0n, 0xffffffffn) : undefined;
  return { natural: Buffer.from(hash, 'hex').reverse(), height: hv, time: tv };
}
