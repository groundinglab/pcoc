//! AC-A5x. Verify each pinned object.
//! The draw is the injected label `pcoc-pack-2.2/draw`.
//! The CASES synthetic index is served on loopback.

#[path = "../../support/repo.rs"]
mod repo;
#[path = "../../support/round2.rs"]
mod round2;

use repo::repo_root;
use round2::{assert_committed_pins, assert_objects_verify, assert_regenerated_candidates};
use std::path::{Path, PathBuf};
use std::process::Command;

fn gen_candidates_bin() -> PathBuf {
    let profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let root = match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("target"),
    };
    root.join(profile).join("gen-candidates")
}

fn regenerate(dir: &Path) {
    let bin = gen_candidates_bin();
    assert!(
        bin.is_file(),
        "gen-candidates is not built at {}",
        bin.display()
    );
    let status = Command::new(&bin)
        .arg("--out")
        .arg(dir)
        .status()
        .unwrap_or_else(|e| panic!("gen-candidates {}: {e}", bin.display()));
    assert!(status.success(), "gen-candidates --out {}", dir.display());
}

#[test]
fn reference_objects_verify() {
    let root = repo_root();
    assert_committed_pins(&root);
    assert_regenerated_candidates(&root, regenerate);
    assert_objects_verify(&root);
}
