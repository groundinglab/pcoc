// Canonical path body and validate_for (SPEC §14.3).

import { sha256 } from './bigref.js';

export interface PathBody {
  leaf: Buffer;
  leafIndex: number;
  treeSize: number;
  siblings: Buffer[];
  root: Buffer;
  txid: Buffer;
  vout: number;
  blockHash: Buffer;
  blockHeight: number;
}

export const canonicalSiblingCount = (treeSize: number): number => {
  let width = treeSize;
  let n = 0;
  while (width > 1) {
    width = Math.floor(width / 2) + (width % 2);
    n++;
  }
  return n;
};

/** Decodes the layout: version 0x01, leaf, leaf_index, tree_size, sibling_count, siblings, root, txid, vout, block_hash, block_height. */
export function decodePathBody(b: Uint8Array): PathBody | undefined {
  if (b.length < 146 || b[0] !== 0x01) return undefined;
  const buf = Buffer.from(b.buffer, b.byteOffset, b.byteLength);
  const sc = buf[41]!;
  if (sc > 32 || buf.length !== 146 + 32 * sc) return undefined;
  const treeSize = buf.readUInt32BE(37);
  const leafIndex = buf.readUInt32BE(33);
  if (treeSize < 1 || leafIndex >= treeSize || canonicalSiblingCount(treeSize) !== sc) return undefined;
  let o = 42;
  const siblings: Buffer[] = [];
  for (let k = 0; k < sc; k++, o += 32) siblings.push(Buffer.from(buf.subarray(o, o + 32)));
  const root = Buffer.from(buf.subarray(o, o + 32));
  const txid = Buffer.from(buf.subarray(o + 32, o + 64));
  const vout = buf.readUInt32BE(o + 64);
  const blockHash = Buffer.from(buf.subarray(o + 68, o + 100));
  const blockHeight = buf.readUInt32BE(o + 100);
  return { leaf: Buffer.from(buf.subarray(1, 33)), leafIndex, treeSize, siblings, root, txid, vout, blockHash, blockHeight };
}

const NODE_LEAF = Buffer.of(0x1a);
const NODE_PARENT = Buffer.of(0x1b);
const parent = (l: Buffer, r: Buffer): Buffer => sha256(NODE_PARENT, l, r);

/** validate_for(waypoint): leaf equals the waypoint and the Merkle path resolves to root. */
export function validateFor(body: PathBody, waypoint: Uint8Array): boolean {
  if (!body.leaf.equals(Buffer.from(waypoint))) return false;
  let node = sha256(NODE_LEAF, body.leaf);
  let index = body.leafIndex;
  let width = body.treeSize;
  for (const sib of body.siblings) {
    if (width % 2 === 1 && index === width - 1) {
      if (!sib.equals(node)) return false;
      node = parent(node, node);
    } else if (index % 2 === 0) node = parent(node, sib);
    else node = parent(sib, node);
    index = Math.floor(index / 2);
    width = Math.floor(width / 2) + (width % 2);
  }
  return node.equals(body.root);
}

/** Builds a canonical body (used by tests and the plant fixtures). */
export function encodePathBody(b: PathBody): Buffer {
  const head = Buffer.alloc(42);
  head[0] = 1;
  b.leaf.copy(head, 1);
  head.writeUInt32BE(b.leafIndex, 33);
  head.writeUInt32BE(b.treeSize, 37);
  head[41] = b.siblings.length;
  const tail = Buffer.alloc(100 + 4);
  b.root.copy(tail, 0);
  b.txid.copy(tail, 32);
  tail.writeUInt32BE(b.vout, 64);
  b.blockHash.copy(tail, 68);
  tail.writeUInt32BE(b.blockHeight, 100);
  return Buffer.concat([head, ...b.siblings, tail]);
}
