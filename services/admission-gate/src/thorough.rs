//! Thorough phase. SPEC §15.5 draws `D = 6 · thorough_rounds` indices from
//! the operating-system CSPRNG. No byte source, no label, no seed (AC-D3).
//!
//! The 2.2 SDK does not yet recompute a caller-supplied index list (parallel
//! rounds are Phase D). After the draw is stored, this phase returns an
//! error. The door holds that error: it does not accept, and it does not
//! treat the missing recompute as a fake leg.

use crate::canon::{self, hex32};
use crate::cbor_limit::{self, MAX_CBOR_NESTING};
use crate::policy::AdmissionPolicy;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const COMMIT_DOMAIN: &[u8] = b"pcoc-deep-v1";

/// Why a stored draw was not accepted. The indices were drawn.
pub const RECOMPUTE_UNAVAILABLE: &str =
    "thorough leg recompute is not in this build";

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

/// Draw from the OS CSPRNG, fsync the commitment, then hold.
///
/// A file already at `pending/<seq>.draw` is reused, so a restart does not
/// draw again. The result is always [`RECOMPUTE_UNAVAILABLE`]: this build
/// does not accept on the draw alone.
pub fn run(
    log_dir: &Path,
    seq: u64,
    object: &[u8],
    object_sha: &[u8; 32],
    policy: &AdmissionPolicy,
) -> Result<ThoroughOut, String> {
    let path = draw_path(log_dir, seq);
    if path.is_file() {
        let _ = load_draw(&path, seq, object_sha)?;
        return Err(RECOMPUTE_UNAVAILABLE.to_owned());
    }
    let legs = leg_total(object)?;
    let rounds = u32::try_from(policy.thorough_six_leg_runs).map_err(|_| "thorough_rounds")?;
    let draw = fresh_draw(seq, object_sha, legs, rounds)?;
    store_draw(log_dir, &draw)?;
    Err(RECOMPUTE_UNAVAILABLE.to_owned())
}

/// Restart resume. Reuses `pending/<seq>.draw` and never draws again.
/// Still held: the stored indices are not recomputed in this build.
pub fn resume(
    log_dir: &Path,
    seq: u64,
    _object: &[u8],
    object_sha: &[u8; 32],
    _policy: &AdmissionPolicy,
) -> Result<ThoroughOut, String> {
    let path = draw_path(log_dir, seq);
    if !path.is_file() {
        return Err("pending draw is missing".to_owned());
    }
    let _ = load_draw(&path, seq, object_sha)?;
    Err(RECOMPUTE_UNAVAILABLE.to_owned())
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

fn leg_total(object: &[u8]) -> Result<u64, String> {
    if cbor_limit::nesting_exceeds(object, MAX_CBOR_NESTING) {
        return Err("thorough object".to_owned());
    }
    let obj = pcoc::decode_object(object).map_err(|_| "thorough object".to_owned())?;
    let total = obj
        .stages
        .iter()
        .try_fold(0u64, |sum, stage| sum.checked_add(stage.checkpoints.len() as u64))
        .ok_or_else(|| "leg total".to_owned())?;
    if total < 6 {
        return Err("fewer than 6 legs".to_owned());
    }
    Ok(total)
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
