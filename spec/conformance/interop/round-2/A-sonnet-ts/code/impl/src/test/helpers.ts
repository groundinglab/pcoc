import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
export const readObject = (name: string): Buffer => readFileSync(join(ROOT, 'objects', name));
export const readCandidate = (name: string): string => readFileSync(join(ROOT, 'candidates', name), 'utf8');
export function candidateMap(name: string): Map<string, string> {
  const m = new Map<string, string>();
  for (const line of readCandidate(name).split('\n')) {
    if (!line) continue;
    const t = line.indexOf('\t');
    m.set(line.slice(0, t), line.slice(t + 1));
  }
  return m;
}
