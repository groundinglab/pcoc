import { resolve } from 'node:path';
import { generate, writeAll } from './candidates.ts';

const out = resolve(process.argv[2] ?? new URL('../../../candidates', import.meta.url).pathname);
const files = generate((s) => console.error(s));
writeAll(out, files);
console.error(`wrote ${Object.keys(files).length} files to ${out}`);
