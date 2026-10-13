//! Produce error paths that must not touch the network.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "satoshi-hike-produce-{name}-{}",
            std::process::id()
        ));
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

fn produce_bin() -> PathBuf {
    // Cargo 1.83 sets `CARGO_BIN_EXE_` from the binary filename, hyphens included.
    // Later Cargo replaces hyphens with underscores. Accept either.
    if let Some(path) = option_env!("CARGO_BIN_EXE_satoshi-hike-produce") {
        return PathBuf::from(path);
    }
    if let Some(path) = option_env!("CARGO_BIN_EXE_satoshi_hike_produce") {
        return PathBuf::from(path);
    }
    let mut path = std::env::current_exe().expect("test executable");
    path.pop();
    path.pop();
    path.push("satoshi-hike-produce");
    path
}

fn produce(args: &[&str]) -> std::process::Output {
    Command::new(produce_bin())
        .args(args)
        .output()
        .expect("satoshi-hike-produce")
}

const FAIL_REPORT: &str = "{\"schema\":\"satoshi-hike-cli-report-v0.2\",\"command\":\"produce\",\"result\":\"not read\",\"rung\":null}\n";

#[test]
fn missing_output_parent_still_copies_the_fail_report() {
    let dir = Scratch::new("missing-parent");
    let report = dir.path("rep.json");
    let output = dir.path("missing").join("out.cbor");
    let out = produce(&[
        "--pubkey-hex",
        &"ab".repeat(32),
        "--output",
        output.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--gossip-peer",
        "127.0.0.1:9",
        "--bari-url",
        "http://127.0.0.1:9",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(std::str::from_utf8(&out.stdout).unwrap(), FAIL_REPORT);
    assert_eq!(fs::read_to_string(&report).unwrap(), FAIL_REPORT);
    assert!(!output.exists());
    assert!(!dir.path("missing").exists());
}

#[test]
fn usage_error_writes_no_report() {
    let dir = Scratch::new("usage");
    let report = dir.path("rep.json");
    let out = produce(&["--report", report.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(!report.exists());
}
