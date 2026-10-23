// Micro-benchmarks used to choose kernel shapes and to produce the timings quoted in RESULTS.md.
// usage: node dist/bench.js fold | fill | leg | timings | all

import { performance } from 'node:perf_hooks';
import { sha256, u64le } from './bigref.js';
import { Terrain, hostInfo, shutdownPool } from './engine.js';
import { DEFAULT_CONFIG, type KernelConfig } from './kernels.js';
import { PRODUCTION, makeProfile } from './profile.js';
import { fillScalar, legScalar } from './scalar.js';

const pkA = Buffer.from('f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9', 'hex');
const seed = sha256(pkA, sha256(Buffer.from('pcoc-pack-2.2/beacon/seed')));

const ms = (t0: number): number => performance.now() - t0;

async function benchFold(): Promise<void> {
  const shapes: KernelConfig['fold'][] = [
    { vec: 0, scalar: 8 },
    { vec: 0, scalar: 4 },
    { vec: 0, scalar: 16 },
    { vec: 4, scalar: 0 },
    { vec: 8, scalar: 0 },
    { vec: 2, scalar: 4 },
    { vec: 1, scalar: 6 },
    { vec: 3, scalar: 2 },
    { vec: 2, scalar: 8 },
    { vec: 4, scalar: 8 },
  ];
  const profile = makeProfile('conformance-small', 16384, 1024, 16384 * 256, 20000);
  console.log('fold shape benchmark: ns/step with software SHA in wasm (production slot, 20000 steps)');
  for (const fold of shapes) {
    const t = await Terrain.build(profile, seed, { threads: 1, config: { ...DEFAULT_CONFIG, fold } });
    t.runLeg(1n, 500, 'wasm');
    const t0 = performance.now();
    t.runLeg(1n, 20000, 'wasm');
    const per = (ms(t0) * 1e6) / 20000;
    console.log(`  vec=${fold.vec} scalar=${fold.scalar}: ${per.toFixed(0)} ns/step  (${(per / 2048).toFixed(3)} ns/word)`);
  }
}

async function benchFill(): Promise<void> {
  const profile = makeProfile('conformance-small', 16384, 1024, 16384 * 4096, 1000);
  console.log('fill benchmark: 64 MiB terrain (4096 production slots), threads=1');
  for (const fillUnroll of [4, 8] as const) {
    const t0 = performance.now();
    await Terrain.build(profile, seed, { threads: 1, config: { ...DEFAULT_CONFIG, fillUnroll } });
    console.log(`  unroll=${fillUnroll}: ${ms(t0).toFixed(0)} ms (x4 for 256 MiB = ${(ms(t0) * 4).toFixed(0)} ms)`);
  }
}

async function benchLeg(): Promise<void> {
  const profile = makeProfile('conformance-small', 16384, 1024, 16384 * 256, 20000);
  const t = await Terrain.build(profile, seed, { threads: 1 });
  for (const sha of ['node', 'wasm'] as const) {
    t.runLeg(u64le(seed), 500, sha);
    const t0 = performance.now();
    t.runLeg(u64le(seed), 20000, sha);
    const per = (ms(t0) * 1e6) / 20000;
    console.log(`  sha=${sha}: ${per.toFixed(0)} ns/step -> ${((per * 2e6) / 1e9).toFixed(2)} s per production leg`);
  }
}

async function benchTimings(): Promise<void> {
  const D = seed;
  const start = u64le(D);
  const line = (label: string, v: number) => console.log(`${label.padEnd(58)} ${v.toFixed(2)} s`);
  console.log('production terrain fill (256 MiB), wall clock:');
  let t0 = performance.now();
  const plain = fillScalar(PRODUCTION, D);
  line('  plain TypeScript, 1 thread, no WebAssembly', ms(t0) / 1000);
  t0 = performance.now();
  const one = await Terrain.build(PRODUCTION, D, { threads: 1 });
  line('  WebAssembly kernel, 1 thread', ms(t0) / 1000);
  if (Buffer.compare(plain, one.bytes) !== 0) throw new Error('plain and wasm terrains differ');
  for (const threads of [2, 4]) {
    t0 = performance.now();
    const t = await Terrain.build(PRODUCTION, D, { threads });
    line(`  WebAssembly kernel, ${threads} threads`, ms(t0) / 1000);
    if (Buffer.compare(plain, t.bytes) !== 0) throw new Error('plain and wasm terrains differ');
  }
  console.log('one production leg (2,000,000 steps), wall clock, 1 thread:');
  t0 = performance.now();
  const rPlain = legScalar(PRODUCTION, plain, start);
  line('  plain TypeScript (32-bit limbs) + node:crypto SHA-256', ms(t0) / 1000);
  t0 = performance.now();
  const rHw = one.runLeg(start, PRODUCTION.stepsPerLeg, 'node');
  line('  WebAssembly fold + node:crypto SHA-256 (SHA-NI)', ms(t0) / 1000);
  t0 = performance.now();
  const rSw = one.runLeg(start, PRODUCTION.stepsPerLeg, 'wasm');
  line('  WebAssembly fold + WebAssembly software SHA-256', ms(t0) / 1000);
  if (rPlain !== rHw || rHw !== rSw) throw new Error('leg results differ');
  console.log('six production legs at once (the §14.6 recompute), wall clock:');
  t0 = performance.now();
  await one.runLegs(Array.from({ length: 6 }, (_, k) => start + BigInt(k)));
  line('  WebAssembly fold + node:crypto SHA-256, 6 workers', ms(t0) / 1000);
}

async function benchProdLeg(): Promise<void> {
  const t = await Terrain.build(PRODUCTION, seed);
  const start = u64le(seed);
  for (let rep = 0; rep < 3; rep++) {
    for (const sha of ['node', 'wasm'] as const) {
      const t0 = performance.now();
      t.runLeg(start, PRODUCTION.stepsPerLeg, sha);
      console.log(`  production leg, sha=${sha}: ${(ms(t0) / 1000).toFixed(2)} s`);
    }
  }
}

const what = process.argv[2] ?? 'all';
console.log(JSON.stringify(hostInfo().cpuModel));
if (what === 'fold' || what === 'all') await benchFold();
if (what === 'fill' || what === 'all') await benchFill();
if (what === 'leg' || what === 'all') await benchLeg();
if (what === 'timings') await benchTimings();
if (what === 'prodleg') await benchProdLeg();
await shutdownPool();
