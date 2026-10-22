//! Erratum 2 (E01, E02). A malformed CBOR value records path fields and counts
//! as `not defined`. A decoded empty checkpoint list still counts as zero, and
//! an empty path byte string on a decoded stage is still `skip`.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

/// SHA-256 of the erratum 2 malformed verify record, including its trailing newline.
const MALFORMED_RECORD_SHA256: &str =
    "eb7bff4d55ddcc47e47c600276693db8a406a0083a47c2526462651ffa5bdb8c";

fn expected_record() -> &'static str {
    include_str!("fixtures/erratum2-malformed-record.json")
}

struct Run {
    code: i32,
    out: String,
}

fn pcoc(args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_pcoc"))
        .args(args)
        .output()
        .expect("pcoc");
    Run {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8(out.stdout).expect("stdout utf-8"),
    }
}

fn temp_bytes(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("pcoc-ac-e2-{name}-{}", std::process::id()));
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(bytes).unwrap();
    path
}

/// Count JSON fields whose key is exactly `key`. The opening quote keeps a
/// shorter key from matching inside a longer one.
fn assert_field(record: &str, key: &str, value: &str, n: usize) {
    let needle = format!("\"{key}\":{value}");
    assert_eq!(record.matches(&needle).count(), n, "{key}");
}

fn assert_malformed_record(out: &str) {
    let expected = expected_record();
    let dig = pcoc::hexutil::hex_encode(&pcoc::hash::sha256(expected.as_bytes()));
    assert_eq!(dig, MALFORMED_RECORD_SHA256, "erratum 2 record pin");
    assert_field(out, "path_state", "\"not defined\"", 16);
    assert_field(out, "plant", "\"not defined\"", 16);
    assert_field(out, "checkpoint_count", "\"not defined\"", 18);
    assert_field(out, "skip_list", "[]", 1);
    assert_field(out, "whole_hike_legs", "\"not defined\"", 1);
    assert_field(out, "hike_legs", "\"not defined\"", 1);
    assert_eq!(out, expected, "full malformed record");
}

fn assert_cli_record(name: &str, bytes: &[u8]) {
    let path = temp_bytes(name, bytes);
    let run = pcoc(&["verify", path.to_str().unwrap()]);
    assert_eq!(run.code, 0, "{name}");
    assert_malformed_record(&run.out);
}

#[test]
fn malformed_empty_input_record() {
    assert_cli_record("empty", b"");
}

#[test]
fn malformed_truncated_item_record() {
    // Byte string that claims five bytes and ends after one.
    assert_cli_record("truncated", &[0x58, 0x05, 0x01]);
}

#[test]
fn malformed_kind_not_utf8_record() {
    // Map { "kind": <one byte 0xff> }. The text is not UTF-8, so the value is
    // malformed (erratum 2 E01). It is not Rule A5 and not a decode failure.
    assert_cli_record(
        "kind-utf8",
        &[0xa1, 0x64, b'k', b'i', b'n', b'd', 0x61, 0xff],
    );
}

fn empty_stage(with_path: bool) -> Vec<u8> {
    let z = pcoc::cbor::encode_bytes(&[0u8; 32]);
    let mut pairs = vec![
        ("beacon".into(), z.clone()),
        ("beacon_height".into(), pcoc::cbor::encode_uint(0)),
        ("checkpoints".into(), pcoc::cbor::encode_array(&[])),
        ("leg_count".into(), pcoc::cbor::encode_uint(0)),
        ("leg_results".into(), pcoc::cbor::encode_array(&[])),
        ("waypoint".into(), z),
    ];
    if with_path {
        pairs.push(("path".into(), pcoc::cbor::encode_bytes(b"")));
    }
    pcoc::cbor::encode_map(&pairs)
}

#[test]
fn decoded_empty_checkpoints_and_empty_path() {
    let bytes = pcoc::cbor::encode_map(&[
        (
            "kind".into(),
            pcoc::cbor::encode_text(pcoc::constants::KIND),
        ),
        (
            "stages".into(),
            pcoc::cbor::encode_array(&[empty_stage(false), empty_stage(true)]),
        ),
    ]);
    let profile = pcoc::profile::Profile::conformance_small(1, 8, 8, 1).expect("profile");
    let mut index = pcoc::verify::EmptyIndex;
    let verified = pcoc::verify(&bytes, &profile, &mut index);
    let pcoc::verify::Verified::Record(rec) = verified else {
        panic!("decoded object");
    };
    assert!(
        rec.contains("\"g\":1,\"path_state\":\"skip\",\"plant\":\"n/a\""),
        "path_state"
    );
    assert!(
        rec.contains("\"g\":0,\"checkpoint_count\":0,"),
        "checkpoint_count"
    );
    assert!(
        rec.contains("\"g\":1,\"checkpoint_count\":0,"),
        "checkpoint_count"
    );
    assert!(
        !rec.contains("\"shape_item\":[\"malformed cbor\"]"),
        "decoded object"
    );
}
