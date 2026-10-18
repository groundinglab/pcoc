//! The quiet report stays on stdout. The measurement record is stderr.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("satoshi-hike-verify-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn verify_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_satoshi-hike-verify") {
        return PathBuf::from(path);
    }
    if let Some(path) = option_env!("CARGO_BIN_EXE_satoshi_hike_verify") {
        return PathBuf::from(path);
    }
    let mut path = std::env::current_exe().expect("test executable");
    path.pop();
    path.pop();
    path.push("satoshi-hike-verify");
    path
}

fn verify(args: &[&str]) -> std::process::Output {
    Command::new(verify_bin())
        .args(args)
        .output()
        .expect("satoshi-hike-verify")
}

fn record_lines(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| line.contains("\"event\":\"hike.verify."))
        .collect()
}

#[test]
fn quiet_default_keeps_the_report_and_omits_the_record() {
    let dir = Scratch::new("quiet");
    let input = dir.path("hike.cbor");
    fs::write(&input, b"not-a-hike").unwrap();
    let report = dir.path("rep.json");
    let args = [
        "--input",
        input.to_str().unwrap(),
        "--index-url",
        "http://127.0.0.1:9",
        "--report",
        report.to_str().unwrap(),
    ];
    let plain = verify(&args);
    assert_eq!(plain.status.code(), Some(1));
    assert!(
        plain.stdout.is_empty(),
        "{:?}",
        std::str::from_utf8(&plain.stdout)
    );
    assert!(
        !report.exists(),
        "no record when the object does not decode"
    );
    let stderr = String::from_utf8(plain.stderr).unwrap();
    assert!(record_lines(&stderr).is_empty(), "{stderr}");

    let mut quiet_args = args.to_vec();
    quiet_args.push("--log");
    quiet_args.push("quiet");
    let quiet = verify(&quiet_args);
    assert_eq!(quiet.stdout, plain.stdout);
    assert!(record_lines(&String::from_utf8(quiet.stderr).unwrap()).is_empty());
}

#[test]
fn fail_and_verbose_write_the_record_without_changing_stdout() {
    let dir = Scratch::new("verbose");
    let input = dir.path("hike.cbor");
    fs::write(&input, b"not-a-hike").unwrap();
    let report = dir.path("rep.json");
    let base = [
        "--input",
        input.to_str().unwrap(),
        "--index-url",
        "http://127.0.0.1:9",
        "--report",
        report.to_str().unwrap(),
    ];
    for degree in ["fail", "verbose"] {
        let mut args = base.to_vec();
        args.push("--log");
        args.push(degree);
        let out = verify(&args);
        assert_eq!(out.status.code(), Some(1), "{degree}");
        assert!(out.stdout.is_empty(), "{degree}");
        assert!(!report.exists(), "{degree}");
        let stderr = String::from_utf8(out.stderr).unwrap();
        let lines = record_lines(&stderr);
        assert_eq!(lines.len(), 7, "{stderr}");
        assert!(lines[0].contains("hike.verify.object"), "{stderr}");
        assert!(lines[0].contains("\"opened\":true"), "{stderr}");
        assert!(lines[0].contains("\"bytes\":10"), "{stderr}");
        assert!(!lines[0].contains("public_key"), "{stderr}");
        assert!(
            lines[1].contains("\"order\":1") && lines[1].contains("\"name\":\"shape\""),
            "{stderr}"
        );
        assert!(
            lines[2].contains("\"name\":\"checkpoints\"") && lines[2].contains("\"found\":null"),
            "{stderr}"
        );
        assert!(lines[3].contains("\"name\":\"waypoints\""), "{stderr}");
        assert!(
            lines[4].contains("\"name\":\"leg rate\"")
                && lines[4].contains("\"found\":\"not read\""),
            "{stderr}"
        );
        assert!(lines[5].contains("\"name\":\"paths included\""), "{stderr}");
        assert!(
            lines[6].contains("\"name\":\"six legs\"")
                && lines[6].contains("\"found\":\"not read\""),
            "{stderr}"
        );
        assert!(
            lines.iter().all(|line| !line.contains("hike.verify.note")),
            "{stderr}"
        );
        assert!(
            lines
                .iter()
                .all(|line| line.contains("hike.verify.object")
                    || line.contains("hike.verify.check")),
            "{stderr}"
        );
        assert!(!stderr.contains("walk output"), "{stderr}");
    }
}

#[test]
fn an_unopened_input_stays_outside_section_14() {
    let dir = Scratch::new("missing");
    let input = dir.path("missing.cbor");
    let out = verify(&[
        "--input",
        input.to_str().unwrap(),
        "--index-url",
        "http://127.0.0.1:9",
        "--log",
        "verbose",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    let lines = record_lines(&stderr);
    assert_eq!(lines.len(), 1, "{stderr}");
    assert!(lines[0].contains("hike.verify.object"), "{stderr}");
    assert!(lines[0].contains("\"opened\":false"), "{stderr}");
    assert!(
        lines.iter().all(|line| !line.contains("hike.verify.check")),
        "{stderr}"
    );
    assert!(
        lines.iter().all(|line| !line.contains("hike.verify.note")),
        "{stderr}"
    );
}

#[test]
fn a_report_write_failure_keeps_exit_zero_when_stdout_has_the_record() {
    let dir = Scratch::new("report-io");
    let input = dir.path("hike.cbor");
    // One empty CBOR map. It decodes, so stdout is the section 14 record.
    fs::write(&input, [0xa0]).unwrap();
    let report = dir.path("missing").join("record.json");
    let out = verify(&[
        "--input",
        input.to_str().unwrap(),
        "--index-url",
        "http://127.0.0.1:9",
        "--report",
        report.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("\"shape\":"), "{stdout}");
    assert!(stdout.ends_with('\n'), "{stdout}");
    assert!(!report.exists());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains(report.to_str().unwrap()), "{stderr}");
    assert!(record_lines(&stderr).is_empty(), "{stderr}");
}

#[test]
fn an_unknown_log_degree_is_a_usage_error() {
    let dir = Scratch::new("usage");
    let input = dir.path("hike.cbor");
    fs::write(&input, b"not-a-hike").unwrap();
    let out = verify(&[
        "--input",
        input.to_str().unwrap(),
        "--index-url",
        "http://127.0.0.1:9",
        "--log",
        "debug",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("--log quiet|verbose"), "{stderr}");
    assert!(record_lines(&stderr).is_empty(), "{stderr}");
}

/// rc.2 (approved change 2): exit 1 only when no top-level map decodes. A map with a
/// trailing byte writes its record and exits 0; a well-formed item that is
/// not a map exits 1 with nothing on stdout.
#[test]
fn exit_one_only_when_no_top_level_map_decodes() {
    let dir = Scratch::new("exit");
    let input = dir.path("hike.cbor");
    for (bytes, code) in [
        (&[0x01u8][..], 1),
        (&[0x80], 1),
        (&[0x60], 1),
        (&[0x01, 0x00], 1),
        (&[0xa0], 0),
        (&[0xa0, 0x00], 0),
    ] {
        fs::write(&input, bytes).unwrap();
        let out = verify(&[
            "--input",
            input.to_str().unwrap(),
            "--index-url",
            "http://127.0.0.1:9",
        ]);
        assert_eq!(out.status.code(), Some(code), "{bytes:?}");
        assert_eq!(out.stdout.is_empty(), code == 1, "{bytes:?}");
        if code == 0 {
            let stdout = String::from_utf8(out.stdout).unwrap();
            assert!(stdout.contains("\"shape\":\"mismatch\""), "{stdout}");
        }
    }
}
