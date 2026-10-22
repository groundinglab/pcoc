//! SPEC §14.3 canonical path body.

use crate::hash::sha256_parts;

/// SPEC §14.3. A decoded canonical path body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathBody {
    pub leaf: [u8; 32],
    pub leaf_index: u32,
    pub tree_size: u32,
    pub siblings: Vec<[u8; 32]>,
    pub root: [u8; 32],
    pub txid: [u8; 32],
    pub vout: u32,
    pub block_hash: [u8; 32],
    pub block_height: u32,
}

fn be_u32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn arr32(b: &[u8]) -> [u8; 32] {
    let mut a = [0u8; 32];
    a.copy_from_slice(b);
    a
}

/// SPEC §14.3. `canonical_sibling_count`.
pub fn canonical_sibling_count(tree_size: u32) -> u32 {
    let mut width = tree_size;
    let mut n = 0u32;
    while width > 1 {
        n += 1;
        width = width / 2 + (width % 2);
    }
    n
}

/// SPEC §14.3. Decode a canonical path body. Any other length does not decode.
pub fn decode_path(bytes: &[u8]) -> Option<PathBody> {
    if bytes.is_empty() || bytes[0] != 0x01 {
        return None;
    }
    if bytes.len() < 146 {
        return None;
    }
    let sibling_count = bytes[1 + 32 + 4 + 4] as usize;
    if sibling_count > 32 {
        return None;
    }
    let expect = 146 + 32 * sibling_count;
    if bytes.len() != expect {
        return None;
    }
    let mut i = 1;
    let leaf = arr32(&bytes[i..i + 32]);
    i += 32;
    let leaf_index = be_u32(&bytes[i..i + 4]);
    i += 4;
    let tree_size = be_u32(&bytes[i..i + 4]);
    i += 4;
    i += 1;
    if tree_size < 1 || leaf_index >= tree_size {
        return None;
    }
    if canonical_sibling_count(tree_size) != sibling_count as u32 {
        return None;
    }
    let mut siblings = Vec::with_capacity(sibling_count);
    for _ in 0..sibling_count {
        siblings.push(arr32(&bytes[i..i + 32]));
        i += 32;
    }
    let root = arr32(&bytes[i..i + 32]);
    i += 32;
    let txid = arr32(&bytes[i..i + 32]);
    i += 32;
    let vout = be_u32(&bytes[i..i + 4]);
    i += 4;
    let block_hash = arr32(&bytes[i..i + 32]);
    i += 32;
    let block_height = be_u32(&bytes[i..i + 4]);
    let _ = i;
    Some(PathBody {
        leaf,
        leaf_index,
        tree_size,
        siblings,
        root,
        txid,
        vout,
        block_hash,
        block_height,
    })
}

/// SPEC §14.3. Read `block_hash` and `block_height` from the canonical layout
/// without `validate_for`. Used for a non-empty `W_17` path.
pub fn peek_claimed(bytes: &[u8]) -> Option<([u8; 32], u32)> {
    if bytes.is_empty() || bytes[0] != 0x01 {
        return None;
    }
    if bytes.len() < 146 {
        return None;
    }
    let sibling_count = bytes[1 + 32 + 4 + 4] as usize;
    if sibling_count > 32 {
        return None;
    }
    if bytes.len() != 146 + 32 * sibling_count {
        return None;
    }
    let after_sib = 1 + 32 + 4 + 4 + 1 + 32 * sibling_count;
    let root_end = after_sib + 32;
    let txid_end = root_end + 32;
    let vout_end = txid_end + 4;
    let hash_end = vout_end + 32;
    let height_end = hash_end + 4;
    if bytes.len() < height_end {
        return None;
    }
    let block_hash = arr32(&bytes[vout_end..hash_end]);
    let block_height = be_u32(&bytes[hash_end..height_end]);
    Some((block_hash, block_height))
}

fn leaf_node(leaf: &[u8; 32]) -> [u8; 32] {
    let tag = [0x1au8];
    sha256_parts(&[&tag, leaf])
}

fn parent_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let tag = [0x1bu8];
    sha256_parts(&[&tag, left, right])
}

/// SPEC §14.3. `validate_for`: leaf equals the waypoint and the Merkle path resolves to `root`.
pub fn validate_for(body: &PathBody, waypoint: &[u8; 32]) -> bool {
    if &body.leaf != waypoint {
        return false;
    }
    let mut node = leaf_node(&body.leaf);
    let mut index = body.leaf_index;
    let mut width = body.tree_size;
    for sib in &body.siblings {
        if width % 2 == 1 && index == width - 1 {
            if sib != &node {
                return false;
            }
            node = parent_node(&node, &node);
        } else if index % 2 == 0 {
            node = parent_node(&node, sib);
        } else {
            node = parent_node(sib, &node);
        }
        index /= 2;
        width = width / 2 + (width % 2);
    }
    node == body.root
}
