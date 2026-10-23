// Terrain / leg engine: WebAssembly kernels, shared-memory terrain, worker-thread parallelism.

import { hash as nodeHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import os from 'node:os';
import { Worker } from 'node:worker_threads';
import { bitrevParent, fillTerrain, lanes as lanesOf, mulParent, sha256 } from './bigref.js';
import {
  CTX_BYTES,
  DEFAULT_CONFIG,
  LANES_OFF,
  MAX_CTX,
  PARENTS_BASE,
  SEAL_OFF,
  TERRAIN_BASE,
  buildKernelBinary,
  terrainPages,
  type KernelConfig,
  type KernelExports,
} from './kernels.js';
import { SEAL_DOMAIN, type Profile } from './profile.js';

export type ShaMode = 'node' | 'wasm';

export interface EngineOptions {
  /** worker threads used for the terrain fill and for parallel legs (default: all CPUs) */
  threads?: number;
  /** `node`: node:crypto (OpenSSL, SHA-NI when the host has it). `wasm`: software SHA-256 in the kernel. */
  sha?: ShaMode | 'auto';
  config?: KernelConfig;
}

export interface HostInfo {
  cpuModel: string;
  cpus: number;
  flags: string[];
  shaInstructions: boolean | 'unknown';
}

export function hostInfo(): HostInfo {
  const cpus = os.cpus();
  let flags: string[] = [];
  try {
    const txt = readFileSync('/proc/cpuinfo', 'utf8');
    const m = /^(?:flags|Features)\s*:\s*(.*)$/m.exec(txt);
    if (m) flags = m[1]!.trim().split(/\s+/);
  } catch {
    /* not linux */
  }
  const sha = flags.length
    ? flags.includes('sha_ni') || flags.includes('sha2') || flags.includes('sha256')
    : ('unknown' as const);
  return { cpuModel: cpus[0]?.model ?? 'unknown', cpus: os.availableParallelism(), flags, shaInstructions: sha };
}

/** SPEC §11.1: hardware SHA-256 wherever the host has it. node:crypto dispatches to SHA-NI/SHA2 at run time. */
export function resolveSha(mode: ShaMode | 'auto' | undefined): ShaMode {
  if (mode === 'node' || mode === 'wasm') return mode;
  const info = hostInfo();
  return info.shaInstructions === false ? 'wasm' : 'node';
}

const moduleCache = new Map<string, WebAssembly.Module>();
export function kernelModule(pages: number, cfg: KernelConfig): WebAssembly.Module {
  const key = `${pages}:${JSON.stringify(cfg)}`;
  let m = moduleCache.get(key);
  if (!m) {
    m = new WebAssembly.Module(buildKernelBinary(pages, cfg) as unknown as BufferSource);
    moduleCache.set(key, m);
  }
  return m;
}

export function instantiate(module: WebAssembly.Module, memory: WebAssembly.Memory): KernelExports {
  return new WebAssembly.Instance(module, { env: { memory } }).exports as unknown as KernelExports;
}

/** One leg (STEPS_PER_LEG steps, or `steps`) on the given context. Used by the main thread and by workers. */
export function runLegOn(
  ex: KernelExports,
  memory: WebAssembly.Memory,
  ctxIndex: number,
  words: number,
  nSlots: number,
  start: bigint,
  steps: number,
  sha: ShaMode,
): bigint {
  const ctx = ctxIndex * CTX_BYTES;
  const u32 = new Uint32Array(memory.buffer, ctx, 2);
  new Uint8Array(memory.buffer, ctx + SEAL_OFF, 12).set(SEAL_DOMAIN);
  u32[0] = Number(start & 0xffffffffn);
  u32[1] = Number(start >> 32n);
  if (sha === 'wasm') {
    let left = steps;
    while (left > 0) {
      const n = Math.min(left, 8192);
      ex.leg_sw(ctx, TERRAIN_BASE, nSlots, words, n);
      left -= n;
    }
  } else {
    const seal = Buffer.from(memory.buffer, ctx + SEAL_OFF, 20);
    for (let t = 0; t < steps; t++) {
      ex.step_hw(ctx, TERRAIN_BASE, nSlots, words);
      const h = nodeHash('sha256', seal, 'buffer');
      u32[0] = h.readUInt32LE(0);
      u32[1] = h.readUInt32LE(4);
    }
  }
  return (BigInt(u32[1]!) << 32n) | BigInt(u32[0]!);
}

export function fillColumns(
  ex: KernelExports,
  memory: WebAssembly.Memory,
  ctxIndex: number,
  nSlots: number,
  words: number,
  j0: number,
  j1: number,
  P: bigint[],
): void {
  const ctx = ctxIndex * CTX_BYTES;
  const dv = new DataView(memory.buffer);
  P.forEach((p, k) => dv.setBigUint64(ctx + LANES_OFF + 8 * k, p, true));
  ex.fill_slot0(0, j0, j1, TERRAIN_BASE, PARENTS_BASE, ctx, words);
  for (let i = 1; i < nSlots; i++) ex.fill_slot(i, j0, j1, TERRAIN_BASE, PARENTS_BASE, ctx, words);
}

// ---- worker pool ------------------------------------------------------------------------------

interface Job {
  msg: Record<string, unknown>;
  resolve: (v: any) => void;
  reject: (e: unknown) => void;
}

class Pool {
  private readonly workers: Worker[] = [];
  private readonly idle: Worker[] = [];
  private readonly queue: Job[] = [];
  private readonly running = new Map<Worker, Job>();

  constructor(size: number) {
    for (let k = 0; k < size; k++) {
      const w = new Worker(new URL('./worker.js', import.meta.url));
      w.unref();
      w.on('message', (res) => {
        const job = this.running.get(w)!;
        this.running.delete(w);
        this.idle.push(w);
        if (res && res.error) job.reject(new Error(res.error));
        else job.resolve(res);
        this.pump();
      });
      w.on('error', (e) => {
        const job = this.running.get(w);
        if (job) job.reject(e);
      });
      this.workers.push(w);
      this.idle.push(w);
    }
  }
  get size(): number {
    return this.workers.length;
  }
  run<T>(msg: Record<string, unknown>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      this.queue.push({ msg, resolve, reject });
      this.pump();
    });
  }
  private pump(): void {
    while (this.idle.length && this.queue.length) {
      const w = this.idle.pop()!;
      const job = this.queue.shift()!;
      this.running.set(w, job);
      w.ref();
      w.postMessage(job.msg);
    }
    if (!this.running.size) for (const w of this.workers) w.unref();
  }
  async terminate(): Promise<void> {
    await Promise.all(this.workers.map((w) => w.terminate()));
  }
}

let pool: Pool | undefined;
export function getPool(size: number): Pool {
  if (!pool || pool.size < size) {
    void pool?.terminate();
    pool = new Pool(size);
  }
  return pool;
}
export async function shutdownPool(): Promise<void> {
  const p = pool;
  pool = undefined;
  await p?.terminate();
}

// ---- Terrain ----------------------------------------------------------------------------------

const PARALLEL_FILL_MIN_WORDS = 1 << 21;

export class Terrain {
  private constructor(
    readonly profile: Profile,
    readonly memory: WebAssembly.Memory,
    readonly module: WebAssembly.Module,
    readonly opts: Omit<EngineOptions, 'sha'> & { sha: ShaMode },
  ) {}

  static async build(profile: Profile, seed: Uint8Array, opts: EngineOptions = {}): Promise<Terrain> {
    const cfg = opts.config ?? DEFAULT_CONFIG;
    const pages = terrainPages(profile.terrainBytes);
    const memory = new WebAssembly.Memory({ initial: pages, maximum: pages, shared: true });
    const module = kernelModule(pages, cfg);
    const t = new Terrain(profile, memory, module, { ...opts, sha: resolveSha(opts.sha) });
    const P = lanesOf(seed);
    const W = profile.wordsPerSlot;
    if (W % cfg.fillUnroll !== 0) {
      // Odd slot widths (below 4·unroll words) only occur in tiny conformance-small profiles.
      t.bytes.set(fillTerrain(profile, P));
      return t;
    }
    const i32 = new Int32Array(memory.buffer, PARENTS_BASE, 2 * profile.nSlots);
    for (let i = 1; i < profile.nSlots; i++) {
      i32[2 * i] = bitrevParent(i);
      i32[2 * i + 1] = mulParent(i);
    }
    const total = profile.nSlots * W;
    const maxThreads = opts.threads ?? os.availableParallelism();
    const threads = Math.max(1, Math.min(maxThreads, Math.floor(W / cfg.fillUnroll), MAX_CTX - 1));
    if (threads === 1 || total < PARALLEL_FILL_MIN_WORDS) {
      const ex = instantiate(module, memory);
      fillColumns(ex, memory, 0, profile.nSlots, W, 0, W, P);
      return t;
    }
    // Column j of every slot depends only on column j of its parents, so column ranges are independent.
    const blocks = W / cfg.fillUnroll;
    const p = getPool(threads);
    const jobs: Promise<unknown>[] = [];
    for (let k = 0; k < threads; k++) {
      const b0 = Math.floor((blocks * k) / threads);
      const b1 = Math.floor((blocks * (k + 1)) / threads);
      jobs.push(
        p.run({
          op: 'fill',
          module,
          memory,
          ctx: k + 1,
          nSlots: profile.nSlots,
          words: W,
          j0: b0 * cfg.fillUnroll,
          j1: b1 * cfg.fillUnroll,
          lanes: P,
        }),
      );
    }
    await Promise.all(jobs);
    return t;
  }

  get bytes(): Uint8Array {
    return new Uint8Array(this.memory.buffer, TERRAIN_BASE, this.profile.terrainBytes);
  }

  digest(): Buffer {
    return sha256(this.bytes);
  }

  get sha(): ShaMode {
    return this.opts.sha;
  }

  /** Runs one leg on the calling thread. */
  runLeg(start: bigint, steps = this.profile.stepsPerLeg, sha: ShaMode = this.opts.sha): bigint {
    const ex = instantiate(this.module, this.memory);
    return runLegOn(ex, this.memory, 0, this.profile.wordsPerSlot, this.profile.nSlots, start, steps, sha);
  }

  /** Runs independent legs concurrently, all started before any result is returned (SPEC §15.5). */
  async runLegs(starts: bigint[], steps = this.profile.stepsPerLeg, sha: ShaMode = this.opts.sha): Promise<bigint[]> {
    if (starts.length === 0) return [];
    const p = getPool(Math.min(starts.length, MAX_CTX - 1));
    const res = await Promise.all(
      starts.map((s, k) =>
        p.run<{ result: bigint }>({
          op: 'leg',
          module: this.module,
          memory: this.memory,
          ctx: k + 1,
          words: this.profile.wordsPerSlot,
          nSlots: this.profile.nSlots,
          start: s,
          steps,
          sha,
        }),
      ),
    );
    return res.map((r) => r.result);
  }

  /** One step with its intermediate values (for fixtures; uses the same kernels). */
  stepDetailed(S: bigint): { slot: number; acc: bigint; next: bigint } {
    const ex = instantiate(this.module, this.memory);
    const slot = Number(S % BigInt(this.profile.nSlots));
    const ctx = 0;
    new DataView(this.memory.buffer).setBigUint64(ctx, S, true);
    ex.fold_probe(
      TERRAIN_BASE + slot * this.profile.slotBytes,
      this.profile.wordsPerSlot,
      ctx,
    );
    const acc = new DataView(this.memory.buffer).getBigUint64(ctx + SEAL_OFF + 12, true);
    new Uint8Array(this.memory.buffer, ctx + SEAL_OFF, 12).set(SEAL_DOMAIN);
    const E = nodeHash('sha256', Buffer.from(this.memory.buffer, ctx + SEAL_OFF, 20), 'buffer');
    return { slot, acc, next: E.readBigUInt64LE(0) };
  }

  slot(i: number): Uint8Array {
    return this.bytes.subarray(i * this.profile.slotBytes, (i + 1) * this.profile.slotBytes);
  }
}
