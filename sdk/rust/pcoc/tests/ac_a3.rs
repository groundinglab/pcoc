//! AC-A3. `sdk/SPEC_PIN` `vector-sha256` is the SHA-256 of the pack manifest.
//! The manifest is read from disk, so a byte flip fails without a rebuild.

#[path = "../../support/repo.rs"]
mod repo;

use repo::repo_root;

#[test]
fn spec_pin_matches_manifest() {
    let root = repo_root();
    let pin = std::fs::read_to_string(root.join("sdk/SPEC_PIN")).expect("SPEC_PIN");
    let manifest =
        std::fs::read(root.join("spec/conformance/pack/manifest.json")).expect("manifest");
    pcoc::pin::check_pin(&pin, &manifest).expect("vector-sha256");
}
