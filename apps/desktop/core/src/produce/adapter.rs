//! Offline produce on the scripted CASES rig.
//!
//! `produce_on_rig` walks `cases_rig`. The live header source is not in this
//! build, so this path does not dial Bitcoin.

use super::events::ProduceEvent;
use pcoc_produce::{
    cases_profile, cases_rig, encode_produced, path_skip_stderr, produce_on_rig, Case,
    ProduceOutcome, Progress,
};
use std::sync::atomic::AtomicBool;

pub const STOPPED: &str = "stopped by the user";

/// Run one CASES rig. `Err` is [`STOPPED`] when `cancel` is set. No object
/// bytes are returned in that case.
pub(crate) fn produce_case(
    public_key: &[u8; 32],
    case: Case,
    cancel: &AtomicBool,
    mut emit: impl FnMut(ProduceEvent),
) -> Result<Vec<u8>, String> {
    let profile = cases_profile(case);
    let rig = cases_rig(case);
    let mut legs_in_stage = 0u64;
    let outcome = produce_on_rig(public_key, &profile, &rig, cancel, |progress| {
        match progress {
            Progress::TerrainFilled => emit(ProduceEvent::TerrainFilled { at_ms: 0 }),
            Progress::Leg { stage, leg, .. } => {
                legs_in_stage = leg as u64 + 1;
                emit(ProduceEvent::LegRecorded {
                    stage,
                    j: leg as u64,
                    stored_in_stage: legs_in_stage,
                    at_ms: 0,
                });
            }
            Progress::Interrupt { stage, height } => {
                emit(ProduceEvent::StageClosed {
                    stage,
                    legs: legs_in_stage,
                    interrupt_height: height,
                });
                legs_in_stage = 0;
            }
            Progress::Closed => emit(ProduceEvent::Settling),
        }
    });
    match outcome {
        ProduceOutcome::Cancelled => Err(STOPPED.to_owned()),
        ProduceOutcome::Object(obj) => {
            let lines = path_skip_stderr().lines().map(str::to_owned).collect();
            emit(ProduceEvent::PathLines(lines));
            Ok(encode_produced(&obj))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::Ordering;

    /// CASES pk_A. O1-S1 is this key on the S1 rig.
    const PK_A: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
    const O1_S1_SHA256: &str = "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246";

    #[test]
    fn s1_with_pk_a_matches_the_o1_s1_pin() {
        let public_key = crate::hex::decode32(PK_A).unwrap();
        let cancel = AtomicBool::new(false);
        let bytes = produce_case(&public_key, Case::S1, &cancel, |_| {}).expect("S1 object");
        assert_eq!(crate::hex::encode(Sha256::digest(&bytes).as_slice()), O1_S1_SHA256);
    }

    #[test]
    fn cancel_before_the_rig_returns_no_object() {
        let cancel = AtomicBool::new(true);
        assert!(cancel.load(Ordering::SeqCst));
        let err = produce_case(&[0x11; 32], Case::S1, &cancel, |_| {}).unwrap_err();
        assert_eq!(err, STOPPED);
    }
}
