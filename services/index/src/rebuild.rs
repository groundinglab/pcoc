//! Rebuild `height-XXXXXXXXXX.json` from public chain data.
//!
//! Each header's previous-hash must be the previous header, and the header
//! hash must recompute from the 80-byte header. The last `reorg_depth` blocks
//! (default 6) are fetched again. A file whose hash changed is rewritten.
//! `tree_size` and `header.timestamp_unix_ms` are off-chain and stay absent.

use crate::chain::{
    header_hash_display, previous_hash_display, BlockHeader, ChainError, ChainSource,
};
use crate::document::{IndexDocument, IndexHeader, IndexPlant};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct RebuildOptions {
    pub from_height: u32,
    pub to_height: u32,
    pub out_dir: PathBuf,
    /// How many of the highest heights to fetch a second time.
    pub reorg_depth: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RebuildReport {
    pub written: u32,
    pub unchanged: u32,
    pub rewritten: u32,
    pub from_height: u32,
    pub to_height: u32,
}

#[derive(Debug)]
pub enum RebuildError {
    Chain(ChainError),
    Io(std::io::Error),
    Json(serde_json::Error),
    Linkage(String),
    Range(String),
}

impl std::fmt::Display for RebuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chain(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::Linkage(message) => write!(f, "{message}"),
            Self::Range(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for RebuildError {}

impl From<ChainError> for RebuildError {
    fn from(error: ChainError) -> Self {
        Self::Chain(error)
    }
}

impl From<std::io::Error> for RebuildError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for RebuildError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Walk `from_height ..= to_height` and write one file per height.
pub fn rebuild(
    source: &dyn ChainSource,
    options: &RebuildOptions,
) -> Result<RebuildReport, RebuildError> {
    if options.to_height < options.from_height {
        return Err(RebuildError::Range(format!(
            "to-height {} is below from-height {}",
            options.to_height, options.from_height
        )));
    }
    fs::create_dir_all(&options.out_dir)?;
    let mut previous: Option<BlockHeader> = None;
    let mut written = 0u32;
    let mut unchanged = 0u32;
    for height in options.from_height..=options.to_height {
        let header = source.header_at_height(height)?;
        check_header(&header, previous.as_ref())?;
        let plants = plants_for(&header, source)?;
        let document = document_for(&header, plants);
        match store_document(&options.out_dir, &document)? {
            StoreOutcome::Written => written = written.saturating_add(1),
            StoreOutcome::Unchanged => unchanged = unchanged.saturating_add(1),
        }
        previous = Some(header);
    }

    let mut rewritten = 0u32;
    let depth = options
        .reorg_depth
        .min(options.to_height - options.from_height + 1);
    let reorg_from = options.to_height - depth + 1;
    let mut anchor: Option<BlockHeader> = None;
    if reorg_from > options.from_height {
        let below = source.header_at_height(reorg_from - 1)?;
        check_header_hash(&below)?;
        anchor = Some(below);
    }
    for height in reorg_from..=options.to_height {
        let header = source.header_at_height(height)?;
        check_header(&header, anchor.as_ref())?;
        let plants = plants_for(&header, source)?;
        let document = document_for(&header, plants);
        if file_changed(&options.out_dir, &document)? {
            store_document(&options.out_dir, &document)?;
            rewritten = rewritten.saturating_add(1);
        }
        anchor = Some(header);
    }

    Ok(RebuildReport {
        written,
        unchanged,
        rewritten,
        from_height: options.from_height,
        to_height: options.to_height,
    })
}

fn plants_for(
    header: &BlockHeader,
    source: &dyn ChainSource,
) -> Result<Vec<IndexPlant>, RebuildError> {
    let found = source.plants_in_block(header)?;
    Ok(found
        .iter()
        .map(|plant| plant.to_index(&header.hash, header.height))
        .collect())
}

fn document_for(header: &BlockHeader, plants: Vec<IndexPlant>) -> IndexDocument {
    // Latest in chain order. Bari's single `plant` is the highest serving
    // batch, which a chain scan cannot recover. One plant is unambiguous.
    let plant = plants.last().cloned();
    IndexDocument {
        version: 2,
        network: "main".to_owned(),
        header: IndexHeader {
            hash: header.hash.clone(),
            height: header.height,
            time: Some(header.time),
            timestamp_unix_ms: None,
        },
        plant,
        plants,
    }
}

/// Previous-hash links to the header already accepted, and the hash recomputes.
pub fn check_header(
    header: &BlockHeader,
    previous: Option<&BlockHeader>,
) -> Result<(), RebuildError> {
    check_header_hash(header)?;
    let Some(previous) = previous else {
        return Ok(());
    };
    if header.height != previous.height + 1 {
        return Err(RebuildError::Linkage(format!(
            "height {} does not follow {}",
            header.height, previous.height
        )));
    }
    match header.previous_hash.as_deref() {
        Some(prev) if prev.eq_ignore_ascii_case(&previous.hash) => Ok(()),
        Some(prev) => Err(RebuildError::Linkage(format!(
            "height {} previous hash {prev} does not link to {}",
            header.height, previous.hash
        ))),
        None => Err(RebuildError::Linkage(format!(
            "height {} has no previous hash and does not follow {}",
            header.height, previous.hash
        ))),
    }
}

pub fn check_header_hash(header: &BlockHeader) -> Result<(), RebuildError> {
    let recomputed = header_hash_display(&header.raw);
    if !recomputed.eq_ignore_ascii_case(&header.hash) {
        return Err(RebuildError::Linkage(format!(
            "height {} header hash {} does not recompute (got {recomputed})",
            header.height, header.hash
        )));
    }
    let prev_in_header = previous_hash_display(&header.raw);
    if let Some(claimed) = header.previous_hash.as_deref() {
        if !claimed.eq_ignore_ascii_case(&prev_in_header) {
            return Err(RebuildError::Linkage(format!(
                "height {} previous hash {claimed} is not the previous hash in the header bytes ({prev_in_header})",
                header.height
            )));
        }
    }
    let n_time = crate::chain::header_time(&header.raw);
    if n_time != header.time {
        return Err(RebuildError::Linkage(format!(
            "height {} time {} is not header nTime {n_time}",
            header.height, header.time
        )));
    }
    Ok(())
}

enum StoreOutcome {
    Written,
    Unchanged,
}

fn store_document(dir: &Path, document: &IndexDocument) -> Result<StoreOutcome, RebuildError> {
    let destination = height_path(dir, document.header.height);
    let bytes = pretty_bytes(document)?;
    if destination.is_file() {
        let existing = fs::read(&destination)?;
        if existing == bytes {
            return Ok(StoreOutcome::Unchanged);
        }
    }
    write_atomic(&destination, &bytes)?;
    Ok(StoreOutcome::Written)
}

fn file_changed(dir: &Path, document: &IndexDocument) -> Result<bool, RebuildError> {
    let destination = height_path(dir, document.header.height);
    if !destination.is_file() {
        return Ok(true);
    }
    let existing = fs::read(&destination)?;
    Ok(existing != pretty_bytes(document)?)
}

fn height_path(dir: &Path, height: u32) -> PathBuf {
    dir.join(format!("height-{height:010}.json"))
}

/// Pretty JSON plus a trailing newline. Same writer Bari uses for the cache file.
fn pretty_bytes(document: &IndexDocument) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(document)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<(), RebuildError> {
    let dir = destination.parent().unwrap_or_else(|| Path::new("."));
    let nonce = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = dir.join(format!(
        ".{}.{}.{}.tmp",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("height"),
        std::process::id(),
        nonce
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, destination)?;
        File::open(dir)?.sync_all()?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{ChainError, ChainSource, FoundPlant};
    use sha2::{Digest, Sha256};
    use std::sync::Mutex;

    fn raw_header(prev_display: &str, n_time: u32, marker: u8) -> [u8; 80] {
        let mut raw = [0u8; 80];
        raw[0] = marker;
        let prev = decode_display(prev_display);
        raw[4..36].copy_from_slice(&prev);
        raw[68..72].copy_from_slice(&n_time.to_le_bytes());
        raw
    }

    fn decode_display(display: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        let bytes = hex_bytes(display);
        for i in 0..32 {
            out[i] = bytes[31 - i];
        }
        out
    }

    fn hex_bytes(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    fn display_hash(raw: &[u8; 80]) -> String {
        let once = Sha256::digest(raw);
        let twice = Sha256::digest(once);
        let mut shown = [0u8; 32];
        for (i, byte) in twice.iter().enumerate() {
            shown[31 - i] = *byte;
        }
        shown.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    struct HeaderFixture {
        headers: Vec<BlockHeader>,
        /// On the second read of these heights, serve `reorg` instead.
        reorg_at: Vec<u32>,
        reorg: Vec<BlockHeader>,
        hits: Mutex<Vec<u32>>,
        /// Plants returned for every block. The fixture does not vary them by height.
        plants: Vec<FoundPlant>,
    }

    impl HeaderFixture {
        fn chain(count: u32) -> Self {
            let mut headers = Vec::new();
            let mut prev = "00".repeat(32);
            for height in 1..=count {
                let raw = raw_header(&prev, 1_700_000_000 + height, height as u8);
                let hash = display_hash(&raw);
                headers.push(BlockHeader {
                    hash: hash.clone(),
                    height,
                    previous_hash: Some(prev),
                    time: u64::from(1_700_000_000 + height),
                    raw,
                });
                prev = hash;
            }
            Self {
                headers,
                reorg_at: Vec::new(),
                reorg: Vec::new(),
                hits: Mutex::new(Vec::new()),
                plants: Vec::new(),
            }
        }
    }

    impl ChainSource for HeaderFixture {
        fn tip_height(&self) -> Result<u32, ChainError> {
            Ok(self.headers.last().map(|header| header.height).unwrap_or(0))
        }

        fn header_at_height(&self, height: u32) -> Result<BlockHeader, ChainError> {
            let mut hits = self.hits.lock().unwrap();
            let seen = hits.iter().filter(|hit| **hit == height).count();
            hits.push(height);
            if seen >= 1 && self.reorg_at.contains(&height) {
                return self
                    .reorg
                    .iter()
                    .find(|header| header.height == height)
                    .cloned()
                    .ok_or_else(|| ChainError::Message(format!("no reorg header {height}")));
            }
            self.headers
                .iter()
                .find(|header| header.height == height)
                .cloned()
                .ok_or_else(|| ChainError::Message(format!("no header {height}")))
        }

        fn plants_in_block(&self, _header: &BlockHeader) -> Result<Vec<FoundPlant>, ChainError> {
            Ok(self.plants.clone())
        }
    }

    #[test]
    fn linkage_failure_refuses_the_range() {
        let mut source = HeaderFixture::chain(3);
        // Break the link at height 3. The header bytes must agree with the
        // claimed previous hash, or the byte check fails first.
        let claimed = source.headers[0].hash.clone();
        let raw = raw_header(&claimed, 1_700_000_003, 3);
        source.headers[2].raw = raw;
        source.headers[2].hash = display_hash(&raw);
        source.headers[2].previous_hash = Some(claimed);
        source.headers[2].time = 1_700_000_003;
        let dir = tempfile::tempdir().unwrap();
        let error = rebuild(
            &source,
            &RebuildOptions {
                from_height: 1,
                to_height: 3,
                out_dir: dir.path().to_path_buf(),
                reorg_depth: 1,
            },
        )
        .unwrap_err();
        let RebuildError::Linkage(message) = error else {
            panic!("expected linkage, got {error}");
        };
        assert!(message.contains("does not link"), "{message}");
        assert!(!dir.path().join("height-0000000003.json").exists());
    }

    #[test]
    fn hash_mismatch_is_a_linkage_failure() {
        let mut source = HeaderFixture::chain(2);
        source.headers[1].hash = "ff".repeat(32);
        let dir = tempfile::tempdir().unwrap();
        let error = rebuild(
            &source,
            &RebuildOptions {
                from_height: 1,
                to_height: 2,
                out_dir: dir.path().to_path_buf(),
                reorg_depth: 0,
            },
        )
        .unwrap_err();
        assert!(matches!(error, RebuildError::Linkage(_)), "{error}");
    }

    #[test]
    fn reorg_rewrites_the_changed_file() {
        let mut source = HeaderFixture::chain(4);
        // A reorg replaces height 4 and keeps it linked to height 3.
        let parent = source.headers[2].hash.clone();
        let raw = raw_header(&parent, 1_800_000_004, 0x7f);
        let hash = display_hash(&raw);
        source.reorg_at = vec![4];
        source.reorg = vec![BlockHeader {
            hash: hash.clone(),
            height: 4,
            previous_hash: Some(parent),
            time: 1_800_000_004,
            raw,
        }];
        let dir = tempfile::tempdir().unwrap();
        let report = rebuild(
            &source,
            &RebuildOptions {
                from_height: 1,
                to_height: 4,
                out_dir: dir.path().to_path_buf(),
                reorg_depth: 2,
            },
        )
        .unwrap();
        assert_eq!(report.written, 4);
        assert_eq!(report.rewritten, 1, "{report:?}");
        let text = std::fs::read_to_string(dir.path().join("height-0000000004.json")).unwrap();
        let doc: IndexDocument = serde_json::from_str(&text).unwrap();
        assert_eq!(doc.header.hash, hash);
        assert_eq!(doc.header.time, Some(1_800_000_004));
        assert!(doc.header.timestamp_unix_ms.is_none());
        assert!(doc.plant.is_none());
        // Height 3 was inside the reorg window and did not change.
        let height_3 = std::fs::read_to_string(dir.path().join("height-0000000003.json")).unwrap();
        assert!(height_3.contains(&source.headers[2].hash));
    }

    #[test]
    fn a_plant_is_written_without_tree_size() {
        let mut source = HeaderFixture::chain(1);
        source.plants = vec![FoundPlant {
            root_hex: "ab".repeat(32),
            txid: "cd".repeat(32),
            vout: 1,
        }];
        let dir = tempfile::tempdir().unwrap();
        rebuild(
            &source,
            &RebuildOptions {
                from_height: 1,
                to_height: 1,
                out_dir: dir.path().to_path_buf(),
                reorg_depth: 0,
            },
        )
        .unwrap();
        let text = std::fs::read_to_string(dir.path().join("height-0000000001.json")).unwrap();
        assert!(!text.contains("tree_size"));
        assert!(!text.contains("timestamp_unix_ms"));
        let doc: IndexDocument = serde_json::from_str(&text).unwrap();
        assert_eq!(doc.plant.as_ref().unwrap().vout, 1);
        assert_eq!(doc.plants.len(), 1);
        assert_eq!(doc.network, "main");
        assert_eq!(doc.version, 2);
    }
}
