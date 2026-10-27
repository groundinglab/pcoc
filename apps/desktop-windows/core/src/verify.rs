//! Measurement after a saved PCOC. Admission is a different path.
//!
//! `measure_saved` calls `pcoc::verify` and returns the section 14 record.
//! It does not say the gate admitted or refused. [`admission`] is a separate
//! module path with no measurement type.

use crate::endpoints::INDEX;
use pcoc::profile::Profile;
use pcoc::verify::{EmptyIndex, Verified};
use std::path::Path;

/// A measurement. The JSON is the section 14 record. No gate word is attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measurement {
    /// Section 14 JSON, no trailing newline, as `pcoc::verify` wrote it.
    pub record_json: String,
    pub bytes: u64,
}

/// Why measurement did not return a record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeasureError {
    CouldNotOpen(String),
    /// The bytes did not decode into a record.
    Unreadable,
}

/// Measure the bytes at `path` with an empty index. This does not dial the
/// portable index and does not ask the admission gate.
pub fn measure_saved(path: &Path, profile: &Profile) -> Result<Measurement, MeasureError> {
    let bytes = std::fs::read(path).map_err(|err| MeasureError::CouldNotOpen(err.to_string()))?;
    measure_bytes(&bytes, profile)
}

pub fn measure_bytes(bytes: &[u8], profile: &Profile) -> Result<Measurement, MeasureError> {
    let mut index = EmptyIndex;
    match pcoc::verify(bytes, profile, &mut index) {
        Verified::Record(json) | Verified::UnknownKind(json) => Ok(Measurement {
            record_json: json,
            bytes: bytes.len() as u64,
        }),
        Verified::DecodeFailure => Err(MeasureError::Unreadable),
    }
}

/// The index host a later read may use. Measurement with [`EmptyIndex`]
/// does not call it. Named so the default stays the Linux one.
pub fn index_host() -> &'static str {
    INDEX
}

/// Admission is not measurement. This module is the separate path.
/// W1 does not submit an object to the gate from verify.
pub mod admission {
    /// A gate opinion. Not a measurement record.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum GateOpinion {
        NotAsked,
    }

    /// Verify does not call this. A harness asserts the two types differ.
    pub fn opinion() -> GateOpinion {
        GateOpinion::NotAsked
    }
}

/// Words a measurement record must not use for a gate decision.
pub fn measurement_claims_gate(record_json: &str) -> bool {
    let lower = record_json.to_ascii_lowercase();
    // A measurement may mention heights and paths. It must not state a gate
    // admit or refuse as if that were the measurement.
    lower.contains("\"admitted\"")
        || lower.contains("\"refused\"")
        || lower.contains("gate admit")
        || lower.contains("gate refuse")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::produce::{produce_and_save, AcceptingWaypointPost};
    use pcoc::profile::Profile;
    use pcoc_produce::{cases_rig, Case};

    fn tiny() -> Profile {
        Profile::conformance_small(32, 64, 65_536, 1).expect("profile")
    }

    #[test]
    fn saved_fixture_returns_a_measurement_and_not_a_gate_word() {
        let dir = std::env::temp_dir().join(format!(
            "pcoc-windows-measure-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let key = [0x33u8; 32];
        let saved = produce_and_save(&key, &cases_rig(Case::S1), &tiny(), &AcceptingWaypointPost, &dir)
            .unwrap()
            .saved;
        let measured = measure_saved(&saved, &tiny()).expect("measurement");
        assert!(measured.bytes > 0);
        let record: serde_json::Value =
            serde_json::from_str(&measured.record_json).expect("section 14 json");
        assert!(record.get("shape").is_some(), "{}", measured.record_json);
        assert!(record.get("admitted").is_none());
        assert!(record.get("refused").is_none());
        assert!(!measurement_claims_gate(&measured.record_json));
        assert_eq!(admission::opinion(), admission::GateOpinion::NotAsked);
        // The six-leg draw reads the OS CSPRNG, so a second measure is a new
        // record. Both are measurements. Neither is a gate opinion.
        let again = measure_bytes(&std::fs::read(&saved).unwrap(), &tiny()).unwrap();
        let again_record: serde_json::Value = serde_json::from_str(&again.record_json).unwrap();
        assert!(again_record.get("shape").is_some());
        assert!(!measurement_claims_gate(&again.record_json));
        assert_eq!(index_host(), "https://index.groundinglab.io");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
