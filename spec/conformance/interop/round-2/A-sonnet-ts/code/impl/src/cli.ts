// CLI. `verify <object>` prints the §14 verify record (§18). `produce` is the offline test-plane producer.

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ChainSource } from './draw.js';
import { shutdownPool, type EngineOptions } from './engine.js';
import { produceFixture, produceStderr, uniformLegs } from './construction.js';
import { FixtureIndex, HttpIndex, NoIndex, serveFixtureIndex, validIndexUrl, type IndexSource } from './indexsource.js';
import { PRODUCTION, S1, S2, makeProfile, type Profile } from './profile.js';
import { verifyBytes } from './verify.js';

class Usage extends Error {}

const USAGE = `usage:
  verify <object> [--profile production|conformance-small|test|live]
         [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N]
         [--index-url URL | --index-fixture] [--draw-label LABEL] [--threads N]
  produce <O1|O2|O3|--all> <dir>
  serve-index [port]
`;

function parseArgs(argv: string[]): { pos: string[]; opt: Map<string, string> } {
  const pos: string[] = [];
  const opt = new Map<string, string>();
  const flags = new Set(['--index-fixture', '--all']);
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (!a.startsWith('--')) {
      pos.push(a);
      continue;
    }
    if (flags.has(a)) {
      opt.set(a, '');
      continue;
    }
    const v = argv[++i];
    if (v === undefined) throw new Usage(`${a} needs a value`);
    opt.set(a, v);
  }
  return { pos, opt };
}

function intOpt(opt: Map<string, string>, name: string): number | undefined {
  const v = opt.get(name);
  if (v === undefined) return undefined;
  if (!/^[1-9][0-9]*$/.test(v) || !Number.isSafeInteger(Number(v))) throw new Usage(`${name} must be a positive decimal integer`);
  return Number(v);
}

function profileFrom(opt: Map<string, string>): Profile {
  const name = opt.get('--profile') ?? 'production';
  const ints = ['--sample-bytes', '--slot-bytes', '--terrain-bytes', '--steps-per-leg'].map((k) => intOpt(opt, k));
  if (name === 'conformance-small') {
    if (ints.some((x) => x === undefined)) throw new Usage('--profile conformance-small needs all four integers');
    try {
      return makeProfile('conformance-small', ints[1]!, ints[0]!, ints[2]!, ints[3]!);
    } catch (e) {
      throw new Usage(e instanceof Error ? e.message : String(e));
    }
  }
  if (!['production', 'test', 'live'].includes(name)) throw new Usage(`unknown profile ${name}`);
  if (ints.some((x) => x !== undefined)) throw new Usage('the four integers belong to conformance-small only');
  return PRODUCTION;
}

async function cmdVerify(pos: string[], opt: Map<string, string>): Promise<number> {
  if (pos.length !== 1) throw new Usage('exactly one object file');
  const profile = profileFrom(opt);
  let index: IndexSource = new NoIndex();
  if (opt.has('--index-url') && opt.has('--index-fixture')) throw new Usage('one index source');
  if (opt.has('--index-url')) {
    const u = opt.get('--index-url')!;
    if (!validIndexUrl(u)) throw new Usage('--index-url must be an absolute http(s) URL with no trailing slash');
    index = new HttpIndex(u);
  } else if (opt.has('--index-fixture')) index = new FixtureIndex();
  const engine: EngineOptions = {};
  const th = intOpt(opt, '--threads');
  if (th !== undefined) engine.threads = th;
  const label = opt.get('--draw-label');
  let bytes: Buffer;
  try {
    bytes = readFileSync(pos[0]!);
  } catch (e) {
    process.stderr.write(`cannot read object file: ${(e as NodeJS.ErrnoException).code ?? 'error'}\n`);
    return 1;
  }
  const out = await verifyBytes(bytes, {
    profile,
    index,
    engine,
    ...(label !== undefined ? { drawBytes: new ChainSource(label) } : {}),
  });
  if (out.kind === 'decode-failure') {
    process.stderr.write('object bytes are one data item with no kind text (SPEC 7.3)\n');
    return 1;
  }
  process.stdout.write(`${out.line}\n`);
  return 0;
}

const OBJECTS: Record<string, { file: string; profile: Profile; legs: number }> = {
  O1: { file: 'O1-S1.cbor', profile: S1, legs: 1 },
  O2: { file: 'O2-S2.cbor', profile: S2, legs: 3 },
  O3: { file: 'O3-PROD.cbor', profile: PRODUCTION, legs: 1 },
};

async function cmdProduce(pos: string[], opt: Map<string, string>): Promise<number> {
  const all = opt.has('--all');
  const names = all ? Object.keys(OBJECTS) : pos.slice(0, 1);
  const dir = pos[all ? 0 : 1];
  if (!dir || names.length !== 1 && !all || names.some((n) => !(n in OBJECTS))) throw new Usage('produce <O1|O2|O3|--all> <dir>');
  mkdirSync(dir, { recursive: true });
  for (const n of names) {
    const spec = OBJECTS[n]!;
    const t0 = process.hrtime.bigint();
    const run = await produceFixture(spec.profile, uniformLegs(spec.legs));
    writeFileSync(join(dir, spec.file), run.bytes);
    const ms = Number((process.hrtime.bigint() - t0) / 1_000_000n);
    process.stdout.write(`${n} ${spec.file} ${run.bytes.length} bytes ${ms} ms\n`);
    process.stderr.write(produceStderr());
  }
  return 0;
}

async function main(): Promise<number> {
  const [cmd, ...rest] = process.argv.slice(2);
  try {
    const { pos, opt } = parseArgs(rest);
    switch (cmd) {
      case 'verify':
        return await cmdVerify(pos, opt);
      case 'produce':
        return await cmdProduce(pos, opt);
      case 'serve-index': {
        const { url } = await serveFixtureIndex(pos[0] ? Number(pos[0]) : 0);
        process.stdout.write(`${url}\n`);
        await new Promise(() => {});
        return 0;
      }
      default:
        throw new Usage('unknown command');
    }
  } catch (e) {
    if (e instanceof Usage) {
      process.stderr.write(`${e.message}\n${USAGE}`);
      return 2;
    }
    throw e;
  } finally {
    await shutdownPool();
  }
}

main().then(
  (code) => process.exit(code),
  (e) => {
    process.stderr.write(`${e instanceof Error ? e.stack : e}\n`);
    process.exit(2);
  },
);
