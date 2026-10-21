//! Rule B10. The first successful fresh tip for a client submission is the
//! decision. A failed or stale read is not stored. Held retries of that
//! submission, and a service-started re-verify, reuse the stored decision.
//! A policy-version bump does not decide the rule again and never turns a
//! stored pass into a deadline refuse. A client resubmission is a new
//! submission id and is decided again. A lower tip does not reopen a refuse.

use crate::canon::hex32;
use crate::log;
use crate::thorough;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The deadline outcome of one successful tip read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// `tip − last_beacon_height ≤ K`. Not a refuse.
    Within { tip: u64 },
    /// The tip is below the last beacon. Held. Not a refuse.
    Below { tip: u64 },
    /// `tip − last_beacon_height > K`. Fast refuse. A later lower tip does
    /// not reopen it.
    Late { tip: u64 },
}

impl Verdict {
    fn tag(self) -> &'static str {
        match self {
            Verdict::Within { .. } => "within",
            Verdict::Below { .. } => "below",
            Verdict::Late { .. } => "late",
        }
    }

    pub fn tip(self) -> u64 {
        match self {
            Verdict::Within { tip } | Verdict::Below { tip } | Verdict::Late { tip } => tip,
        }
    }

    fn from_parts(tag: &str, tip: u64) -> Option<Self> {
        match tag {
            "within" => Some(Verdict::Within { tip }),
            "below" => Some(Verdict::Below { tip }),
            "late" => Some(Verdict::Late { tip }),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct Row {
    submission: [u8; 32],
    public_key: [u8; 32],
    object_sha256: [u8; 32],
    policy_id: String,
    verdict: Verdict,
}

/// `{log_dir}/deadline.json`, mode 0600. Not keyed by `policy_id`.
pub struct Book {
    path: PathBuf,
    rows: HashMap<[u8; 32], Row>,
}

impl Book {
    pub fn open(log_dir: &Path) -> Result<Self, String> {
        let path = log_dir.join("deadline.json");
        let rows = if path.exists() {
            load(&path)?
        } else {
            HashMap::new()
        };
        Ok(Self { path, rows })
    }

    /// The decision already stored for this client submission.
    pub fn for_submission(&self, submission: &[u8; 32]) -> Option<Verdict> {
        self.rows.get(submission).map(|row| row.verdict)
    }

    /// A service-started re-verify. The submission's own row wins. Otherwise
    /// a stored pass for this object wins, so a policy bump cannot revoke on
    /// this rule. A stored refuse is not reopened.
    pub fn for_reverify(
        &self,
        submission: &[u8; 32],
        public_key: &[u8; 32],
        object: &[u8; 32],
    ) -> Option<Verdict> {
        if let Some(verdict) = self.for_submission(submission) {
            return Some(verdict);
        }
        let same: Vec<Verdict> = self
            .rows
            .values()
            .filter(|row| row.public_key == *public_key && row.object_sha256 == *object)
            .map(|row| row.verdict)
            .collect();
        if let Some(pass) = same
            .iter()
            .copied()
            .find(|verdict| matches!(verdict, Verdict::Within { .. }))
        {
            return Some(pass);
        }
        if let Some(held) = same
            .iter()
            .copied()
            .find(|verdict| matches!(verdict, Verdict::Below { .. }))
        {
            return Some(held);
        }
        same.into_iter().next()
    }

    /// Record the first successful tip. A second call for the same submission
    /// leaves the first decision in place.
    pub fn store(
        &mut self,
        submission: [u8; 32],
        public_key: [u8; 32],
        object: [u8; 32],
        policy_id: &str,
        verdict: Verdict,
    ) -> Result<(), String> {
        if self.rows.contains_key(&submission) {
            return Ok(());
        }
        self.rows.insert(
            submission,
            Row {
                submission,
                public_key,
                object_sha256: object,
                policy_id: policy_id.to_owned(),
                verdict,
            },
        );
        self.flush()
    }

    fn flush(&self) -> Result<(), String> {
        let mut map = Map::new();
        for row in self.rows.values() {
            map.insert(
                hex32(&row.submission),
                serde_json::json!({
                    "public_key": hex32(&row.public_key),
                    "object_sha256": hex32(&row.object_sha256),
                    "policy_id": row.policy_id,
                    "verdict": row.verdict.tag(),
                    "tip": row.verdict.tip(),
                }),
            );
        }
        let bytes = serde_json::to_vec(&Value::Object(map)).map_err(|err| err.to_string())?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut file = thorough::private_file(&tmp)?;
            file.write_all(&bytes).map_err(|err| err.to_string())?;
            file.sync_all().map_err(|err| err.to_string())?;
        }
        fs::rename(&tmp, &self.path).map_err(|err| err.to_string())?;
        if let Some(parent) = self.path.parent() {
            let _ = fs::File::open(parent).and_then(|handle| handle.sync_all());
        }
        Ok(())
    }
}

/// Highest tip among the sources that were read. A failed or stale source
/// is dropped. No successful source is `not read`.
pub fn highest_tip(reads: impl IntoIterator<Item = Result<u64, ()>>) -> Result<u64, ()> {
    let mut best: Option<u64> = None;
    for read in reads {
        if let Ok(tip) = read {
            best = Some(best.map_or(tip, |have| have.max(tip)));
        }
    }
    best.ok_or(())
}

/// One successful tip against the last beacon. `None` when the last beacon
/// was not read: that is not a verdict and must not be stored.
pub fn judge(tip: u64, last: Option<u64>, k: u64) -> Option<Verdict> {
    let last = last?;
    if tip < last {
        Some(Verdict::Below { tip })
    } else if tip.saturating_sub(last) > k {
        Some(Verdict::Late { tip })
    } else {
        Some(Verdict::Within { tip })
    }
}

fn load(path: &Path) -> Result<HashMap<[u8; 32], Row>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("could not read {}: {err}", path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|err| format!("{} is not JSON: {err}", path.display()))?;
    let Some(map) = value.as_object() else {
        return Err(format!("{} is not an object", path.display()));
    };
    let mut rows = HashMap::new();
    for (key, item) in map {
        let Some(submission) = log::parse_hex32(key) else {
            continue;
        };
        let Some(item) = item.as_object() else {
            continue;
        };
        let Some(public_key) = item
            .get("public_key")
            .and_then(|value| value.as_str())
            .and_then(log::parse_hex32)
        else {
            continue;
        };
        let Some(object) = item
            .get("object_sha256")
            .and_then(|value| value.as_str())
            .and_then(log::parse_hex32)
        else {
            continue;
        };
        let Some(policy_id) = item.get("policy_id").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(tag) = item.get("verdict").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(tip) = item.get("tip").and_then(|value| value.as_u64()) else {
            continue;
        };
        let Some(verdict) = Verdict::from_parts(tag, tip) else {
            continue;
        };
        rows.insert(
            submission,
            Row {
                submission,
                public_key,
                object_sha256: object,
                policy_id: policy_id.to_owned(),
                verdict,
            },
        );
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("pcoc-deadline-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_highest_read_tip_wins_and_a_total_miss_is_not_read() {
        assert_eq!(highest_tip([Ok(10), Err(()), Ok(14)]), Ok(14));
        assert_eq!(highest_tip([Err(()), Ok(7)]), Ok(7));
        assert!(highest_tip([Err(()), Err(())]).is_err());
        assert!(highest_tip(std::iter::empty()).is_err());
    }

    #[test]
    fn gap_three_is_within_and_gap_four_is_late() {
        assert_eq!(judge(103, Some(100), 3), Some(Verdict::Within { tip: 103 }));
        assert_eq!(judge(104, Some(100), 3), Some(Verdict::Late { tip: 104 }));
        assert_eq!(judge(99, Some(100), 3), Some(Verdict::Below { tip: 99 }));
        assert!(judge(104, None, 3).is_none());
    }

    #[test]
    fn the_first_tip_stays_and_a_bump_does_not_revoke() {
        let dir = scratch();
        let mut book = Book::open(&dir).unwrap();
        let submission = [1u8; 32];
        let public_key = [2u8; 32];
        let object = [3u8; 32];
        book.store(
            submission,
            public_key,
            object,
            "gl-admit-v8",
            Verdict::Within { tip: 103 },
        )
        .unwrap();
        book.store(
            submission,
            public_key,
            object,
            "gl-admit-v9",
            Verdict::Late { tip: 200 },
        )
        .unwrap();
        assert_eq!(
            book.for_submission(&submission),
            Some(Verdict::Within { tip: 103 })
        );
        let resubmit = [9u8; 32];
        assert!(book.for_submission(&resubmit).is_none());
        assert_eq!(
            book.for_reverify(&resubmit, &public_key, &object),
            Some(Verdict::Within { tip: 103 }),
            "a policy bump reuses the pass and does not revoke"
        );
        let mode = fs::metadata(dir.join("deadline.json")).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(mode.mode() & 0o777, 0o600);
        let reopened = Book::open(&dir).unwrap();
        assert_eq!(
            reopened.for_submission(&submission),
            Some(Verdict::Within { tip: 103 })
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stored_refuse_is_not_reopened_by_a_lower_tip() {
        let dir = scratch();
        let mut book = Book::open(&dir).unwrap();
        let submission = [4u8; 32];
        book.store(
            submission,
            [2u8; 32],
            [3u8; 32],
            "gl-admit-v8",
            Verdict::Late { tip: 104 },
        )
        .unwrap();
        book.store(
            submission,
            [2u8; 32],
            [3u8; 32],
            "gl-admit-v8",
            Verdict::Within { tip: 100 },
        )
        .unwrap();
        assert_eq!(
            book.for_submission(&submission),
            Some(Verdict::Late { tip: 104 })
        );
        assert_eq!(
            book.for_reverify(&submission, &[2u8; 32], &[3u8; 32]),
            Some(Verdict::Late { tip: 104 })
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
