//! Thorough phase. SPEC §15.5 draws `D = 6 · thorough_rounds` indices from
//! the operating-system CSPRNG. No byte source, no label, no seed (AC-D3).
//!
//! The release entry measures production constants only. Each check fills
//! one terrain from that object's seed (§10.4) and drops it before return.
//! A round starts all six legs before any result of that round is recorded.

use crate::canon::{self, hex32};
use crate::cbor_limit::{self, MAX_CBOR_NESTING};
use crate::policy::AdmissionPolicy;
use pcoc::constants::SIX_LEG_COUNT;
use pcoc::object::HikeObject;
use pcoc::profile::Profile;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Barrier;

pub const COMMIT_DOMAIN: &[u8] = b"pcoc-deep-v1";

/// Release entry on an object that is not section 6.
pub const RELEASE_PRODUCTION_ONLY: &str = "release entry measures production constants only";

/// Terrain fills performed by this process's recompute path.
static FILLS: AtomicU64 = AtomicU64::new(0);
/// OS draws performed by [`fresh_draw`]. A resume does not increment this.
static DRAWS: AtomicU64 = AtomicU64::new(0);
/// Legs that have entered the current round.
static ROUND_STARTED: AtomicU64 = AtomicU64::new(0);
/// Legs whose results have been recorded.
static ROUND_RECORDED: AtomicU64 = AtomicU64::new(0);
/// Highest `ROUND_RECORDED` observed by a leg as it passed the round barrier.
static RECORDED_AT_BARRIER: AtomicU64 = AtomicU64::new(0);

pub fn fills() -> u64 {
    FILLS.load(Ordering::SeqCst)
}

pub fn draws() -> u64 {
    DRAWS.load(Ordering::SeqCst)
}

pub fn round_started() -> u64 {
    ROUND_STARTED.load(Ordering::SeqCst)
}

pub fn recorded_at_barrier() -> u64 {
    RECORDED_AT_BARRIER.load(Ordering::SeqCst)
}

#[cfg(test)]
pub fn reset_counts() {
    FILLS.store(0, Ordering::SeqCst);
    DRAWS.store(0, Ordering::SeqCst);
    ROUND_STARTED.store(0, Ordering::SeqCst);
    ROUND_RECORDED.store(0, Ordering::SeqCst);
    RECORDED_AT_BARRIER.store(0, Ordering::SeqCst);
}

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
    pub seq: u64,
    pub matched: bool,
    pub runs_done: u64,
    pub legs_matched: u64,
    pub legs_total: u64,
    pub failed: Option<LegHit>,
    pub digest_mismatch: bool,
    pub salt: String,
    pub indices: Vec<Vec<u64>>,
    pub commitment: String,
}

/// Draw from the OS CSPRNG, fsync the commitment, then recompute the drawn
/// legs at production. A file already at `pending/<seq>.draw` is loaded and
/// not drawn again.
pub fn run(
    log_dir: &Path,
    seq: u64,
    object: &[u8],
    object_sha: &[u8; 32],
    policy: &AdmissionPolicy,
) -> Result<ThoroughOut, String> {
    let obj = decode_thorough(object)?;
    let path = draw_path(log_dir, seq);
    let loaded = if path.is_file() {
        Some(load_draw(&path, seq, object_sha)?)
    } else {
        None
    };
    if !production_shaped(&obj) {
        return Err(RELEASE_PRODUCTION_ONLY.to_owned());
    }
    let draw = match loaded {
        Some(draw) => draw,
        None => {
            let legs = leg_total_obj(&obj)?;
            let rounds =
                u32::try_from(policy.thorough_six_leg_runs).map_err(|_| "thorough_rounds")?;
            let draw = fresh_draw(seq, object_sha, legs, rounds)?;
            store_draw(log_dir, &draw)?;
            draw
        }
    };
    recompute_production(&obj, &draw, policy)
}

/// Restart resume. Reuses `pending/<seq>.draw` and never draws again.
pub fn resume(
    log_dir: &Path,
    seq: u64,
    object: &[u8],
    object_sha: &[u8; 32],
    policy: &AdmissionPolicy,
) -> Result<ThoroughOut, String> {
    let obj = decode_thorough(object)?;
    let path = draw_path(log_dir, seq);
    if !path.is_file() {
        return Err("pending draw is missing".to_owned());
    }
    let draw = load_draw(&path, seq, object_sha)?;
    if !production_shaped(&obj) {
        return Err(RELEASE_PRODUCTION_ONLY.to_owned());
    }
    recompute_production(&obj, &draw, policy)
}

fn recompute_production(
    obj: &HikeObject,
    draw: &DrawFile,
    policy: &AdmissionPolicy,
) -> Result<ThoroughOut, String> {
    let profile = Profile::production();
    recompute_loaded(obj, draw, &profile, policy.thorough_early_stop)
}

fn recompute_loaded(
    obj: &HikeObject,
    draw: &DrawFile,
    profile: &Profile,
    early_stop: bool,
) -> Result<ThoroughOut, String> {
    let (terrain, seed_d, _lanes) =
        pcoc::terrain_from_seed(&obj.public_key, &obj.seed_beacon, profile);
    FILLS.fetch_add(1, Ordering::SeqCst);
    let mut legs_matched = 0u64;
    let mut legs_total = 0u64;
    let mut failed: Option<LegHit> = None;
    let mut digest_mismatch = false;
    let mut runs_done = 0u64;
    for (run_i, run) in draw.runs.iter().enumerate() {
        if run.len() != SIX_LEG_COUNT {
            return Err("draw round".to_owned());
        }
        let hits = run_round(obj, &seed_d, &terrain, profile, run, run_i as u64)?;
        runs_done = run_i as u64 + 1;
        for hit in hits {
            legs_total += 1;
            if hit.matched {
                legs_matched += 1;
            } else if failed.is_none() {
                digest_mismatch = hit.digest_mismatch;
                failed = Some(hit.leg);
            }
        }
        if failed.is_some() && early_stop {
            break;
        }
    }
    drop(terrain);
    Ok(ThoroughOut {
        seq: draw.seq,
        matched: failed.is_none(),
        runs_done,
        legs_matched,
        legs_total,
        failed,
        digest_mismatch,
        salt: draw.salt.clone(),
        indices: draw.runs.clone(),
        commitment: draw.commitment.clone(),
    })
}

struct RoundHit {
    matched: bool,
    digest_mismatch: bool,
    leg: LegHit,
}

/// Start every leg in `run`, then record the results after they have all started.
fn run_round(
    obj: &HikeObject,
    seed_d: &[u8; 32],
    terrain: &[u8],
    profile: &Profile,
    run: &[u64],
    run_i: u64,
) -> Result<Vec<RoundHit>, String> {
    let barrier = Barrier::new(run.len());
    let started = AtomicU64::new(0);
    let mut hits = Vec::with_capacity(run.len());
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(run.len());
        for index in run {
            let index = *index;
            let barrier = &barrier;
            let started = &started;
            handles.push(scope.spawn(move || {
                started.fetch_add(1, Ordering::SeqCst);
                ROUND_STARTED.fetch_add(1, Ordering::SeqCst);
                barrier.wait();
                let seen = ROUND_RECORDED.load(Ordering::SeqCst);
                RECORDED_AT_BARRIER.fetch_max(seen, Ordering::SeqCst);
                one_leg(obj, seed_d, terrain, profile, index, run_i)
            }));
        }
        for handle in handles {
            hits.push(handle.join().unwrap_or(Err("leg worker".to_owned())));
        }
    });
    let hits = hits.into_iter().collect::<Result<Vec<_>, _>>()?;
    ROUND_RECORDED.fetch_add(hits.len() as u64, Ordering::SeqCst);
    Ok(hits)
}

fn one_leg(
    obj: &HikeObject,
    seed_d: &[u8; 32],
    terrain: &[u8],
    profile: &Profile,
    index: u64,
    run_i: u64,
) -> Result<RoundHit, String> {
    let located = locate(obj, seed_d, index).ok_or_else(|| "leg index".to_owned())?;
    let recomputed = pcoc::leg_result(terrain, profile, &located.prev);
    let digest = pcoc::checkpoint(
        &located.prev,
        &located.limb,
        terrain,
        profile,
        recomputed,
    );
    let lr_ok = recomputed.to_le_bytes() == located.stored_lr;
    let cp_ok = digest == located.stored_cp;
    let leg = LegHit {
        run: run_i,
        index,
        stage: located.stage,
        j: located.j,
        checkpoint: hex::encode(located.stored_cp),
    };
    Ok(RoundHit {
        matched: lr_ok && cp_ok,
        digest_mismatch: !cp_ok,
        leg,
    })
}

struct Located {
    stage: u64,
    j: u64,
    prev: [u8; 32],
    limb: [u8; 32],
    stored_lr: [u8; 8],
    stored_cp: [u8; 32],
}

fn locate(obj: &HikeObject, seed_d: &[u8; 32], q: u64) -> Option<Located> {
    let mut start = 0u64;
    for (g, stage) in obj.stages.iter().enumerate() {
        let n = stage.checkpoints.len() as u64;
        if q < start + n {
            let j = (q - start) as usize;
            let limb = if g == 0 {
                *seed_d
            } else {
                stage.waypoint
            };
            let prev = if j == 0 {
                limb
            } else {
                arr32(stage.checkpoints.get(j - 1)?)?
            };
            let raw = stage.leg_results.get(j)?;
            if raw.len() != 8 {
                return None;
            }
            let mut stored_lr = [0u8; 8];
            stored_lr.copy_from_slice(raw);
            let stored_cp = arr32(stage.checkpoints.get(j)?)?;
            return Some(Located {
                stage: g as u64,
                j: j as u64,
                prev,
                limb,
                stored_lr,
                stored_cp,
            });
        }
        start += n;
    }
    None
}

fn arr32(bytes: &[u8]) -> Option<[u8; 32]> {
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    Some(out)
}

fn production_shaped(obj: &HikeObject) -> bool {
    obj.fill == pcoc::constants::FILL
        && obj.slot_bytes == pcoc::constants::SLOT_BYTES as u64
        && obj.sample_bytes == pcoc::constants::SAMPLE_BYTES as u64
        && obj.steps_per_leg == pcoc::constants::STEPS_PER_LEG
        && obj.stages.len() == pcoc::constants::STAGES
        && obj.n_slots == pcoc::constants::N_SLOTS as u64
}

fn decode_thorough(object: &[u8]) -> Result<HikeObject, String> {
    if cbor_limit::nesting_exceeds(object, MAX_CBOR_NESTING) {
        return Err("thorough object".to_owned());
    }
    pcoc::decode_object(object).map_err(|_| "thorough object".to_owned())
}

#[cfg(test)]
pub fn recompute_at(
    object: &[u8],
    runs: &[Vec<u64>],
    profile: &Profile,
) -> Result<ThoroughOut, String> {
    let obj = decode_thorough(object)?;
    let draw = DrawFile {
        seq: 0,
        object_sha256: String::new(),
        salt: String::new(),
        runs: runs.to_vec(),
        commitment: String::new(),
    };
    recompute_loaded(&obj, &draw, profile, true)
}

#[cfg(test)]
pub fn store_test_draw(log_dir: &Path, draw: &DrawFile) -> Result<(), String> {
    store_draw(log_dir, draw)
}

/// One OS-CSPRNG thorough draw, split into rounds of six. Tests call this
/// twice and require the index lists to differ.
pub fn os_draw(leg_total: u64, thorough_rounds: u32) -> Result<Vec<Vec<u64>>, String> {
    let flat = pcoc::thorough_draw(leg_total, thorough_rounds).map_err(|_| "could not draw")?;
    if flat.len() != (thorough_rounds as usize).saturating_mul(6) {
        return Err("draw length".to_owned());
    }
    Ok(flat.chunks(6).map(|chunk| chunk.to_vec()).collect())
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
    rounds: u32,
) -> Result<DrawFile, String> {
    DRAWS.fetch_add(1, Ordering::SeqCst);
    let runs = os_draw(legs, rounds)?;
    let mut salt = [0u8; 32];
    for chunk in salt.chunks_mut(8) {
        let word = pcoc::draw::os_csprng_next8().ok_or_else(|| "could not draw a salt".to_owned())?;
        chunk.copy_from_slice(&word[..chunk.len()]);
    }
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

/// Create or truncate `path` at mode 0600, independent of the umask.
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

/// The draw file is not served.
pub fn draw_is_public() -> bool {
    false
}

fn leg_total_obj(obj: &HikeObject) -> Result<u64, String> {
    let total = obj
        .stages
        .iter()
        .try_fold(0u64, |sum, stage| {
            sum.checked_add(stage.checkpoints.len() as u64)
        })
        .ok_or_else(|| "leg total".to_owned())?;
    if total < 6 {
        return Err("fewer than 6 legs".to_owned());
    }
    Ok(total)
}

/// One leg, below the release entry. Fills one terrain for `profile` and drops it.
#[cfg(test)]
pub fn recompute_one(
    object: &[u8],
    index: u64,
    profile: &Profile,
) -> Result<ThoroughOut, String> {
    let obj = decode_thorough(object)?;
    let (terrain, seed_d, _lanes) =
        pcoc::terrain_from_seed(&obj.public_key, &obj.seed_beacon, profile);
    FILLS.fetch_add(1, Ordering::SeqCst);
    let hit = one_leg(&obj, &seed_d, &terrain, profile, index, 0)?;
    drop(terrain);
    Ok(ThoroughOut {
        seq: 0,
        matched: hit.matched,
        runs_done: 1,
        legs_matched: u64::from(hit.matched),
        legs_total: 1,
        failed: if hit.matched { None } else { Some(hit.leg) },
        digest_mismatch: hit.digest_mismatch,
        salt: String::new(),
        indices: vec![vec![index]],
        commitment: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_os_draws_differ() {
        let first = os_draw(64, 3).expect("draw");
        let second = os_draw(64, 3).expect("draw");
        assert_eq!(first.len(), 3);
        assert!(first.iter().all(|run| run.len() == 6));
        assert_ne!(first, second);
    }
}
