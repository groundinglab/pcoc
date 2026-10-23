//! SPEC §15.4 Rule B2. The admission rate is `(hike_legs, hike_span)`.
//!
//! The gate calls `pcoc::leg_rate_floor` (`sdk/rust/pcoc/src/compare.rs`).
//! Bookend `N` and `S` are on the record and are not that comparison.

use pcoc_admission::policy::{self, AdmissionPolicy, Judgment};
use pcoc_admission::verifier::Measured;
use pcoc_admission::verifier::{Measures, Report, SPEC_KIND};

/// Floor `1/5`, the SPEC §15.4 default.
fn judge(record: &str) -> Judgment {
    policy::judge(&Report::Read(measured(record)), 1, 5, 0)
}

fn measured(record: &str) -> Measured {
    Measured {
        earlier_version: false,
        kind: SPEC_KIND.to_owned(),
        record: record.to_owned(),
        verifier_notes: Vec::new(),
        measures: Measures {
            legs: None,
            span_s: None,
            leg_rate: None,
            waypoints_fetched: None,
            waypoints_total: 18,
            paths_included: None,
            legs_recomputed_ok: None,
        },
    }
}

/// Bookend `S` is a separate field. The hike pair is `analysis`.
fn record(legs: &str, span: &str, bookend_s: &str) -> String {
    format!(
        "{{\"profile\":\"production\",\"N\":9,\"S\":{bookend_s},\"analysis\":{{\"hike_legs\":{legs},\"hike_span\":{span}}}}}\n"
    )
}

fn refused(judgment: &Judgment) -> bool {
    matches!(judgment, Judgment::Refuse { reasons, .. } if reasons.iter().any(|r| r == "leg_rate_floor"))
}

/// SPEC §15.4 Rule B2 (L1974) and §14 `hike_span` (L1592): span 0 is `refuse`,
/// not `held`, and it does not meet the floor.
#[test]
fn hike_span_zero_is_refuse() {
    assert_eq!(pcoc::leg_rate_floor(4, 0, 1, 5), "refuse");
    let judgment = judge(&record("4", "0", "100"));
    assert!(refused(&judgment), "{judgment:?}");
    assert!(!matches!(judgment, Judgment::Held { .. }));
}

/// SPEC §15.4 Rule B2 (L1974): a negative `hike_span` is `refuse`, not `held`.
#[test]
fn hike_span_negative_is_refuse() {
    assert_eq!(pcoc::leg_rate_floor(4, -3, 1, 5), "refuse");
    let judgment = judge(&record("4", "-3", "100"));
    assert!(refused(&judgment), "{judgment:?}");
    assert!(!matches!(judgment, Judgment::Held { .. }));
}

/// SPEC §15.4 Rule B2 (L1974): a numeric shortfall is `refuse`.
/// `q · hike_legs = 5 · 1` is less than `p · hike_span = 1 · 6`.
#[test]
fn hike_span_shortfall_is_refuse() {
    assert_eq!(pcoc::leg_rate_floor(1, 6, 1, 5), "refuse");
    let judgment = judge(&record("1", "6", "1"));
    assert!(refused(&judgment), "{judgment:?}");
}

/// SPEC §15.4 Rule B2 (L1974) and erratum E05: exact equality is `meets`.
/// `5 · 1 ≥ 1 · 5`. `meets` admits; it is not itself an outcome token.
#[test]
fn hike_span_exact_equality_meets() {
    assert_eq!(pcoc::leg_rate_floor(1, 5, 1, 5), "meets");
    let record = record("1", "5", "0");
    assert!(matches!(judge(&record), Judgment::Admit { record: stored } if stored == record));
}

/// SPEC §15.4 Rule B0 (L1970) and Rule B2 (L1974): `hike_span` `not read`
/// is not a comparison. The state is `held`.
#[test]
fn hike_span_not_read_is_held() {
    let record = record("4", "\"not read\"", "0");
    assert!(matches!(judge(&record), Judgment::Held { record: stored } if stored == record));
}

/// SPEC L237 and Rule B6 (L1990): bookend `S = 0` with a good hike pair does
/// not change the outcome. The same pair with a large bookend `S` admits too.
/// A short hike pair stays `refuse` when bookend `S` is 0.
#[test]
fn bookend_s_zero_does_not_change_the_hike_outcome() {
    let good_zero = record("2", "5", "0");
    let good_wide = record("2", "5", "9999");
    assert!(matches!(judge(&good_zero), Judgment::Admit { .. }));
    assert!(matches!(judge(&good_wide), Judgment::Admit { .. }));
    assert_eq!(
        std::mem::discriminant(&judge(&good_zero)),
        std::mem::discriminant(&judge(&good_wide))
    );
    let short_zero = record("1", "6", "0");
    assert!(refused(&judge(&short_zero)));
}

/// `policy_checks` uses the same SDK comparison. A stored span of 0 does not meet.
#[test]
fn policy_checks_calls_the_sdk_and_refuses_span_zero() {
    let mut measures = measured("").measures;
    measures.legs = Some(1);
    measures.span_s = Some(0);
    let checks = policy::policy_checks(&measures, &AdmissionPolicy::default());
    let rate = checks
        .iter()
        .find(|check| check.field == "leg_rate")
        .unwrap();
    assert!(!rate.met);
    assert_eq!(pcoc::leg_rate_floor(1, 0, 1, 5), "refuse");
    measures.span_s = Some(5);
    let checks = policy::policy_checks(&measures, &AdmissionPolicy::default());
    let rate = checks
        .iter()
        .find(|check| check.field == "leg_rate")
        .unwrap();
    assert!(rate.met);
    assert_eq!(pcoc::leg_rate_floor(1, 5, 1, 5), "meets");
}
