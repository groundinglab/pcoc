//! Rule A5. An object whose kind is the earlier kind string is not measured.
//! Sections 14.1–14.6 do not run. The record is the version line.

const EARLIER_KIND: &[u8] = b"satoshi-hike-v1.35-object";

/// One-field CBOR map: `kind` = `satoshi-hike-v1.35-object`.
fn earlier_kind_object() -> Vec<u8> {
    let mut out = vec![0xa1, 0x64];
    out.extend_from_slice(b"kind");
    out.push(0x78);
    out.push(u8::try_from(EARLIER_KIND.len()).expect("kind length"));
    out.extend_from_slice(EARLIER_KIND);
    out
}

#[test]
fn a5_earlier_kind_records_earlier_version_and_is_not_measured() {
    let bytes = earlier_kind_object();
    let committed = include_bytes!("../../../../spec/conformance/v2.0/earlier-kind.cbor");
    assert_eq!(bytes, committed);
    let record = earlier_record(&bytes);
    assert_eq!(
        record,
        "{\"version\":\"earlier version\",\"kind\":\"satoshi-hike-v1.35-object\"}\n"
    );
    assert!(!record.contains("mismatch"));
    let measurements = hike::verify(
        &bytes,
        hike::AuditDraw::PinnedSeed([0u8; 32]),
        |_| panic!("an earlier object is not measured"),
        |_| -> hike::ChainRead { panic!("an earlier object is not measured") },
    );
    assert_eq!(hike::failure_facts(&measurements), vec!["earlier_version"]);
    assert!(!hike::admit(&measurements));
}

fn earlier_record(bytes: &[u8]) -> String {
    let measurements = hike::verify(
        bytes,
        hike::AuditDraw::PinnedSeed([0u8; 32]),
        |_| panic!("an earlier object is not measured"),
        |_| -> hike::ChainRead { panic!("an earlier object is not measured") },
    );
    hike::verify_record_json("production", &measurements)
}
