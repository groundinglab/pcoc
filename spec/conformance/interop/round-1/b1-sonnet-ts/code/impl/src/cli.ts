import { readFileSync, existsSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { verify, DecodeFailure } from './verify.ts';
import { PROD, S1, S2, type Profile } from './core.ts';
import { produceAll } from './testplane.ts';

function usage(msg?: string): never {
  if (msg) process.stderr.write(msg + '\n');
  process.stderr.write(
    'usage: cli.ts verify <object> [--index-dir DIR] [--profile production|conformance-small|S1|S2 ...]\n' +
      '       [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N] [--draw-bytes-hex HEX]\n' +
      '       cli.ts produce-test-plane <outdir>\n',
  );
  process.exit(2);
}

const args = process.argv.slice(2);
const cmd = args.shift();

if (cmd === 'produce-test-plane') {
  const dir = resolve(args[0] ?? usage());
  for (const p of produceAll(dir)) {
    process.stderr.write(`${p.name}: ${p.legs} legs in ${p.seconds.toFixed(1)}s (${(p.legs / p.seconds).toFixed(3)} legs/s) -> ${p.file}\n`);
  }
} else if (cmd === 'verify') {
  const file = args.shift() ?? usage();
  const opt = new Map<string, string>();
  while (args.length) {
    const k = args.shift()!;
    if (!k.startsWith('--') || args.length === 0) usage(`bad argument ${k}`);
    opt.set(k.slice(2), args.shift()!);
  }
  let profile: Profile = PROD;
  const pname = opt.get('profile') ?? 'production';
  const ints = ['sample-bytes', 'slot-bytes', 'terrain-bytes', 'steps-per-leg'];
  if (pname === 'conformance-small') {
    if (!ints.every((k) => opt.has(k))) usage('conformance-small needs all four integers');
    const n = (k: string) => Number(opt.get(k));
    profile = { name: 'conformance-small', sampleBytes: n('sample-bytes'), slotBytes: n('slot-bytes'), terrainBytes: n('terrain-bytes'), stepsPerLeg: n('steps-per-leg') };
  } else if (pname === 'S1') profile = S1;
  else if (pname === 'S2') profile = S2;
  else if (!['production', 'test', 'live'].includes(pname)) usage(`unknown profile ${pname}`);
  const indexDir = resolve(opt.get('index-dir') ?? join(dirname(resolve(file)), 'index'));
  let bytes: Buffer;
  try {
    bytes = readFileSync(file);
  } catch {
    process.stderr.write('cannot read object file\n');
    process.exit(1);
  }
  const drawHex = opt.get('draw-bytes-hex');
  try {
    const rec = verify(bytes, {
      profile,
      readIndex: (h) => {
        const f = join(indexDir, `${h}.json`);
        return existsSync(f) ? readFileSync(f) : null;
      },
      drawBytes: drawHex ? Buffer.from(drawHex, 'hex') : undefined,
    });
    process.stdout.write(JSON.stringify(rec) + '\n');
  } catch (e) {
    if (e instanceof DecodeFailure) {
      process.stderr.write(`cannot decode object: ${e.message}\n`);
      process.exit(1);
    }
    throw e;
  }
} else usage();
