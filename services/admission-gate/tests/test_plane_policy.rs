//! Test-plane gate policy. The header source and Bari client are not in the SDK yet:
//! spec defines measurements, a policy sets the values (QA B2). This is the
//! test-plane judgment Bari's test-plane gate tests use; it is a fixture
//! here, not part of the admission door's v7 policy, and the door does not
//! call it. A skip is recorded beside the gate. It is not a pass.
#![allow(dead_code)]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateJudgment {
    Pass,
    Fail,
}

/// Recorded beside the gate. Not an input to [`judge_test_policy`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateSkip {
    pub reason: String,
}

/// Measurements the test policy judges. Verify against the test index is
/// `index_matches_tip`. This does not call the live door.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestGateMeasurement {
    pub planted: bool,
    pub path_status: u16,
    pub index_is_live: bool,
    pub index_matches_tip: bool,
}

/// Test admit policy. A skip is not passed here. Live-index measurements fail.
pub fn judge_test_policy(measurement: &TestGateMeasurement) -> GateJudgment {
    if measurement.planted
        && measurement.path_status == 200
        && !measurement.index_is_live
        && measurement.index_matches_tip
    {
        GateJudgment::Pass
    } else {
        GateJudgment::Fail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_passes_only_the_whole_gate() {
        let pass = TestGateMeasurement {
            planted: true,
            path_status: 200,
            index_is_live: false,
            index_matches_tip: true,
        };
        assert_eq!(judge_test_policy(&pass), GateJudgment::Pass);
        assert_eq!(
            judge_test_policy(&TestGateMeasurement {
                path_status: 404,
                ..pass
            }),
            GateJudgment::Fail
        );
        assert_eq!(
            judge_test_policy(&TestGateMeasurement {
                index_is_live: true,
                ..pass
            }),
            GateJudgment::Fail
        );
        assert_eq!(
            judge_test_policy(&TestGateMeasurement {
                planted: false,
                ..pass
            }),
            GateJudgment::Fail
        );
        assert_eq!(
            judge_test_policy(&TestGateMeasurement {
                index_matches_tip: false,
                ..pass
            }),
            GateJudgment::Fail
        );
    }
}
