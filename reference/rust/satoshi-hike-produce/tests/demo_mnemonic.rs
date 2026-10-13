//! Gold check: `--demo-mnemonic` prints the same x-only key as `nip06`.
#![forbid(unsafe_code)]

use std::process::Command;

#[test]
fn demo_mnemonic_matches_the_nip06_vector() {
    let bin = env!("CARGO_BIN_EXE_satoshi-hike-produce");
    let output = Command::new(bin)
        .args(["--demo-mnemonic", nip06::VECTOR_MNEMONIC])
        .output()
        .expect("spawn produce");
    assert!(
        output.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["schema"], nip06::DEMO_MNEMONIC_SCHEMA);
    assert_eq!(doc["public_key"], nip06::VECTOR_PUBLIC_HEX);
    assert_eq!(doc["npub"], nip06::VECTOR_NPUB);
    assert!(!stdout.contains(nip06::VECTOR_SECRET_HEX));
    assert!(!stdout.contains(nip06::VECTOR_NSEC));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("DEMO ONLY"));
    assert!(stderr.contains(nip06::VECTOR_PUBLIC_HEX));
    assert!(!stderr.contains(nip06::VECTOR_SECRET_HEX));
}

#[test]
fn demo_mnemonic_with_pubkey_hex_is_a_usage_error() {
    let bin = env!("CARGO_BIN_EXE_satoshi-hike-produce");
    let output = Command::new(bin)
        .args([
            "--demo-mnemonic",
            nip06::VECTOR_MNEMONIC,
            "--pubkey-hex",
            nip06::VECTOR_PUBLIC_HEX,
            "--output",
            "out.cbor",
        ])
        .output()
        .expect("spawn produce");
    assert_eq!(output.status.code(), Some(2));
}
