//! Canonical path body and commitment script.

use crate::merkle::{canonical_sibling_count, MerkleError, MerklePath};
use crate::Hash32;
use thiserror::Error;

pub const PROOF_BODY_VERSION: u8 = 0x01;
pub const ANCHOR_VERSION: u8 = 0x02;
pub const MAX_SIBLINGS: usize = 32;
pub const FIXED_BODY_BYTES: usize = 146;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalPathBody {
    pub path: MerklePath,
    pub root: Hash32,
    pub txid: Hash32,
    pub vout: u32,
    pub block_hash: Hash32,
    pub block_height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ProofError {
    #[error("unsupported proof-body version {0:#04x}")]
    UnsupportedVersion(u8),
    #[error("proof body is truncated")]
    Truncated,
    #[error("proof body length {actual} differs from exact canonical length {expected}")]
    WrongLength { actual: usize, expected: usize },
    #[error("tree size must be non-zero")]
    EmptyTree,
    #[error("leaf index {index} is outside tree size {tree_size}")]
    LeafIndexOutOfRange { index: u32, tree_size: u32 },
    #[error("sibling count {0} exceeds the maximum")]
    TooManySiblings(usize),
    #[error("sibling count {actual} differs from canonical count {expected}")]
    NonCanonicalSiblingCount { actual: usize, expected: usize },
    #[error("proof leaf differs from the requested waypoint")]
    WrongLeaf,
    #[error(transparent)]
    Merkle(#[from] MerkleError),
}

impl CanonicalPathBody {
    pub fn encode(&self) -> Result<Vec<u8>, ProofError> {
        self.validate_for(&self.path.leaf)?;
        let mut bytes = Vec::with_capacity(FIXED_BODY_BYTES + self.path.siblings.len() * 32);
        bytes.push(PROOF_BODY_VERSION);
        bytes.extend_from_slice(&self.path.leaf);
        bytes.extend_from_slice(&self.path.leaf_index.to_be_bytes());
        bytes.extend_from_slice(&self.path.tree_size.to_be_bytes());
        bytes.push(self.path.siblings.len() as u8);
        for sibling in &self.path.siblings {
            bytes.extend_from_slice(sibling);
        }
        bytes.extend_from_slice(&self.root);
        bytes.extend_from_slice(&self.txid);
        bytes.extend_from_slice(&self.vout.to_be_bytes());
        bytes.extend_from_slice(&self.block_hash);
        bytes.extend_from_slice(&self.block_height.to_be_bytes());
        debug_assert_eq!(
            bytes.len(),
            FIXED_BODY_BYTES + self.path.siblings.len() * 32
        );
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProofError> {
        if bytes.len() < FIXED_BODY_BYTES {
            return Err(ProofError::Truncated);
        }
        if bytes[0] != PROOF_BODY_VERSION {
            return Err(ProofError::UnsupportedVersion(bytes[0]));
        }
        let sibling_count = bytes[41] as usize;
        if sibling_count > MAX_SIBLINGS {
            return Err(ProofError::TooManySiblings(sibling_count));
        }
        let expected_len = FIXED_BODY_BYTES + sibling_count * 32;
        if bytes.len() != expected_len {
            return Err(ProofError::WrongLength {
                actual: bytes.len(),
                expected: expected_len,
            });
        }

        let leaf = array32(&bytes[1..33]);
        let leaf_index = be_u32(&bytes[33..37]);
        let tree_size = be_u32(&bytes[37..41]);
        let siblings_start = 42;
        let siblings_end = siblings_start + sibling_count * 32;
        let siblings = bytes[siblings_start..siblings_end]
            .chunks_exact(32)
            .map(array32)
            .collect();
        let mut offset = siblings_end;
        let root = array32(&bytes[offset..offset + 32]);
        offset += 32;
        let txid = array32(&bytes[offset..offset + 32]);
        offset += 32;
        let vout = be_u32(&bytes[offset..offset + 4]);
        offset += 4;
        let block_hash = array32(&bytes[offset..offset + 32]);
        offset += 32;
        let block_height = be_u32(&bytes[offset..offset + 4]);

        let body = Self {
            path: MerklePath {
                leaf,
                leaf_index,
                tree_size,
                siblings,
            },
            root,
            txid,
            vout,
            block_hash,
            block_height,
        };
        body.validate_for(&leaf)?;
        Ok(body)
    }

    pub fn validate_for(&self, requested_leaf: &Hash32) -> Result<(), ProofError> {
        if &self.path.leaf != requested_leaf {
            return Err(ProofError::WrongLeaf);
        }
        if self.path.tree_size == 0 {
            return Err(ProofError::EmptyTree);
        }
        if self.path.leaf_index >= self.path.tree_size {
            return Err(ProofError::LeafIndexOutOfRange {
                index: self.path.leaf_index,
                tree_size: self.path.tree_size,
            });
        }
        if self.path.siblings.len() > MAX_SIBLINGS {
            return Err(ProofError::TooManySiblings(self.path.siblings.len()));
        }
        let expected = canonical_sibling_count(self.path.tree_size)?;
        if self.path.siblings.len() != expected {
            return Err(ProofError::NonCanonicalSiblingCount {
                actual: self.path.siblings.len(),
                expected,
            });
        }
        self.path.verify(&self.root)?;
        Ok(())
    }
}

pub fn commitment_script(root: &Hash32) -> Vec<u8> {
    let mut script = Vec::with_capacity(39);
    script.extend_from_slice(&[0x6A, 0x25]);
    script.extend_from_slice(b"BARI");
    script.push(ANCHOR_VERSION);
    script.extend_from_slice(root);
    script
}

fn array32(bytes: &[u8]) -> Hash32 {
    bytes
        .try_into()
        .expect("caller supplied a fixed 32-byte slice")
}

fn be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(
        bytes
            .try_into()
            .expect("caller supplied a fixed 4-byte slice"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::MerkleTree;

    fn body() -> CanonicalPathBody {
        let leaves = vec![[0x11; 32], [0x22; 32], [0x33; 32]];
        let tree = MerkleTree::build(leaves).unwrap();
        CanonicalPathBody {
            path: tree.path(&[0x22; 32]).unwrap(),
            root: tree.root(),
            txid: [0x44; 32],
            vout: 2,
            block_hash: [0x55; 32],
            block_height: 123_456,
        }
    }

    #[test]
    fn canonical_body_round_trips_and_exhausts_exactly() {
        let body = body();
        let bytes = body.encode().unwrap();
        assert_eq!(bytes.len(), 210);
        assert_eq!(CanonicalPathBody::decode(&bytes).unwrap(), body);

        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            CanonicalPathBody::decode(&trailing),
            Err(ProofError::WrongLength {
                actual: 211,
                expected: 210,
            })
        );
    }

    #[test]
    fn binary_body_preserves_asymmetric_bitcoin_identifiers_in_carried_order() {
        let mut body = body();
        body.txid = std::array::from_fn(|index| index as u8);
        body.block_hash = std::array::from_fn(|index| (0xFF - index) as u8);
        let bytes = body.encode().unwrap();
        let decoded = CanonicalPathBody::decode(&bytes).unwrap();
        assert_eq!(decoded.txid, body.txid);
        assert_eq!(decoded.block_hash, body.block_hash);
        let txid_offset = 74 + body.path.siblings.len() * 32;
        let block_hash_offset = txid_offset + 32 + 4;
        assert_eq!(&bytes[txid_offset..txid_offset + 32], &body.txid);
        assert_eq!(
            &bytes[block_hash_offset..block_hash_offset + 32],
            &body.block_hash
        );
    }

    #[test]
    fn request_leaf_and_wrong_root_fail_closed() {
        let body = body();
        assert_eq!(body.validate_for(&[0x99; 32]), Err(ProofError::WrongLeaf));
        let mut wrong = body;
        wrong.root[0] ^= 1;
        assert!(matches!(
            wrong.validate_for(&wrong.path.leaf),
            Err(ProofError::Merkle(MerkleError::WrongRoot))
        ));
    }

    #[test]
    fn commitment_script_is_exact_literal_bytes() {
        let root = [0xAB; 32];
        assert_eq!(
            hex::encode(commitment_script(&root)),
            format!("6a254241524902{}", "ab".repeat(32))
        );
        assert_eq!(commitment_script(&root).len(), 39);
    }

    #[test]
    fn complete_one_leaf_wire_body_has_independent_literal_kat() {
        let leaf = [0x11; 32];
        let tree = MerkleTree::build(vec![leaf]).unwrap();
        let body = CanonicalPathBody {
            path: tree.path(&leaf).unwrap(),
            root: tree.root(),
            txid: [0x44; 32],
            vout: 2,
            block_hash: [0x55; 32],
            block_height: 123_456,
        };
        assert_eq!(
            hex::encode(body.encode().unwrap()),
            "011111111111111111111111111111111111111111111111111111111111111111000000000000000100b7959d170da4982cf2361b9b75c52b70c1b42596e6b1f40665068cfc82ede39544444444444444444444444444444444444444444444444444444444444444440000000255555555555555555555555555555555555555555555555555555555555555550001e240"
        );
    }
}
