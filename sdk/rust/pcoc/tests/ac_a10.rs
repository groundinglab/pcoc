//! AC-A10. The script is the check. It builds the release library and binary
//! in its own target directory, then runs the three checks.

#[path = "../../support/repo.rs"]
mod repo;

use repo::repo_root;
use std::process::Command;

#[test]
fn negatives_script_passes() {
    let root = repo_root();
    let script = root.join("sdk/rust/check/negatives.sh");
    let out = Command::new("sh").arg(&script).output().expect("script");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "stdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.lines().any(|l| l == "PASS"), "{stdout}");
    assert!(stdout.contains("check 4 mutants: N/A"));
}
