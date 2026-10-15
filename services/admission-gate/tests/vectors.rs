//! The vectors under `tests/vectors/` are pinned copies of `spec/conformance/v2.0/`.
//! Each file must match its line in `tests/vectors/SHA256SUMS`, and every
//! file in that directory must be listed.

#![forbid(unsafe_code)]

use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn pinned_vectors_match_their_hashes() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors");
    let sums = std::fs::read_to_string(dir.join("SHA256SUMS")).expect("SHA256SUMS");
    let mut listed = BTreeSet::new();
    for line in sums.lines().filter(|line| !line.trim().is_empty()) {
        let (hash, name) = line.split_once("  ").expect("`<sha256>  <file>` line");
        let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|err| panic!("{name}: {err}"));
        assert_eq!(hex::encode(Sha256::digest(&bytes)), hash, "{name} changed");
        listed.insert(name.to_owned());
    }
    let present: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name != "SHA256SUMS")
        .collect();
    assert_eq!(present, listed, "tests/vectors and SHA256SUMS disagree");
}
