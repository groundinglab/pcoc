//! Shape measurements the running verifier already records.
//! Exact `shape_item` arrays for the small profile are in `rule_ids` and `object`.

#[path = "common/support.rs"]
mod support;

fn shape(bytes: &[u8]) -> String {
    support::section14(&support::verify_offline(bytes)).shape
}

/// Rule A4.1. A malformed CBOR value is shape `mismatch`.
/// main @ 546d24f L976 and L1014. The `shape_item` array is pending code.
#[test]
fn a4_1_malformed_cbor_is_mismatch() {
    assert_eq!(shape(&[0xff]), "mismatch");
}

/// The harness reads the section 14 record. `headers` has 19 rows, and a
/// short draw is `not read`, not an empty `six_leg_runs` array.
#[test]
fn malformed_cbor_record_has_nineteen_not_read_headers() {
    let record = support::section14(&support::verify_offline(&[0xff]));
    let headers = record.headers.expect("headers");
    assert_eq!(headers.len(), 19);
    assert!(headers.iter().all(|row| row.index_comparison == "not read"));
    assert!(matches!(headers[18].stored_hash, support::Field::NotDefined));
    assert!(matches!(headers[18].stored_height, support::Field::NotDefined));
    assert!(matches!(headers[18].t, support::Field::NotRead));
    assert_eq!(record.shape, "mismatch");
    let items = record.shape_item.expect("shape_item");
    assert!(items.iter().any(|item| item == "malformed cbor"));
    assert_eq!(record.six_leg, support::SixLegRecord::NotRead);
    assert_eq!(record.seed_value.as_deref(), Some("not defined"));
}

/// Rule A4.1. `stages` length other than 18 is shape `mismatch`.
/// main @ 546d24f L965.
#[test]
fn a4_1_seventeen_stages_is_mismatch() {
    let mut hike = support::quiet_hike();
    hike.stages.pop();
    assert_eq!(hike.stages.len(), 17);
    assert_eq!(shape(&hike::encode_hike(&hike)), "mismatch");
}

/// Rule A4.1. A stored height that is not strictly greater is shape `mismatch`.
/// main @ 546d24f L971.
#[test]
fn a4_1_height_not_greater_is_mismatch() {
    let mut hike = hike::structural_object();
    hike.stages[2].beacon_height = hike.stages[1].beacon_height;
    assert_eq!(shape(&hike::encode_hike(&hike)), "mismatch");
}

/// Rule A4.1. A stored header hash that repeats the header before it is shape `mismatch`.
/// main @ 546d24f L972.
#[test]
fn a4_1_header_hash_not_different_is_mismatch() {
    let mut hike = hike::structural_object();
    hike.stages[2].beacon = hike.stages[1].beacon;
    assert_eq!(shape(&hike::encode_hike(&hike)), "mismatch");
}

/// Rule A4.1. `stages[0].beacon` other than `seed_beacon` is shape `mismatch`.
/// main @ 546d24f L969.
#[test]
fn a4_1_seed_beacon_disagrees_is_mismatch() {
    let mut hike = hike::structural_object();
    hike.seed_beacon[0] ^= 0xff;
    assert_eq!(shape(&hike::encode_hike(&hike)), "mismatch");
}

/// Rule A4.1. `leg_count < 1` is shape `mismatch`.
/// main @ 546d24f L966.
#[test]
fn a4_1_leg_count_zero_is_mismatch() {
    let hike = support::quiet_hike();
    assert_eq!(shape(&hike::encode_hike(&hike)), "mismatch");
}

/// Rule A4.4. Fewer than six legs leaves `six_leg` `not defined`.
/// main @ 546d24f L1004 and L1266.
#[test]
fn a4_4_fewer_than_six_legs_not_defined() {
    let mut hike = support::quiet_hike();
    hike.stages.truncate(5);
    let record = support::section14(&support::verify_offline(&hike::encode_hike(&hike)));
    assert_eq!(record.six_leg, support::SixLegRecord::NotDefined);
}
