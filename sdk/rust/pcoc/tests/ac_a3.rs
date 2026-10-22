//! AC-A3. `sdk/SPEC_PIN` `vector-sha256` is the SHA-256 of the pack manifest.
//! The manifest is read from disk, so a byte flip fails without a rebuild.

use std::path::PathBuf;

#[test]
fn spec_pin_matches_manifest() {
    let sdk = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pin = std::fs::read_to_string(sdk.join("SPEC_PIN")).expect("SPEC_PIN");
    let manifest = std::fs::read(sdk.join("../spec/conformance/pack/manifest.json")).expect("manifest");
    pcoc::pin::check_pin(&pin, &manifest).expect("vector-sha256");
}
