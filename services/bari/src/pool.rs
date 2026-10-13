//! Tip-keyed durable open waypoint pools.
//!
//! Disk layout under `{data_dir}/pools/`:
//!   `{height:010}-{tip_hash_hex}/meta.json`
//!   `{height:010}-{tip_hash_hex}/leaves.bin`  — append-only 32-byte records
//!
//! On accept: append 32 bytes then `fsync` before 204 (safer default).
//! Pools stay keyed by tip. A fee or input deferral (#216, #232) is the one carry:
//! those leaves are appended into the next tip's open pool and the old
//! directory is sealed only after that append. A carried leaf keeps the height
//! it was accepted at, in `posts.bin`, so its ask window does not restart.
//! A normal seal does not merge.

use crate::{encode_hash32, Hash32};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Tip identity for a pool (hash + height). `previous_hash` is not part of the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolTipKey {
    pub hash: Hash32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolState {
    Open,
    Handed,
    Sealed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolMeta {
    pub height: u32,
    pub tip_hash: String,
    pub state: PoolState,
}

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("pool I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("pool JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("pool meta tip hash corrupt at {0}")]
    CorruptMeta(PathBuf),
    #[error("pool leaves length not a multiple of 32 at {0}")]
    CorruptLeaves(PathBuf),
}

/// A pooled waypoint and the tip height of the POST that accepted it.
///
/// A deferral carries `post_height` forward. It is not the height of the pool
/// the leaf is sitting in now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PooledWaypoint {
    pub leaf: Hash32,
    pub post_height: u32,
}

/// In-memory + on-disk open (or handed) pool for one tip.
pub struct TipPool {
    pub tip: PoolTipKey,
    leaves: BTreeSet<Hash32>,
    /// Absent means the leaf was accepted to this pool, so the post height is
    /// [`PoolTipKey::height`].
    posts: std::collections::BTreeMap<Hash32, u32>,
    dir: PathBuf,
    file: File,
    state: PoolState,
}

impl TipPool {
    fn dir_name(tip: &PoolTipKey) -> String {
        format!("{:010}-{}", tip.height, encode_hash32(&tip.hash))
    }

    fn pool_dir(root: &Path, tip: &PoolTipKey) -> PathBuf {
        root.join("pools").join(Self::dir_name(tip))
    }

    pub fn open_or_create(data_root: &Path, tip: PoolTipKey) -> Result<Self, PoolError> {
        let dir = Self::pool_dir(data_root, &tip);
        fs::create_dir_all(&dir)?;
        let meta_path = dir.join("meta.json");
        let leaves_path = dir.join("leaves.bin");

        let state = if meta_path.exists() {
            let meta: PoolMeta = serde_json::from_reader(File::open(&meta_path)?)?;
            if meta.tip_hash != encode_hash32(&tip.hash) || meta.height != tip.height {
                return Err(PoolError::CorruptMeta(meta_path));
            }
            if meta.state == PoolState::Sealed {
                // Re-open sealed as empty open only if caller insists — treat as corrupt use.
                return Err(PoolError::CorruptMeta(meta_path));
            }
            meta.state
        } else {
            let meta = PoolMeta {
                height: tip.height,
                tip_hash: encode_hash32(&tip.hash),
                state: PoolState::Open,
            };
            write_meta_atomic(&dir, &meta)?;
            PoolState::Open
        };

        let mut leaves = BTreeSet::new();
        if leaves_path.exists() {
            let mut bytes = Vec::new();
            File::open(&leaves_path)?.read_to_end(&mut bytes)?;
            if bytes.len() % 32 != 0 {
                return Err(PoolError::CorruptLeaves(leaves_path));
            }
            for chunk in bytes.chunks_exact(32) {
                let mut leaf = [0u8; 32];
                leaf.copy_from_slice(chunk);
                leaves.insert(leaf);
            }
        }
        let posts = read_posts(&dir.join("posts.bin"))?;

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&leaves_path)?;

        Ok(Self {
            tip,
            leaves,
            posts,
            dir,
            file,
            state,
        })
    }

    pub fn state(&self) -> PoolState {
        self.state
    }

    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    pub fn contains(&self, waypoint: &Hash32) -> bool {
        self.leaves.contains(waypoint)
    }

    pub fn leaves(&self) -> &BTreeSet<Hash32> {
        &self.leaves
    }

    pub fn post_height(&self, waypoint: &Hash32) -> u32 {
        self.posts.get(waypoint).copied().unwrap_or(self.tip.height)
    }

    /// Lowest post height in the open pool. `None` when the pool is empty.
    pub fn oldest_post_height(&self) -> Option<u32> {
        self.leaves.iter().map(|leaf| self.post_height(leaf)).min()
    }

    /// Append a waypoint accepted to this tip, then fsync.
    pub fn append(&mut self, waypoint: Hash32) -> Result<bool, PoolError> {
        self.append_posted(waypoint, self.tip.height)
    }

    /// Append a waypoint that was accepted at `post_height`.
    ///
    /// A height other than this pool's tip is written to `posts.bin` and
    /// fsynced before the leaf byte string, so a crash cannot restart the
    /// ask window by forgetting the original post.
    pub fn append_posted(&mut self, waypoint: Hash32, post_height: u32) -> Result<bool, PoolError> {
        if self.leaves.contains(&waypoint) {
            return Ok(false);
        }
        if post_height != self.tip.height {
            write_post(&self.dir, waypoint, post_height)?;
            self.posts.insert(waypoint, post_height);
        }
        self.file.write_all(&waypoint)?;
        self.file.sync_all()?;
        if let Ok(dir) = File::open(&self.dir) {
            let _ = dir.sync_all();
        }
        self.leaves.insert(waypoint);
        Ok(true)
    }

    /// Stop writes; mark handed on disk. Returns leaves sorted, with post heights.
    pub fn handoff(mut self) -> Result<(PoolTipKey, Vec<PooledWaypoint>, PathBuf), PoolError> {
        self.state = PoolState::Handed;
        let meta = PoolMeta {
            height: self.tip.height,
            tip_hash: encode_hash32(&self.tip.hash),
            state: PoolState::Handed,
        };
        write_meta_atomic(&self.dir, &meta)?;
        let leaves = self
            .leaves
            .iter()
            .copied()
            .map(|leaf| PooledWaypoint {
                leaf,
                post_height: self.post_height(&leaf),
            })
            .collect();
        let tip = self.tip;
        let dir = self.dir.clone();
        drop(self.file);
        Ok((tip, leaves, dir))
    }

    pub fn mark_sealed(dir: &Path, tip: &PoolTipKey) -> Result<(), PoolError> {
        let meta = PoolMeta {
            height: tip.height,
            tip_hash: encode_hash32(&tip.hash),
            state: PoolState::Sealed,
        };
        write_meta_atomic(dir, &meta)
    }
}

fn write_meta_atomic(dir: &Path, meta: &PoolMeta) -> Result<(), PoolError> {
    let destination = dir.join("meta.json");
    let temporary = dir.join(format!(".meta.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        serde_json::to_writer_pretty(&mut file, meta)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, &destination)?;
        if let Ok(d) = File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[derive(Clone, Debug)]
pub struct ListedPool {
    pub tip: PoolTipKey,
    pub state: PoolState,
    pub dir: PathBuf,
    pub leaves: Vec<PooledWaypoint>,
}

/// List all on-disk pools (any state), sorted by height then tip hash hex.
pub fn list_pools(data_root: &Path) -> Result<Vec<ListedPool>, PoolError> {
    let pools_root = data_root.join("pools");
    if !pools_root.exists() {
        return Ok(Vec::new());
    }
    let mut dirs: Vec<PathBuf> = fs::read_dir(&pools_root)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    let mut out = Vec::new();
    for dir in dirs {
        let meta_path = dir.join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        let meta: PoolMeta = serde_json::from_reader(File::open(&meta_path)?)?;
        let tip_hash = decode_tip_hash(&meta.tip_hash)
            .ok_or_else(|| PoolError::CorruptMeta(meta_path.clone()))?;
        let leaves_path = dir.join("leaves.bin");
        let mut leaves = Vec::new();
        if leaves_path.exists() {
            let mut bytes = Vec::new();
            File::open(&leaves_path)?.read_to_end(&mut bytes)?;
            if bytes.len() % 32 != 0 {
                return Err(PoolError::CorruptLeaves(leaves_path));
            }
            for chunk in bytes.chunks_exact(32) {
                let mut leaf = [0u8; 32];
                leaf.copy_from_slice(chunk);
                leaves.push(leaf);
            }
            leaves.sort();
            leaves.dedup();
        }
        let posts = read_posts(&dir.join("posts.bin"))?;
        let tip = PoolTipKey {
            hash: tip_hash,
            height: meta.height,
        };
        let leaves = leaves
            .into_iter()
            .map(|leaf| PooledWaypoint {
                post_height: posts.get(&leaf).copied().unwrap_or(tip.height),
                leaf,
            })
            .collect();
        out.push(ListedPool {
            tip,
            state: meta.state,
            dir,
            leaves,
        });
    }
    Ok(out)
}

fn read_posts(path: &Path) -> Result<std::collections::BTreeMap<Hash32, u32>, PoolError> {
    let mut posts = std::collections::BTreeMap::new();
    if !path.exists() {
        return Ok(posts);
    }
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    if bytes.len() % 36 != 0 {
        return Err(PoolError::CorruptLeaves(path.to_path_buf()));
    }
    for chunk in bytes.chunks_exact(36) {
        let mut leaf = [0u8; 32];
        leaf.copy_from_slice(&chunk[..32]);
        let height = u32::from_le_bytes(chunk[32..36].try_into().expect("4 bytes"));
        posts.insert(leaf, height);
    }
    Ok(posts)
}

fn write_post(dir: &Path, waypoint: Hash32, post_height: u32) -> Result<(), PoolError> {
    let path = dir.join("posts.bin");
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(&waypoint)?;
    file.write_all(&post_height.to_le_bytes())?;
    file.sync_all()?;
    if let Ok(directory) = File::open(dir) {
        let _ = directory.sync_all();
    }
    Ok(())
}

fn decode_tip_hash(encoded: &str) -> Option<Hash32> {
    if encoded.len() != 64 {
        return None;
    }
    let decoded = hex::decode(encoded).ok()?;
    <[u8; 32]>::try_from(decoded.as_slice()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tip(byte: u8, height: u32) -> PoolTipKey {
        PoolTipKey {
            hash: [byte; 32],
            height,
        }
    }

    #[test]
    fn append_fsync_survives_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let t = tip(0xAB, 42);
        {
            let mut pool = TipPool::open_or_create(temp.path(), t).unwrap();
            assert!(pool.append([1; 32]).unwrap());
            assert!(!pool.append([1; 32]).unwrap());
            assert!(pool.append([2; 32]).unwrap());
            assert_eq!(pool.len(), 2);
        }
        let reopened = TipPool::open_or_create(temp.path(), t).unwrap();
        assert_eq!(reopened.len(), 2);
        assert!(reopened.contains(&[1; 32]));
        assert!(reopened.contains(&[2; 32]));
    }

    #[test]
    fn handoff_marks_handed_and_list_finds_it() {
        let temp = tempfile::tempdir().unwrap();
        let t = tip(0xCD, 7);
        let mut pool = TipPool::open_or_create(temp.path(), t).unwrap();
        pool.append([9; 32]).unwrap();
        let (tip_out, leaves, dir) = pool.handoff().unwrap();
        assert_eq!(tip_out.hash, t.hash);
        assert_eq!(
            leaves,
            vec![PooledWaypoint {
                leaf: [9; 32],
                post_height: 7,
            }]
        );
        let listed = list_pools(temp.path()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].state, PoolState::Handed);
        assert_eq!(listed[0].dir, dir);
        TipPool::mark_sealed(&dir, &t).unwrap();
        let listed2 = list_pools(temp.path()).unwrap();
        assert_eq!(listed2[0].state, PoolState::Sealed);
    }

    #[test]
    fn never_merge_across_tip_hashes() {
        let temp = tempfile::tempdir().unwrap();
        let a = tip(1, 1);
        let b = tip(2, 2);
        TipPool::open_or_create(temp.path(), a)
            .unwrap()
            .append([0xAA; 32])
            .unwrap();
        TipPool::open_or_create(temp.path(), b)
            .unwrap()
            .append([0xBB; 32])
            .unwrap();
        let listed = list_pools(temp.path()).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(
            listed[0].leaves,
            vec![PooledWaypoint {
                leaf: [0xAA; 32],
                post_height: 1,
            }]
        );
        assert_eq!(
            listed[1].leaves,
            vec![PooledWaypoint {
                leaf: [0xBB; 32],
                post_height: 2,
            }]
        );
    }

    #[test]
    fn a_carried_post_height_survives_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let later = tip(2, 9);
        {
            let mut pool = TipPool::open_or_create(temp.path(), later).unwrap();
            assert!(pool.append_posted([0xC0; 32], 7).unwrap());
            assert_eq!(pool.post_height(&[0xC0; 32]), 7);
        }
        let reopened = TipPool::open_or_create(temp.path(), later).unwrap();
        assert_eq!(reopened.post_height(&[0xC0; 32]), 7);
        let listed = list_pools(temp.path()).unwrap();
        assert_eq!(listed[0].leaves[0].post_height, 7);
    }
}
