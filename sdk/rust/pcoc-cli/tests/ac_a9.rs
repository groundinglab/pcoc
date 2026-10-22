//! AC-A9. Library version and `pcoc --version` report 2.2.0, the spec tag, and the pack pin.

#[path = "../../support/repo.rs"]
mod repo;

use repo::repo_root;
use std::process::Command;

#[test]
fn version_reports_pin() {
    let root = repo_root();
    let file = std::fs::read_to_string(root.join("sdk/VERSION")).unwrap();
    assert_eq!(file.trim(), pcoc::VERSION);
    assert_eq!(pcoc::VERSION, "2.2.0");
    let pin =
        pcoc::pin::read_pin(&std::fs::read_to_string(root.join("sdk/SPEC_PIN")).unwrap()).unwrap();
    let info = pcoc::build_info();
    assert_eq!(info.version, "2.2.0");
    assert_eq!(info.spec_tag, "spec-2.2.0");
    assert_eq!(info.spec_tag, pin.tag);
    assert_eq!(info.pack_pin, pin.vector_sha256);
    let out = Command::new(env!("CARGO_BIN_EXE_pcoc"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        text,
        format!("2.2.0\n{}\n{}\n", info.spec_tag, info.pack_pin)
    );
}
