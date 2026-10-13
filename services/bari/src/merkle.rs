//! Merkle construction for one waypoint batch.

use crate::Hash32;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const LEAF_DOMAIN: u8 = 0x1A;
pub const PARENT_DOMAIN: u8 = 0x1B;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum MerkleError {
    #[error("a BARI batch cannot be empty")]
    EmptyBatch,
    #[error("leaf is not in this batch")]
    LeafAbsent,
    #[error("tree size must be non-zero")]
    EmptyPathTree,
    #[error("leaf index {index} is outside tree size {tree_size}")]
    LeafIndexOutOfRange { index: u32, tree_size: u32 },
    #[error("sibling count {actual} differs from canonical count {expected}")]
    NonCanonicalSiblingCount { actual: usize, expected: usize },
    #[error("odd-final path step does not carry the required self-sibling")]
    WrongSelfSibling,
    #[error("path root differs from the carried root")]
    WrongRoot,
    #[error("tree exceeds the u32 proof grammar")]
    TreeTooLarge,
}

#[inline]
pub fn leaf_node(leaf: &Hash32) -> Hash32 {
    let mut hash = Sha256::new();
    hash.update([LEAF_DOMAIN]);
    hash.update(leaf);
    hash.finalize().into()
}

#[inline]
pub fn parent_node(left: &Hash32, right: &Hash32) -> Hash32 {
    let mut hash = Sha256::new();
    hash.update([PARENT_DOMAIN]);
    hash.update(left);
    hash.update(right);
    hash.finalize().into()
}

pub fn canonical_sibling_count(mut width: u32) -> Result<usize, MerkleError> {
    if width == 0 {
        return Err(MerkleError::EmptyPathTree);
    }
    let mut count = 0usize;
    while width > 1 {
        width = width / 2 + width % 2;
        count += 1;
    }
    Ok(count)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MerklePath {
    pub leaf: Hash32,
    pub leaf_index: u32,
    pub tree_size: u32,
    pub siblings: Vec<Hash32>,
}

impl MerklePath {
    pub fn resolve(&self) -> Result<Hash32, MerkleError> {
        if self.tree_size == 0 {
            return Err(MerkleError::EmptyPathTree);
        }
        if self.leaf_index >= self.tree_size {
            return Err(MerkleError::LeafIndexOutOfRange {
                index: self.leaf_index,
                tree_size: self.tree_size,
            });
        }
        let expected = canonical_sibling_count(self.tree_size)?;
        if self.siblings.len() != expected {
            return Err(MerkleError::NonCanonicalSiblingCount {
                actual: self.siblings.len(),
                expected,
            });
        }

        let mut node = leaf_node(&self.leaf);
        let mut index = self.leaf_index;
        let mut width = self.tree_size;
        for sibling in &self.siblings {
            node = if width % 2 == 1 && index == width - 1 {
                if sibling != &node {
                    return Err(MerkleError::WrongSelfSibling);
                }
                parent_node(&node, &node)
            } else if index % 2 == 0 {
                parent_node(&node, sibling)
            } else {
                parent_node(sibling, &node)
            };
            index /= 2;
            width = width / 2 + width % 2;
        }
        debug_assert_eq!(width, 1);
        debug_assert_eq!(index, 0);
        Ok(node)
    }

    pub fn verify(&self, expected_root: &Hash32) -> Result<(), MerkleError> {
        if &self.resolve()? == expected_root {
            Ok(())
        } else {
            Err(MerkleError::WrongRoot)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MerkleTree {
    leaves: Vec<Hash32>,
    levels: Vec<Vec<Hash32>>,
}

impl MerkleTree {
    pub fn build(mut submitted: Vec<Hash32>) -> Result<Self, MerkleError> {
        submitted.sort_unstable();
        submitted.dedup();
        if submitted.is_empty() {
            return Err(MerkleError::EmptyBatch);
        }
        if submitted.len() > u32::MAX as usize {
            return Err(MerkleError::TreeTooLarge);
        }

        let mut levels = vec![submitted.iter().map(leaf_node).collect::<Vec<_>>()];
        while levels.last().expect("one non-empty level").len() > 1 {
            let current = levels.last().expect("one non-empty level");
            let mut next = Vec::with_capacity(current.len().div_ceil(2));
            for pair in current.chunks(2) {
                let right = pair.get(1).unwrap_or(&pair[0]);
                next.push(parent_node(&pair[0], right));
            }
            levels.push(next);
        }
        Ok(Self {
            leaves: submitted,
            levels,
        })
    }

    pub fn leaves(&self) -> &[Hash32] {
        &self.leaves
    }

    pub fn tree_size(&self) -> u32 {
        self.leaves.len() as u32
    }

    pub fn root(&self) -> Hash32 {
        self.levels.last().expect("tree has a root")[0]
    }

    pub fn path(&self, leaf: &Hash32) -> Result<MerklePath, MerkleError> {
        let mut index = self
            .leaves
            .binary_search(leaf)
            .map_err(|_| MerkleError::LeafAbsent)?;
        let leaf_index = index as u32;
        let mut width = self.leaves.len();
        let mut siblings = Vec::with_capacity(self.levels.len().saturating_sub(1));
        for level in &self.levels[..self.levels.len() - 1] {
            let sibling_index = if width % 2 == 1 && index == width - 1 {
                index
            } else {
                index ^ 1
            };
            siblings.push(level[sibling_index]);
            index /= 2;
            width = width.div_ceil(2);
        }
        Ok(MerklePath {
            leaf: *leaf,
            leaf_index,
            tree_size: self.tree_size(),
            siblings,
        })
    }

    pub fn from_sorted_distinct(leaves: Vec<Hash32>) -> Result<Self, MerkleError> {
        if leaves.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Self::build(leaves);
        }
        Self::build(leaves)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_leaf_literal_known_answer() {
        let leaf = [0x00; 32];
        assert_eq!(
            hex::encode(leaf_node(&leaf)),
            "a785c0de376b0efc77247f5c3a028933d6a877579c76b026a57f921c8aece0fc"
        );
        let tree = MerkleTree::build(vec![leaf]).unwrap();
        let path = tree.path(&leaf).unwrap();
        assert!(path.siblings.is_empty());
        path.verify(&tree.root()).unwrap();
    }

    #[test]
    fn sort_dedupe_odd_width_and_paths_have_literal_answers() {
        let a = [0x11; 32];
        let b = [0x22; 32];
        let c = [0x33; 32];
        let tree = MerkleTree::build(vec![c, a, b, a]).unwrap();
        assert_eq!(tree.leaves(), &[a, b, c]);
        assert_eq!(
            hex::encode(tree.root()),
            "9c9a3f601444d1adc90e749e6a7972ea850c3029d2e8504ddfdaf111e4ecac90"
        );
        for leaf in [a, b, c] {
            tree.path(&leaf).unwrap().verify(&tree.root()).unwrap();
        }
        let odd_path = tree.path(&c).unwrap();
        assert_eq!(odd_path.siblings[0], leaf_node(&c));
    }

    #[test]
    fn malformed_self_sibling_and_wrong_root_reject() {
        let tree = MerkleTree::build(vec![[1; 32], [2; 32], [3; 32]]).unwrap();
        let mut path = tree.path(&[3; 32]).unwrap();
        path.siblings[0][0] ^= 1;
        assert_eq!(path.resolve(), Err(MerkleError::WrongSelfSibling));

        let valid = tree.path(&[1; 32]).unwrap();
        assert_eq!(valid.verify(&[0; 32]), Err(MerkleError::WrongRoot));
    }
}
