//! Verify, in-process (DESIGN-v2 sections 7.5 and 10). The pinned crate
//! measures the object and returns the §14 record; the app shows that
//! record and saves its bytes unchanged. The three old exit codes are the
//! three outcomes here: a record, an unreadable object, and a file that
//! could not be opened. None of them is a verdict.

use crate::format::now_unix_s;
use crate::record::{present, Presented};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Debug)]
pub enum Outcome {
    Record {
        /// The §14 JSON exactly as the crate wrote it, trailing newline included.
        json: String,
        presented: Presented,
        bytes: u64,
        sha256: [u8; 32],
    },
    /// The bytes are not a value this verifier decodes.
    Unreadable,
    CouldNotOpen(String),
}

#[derive(Clone, Debug, Default)]
pub struct IndexReads {
    pub issued: u32,
    pub answered: u32,
    pub total_ms: u64,
}

impl IndexReads {
    pub fn mean_ms(&self) -> Option<u64> {
        (self.answered > 0).then(|| self.total_ms / u64::from(self.answered))
    }

    /// `Portable index reachable · index.groundinglab.io · 88 ms · 33 reads`.
    pub fn status(&self, index_url: &str) -> String {
        let host = index_url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let reads = if self.issued == 1 {
            "1 read".to_owned()
        } else {
            format!("{} reads", self.issued)
        };
        match (self.issued, self.mean_ms()) {
            (0, _) => format!("Portable index not read · {host}"),
            (_, Some(ms)) => format!("Portable index reachable · {host} · {ms} ms · {reads}"),
            (_, None) => format!("Portable index did not answer · {host} · {reads}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VerifyRun {
    pub path: PathBuf,
    pub outcome: Outcome,
    pub reads: IndexReads,
    pub elapsed_s: u64,
    pub threads: usize,
    pub finished_unix_s: u64,
}

pub struct VerifyConfig<'a> {
    pub index_url: &'a str,
    pub threads: usize,
    pub sizes: hike::Sizes,
    pub profile: &'a str,
}

impl VerifyConfig<'_> {
    pub fn production(threads: usize) -> VerifyConfig<'static> {
        VerifyConfig {
            index_url: crate::endpoints::INDEX,
            threads,
            sizes: hike::Sizes::production(),
            profile: "production",
        }
    }
}

pub fn run(path: &Path, config: &VerifyConfig<'_>) -> VerifyRun {
    let started = Instant::now();
    let reads = Cell::new(IndexReads {
        issued: 0,
        answered: 0,
        total_ms: 0,
    });
    let note = |ok: bool, since: Instant| {
        let mut r = reads.take();
        r.issued += 1;
        if ok {
            r.answered += 1;
            r.total_ms += since.elapsed().as_millis() as u64;
        }
        reads.set(r);
    };
    let outcome = match std::fs::read(path) {
        Err(err) => Outcome::CouldNotOpen(err.to_string()),
        Ok(bytes) if !object_bytes_are_measurable(&bytes) => Outcome::Unreadable,
        Ok(bytes) => {
            let agent = hike::http_agent();
            let measurements = hike::verify_with_bind_threads_sized(
                &bytes,
                hike::AuditDraw::OsCsprng,
                |height| {
                    let t = Instant::now();
                    let read =
                        hike::read_index_beacon(&agent, config.index_url, height).map_err(|_| ());
                    note(read.is_ok(), t);
                    read
                },
                |loc| {
                    let t = Instant::now();
                    match hike::index_document(
                        &agent,
                        config.index_url,
                        u64::from(loc.block_height),
                    ) {
                        Ok(document) => {
                            note(true, t);
                            match hike::proxy_matches(&document, loc) {
                                Ok(()) => hike::ChainRead::Match,
                                Err(()) => hike::ChainRead::Mismatch,
                            }
                        }
                        Err(_) => {
                            note(false, t);
                            hike::ChainRead::NotRead
                        }
                    }
                },
                config.threads.max(1),
                config.sizes,
            );
            if matches!(
                hike::yielded_public_key(&bytes),
                hike::YieldedKey::Undecodable
            ) {
                Outcome::Unreadable
            } else {
                let json = hike::verify_record_json(config.profile, &measurements);
                match present(&json) {
                    Ok(presented) => Outcome::Record {
                        presented,
                        bytes: bytes.len() as u64,
                        sha256: Sha256::digest(&bytes).into(),
                        json,
                    },
                    Err(_) => Outcome::Unreadable,
                }
            }
        }
    };
    VerifyRun {
        path: path.to_owned(),
        outcome,
        reads: reads.take(),
        elapsed_s: started.elapsed().as_secs(),
        threads: config.threads.max(1),
        finished_unix_s: now_unix_s(),
    }
}

/// Save record writes the §14 JSON bytes unchanged.
pub fn save_record(path: &Path, json: &str) -> std::io::Result<()> {
    std::fs::write(path, json.as_bytes())
}

/// A file Verify can measure: a current hike object, or an earlier kind.
///
/// A non-CBOR file, a truncated item, or an old map the decoder rejects is
/// `None`. That case is an error line. It does not start an index read.
pub fn object_bytes_are_measurable(bytes: &[u8]) -> bool {
    if hike::earlier_object_kind(bytes).is_some() {
        return true;
    }
    matches!(
        hike::yielded_public_key(bytes),
        hike::YieldedKey::PublicKey(_)
    )
}

/// One plain line for a file that is not measurable. The index is not named,
/// because this path never asks it.
pub fn unreadable_object_line(path: &Path) -> String {
    format!("Fail. {} is not a PCOC this app can read.", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A closed local port: every index read fails at once and is `not read`.
    const NO_INDEX: &str = "http://127.0.0.1:9";

    fn small() -> VerifyConfig<'static> {
        VerifyConfig {
            index_url: NO_INDEX,
            threads: 2,
            sizes: hike::Sizes::conformance_small(),
            profile: "conformance-small",
        }
    }

    #[test]
    fn a_record_with_no_index_is_measured_not_failed() {
        let dir = std::env::temp_dir().join(format!("pcoc-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/v2.0/kat-small.json"))
                .unwrap();
        let bytes = crate::hex::decode(v["object_hex"].as_str().unwrap()).unwrap();
        let path = dir.join("kat.cbor");
        std::fs::write(&path, &bytes).unwrap();

        let run = run(&path, &small());
        let Outcome::Record {
            json,
            presented: Presented::Record(view),
            bytes: n,
            ..
        } = run.outcome
        else {
            panic!("expected a record");
        };
        assert_eq!(n, bytes.len() as u64);
        assert!(json.ends_with('\n'));
        assert_eq!(view.profile, "conformance-small");
        assert_eq!(view.shape, "match");
        assert!(view
            .header_comparisons()
            .0
            .iter()
            .any(|(t, _)| t == "not read"));
        assert!(run.reads.issued > 0);
        assert_eq!(run.reads.answered, 0);
        assert!(run
            .reads
            .status(NO_INDEX)
            .starts_with("Portable index did not answer"));

        let saved = dir.join("record.json");
        save_record(&saved, &json).unwrap();
        assert_eq!(std::fs::read_to_string(&saved).unwrap(), json);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unreadable_and_missing_files() {
        let dir = std::env::temp_dir().join(format!("pcoc-verify-junk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("junk.cbor");
        std::fs::write(&path, [0xffu8, 0x00, 0x01]).unwrap();
        assert!(matches!(run(&path, &small()).outcome, Outcome::Unreadable));
        assert!(matches!(
            run(&dir.join("absent.cbor"), &small()).outcome,
            Outcome::CouldNotOpen(_)
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// AC4. A non-CBOR file and a truncated .cbor are unreadable, issue no
    /// index read, and leave the next good object free to measure.
    #[test]
    fn bad_and_truncated_files_reset_without_an_index_read() {
        let dir = std::env::temp_dir().join(format!("pcoc-verify-reset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let text = dir.join("notes.txt");
        std::fs::write(&text, b"this is not cbor\n").unwrap();
        let bad = run(&text, &small());
        assert!(matches!(bad.outcome, Outcome::Unreadable));
        assert_eq!(bad.reads.issued, 0);
        assert_eq!(bad.reads.answered, 0);
        let line = unreadable_object_line(&text);
        assert_eq!(line.matches('\n').count(), 0);
        assert!(!line.to_ascii_lowercase().contains("not ready"));
        assert!(!line.to_ascii_lowercase().contains("portable index"));

        let truncated = dir.join("old.cbor");
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/v2.0/kat-small.json"))
                .unwrap();
        let bytes = crate::hex::decode(v["object_hex"].as_str().unwrap()).unwrap();
        std::fs::write(&truncated, &bytes[..bytes.len() / 3]).unwrap();
        let old = run(&truncated, &small());
        assert!(matches!(old.outcome, Outcome::Unreadable));
        assert_eq!(old.reads.issued, 0);

        let good = dir.join("kat.cbor");
        std::fs::write(&good, &bytes).unwrap();
        let run = run(&good, &small());
        assert!(matches!(run.outcome, Outcome::Record { .. }));
        assert!(run.reads.issued > 0);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn index_status_line() {
        let r = IndexReads {
            issued: 33,
            answered: 33,
            total_ms: 33 * 88,
        };
        assert_eq!(
            r.status("https://index.groundinglab.io"),
            "Portable index reachable · index.groundinglab.io · 88 ms · 33 reads"
        );
    }
}
