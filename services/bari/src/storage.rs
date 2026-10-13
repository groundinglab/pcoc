//! Durable batch records and append-only operator events.

use crate::Hash32;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(test)]
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredState {
    Persisted,
    Broadcast,
    PlantFailed,
    Included,
    Skipped,
    Flushed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredPlant {
    pub txid: Hash32,
    pub vout: u32,
    pub fee_rate_sat_vb: f64,
    pub fee_source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredInclusion {
    pub block_hash: Hash32,
    pub block_height: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistedBatch {
    pub batch_sequence: u64,
    pub opened_tip: Hash32,
    pub closed_tip: Hash32,
    pub leaves: Vec<Hash32>,
    pub tree_size: u32,
    pub root: Hash32,
    pub state: StoredState,
    pub plant: Option<StoredPlant>,
    pub inclusion: Option<StoredInclusion>,
    pub skip_reason: Option<String>,
    /// Height of the tip the waypoint was posted on. The serve window is
    /// counted from this height. Absent on records written before that pin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_tip_height: Option<u32>,
    /// Broadcast height B. Set when `plant` returns. Older records are filled
    /// once at open. Absent means the page leaves the tree out of the ledger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broadcast_height: Option<u32>,
    /// Next-block minimum read at the decision that sent this tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_block_min_sat_vb: Option<f64>,
    /// Absolute fee of the funded transaction, in sats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_sat: Option<u64>,
    /// vsize of the signed transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vsize: Option<u32>,
    /// Tip height of the POST that accepted each leaf, in the same order as
    /// `leaves`. A carried waypoint keeps the height it was first accepted at.
    /// Absent on records written before that pin. The page uses it to check
    /// that every waypoint in a sent tree was pooled before the send height.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub leaf_post_heights: Vec<u32>,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("storage JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("batch {0} already exists")]
    BatchExists(u64),
    #[error("batch filename and record sequence disagree at {0}")]
    SequenceMismatch(PathBuf),
    #[error("operator-event fields must be a JSON object")]
    EventFields,
}

/// Durable portable-index tip blob (L1). Same grammar as live `GET /index` (v2).
/// Header hash / plant txid / plant block_hash use Bitcoin display hex; plant.root
/// uses natural-order hex — identical presentation to the HTTP index surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPortableIndex {
    pub version: u8,
    pub network: String,
    pub header: StoredIndexHeader,
    pub plant: Option<StoredIndexPlant>,
    /// Every plant included in this block. Empty when this height included none.
    /// One `plant` slot is not the whole block.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plants: Vec<StoredIndexPlant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredIndexHeader {
    pub hash: String,
    pub height: u32,
    /// Bitcoin header `nTime` in **unix seconds** (the integer `getblockheader`
    /// returns as `time`, and gossip header bytes 68..72). Not milliseconds.
    /// Omitted when that header time was not read. This is not Bari's
    /// wall-clock observation (`timestamp_unix_ms`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,
    /// Unix milliseconds of Bari's wall-clock observation (`SystemTime::now`
    /// on the `selected_tip_observed` line) for this header hash. Written when
    /// the tip is persisted. Omitted when that event was not recorded.
    /// This is not Bitcoin `nTime`. Verifiers that still read an observation
    /// clock use this field; on-chain time is `time`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_unix_ms: Option<u64>,
}

/// Result of stamping retained index blobs from `selected_tip_observed`.
/// Crate tests only. There is no public backfill binary.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexObservationBackfill {
    /// `selected_tip_observed` lines seen in `events.jsonl`.
    pub observed_events: u64,
    /// Distinct (height, hash) pairs that carried a positive timestamp.
    pub distinct_tips: u64,
    /// Blobs rewritten because they had no observation stamp.
    pub rewritten: u64,
    /// Blobs that already had `header.timestamp_unix_ms` (left unchanged).
    pub already_stamped: u64,
    /// Retained blobs with no matching observation event.
    pub unmatched: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredIndexPlant {
    pub root: String,
    pub tree_size: u32,
    pub txid: String,
    pub vout: u32,
    pub block_hash: String,
    pub block_height: u32,
}

#[derive(Clone, Debug)]
pub struct FileStore {
    root: PathBuf,
    batches: PathBuf,
    index_tips: PathBuf,
    events: PathBuf,
}

impl FileStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StorageError> {
        let root = root.as_ref().to_path_buf();
        let batches = root.join("batches");
        fs::create_dir_all(&batches)?;
        let index_tips = root.join("index");
        fs::create_dir_all(&index_tips)?;
        let events = root.join("events.jsonl");
        Ok(Self {
            root,
            batches,
            index_tips,
            events,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn batch_path(&self, batch_sequence: u64) -> PathBuf {
        self.batches
            .join(format!("batch-{batch_sequence:020}.json"))
    }

    pub fn persist_new(&self, batch: &PersistedBatch) -> Result<(), StorageError> {
        let destination = self.batch_path(batch.batch_sequence);
        if destination.exists() {
            return Err(StorageError::BatchExists(batch.batch_sequence));
        }
        self.write_atomic(batch, &destination, true)
    }

    pub fn update(&self, batch: &PersistedBatch) -> Result<(), StorageError> {
        let destination = self.batch_path(batch.batch_sequence);
        self.write_atomic(batch, &destination, false)
    }

    /// Remove a batch file that was persisted and then not broadcast.
    /// A missing file is already gone.
    pub fn discard_unbroadcast(&self, batch_sequence: u64) -> Result<(), StorageError> {
        let path = self.batch_path(batch_sequence);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(StorageError::Io(error)),
        }
    }

    pub fn load_batches(&self) -> Result<Vec<PersistedBatch>, StorageError> {
        let mut paths = fs::read_dir(&self.batches)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        paths.sort();

        let mut batches = Vec::with_capacity(paths.len());
        for path in paths {
            let file = File::open(&path)?;
            let batch: PersistedBatch = serde_json::from_reader(file)?;
            if self.batch_path(batch.batch_sequence) != path {
                return Err(StorageError::SequenceMismatch(path));
            }
            batches.push(batch);
        }
        Ok(batches)
    }

    /// Event lines whose `event` field equals `name`, in file order.
    /// A missing log is an empty list.
    pub fn load_events_named(&self, name: &str) -> Result<Vec<Value>, StorageError> {
        if !self.events.is_file() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.events)?;
        let mut found = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(line)?;
            if value.get("event").and_then(Value::as_str) == Some(name) {
                found.push(value);
            }
        }
        Ok(found)
    }

    /// Append an observation at the time it occurs. The event name and
    /// timestamp are owned by this boundary rather than reconstructed later.
    /// Returns the `timestamp_unix_ms` written on the line.
    pub fn append_event(&self, event: &str, fields: Value) -> Result<u64, StorageError> {
        let Value::Object(mut record) = fields else {
            return Err(StorageError::EventFields);
        };
        record.insert("event".to_owned(), Value::String(event.to_owned()));
        let observed_ms = timestamp_unix_ms();
        record.insert("timestamp_unix_ms".to_owned(), Value::from(observed_ms));
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.events)?;
        serde_json::to_writer(&mut file, &Value::Object(record))?;
        file.write_all(b"\n")?;
        file.sync_data()?;
        Ok(observed_ms)
    }

    pub fn index_tip_path(&self, height: u32) -> PathBuf {
        self.index_tips.join(format!("height-{height:010}.json"))
    }

    /// Persist / replace the portable-index blob for this tip height (L1).
    /// Last write wins for a height key (reorg / re-publish).
    pub fn persist_index_tip(&self, tip: &StoredPortableIndex) -> Result<(), StorageError> {
        write_index_tip(&self.index_tips, tip)
    }

    pub fn load_index_tip(&self, height: u32) -> Result<Option<StoredPortableIndex>, StorageError> {
        let path = self.index_tip_path(height);
        if !path.exists() {
            return Ok(None);
        }
        let file = File::open(path)?;
        Ok(Some(serde_json::from_reader(file)?))
    }

    /// Highest retained tip by height filename (`height-XXXXXXXXXX.json`).
    /// Scans the index cache directory; does not use mtime (reorgs replace in place).
    pub fn load_latest_index_tip(&self) -> Result<Option<StoredPortableIndex>, StorageError> {
        IndexCache::open(&self.index_tips)?.load_latest()
    }

    /// Absolute path to the on-disk index cache directory (`…/index`).
    pub fn index_dir(&self) -> &Path {
        &self.index_tips
    }

    /// Stamp retained `height-*.json` blobs with the `timestamp_unix_ms` from
    /// matching `selected_tip_observed` lines. Last event for a (height, hash)
    /// pair wins. A blob that already has `header.timestamp_unix_ms` is left
    /// as written. Blobs with no matching event stay unstamped — this does
    /// not invent a cadence.
    ///
    /// Held off the public binary. Compiled for crate tests only.
    #[cfg(test)]
    pub(crate) fn backfill_index_observation_times(
        &self,
    ) -> Result<IndexObservationBackfill, StorageError> {
        let mut by_tip: HashMap<(u32, String), u64> = HashMap::new();
        let mut observed_events = 0u64;
        if self.events.is_file() {
            let text = fs::read_to_string(&self.events)?;
            for line in text.lines() {
                let Ok(value) = serde_json::from_str::<Value>(line) else {
                    continue;
                };
                if value.get("event").and_then(Value::as_str) != Some("selected_tip_observed") {
                    continue;
                }
                observed_events += 1;
                let Some(height) = value
                    .get("height")
                    .and_then(Value::as_u64)
                    .and_then(|height| u32::try_from(height).ok())
                else {
                    continue;
                };
                let Some(hash) = value.get("hash").and_then(Value::as_str) else {
                    continue;
                };
                let Some(ms) = value
                    .get("timestamp_unix_ms")
                    .and_then(Value::as_u64)
                    .filter(|ms| *ms > 0)
                else {
                    continue;
                };
                by_tip.insert((height, hash.to_owned()), ms);
            }
        }

        let distinct_tips = by_tip.len() as u64;
        let mut rewritten = 0u64;
        let mut already_stamped = 0u64;
        let mut unmatched = 0u64;
        for entry in fs::read_dir(&self.index_tips)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(rest) = name
                .strip_prefix("height-")
                .and_then(|rest| rest.strip_suffix(".json"))
            else {
                continue;
            };
            if rest.len() != 10 || !rest.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let path = entry.path();
            let mut tip: StoredPortableIndex = serde_json::from_reader(File::open(&path)?)?;
            if tip.header.timestamp_unix_ms.is_some() {
                already_stamped += 1;
                continue;
            }
            let key = (tip.header.height, tip.header.hash.clone());
            let Some(ms) = by_tip.get(&key).copied() else {
                unmatched += 1;
                continue;
            };
            tip.header.timestamp_unix_ms = Some(ms);
            self.persist_index_tip(&tip)?;
            rewritten += 1;
        }

        Ok(IndexObservationBackfill {
            observed_events,
            distinct_tips,
            rewritten,
            already_stamped,
            unmatched,
        })
    }

    fn write_atomic(
        &self,
        batch: &PersistedBatch,
        destination: &Path,
        require_absent: bool,
    ) -> Result<(), StorageError> {
        let nonce = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = self.batches.join(format!(
            ".batch-{:020}.{}.{}.tmp",
            batch.batch_sequence,
            std::process::id(),
            nonce
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            serde_json::to_writer_pretty(&mut file, batch)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            if require_absent && destination.exists() {
                return Err(StorageError::BatchExists(batch.batch_sequence));
            }
            fs::rename(&temporary, destination)?;
            File::open(&self.batches)?.sync_all()?;
            Ok(())
        })();

        if temporary.exists() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn index_tip_filename(height: u32) -> String {
    format!("height-{height:010}.json")
}

fn write_index_tip(dir: &Path, tip: &StoredPortableIndex) -> Result<(), StorageError> {
    let destination = dir.join(index_tip_filename(tip.header.height));
    let nonce = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = dir.join(format!(
        ".height-{:010}.{}.{}.tmp",
        tip.header.height,
        std::process::id(),
        nonce
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        serde_json::to_writer_pretty(&mut file, tip)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temporary, &destination)?;
        File::open(dir)?.sync_all()?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn timestamp_unix_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

/// Read-only view of a Bari portable-index cache directory (`height-XXXXXXXXXX.json`).
///
/// An index reader's data directory should point at this directory
/// (typically `$BARI_DATA_DIR/index`), not the Bari data root.
#[derive(Clone, Debug)]
pub struct IndexCache {
    dir: PathBuf,
}

impl IndexCache {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, StorageError> {
        let dir = dir.as_ref().to_path_buf();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        if !dir.is_dir() {
            return Err(StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("index cache path is not a directory: {}", dir.display()),
            )));
        }
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn tip_path(&self, height: u32) -> PathBuf {
        self.dir.join(format!("height-{height:010}.json"))
    }

    pub fn load_at_height(&self, height: u32) -> Result<Option<StoredPortableIndex>, StorageError> {
        let path = self.tip_path(height);
        if !path.exists() {
            return Ok(None);
        }
        let file = File::open(path)?;
        Ok(Some(serde_json::from_reader(file)?))
    }

    pub fn load_latest(&self) -> Result<Option<StoredPortableIndex>, StorageError> {
        let mut best_height: Option<u32> = None;
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(rest) = name
                .strip_prefix("height-")
                .and_then(|s| s.strip_suffix(".json"))
            else {
                continue;
            };
            if rest.len() != 10 || !rest.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(height) = rest.parse::<u32>() else {
                continue;
            };
            if best_height.is_none_or(|h| height > h) {
                best_height = Some(height);
            }
        }
        match best_height {
            Some(height) => self.load_at_height(height),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch() -> PersistedBatch {
        PersistedBatch {
            batch_sequence: 3,
            opened_tip: [1; 32],
            closed_tip: [2; 32],
            leaves: vec![[3; 32], [4; 32]],
            tree_size: 2,
            root: [5; 32],
            state: StoredState::Persisted,
            plant: None,
            inclusion: None,
            skip_reason: None,
            opened_tip_height: None,
            broadcast_height: None,
            next_block_min_sat_vb: None,
            fee_sat: None,
            vsize: None,
            leaf_post_heights: Vec::new(),
        }
    }

    #[test]
    fn create_is_non_truncating_and_update_is_atomic() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileStore::open(temp.path()).unwrap();
        let mut record = batch();
        store.persist_new(&record).unwrap();
        assert!(matches!(
            store.persist_new(&record),
            Err(StorageError::BatchExists(3))
        ));

        record.state = StoredState::Skipped;
        record.skip_reason = Some("not-mined".to_owned());
        store.update(&record).unwrap();
        assert_eq!(store.load_batches().unwrap(), vec![record]);
        assert!(std::fs::read_dir(temp.path().join("batches"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")));
    }

    #[test]
    fn event_log_appends_without_rewriting_batches() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileStore::open(temp.path()).unwrap();
        let fields = serde_json::json!({
            "batch_sequence": 9,
            "included_at_b2": false,
            "skip_reason": "fee",
        });
        store.append_event("b2_skipped", fields.clone()).unwrap();
        store.append_event("b2_skipped", fields).unwrap();
        let text = std::fs::read_to_string(temp.path().join("events.jsonl")).unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["event"], "b2_skipped");
        assert_eq!(first["batch_sequence"], 9);
        assert!(first["timestamp_unix_ms"].as_u64().is_some());
    }

    #[test]
    fn index_tip_round_trip_and_replace() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileStore::open(temp.path()).unwrap();
        let tip = StoredPortableIndex {
            version: 2,
            network: "synthetic".to_owned(),
            header: StoredIndexHeader {
                hash: "aa".repeat(32),
                height: 42,
                time: None,
                timestamp_unix_ms: None,
            },
            plant: None,
            plants: Vec::new(),
        };
        store.persist_index_tip(&tip).unwrap();
        assert_eq!(store.load_index_tip(42).unwrap().as_ref(), Some(&tip));
        assert!(store.load_index_tip(41).unwrap().is_none());
        let mut replaced = tip.clone();
        replaced.plant = Some(StoredIndexPlant {
            root: "bb".repeat(32),
            tree_size: 1,
            txid: "cc".repeat(32),
            vout: 0,
            block_hash: "dd".repeat(32),
            block_height: 42,
        });
        store.persist_index_tip(&replaced).unwrap();
        assert_eq!(store.load_index_tip(42).unwrap().as_ref(), Some(&replaced));
    }

    #[test]
    fn backfill_stamps_matching_observations_and_leaves_the_rest() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileStore::open(temp.path()).unwrap();
        let hash_a = "aa".repeat(32);
        let hash_b = "bb".repeat(32);
        let bare = |height: u32, hash: &str| StoredPortableIndex {
            version: 2,
            network: "synthetic".to_owned(),
            header: StoredIndexHeader {
                hash: hash.to_owned(),
                height,
                time: None,
                timestamp_unix_ms: None,
            },
            plant: None,
            plants: Vec::new(),
        };
        store.persist_index_tip(&bare(10, &hash_a)).unwrap();
        store.persist_index_tip(&bare(11, &hash_b)).unwrap();
        let mut stamped = bare(12, &hash_a);
        stamped.header.timestamp_unix_ms = Some(1_700_000_000_000);
        store.persist_index_tip(&stamped).unwrap();

        // Two observations of height 10: the later line for that hash wins.
        // Height 11's event names a different hash, so the file stays bare.
        let events = format!(
            "{{\"event\":\"selected_tip_observed\",\"height\":10,\"hash\":\"{hash_a}\",\"timestamp_unix_ms\":1700000001000}}\n\
             {{\"event\":\"index_tip_retained\",\"height\":10,\"timestamp_unix_ms\":1}}\n\
             {{\"event\":\"selected_tip_observed\",\"height\":10,\"hash\":\"{hash_a}\",\"timestamp_unix_ms\":1700000002000}}\n\
             {{\"event\":\"selected_tip_observed\",\"height\":11,\"hash\":\"{hash_other}\",\"timestamp_unix_ms\":1700000003000}}\n\
             not-json\n",
            hash_other = "cc".repeat(32),
        );
        std::fs::write(temp.path().join("events.jsonl"), events).unwrap();

        let report = store.backfill_index_observation_times().unwrap();
        assert_eq!(report.observed_events, 3);
        assert_eq!(report.distinct_tips, 2);
        assert_eq!(report.rewritten, 1);
        assert_eq!(report.already_stamped, 1);
        assert_eq!(report.unmatched, 1);

        let height_10 = store.load_index_tip(10).unwrap().unwrap();
        assert_eq!(height_10.header.timestamp_unix_ms, Some(1_700_000_002_000));
        assert!(height_10.header.time.is_none());
        assert!(store
            .load_index_tip(11)
            .unwrap()
            .unwrap()
            .header
            .timestamp_unix_ms
            .is_none());
        assert_eq!(
            store
                .load_index_tip(12)
                .unwrap()
                .unwrap()
                .header
                .timestamp_unix_ms,
            Some(1_700_000_000_000)
        );

        let again = store.backfill_index_observation_times().unwrap();
        assert_eq!(again.rewritten, 0);
        assert_eq!(again.already_stamped, 2);
        assert_eq!(again.unmatched, 1);
    }
}
