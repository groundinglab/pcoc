//! Thorough phase. Runs only after a fast `provisional`.
//!
//! Three runs of six distinct indices, drawn from `OsRng` by the section 14.6
//! rejection rule, uniform over every leg. The commitment
//! `H("pcoc-deep-v1" ‖ seq ‖ object_sha ‖ salt ‖ indices)` is fsynced before
//! any leg is walked. Runs stop at the first mismatch. Rule A4.3 digests for
//! every leg run after the walks. A mismatch revokes when a grant was issued
//! and, under this policy, bans the public key. The salt and the indices are
//! revealed on the result. They are not readable over HTTP before that.

use crate::canon::{self, hex32};
use crate::policy::AdmissionPolicy;
use hike::{draw_distinct, leg_result, Entropy, OsEntropy};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const COMMIT_DOMAIN: &[u8] = b"pcoc-deep-v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DrawFile {
    pub seq: u64,
    pub object_sha256: String,
    pub salt: String,
    /// One run per entry. Each run is six distinct indices, in draw order.
    pub runs: Vec<Vec<u64>>,
    pub commitment: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegHit {
    pub run: u64,
    pub index: u64,
    pub stage: u64,
    pub j: u64,
    pub checkpoint: String,
}

#[derive(Clone, Debug)]
pub struct ThoroughOut {
    pub matched: bool,
    pub runs_done: u64,
    pub legs_matched: u64,
    pub legs_total: u64,
    pub failed: Option<LegHit>,
    /// Digest mismatch, after the walks. Also a revoke.
    pub digest_mismatch: bool,
    pub salt: String,
    pub indices: Vec<Vec<u64>>,
    pub commitment: String,
}

/// Draw, fsync the commitment, then walk. A file already at `pending/<seq>.draw`
/// is reused, so a restart does not draw again.
pub fn run(
    log_dir: &Path,
    seq: u64,
    object: &[u8],
    object_sha: &[u8; 32],
    policy: &AdmissionPolicy,
) -> Result<ThoroughOut, String> {
    let hike = hike_of(object)?;
    let legs = leg_total(&hike).ok_or_else(|| "leg total".to_owned())?;
    let path = draw_path(log_dir, seq);
    let draw = if path.is_file() {
        load_draw(&path, seq, object_sha)?
    } else {
        let draw = fresh_draw(seq, object_sha, legs, policy)?;
        store_draw(log_dir, &draw)?;
        draw
    };
    let pin = hike::fill_pin(&hike::production_seed(&hike.public_key, &hike.seed_beacon));
    let terrain = hike::fill_terrain(pin);
    let seed = canon::sha256(&hike::production_seed(&hike.public_key, &hike.seed_beacon));
    let mut runs_done = 0u64;
    let mut legs_matched = 0u64;
    let legs_total = draw.runs.iter().map(|run| run.len() as u64).sum();
    let mut failed = None;
    for (run_index, indices) in draw.runs.iter().enumerate() {
        runs_done += 1;
        let mut run_failed = false;
        for index in indices {
            match eval_leg(&hike, &terrain, *index, &seed) {
                Ok(true) => legs_matched += 1,
                Ok(false) => {
                    failed = Some(hit(&hike, run_index as u64, *index));
                    run_failed = true;
                    break;
                }
                Err(err) => return Err(err),
            }
        }
        if run_failed && policy.thorough_early_stop {
            break;
        }
        if run_failed {
            break;
        }
    }
    let digest_mismatch = failed.is_none() && digest_miss(&hike, &terrain, &seed);
    Ok(ThoroughOut {
        matched: failed.is_none() && !digest_mismatch,
        runs_done,
        legs_matched,
        legs_total,
        failed,
        digest_mismatch,
        salt: draw.salt,
        indices: draw.runs,
        commitment: draw.commitment,
    })
}

fn hike_of(object: &[u8]) -> Result<hike::Hike, String> {
    hike::decoded_hike(object).map_err(|_| "thorough object".to_owned())
}

pub fn commitment(seq: u64, object_sha: &[u8; 32], salt: &[u8; 32], runs: &[Vec<u64>]) -> [u8; 32] {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(COMMIT_DOMAIN);
    preimage.extend_from_slice(&seq.to_be_bytes());
    preimage.extend_from_slice(object_sha);
    preimage.extend_from_slice(salt);
    let mut flat = Vec::new();
    for run in runs {
        for index in run {
            flat.push(*index);
        }
    }
    flat.sort_unstable();
    for index in flat {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        preimage.extend_from_slice(&index.to_be_bytes());
    }
    canon::sha256(&preimage)
}

fn fresh_draw(
    seq: u64,
    object_sha: &[u8; 32],
    legs: u64,
    policy: &AdmissionPolicy,
) -> Result<DrawFile, String> {
    let mut entropy = OsEntropy;
    let mut runs = Vec::with_capacity(policy.thorough_six_leg_runs as usize);
    for _ in 0..policy.thorough_six_leg_runs {
        let indices = draw_distinct(legs, 6, &mut entropy as &mut dyn Entropy)
            .map_err(|_| "could not draw six legs".to_owned())?;
        runs.push(indices);
    }
    let mut salt = [0u8; 32];
    (&mut entropy as &mut dyn Entropy)
        .fill_bytes(&mut salt)
        .map_err(|_| "could not draw a salt".to_owned())?;
    let digest = commitment(seq, object_sha, &salt, &runs);
    Ok(DrawFile {
        seq,
        object_sha256: hex32(object_sha),
        salt: hex::encode(salt),
        runs,
        commitment: hex32(&digest),
    })
}

fn store_draw(log_dir: &Path, draw: &DrawFile) -> Result<(), String> {
    let path = draw_path(log_dir, draw.seq);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let bytes = serde_json::to_vec(draw).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("tmp");
    {
        let mut file = File::create(&tmp).map_err(|err| err.to_string())?;
        file.write_all(&bytes).map_err(|err| err.to_string())?;
        file.sync_all().map_err(|err| err.to_string())?;
    }
    fs::rename(&tmp, &path).map_err(|err| err.to_string())?;
    if let Some(parent) = path.parent() {
        let dir = File::open(parent).map_err(|err| err.to_string())?;
        dir.sync_all().map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn load_draw(path: &Path, seq: u64, object_sha: &[u8; 32]) -> Result<DrawFile, String> {
    let bytes = fs::read(path).map_err(|err| err.to_string())?;
    let draw: DrawFile = serde_json::from_slice(&bytes).map_err(|err| err.to_string())?;
    if draw.seq != seq || draw.object_sha256 != hex32(object_sha) {
        return Err("pending draw does not match this object".to_owned());
    }
    let salt = decode_salt(&draw.salt)?;
    let digest = commitment(seq, object_sha, &salt, &draw.runs);
    if hex32(&digest) != draw.commitment {
        return Err("pending draw commitment does not match".to_owned());
    }
    Ok(draw)
}

fn draw_path(log_dir: &Path, seq: u64) -> PathBuf {
    log_dir.join("pending").join(format!("{seq}.draw"))
}

fn decode_salt(text: &str) -> Result<[u8; 32], String> {
    let raw = hex::decode(text).map_err(|err| err.to_string())?;
    let bytes: [u8; 32] = raw.try_into().map_err(|_| "salt".to_owned())?;
    Ok(bytes)
}

/// The draw file is not served. Callers refuse every path under `pending/`.
pub fn draw_is_public() -> bool {
    false
}

fn leg_total(hike: &hike::Hike) -> Option<u64> {
    hike.stages.iter().try_fold(0u64, |sum, stage| {
        sum.checked_add(stage.checkpoints.len() as u64)
    })
}

fn eval_leg(hike: &hike::Hike, terrain: &[u8], index: u64, seed: &[u8; 32]) -> Result<bool, String> {
    let (stage_index, j) = locate(hike, index).ok_or_else(|| format!("leg {index}"))?;
    let stage = &hike.stages[stage_index];
    if j >= stage.leg_results.len() || j >= stage.checkpoints.len() {
        return Err(format!("leg {index} has no stored result"));
    }
    let previous = if j == 0 {
        if stage_index == 0 {
            *seed
        } else {
            stage.waypoint
        }
    } else {
        stage.checkpoints[j - 1]
    };
    let stored = u64::from_le_bytes(stage.leg_results[j]);
    Ok(leg_result(terrain, &previous) == stored)
}

fn locate(hike: &hike::Hike, q: u64) -> Option<(usize, usize)> {
    let mut start = 0u64;
    for (stage_index, stage) in hike.stages.iter().enumerate() {
        let len = stage.checkpoints.len() as u64;
        if q < start + len {
            return Some((stage_index, (q - start) as usize));
        }
        start += len;
    }
    None
}

fn hit(hike: &hike::Hike, run: u64, index: u64) -> LegHit {
    let (stage, j) = locate(hike, index).unwrap_or((0, 0));
    let checkpoint = hike
        .stages
        .get(stage)
        .and_then(|stage| stage.checkpoints.get(j))
        .map(hex::encode)
        .unwrap_or_default();
    LegHit {
        run,
        index,
        stage: stage as u64,
        j: j as u64,
        checkpoint,
    }
}

fn digest_miss(hike: &hike::Hike, terrain: &[u8], seed: &[u8; 32]) -> bool {
    let sizes = hike::Sizes::production();
    for (stage_index, stage) in hike.stages.iter().enumerate() {
        for (j, stored) in stage.checkpoints.iter().enumerate() {
            if j >= stage.leg_results.len() {
                return true;
            }
            let previous = if j == 0 {
                if stage_index == 0 {
                    *seed
                } else {
                    stage.waypoint
                }
            } else {
                stage.checkpoints[j - 1]
            };
            let limb = if stage_index == 0 {
                *seed
            } else {
                stage.waypoint
            };
            let sample = hike::terrain_sample_sized(terrain, &previous, sizes);
            let leg = u64::from_le_bytes(stage.leg_results[j]);
            let digest = hike::checkpoint_sized(&previous, &limb, &sample, leg, sizes);
            if &digest != stored {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pending_draw_is_reused_and_the_commitment_matches() {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-draw-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let object = [7u8; 32];
        let mut entropy = OsEntropy;
        let runs = vec![draw_distinct(20, 6, &mut entropy).unwrap()];
        let mut salt = [0u8; 32];
        (&mut entropy as &mut dyn Entropy).fill_bytes(&mut salt).unwrap();
        let digest = commitment(3, &object, &salt, &runs);
        let draw = DrawFile {
            seq: 3,
            object_sha256: hex32(&object),
            salt: hex::encode(salt),
            runs: runs.clone(),
            commitment: hex32(&digest),
        };
        store_draw(&dir, &draw).unwrap();
        let loaded = load_draw(&draw_path(&dir, 3), 3, &object).unwrap();
        assert_eq!(loaded.runs, runs);
        assert_eq!(loaded.commitment, draw.commitment);
        assert!(!draw_is_public());
        let _ = fs::remove_dir_all(&dir);
    }
}
