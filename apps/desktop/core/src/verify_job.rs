//! Verify, in-process (DESIGN-v2 sections 7.5 and 10).
//!
//! The measurement is `pcoc::verify`, the same function the `pcoc verify`
//! CLI calls. The app shows that record and saves its bytes unchanged.
//! A record, an unreadable object, and a file that could not be opened are
//! the three outcomes. None of them is a verdict.

use crate::format::now_unix_s;
use crate::object_key;
use crate::record::{present, Presented};
use pcoc::profile::Profile;
use pcoc::verify::{EmptyIndex, HeightIndex, Precheck, Verified};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub enum Outcome {
    Record {
        /// The §14 JSON exactly as `pcoc::verify` wrote it. No trailing newline.
        json: String,
        presented: Presented,
        bytes: u64,
        sha256: [u8; 32],
    },
    /// The bytes are not a value this verifier decodes into a record.
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
    pub(crate) profile: Profile,
}

impl VerifyConfig<'_> {
    pub fn production(threads: usize) -> VerifyConfig<'static> {
        VerifyConfig {
            index_url: crate::endpoints::INDEX,
            threads,
            profile: Profile::production(),
        }
    }
}

struct HttpIndex<'a> {
    base: String,
    agent: ureq::Agent,
    reads: &'a mut IndexReads,
}

impl HeightIndex for HttpIndex<'_> {
    fn read_height(&mut self, height: u64) -> Option<Vec<u8>> {
        self.reads.issued += 1;
        let url = format!(
            "{}/index/{height}",
            self.base.trim_end_matches('/')
        );
        let started = Instant::now();
        let response = self.agent.get(&url).call().ok()?;
        if response.status() != 200 {
            return None;
        }
        let mut body = Vec::new();
        if response.into_reader().take(1 << 20).read_to_end(&mut body).is_err() {
            return None;
        }
        self.reads.answered += 1;
        self.reads.total_ms += started.elapsed().as_millis() as u64;
        Some(body)
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(10))
        .user_agent("pcoc-hike-desktop")
        .build()
}

pub fn run(path: &Path, config: &VerifyConfig<'_>) -> VerifyRun {
    let started = Instant::now();
    let outcome = match std::fs::read(path) {
        Err(err) => (Outcome::CouldNotOpen(err.to_string()), IndexReads::default()),
        Ok(bytes) => measure(&bytes, config),
    };
    VerifyRun {
        path: path.to_owned(),
        outcome: outcome.0,
        reads: outcome.1,
        elapsed_s: started.elapsed().as_secs(),
        threads: config.threads.max(1),
        finished_unix_s: now_unix_s(),
    }
}

fn measure(bytes: &[u8], config: &VerifyConfig<'_>) -> (Outcome, IndexReads) {
    if !object_bytes_are_measurable(bytes) {
        return (Outcome::Unreadable, IndexReads::default());
    }
    let mut reads = IndexReads::default();
    let url = config.index_url.trim();
    let verified = if url.is_empty() {
        let mut index = EmptyIndex;
        pcoc::verify(bytes, &config.profile, &mut index)
    } else {
        let mut index = HttpIndex {
            base: url.to_owned(),
            agent: agent(),
            reads: &mut reads,
        };
        pcoc::verify(bytes, &config.profile, &mut index)
    };
    let outcome = match verified {
        Verified::Record(json) | Verified::UnknownKind(json) => match present(&json) {
            Ok(presented) => Outcome::Record {
                presented,
                bytes: bytes.len() as u64,
                sha256: Sha256::digest(bytes).into(),
                json,
            },
            Err(_) => Outcome::Unreadable,
        },
        Verified::DecodeFailure => Outcome::Unreadable,
    };
    (outcome, reads)
}

/// Save record writes the §14 JSON bytes unchanged.
pub fn save_record(path: &Path, json: &str) -> std::io::Result<()> {
    std::fs::write(path, json.as_bytes())
}

/// A file Verify can measure: kind `v2.2-pcoc-object`, an unknown kind
/// (Rule A5), or a malformed item that still yields a §14 record.
///
/// Nesting deeper than 63 is rejected before the decoder runs. A well-formed
/// item that is not an object map is not measurable. That case is an error
/// line. It does not start an index read.
pub fn object_bytes_are_measurable(bytes: &[u8]) -> bool {
    if object_key::nesting_too_deep(bytes) {
        return false;
    }
    !matches!(
        pcoc::precheck(bytes, &Profile::production()),
        Precheck::DecodeFailure
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
    use pcoc_produce::Case;

    fn object_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../../sdk/rust/objects/{name}"))
    }

    fn config_for(profile: Profile, index_url: &'static str) -> VerifyConfig<'static> {
        VerifyConfig {
            index_url,
            threads: 2,
            profile,
        }
    }

    /// `six_leg` is an OS draw inside the one `pcoc::verify` call. A second
    /// call draws again, so the comparison drops those two keys.
    fn stable(json: &str) -> serde_json::Value {
        let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
        let obj = value.as_object_mut().unwrap();
        obj.remove("six_leg");
        obj.remove("six_leg_runs");
        value
    }

    fn json_of(outcome: &Outcome) -> &str {
        match outcome {
            Outcome::Record { json, .. } => json,
            other => panic!("expected a record, got {other:?}"),
        }
    }

    #[test]
    fn o1_and_o2_match_pcoc_verify_on_the_same_profile() {
        let cases = [
            (
                "O1-S1.cbor",
                pcoc_produce::cases_profile(Case::S1),
                "conformance-small",
            ),
            (
                "O2-S2.cbor",
                pcoc_produce::cases_profile(Case::S2),
                "conformance-small",
            ),
        ];
        for (name, profile, profile_name) in cases {
            let path = object_path(name);
            let bytes = std::fs::read(&path).unwrap();
            let run = run(&path, &config_for(profile.clone(), ""));
            let Outcome::Record {
                presented: Presented::Record(view),
                bytes: n,
                ..
            } = &run.outcome
            else {
                panic!("{name} did not present a record");
            };
            assert_eq!(*n, bytes.len() as u64);
            assert_eq!(view.profile, profile_name);
            assert_eq!(run.reads.issued, 0);
            let mut index = EmptyIndex;
            let sdk = pcoc::verify(&bytes, &profile, &mut index);
            let Verified::Record(sdk_json) = sdk else {
                panic!("{name} sdk verify was not a record");
            };
            assert_eq!(stable(json_of(&run.outcome)), stable(&sdk_json));
        }
    }

    /// One production verify. The terrain is 256 MiB and the walk is the
    /// section 14 measurement. Not timed.
    #[test]
    fn o3_verify_is_one_production_record() {
        let path = object_path("O3-PROD.cbor");
        let bytes = std::fs::read(&path).unwrap();
        let run = run(&path, &config_for(Profile::production(), ""));
        let Outcome::Record {
            json,
            presented: Presented::Record(view),
            ..
        } = &run.outcome
        else {
            panic!("O3 did not present a record");
        };
        assert_eq!(view.profile, "production");
        assert_eq!(view.headers.len(), 19);
        assert!(json.starts_with('{'));
        assert!(!json.contains("satoshi-hike"));
        assert_eq!(run.reads.issued, 0);
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(value["profile"], "production");
        assert!(value.get("six_leg").is_some());
        let _ = bytes;
    }

    #[test]
    fn a_refused_index_read_matches_an_empty_index() {
        let path = object_path("O1-S1.cbor");
        let bytes = std::fs::read(&path).unwrap();
        let profile = pcoc_produce::cases_profile(Case::S1);
        let run = run(&path, &config_for(profile.clone(), "http://127.0.0.1:9"));
        assert!(run.reads.issued > 0);
        assert_eq!(run.reads.answered, 0);
        assert!(run
            .reads
            .status("http://127.0.0.1:9")
            .starts_with("Portable index did not answer"));
        let mut index = EmptyIndex;
        let Verified::Record(sdk_json) = pcoc::verify(&bytes, &profile, &mut index) else {
            panic!("record");
        };
        assert_eq!(stable(json_of(&run.outcome)), stable(&sdk_json));
    }

    #[test]
    fn unreadable_and_missing_files() {
        let dir = std::env::temp_dir().join(format!("pcoc-verify-junk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty-map.cbor");
        std::fs::write(&path, [0xa0]).unwrap();
        let opened = run(&path, &config_for(Profile::production(), "http://127.0.0.1:9"));
        assert!(matches!(opened.outcome, Outcome::Unreadable));
        assert_eq!(opened.reads.issued, 0);
        assert!(matches!(
            run(
                &dir.join("absent.cbor"),
                &config_for(Profile::production(), "")
            )
            .outcome,
            Outcome::CouldNotOpen(_)
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn malformed_bytes_match_pcoc_verify_and_a_deep_nest_does_not_decode() {
        let dir = std::env::temp_dir().join(format!("pcoc-verify-reset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = dir.join("notes.txt");
        std::fs::write(&text, b"this is not cbor\n").unwrap();
        let profile = Profile::production();
        let bad = run(&text, &config_for(profile.clone(), ""));
        let Outcome::Record { json, .. } = &bad.outcome else {
            panic!("malformed input is still a record");
        };
        let mut index = EmptyIndex;
        let Verified::Record(sdk_json) = pcoc::verify(b"this is not cbor\n", &profile, &mut index)
        else {
            panic!("sdk record");
        };
        assert_eq!(json, &sdk_json);
        assert_eq!(bad.reads.issued, 0);
        let line = unreadable_object_line(&text);
        assert_eq!(line.matches('\n').count(), 0);
        assert!(!line.to_ascii_lowercase().contains("not ready"));
        assert!(!line.to_ascii_lowercase().contains("portable index"));

        let mut deep = pcoc::cbor::encode_map(&[(
            "public_key".into(),
            pcoc::cbor::encode_bytes(&[0x11; 32]),
        )]);
        for _ in 0..64 {
            deep = pcoc::cbor::encode_array(&[deep]);
        }
        let deep_path = dir.join("deep.cbor");
        std::fs::write(&deep_path, &deep).unwrap();
        let deep_run = run(&deep_path, &config_for(profile, "http://127.0.0.1:9"));
        assert!(matches!(deep_run.outcome, Outcome::Unreadable));
        assert_eq!(deep_run.reads.issued, 0);
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
