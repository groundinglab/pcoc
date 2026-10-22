//! AC-A7. Verify CLI: the section 14 record, and exit codes from section 18.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

struct Run {
    code: i32,
    out: String,
    err: String,
}

fn pcoc(args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_pcoc"))
        .args(args)
        .output()
        .expect("pcoc");
    Run {
        code: out.status.code().unwrap_or(-1),
        out: String::from_utf8(out.stdout).expect("stdout utf-8"),
        err: String::from_utf8(out.stderr).expect("stderr utf-8"),
    }
}

fn forbid(text: &str) {
    let lower = text.to_ascii_lowercase();
    for word in lower.split(|c: char| !c.is_ascii_alphanumeric()) {
        assert!(
            word != "pass" && word != "fail" && word != "accept" && word != "refuse",
            "forbidden word in: {text}"
        );
    }
}

fn assert_keys(stdout: &str, small: bool) {
    assert!(stdout.ends_with('\n'), "trailing newline");
    assert_eq!(stdout.matches('\n').count(), 1, "one line");
    let rec = stdout.trim_end_matches('\n');
    let mut keys = vec![
        "profile",
        "shape",
        "shape_item",
        "headers",
        "seed_value",
        "checkpoints",
        "waypoints",
        "paths",
        "anchors",
        "anchored_total",
        "skip_list",
        "a",
        "b",
        "N",
        "S",
        "delta_h",
        "fixed_start",
        "P_b",
        "height_P_b",
        "index_comparison_P_b",
        "T_P_b",
        "height_I_a_minus_1",
        "T_I_a_minus_1",
        "six_leg",
        "six_leg_runs",
        "analysis",
    ];
    if small {
        keys.splice(
            1..1,
            [
                "sample_bytes",
                "slot_bytes",
                "terrain_bytes",
                "steps_per_leg",
            ],
        );
    }
    let mut pos = 0;
    for key in keys {
        let needle = format!("\"{key}\":");
        let at = rec[pos..].find(&needle).unwrap_or_else(|| panic!("missing {key} in {rec}"));
        pos += at + needle.len();
    }
}

fn temp_bytes(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("pcoc-ac-a7-{name}-{}", std::process::id()));
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(bytes).unwrap();
    path
}

fn s1_args(path: &str) -> Vec<String> {
    [
        "verify",
        "--profile",
        "conformance-small",
        "--sample-bytes",
        "32",
        "--slot-bytes",
        "64",
        "--terrain-bytes",
        "65536",
        "--steps-per-leg",
        "100",
        path,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[test]
fn verify_cli() {
    let empty = temp_bytes("empty", b"");
    let empty_s = empty.to_str().unwrap();
    let run = pcoc(&["verify", empty_s]);
    assert_eq!(run.code, 0);
    assert!(run.out.contains("\"shape_item\":[\"malformed cbor\"]"));
    assert_keys(&run.out, false);
    forbid(&run.out);
    forbid(&run.err);

    for profile in ["production", "test", "live"] {
        let run = pcoc(&["verify", "--profile", profile, "--draw-label", "pcoc-pack-2.2/draw", empty_s]);
        assert_eq!(run.code, 0, "{profile}");
        assert!(run.err.contains("draw: injected label (rig input)"), "{profile} {}", run.err);
        forbid(&run.out);
        forbid(&run.err);
    }
    let small = pcoc(&[
        "verify",
        "--profile",
        "conformance-small",
        "--sample-bytes",
        "32",
        "--slot-bytes",
        "64",
        "--terrain-bytes",
        "65536",
        "--steps-per-leg",
        "100",
        "--draw-label",
        "pcoc-pack-2.2/draw",
        empty_s,
    ]);
    assert_eq!(small.code, 0);
    assert!(small.err.contains("draw: injected label (rig input)"));
    assert_keys(&small.out, true);
    forbid(&small.out);
    forbid(&small.err);

    let missing = pcoc(&["verify", "--profile", "conformance-small", empty_s]);
    assert_eq!(missing.code, 2);
    forbid(&missing.out);
    forbid(&missing.err);

    let extra = pcoc(&["verify", "--profile", "production", "--sample-bytes", "32", empty_s]);
    assert_eq!(extra.code, 2);
    forbid(&extra.err);
    for profile in ["test", "live"] {
        let run = pcoc(&["verify", "--profile", profile, "--steps-per-leg", "1", empty_s]);
        assert_eq!(run.code, 2, "{profile}");
        forbid(&run.err);
    }

    let none = pcoc(&[]);
    assert_eq!(none.code, 2);
    forbid(&none.err);

    let unread = pcoc(&["verify", "/no/such/pcoc-object-ac-a7"]);
    assert_eq!(unread.code, 1);
    assert!(unread.err.contains("unreadable file"));
    assert!(unread.out.is_empty());
    forbid(&unread.err);

    // SPEC §7.3 second sort: one well-formed item that is not a map. `0x00` is unsigned 0.
    let uint = temp_bytes("uint0", &[0x00]);
    let decoded = pcoc(&["verify", uint.to_str().unwrap()]);
    assert_eq!(decoded.code, 1);
    assert!(decoded.out.is_empty());
    assert!(decoded.err.contains("section 7.3: no record"));
    forbid(&decoded.out);
    forbid(&decoded.err);

    let bad = temp_bytes("ff", &[0xff]);
    let malformed = pcoc(&["verify", bad.to_str().unwrap()]);
    assert_eq!(malformed.code, 0);
    assert!(malformed.out.contains("malformed cbor"));
    forbid(&malformed.out);

    // Invalid UTF-8 text is not a definite-length text string, so the item is malformed.
    let utf8 = temp_bytes("utf8", &[0xa1, 0x64, b'k', b'i', b'n', b'd', 0x61, 0xff]);
    let kind = pcoc(&["verify", utf8.to_str().unwrap()]);
    assert_eq!(kind.code, 0);
    assert!(kind.out.contains("malformed cbor"));
    forbid(&kind.out);

    let unknown = temp_bytes(
        "kind",
        &[0xa1, 0x64, b'k', b'i', b'n', b'd', 0x64, b'n', b'o', b'p', b'e'],
    );
    let rule = pcoc(&["verify", unknown.to_str().unwrap()]);
    assert_eq!(rule.code, 0);
    assert_eq!(rule.out, "{\"kind\":\"unknown kind\",\"kind_text\":\"nope\"}\n");
    forbid(&rule.out);
    forbid(&rule.err);

    let o1 = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../objects/O1-S1.cbor");
    let o1s = o1.to_str().unwrap();
    let mut args = s1_args(o1s);
    let owned: Vec<&str> = args.iter().map(String::as_str).collect();
    let quiet = pcoc(&owned);
    assert_eq!(quiet.code, 0, "{}", quiet.err);
    assert!(!quiet.err.contains("injected"));
    assert_keys(&quiet.out, true);
    forbid(&quiet.out);
    forbid(&quiet.err);
    let again = pcoc(&owned);
    assert_eq!(again.code, 0);
    let i1 = quiet.out.find("\"indices\":").unwrap();
    let i2 = again.out.find("\"indices\":").unwrap();
    assert_ne!(&quiet.out[i1..i1 + 40], &again.out[i2..i2 + 40], "draw repeated");

    args.insert(1, "--draw-label".into());
    args.insert(2, "pcoc-pack-2.2/draw".into());
    let owned: Vec<&str> = args.iter().map(String::as_str).collect();
    let labeled = pcoc(&owned);
    assert_eq!(labeled.code, 0);
    assert!(labeled.err.contains("draw: injected label (rig input)"));
    assert_keys(&labeled.out, true);
    forbid(&labeled.out);
    forbid(&labeled.err);

    let mut mismatch_args = s1_args(o1s);
    let steps = mismatch_args.iter().position(|s| s == "100").unwrap();
    mismatch_args[steps] = "99".into();
    let owned: Vec<&str> = mismatch_args.iter().map(String::as_str).collect();
    let mismatch = pcoc(&owned);
    assert_eq!(mismatch.code, 0);
    assert!(mismatch.out.contains("\"shape\":\"mismatch\""));
    forbid(&mismatch.out);
    forbid(&mismatch.err);

    let live = pcoc(&["produce"]);
    assert_eq!(live.code, 2);
    assert!(live.err.contains("live header source is not in this build"));
    forbid(&live.out);
    forbid(&live.err);
}
