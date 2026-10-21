//! Thorough phase. Runs only after a fast `provisional`.
//!
//! Eighteen distinct indices, drawn once from `OsRng` by the section 14.6
//! rejection rule, then split into three rounds of six. The commitment
//! `H("pcoc-deep-v1" ‖ seq ‖ object_sha ‖ salt ‖ indices)` is fsynced before
//! any leg is checked. A round's six legs run together. The next round does
//! not start after a mismatch. A stored draw is reused, including on restart
//! and on a later policy id: re-verify does not draw again. Rule A4.3 digests
//! for every leg run after the walks. A fake leg revokes when a grant was
//! issued and, under this policy, bans the public key. A short hike is not a
//! ban. The salt and the indices are revealed on the result. They are not
//! readable over HTTP before that.

use crate::canon::{self, hex32};
use crate::policy::AdmissionPolicy;
use hike::{draw_distinct, Entropy, OsEntropy};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
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
    /// The job's own provisional seq. The draw file and the commitment are
    /// keyed on it, and the thorough line's `fast_seq` is this value.
    pub seq: u64,
    pub matched: bool,
    pub runs_done: u64,
    pub legs_matched: u64,
    pub legs_total: u64,
    pub failed: Option<LegHit>,
    /// Digest mismatch, after the walks. Also a revoke.
    pub digest_mismatch: bool,
    /// A walked leg or a digest disagreed. A short draw is not this.
    /// The door bans only when this is set.
    pub fake_leg: bool,
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
    walk(
        log_dir,
        seq,
        &hike,
        object_sha,
        policy,
        hike::Sizes::production(),
    )
}

/// Restart resume of an unsettled provisional. Reuses `pending/<seq>.draw`
/// and never draws again: a missing draw file is an error, so the caller
/// holds the provisional instead of judging it with a fresh draw.
pub fn resume(
    log_dir: &Path,
    seq: u64,
    object: &[u8],
    object_sha: &[u8; 32],
    policy: &AdmissionPolicy,
) -> Result<ThoroughOut, String> {
    let hike = hike_of(object)?;
    run_draws(
        log_dir,
        seq,
        &hike,
        object_sha,
        policy,
        hike::Sizes::production(),
        Draw::StoredOnly,
    )
}

/// [`resume`] at `conformance-small` sizes, on a hike the test already built.
#[cfg(test)]
pub(crate) fn resume_for_test(
    log_dir: &Path,
    seq: u64,
    hike: &hike::Hike,
    object_sha: &[u8; 32],
    policy: &AdmissionPolicy,
) -> Result<ThoroughOut, String> {
    run_draws(
        log_dir,
        seq,
        hike,
        object_sha,
        policy,
        hike::Sizes::conformance_small(),
        Draw::StoredOnly,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Draw {
    /// Reuse a stored draw, or draw once and store it before any leg runs.
    StoredOrFresh,
    /// Reuse a stored draw. No stored draw is an error.
    StoredOnly,
}

/// [`run`] at `conformance-small` sizes, on a hike the test already built.
///
/// Production [`run`] decodes with section 6 and walks that terrain. This
/// entry is not in the service binary. A conformance-small object is not
/// offered to production.
#[cfg(test)]
pub(crate) fn run_for_test(
    log_dir: &Path,
    seq: u64,
    hike: &hike::Hike,
    object_sha: &[u8; 32],
    policy: &AdmissionPolicy,
) -> Result<ThoroughOut, String> {
    walk(
        log_dir,
        seq,
        hike,
        object_sha,
        policy,
        hike::Sizes::conformance_small(),
    )
}

fn walk(
    log_dir: &Path,
    seq: u64,
    hike: &hike::Hike,
    object_sha: &[u8; 32],
    policy: &AdmissionPolicy,
    sizes: hike::Sizes,
) -> Result<ThoroughOut, String> {
    run_draws(
        log_dir,
        seq,
        hike,
        object_sha,
        policy,
        sizes,
        Draw::StoredOrFresh,
    )
}

fn run_draws(
    log_dir: &Path,
    seq: u64,
    hike: &hike::Hike,
    object_sha: &[u8; 32],
    policy: &AdmissionPolicy,
    sizes: hike::Sizes,
    mode: Draw,
) -> Result<ThoroughOut, String> {
    let legs = leg_total(hike).ok_or_else(|| "leg total".to_owned())?;
    let path = draw_path(log_dir, seq);
    let draw = if path.is_file() {
        load_draw(&path, seq, object_sha)?
    } else if mode == Draw::StoredOnly {
        return Err(format!("no stored draw for seq {seq}; not drawn again"));
    } else if legs < 6 {
        // The fast phase already refuses this. A thorough result here would
        // revoke a grant. There is no grant to revoke, and this is not a ban.
        return Err("fewer than 6 legs".to_owned());
    } else {
        let draw = match fresh_draw(seq, object_sha, legs, policy) {
            Ok(draw) => draw,
            Err(err) if err.starts_with("short of") => return Ok(not_drawn(seq, legs)),
            Err(err) => return Err(err),
        };
        store_draw(log_dir, &draw)?;
        draw
    };
    let seed = hike::seed(&hike.public_key, &hike.seed_beacon);
    let terrain = hike::fill_terrain_sized(&seed, sizes);
    let mut runs_done = 0u64;
    let mut legs_matched = 0u64;
    let legs_total = draw.runs.iter().map(|run| run.len() as u64).sum();
    let mut failed = None;
    for (run_index, indices) in draw.runs.iter().enumerate() {
        runs_done += 1;
        // One round is `thorough_legs_per_run` legs, evaluated together.
        // A later round does not start after a mismatch in this round.
        let results = eval_round_with(indices, |index| eval_leg(hike, &terrain, index, &seed, sizes))?;
        let mut run_failed = false;
        for (index, result) in indices.iter().zip(results) {
            match result {
                Ok(true) => legs_matched += 1,
                Ok(false) => {
                    if failed.is_none() {
                        failed = Some(hit(hike, run_index as u64, *index));
                    }
                    run_failed = true;
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
    let digest_mismatch = failed.is_none() && digest_miss(hike, &terrain, &seed, sizes);
    let fake_leg = failed.is_some() || digest_mismatch;
    Ok(ThoroughOut {
        seq,
        matched: !fake_leg,
        runs_done,
        legs_matched,
        legs_total,
        failed,
        digest_mismatch,
        fake_leg,
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

/// Fewer than six legs, or fewer legs than one full distinct draw. Not a fake leg.
fn not_drawn(seq: u64, legs: u64) -> ThoroughOut {
    ThoroughOut {
        seq,
        matched: false,
        runs_done: 0,
        legs_matched: 0,
        legs_total: legs,
        failed: None,
        digest_mismatch: false,
        fake_leg: false,
        salt: String::new(),
        indices: Vec::new(),
        commitment: String::new(),
    }
}

fn fresh_draw(
    seq: u64,
    object_sha: &[u8; 32],
    legs: u64,
    policy: &AdmissionPolicy,
) -> Result<DrawFile, String> {
    let mut entropy = OsEntropy;
    let width = policy.legs_per_run();
    let rounds = policy.thorough_six_leg_runs as usize;
    let sample = width
        .checked_mul(rounds)
        .ok_or_else(|| "thorough draw count overflowed".to_owned())?;
    if (legs as usize) < sample {
        return Err(format!("short of {sample} distinct legs"));
    }
    // One draw, without replacement. The rounds are a split of that draw,
    // so an index is not selected twice across rounds.
    let drawn = draw_distinct(legs, sample, &mut entropy as &mut dyn Entropy)
        .map_err(|_| format!("could not draw {sample} legs"))?;
    let runs: Vec<Vec<u64>> = drawn.chunks(width).map(|chunk| chunk.to_vec()).collect();
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

/// Private draw file: `pending/` is 0700 and `<seq>.draw` is 0600, set in
/// code whatever the process umask is.
fn store_draw(log_dir: &Path, draw: &DrawFile) -> Result<(), String> {
    let path = draw_path(log_dir, draw.seq);
    if let Some(parent) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(PRIVATE_DIR_MODE)
            .create(parent)
            .map_err(|err| err.to_string())?;
        fs::set_permissions(parent, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
            .map_err(|err| err.to_string())?;
    }
    let bytes = serde_json::to_vec(draw).map_err(|err| err.to_string())?;
    let tmp = path.with_extension("tmp");
    {
        let mut file = private_file(&tmp)?;
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

/// Mode of a private gate file (`pending/<seq>.draw`, `banned_keys.json`).
pub const PRIVATE_FILE_MODE: u32 = 0o600;
/// Mode of the private `pending/` directory.
pub const PRIVATE_DIR_MODE: u32 = 0o700;

/// Create or truncate `path` at mode 0600, independent of the umask. An
/// existing file is set to 0600 too.
pub fn private_file(path: &Path) -> Result<File, String> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(PRIVATE_FILE_MODE)
        .open(path)
        .map_err(|err| err.to_string())?;
    file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))
        .map_err(|err| err.to_string())?;
    Ok(file)
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

/// Evaluate every index in one round at the same time. A sequential loop does
/// not satisfy the barrier a caller can put in `eval`.
pub(crate) fn eval_round_with<F>(indices: &[u64], eval: F) -> Result<Vec<Result<bool, String>>, String>
where
    F: Fn(u64) -> Result<bool, String> + Sync,
{
    std::thread::scope(|scope| {
        let eval = &eval;
        let joins: Vec<_> = indices
            .iter()
            .copied()
            .map(|index| scope.spawn(move || eval(index)))
            .collect();
        let mut out = Vec::with_capacity(joins.len());
        for join in joins {
            out.push(join.join().map_err(|_| "leg evaluator panicked".to_owned())?);
        }
        Ok(out)
    })
}

fn eval_leg(
    hike: &hike::Hike,
    terrain: &[u8],
    index: u64,
    seed: &[u8; 32],
    sizes: hike::Sizes,
) -> Result<bool, String> {
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
    Ok(hike::leg_result_sized(terrain, &previous, sizes) == stored)
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

fn digest_miss(hike: &hike::Hike, terrain: &[u8], seed: &[u8; 32], sizes: hike::Sizes) -> bool {
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

    /// A leg stored from the v2.0 fold does not match the honest walk.
    #[test]
    fn a_v2_0_fold_leg_is_refused_by_the_thorough_walk() {
        let sizes = hike::Sizes::conformance_small();
        let public_key = [0x11u8; 32];
        let mut beacon = [0u8; 32];
        beacon[0] = 1;
        let terrain = hike::rejected::honest_terrain(&public_key, &beacon, sizes);
        let seed = hike::seed(&public_key, &beacon);
        let mut stages = Vec::new();
        for j in 0..18u64 {
            let mut previous = seed;
            previous[0] ^= j as u8;
            let bad = hike::rejected::leg_result_with(
                &terrain,
                &previous,
                sizes,
                hike::rejected::step_v2_0_fold,
            );
            stages.push(hike::Stage {
                beacon,
                beacon_height: 10 + j,
                leg_count: 1,
                waypoint: previous,
                path: Vec::new(),
                checkpoints: vec![[0xCD; 32]],
                leg_results: vec![bad.to_le_bytes()],
                checkpoint_rows_defined: true,
                leg_result_missing: Vec::new(),
            });
        }
        let hike = hike::Hike {
            public_key,
            seed_beacon: beacon,
            stages,
            last_interrupt: [9u8; 32],
            last_interrupt_height: 30,
            last_interrupt_absent: false,
            last_interrupt_hash_absent: false,
            last_interrupt_height_absent: false,
        };
        let dir = std::env::temp_dir().join(format!(
            "pcoc-v2-0-fold-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let out = run_for_test(&dir, 1, &hike, &[7u8; 32], &crate::policy::AdmissionPolicy::default())
            .unwrap();
        assert!(!out.matched, "v2.0 fold was accepted");
        assert!(out.fake_leg);
        assert!(out.failed.is_some());
        let mut flat: Vec<u64> = out.indices.iter().flatten().copied().collect();
        let n = flat.len();
        flat.sort_unstable();
        flat.dedup();
        assert_eq!(flat.len(), n, "a thorough index was drawn twice");
        assert_eq!(n, 18);
        assert_eq!(out.runs_done, 1, "a later round started after the mismatch");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Fails when the six legs of a round are walked one after another.
    /// A barrier of 6 only opens if six evaluators are inside at once.
    #[test]
    fn a_round_is_not_walked_sequentially() {
        use std::sync::mpsc;
        use std::sync::{Arc, Barrier};
        use std::time::Duration;
        let barrier = Arc::new(Barrier::new(6));
        let (tx, rx) = mpsc::channel();
        let indices = [0u64, 1, 2, 3, 4, 5];
        std::thread::spawn(move || {
            let result = eval_round_with(&indices, move |_index| {
                barrier.wait();
                Ok(true)
            });
            let _ = tx.send(result);
        });
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(results)) => {
                assert_eq!(results.len(), 6);
                assert!(results.iter().all(|item| item.as_ref().ok() == Some(&true)));
            }
            Ok(Err(err)) => panic!("round failed: {err}"),
            Err(_) => panic!("timed out: the six legs were walked sequentially"),
        }
    }

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
        (&mut entropy as &mut dyn Entropy)
            .fill_bytes(&mut salt)
            .unwrap();
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
