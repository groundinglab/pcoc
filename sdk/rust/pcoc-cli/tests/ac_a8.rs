//! AC-A8. SHA-256 backend is the one the host is actually using.

#[path = "../../support/repo.rs"]
mod repo;

use repo::repo_root;
use std::process::Command;

#[test]
fn selftest_sha_backend() {
    let _root = repo_root();
    let cpu = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let has_sha_ni = cpu.split_whitespace().any(|w| w == "sha_ni");
    let backend = pcoc::sha_backend();
    if has_sha_ni {
        assert_eq!(backend, "sha-ni");
    }
    let out = Command::new(env!("CARGO_BIN_EXE_pcoc"))
        .arg("selftest")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(&format!("sha256: {backend}")), "{text}");
    if has_sha_ni {
        assert!(text.contains("sha256: sha-ni"));
        assert!(!text.contains("software"));
    }
    let info = pcoc::build_info();
    assert_eq!(info.sha_backend, backend);
}
