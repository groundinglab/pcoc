import { parentPort } from 'node:worker_threads';
import { fillColumns, instantiate, runLegOn, type ShaMode } from './engine.js';

parentPort!.on('message', (m: any) => {
  try {
    const ex = instantiate(m.module as WebAssembly.Module, m.memory as WebAssembly.Memory);
    if (m.op === 'fill') {
      fillColumns(ex, m.memory, m.ctx, m.nSlots, m.words, m.j0, m.j1, m.lanes as bigint[]);
      parentPort!.postMessage({ ok: true });
    } else if (m.op === 'leg') {
      const result = runLegOn(
        ex,
        m.memory,
        m.ctx,
        m.words,
        m.nSlots,
        m.start as bigint,
        m.steps as number,
        m.sha as ShaMode,
      );
      parentPort!.postMessage({ result });
    } else {
      parentPort!.postMessage({ error: `unknown op ${String(m.op)}` });
    }
  } catch (e) {
    parentPort!.postMessage({ error: e instanceof Error ? e.stack ?? e.message : String(e) });
  }
});
