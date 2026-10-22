//! SPEC §16 / Rule I5. `sdk/SPEC_PIN` names one pack manifest.

use crate::hash::sha256;
use crate::hexutil::hex_encode;

/// The pin this SDK was built against. Read from the file; not a second copy of the hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecPin {
    pub tag: String,
    pub vector_sha256: String,
}

/// Read `tag=` and `vector-sha256=` lines. Blank lines and `#` comments are ignored.
pub fn read_pin(text: &str) -> Result<SpecPin, String> {
    let mut tag = None;
    let mut vector = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("tag=") {
            tag = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("vector-sha256=") {
            vector = Some(rest.to_string());
        }
    }
    let tag = tag.ok_or_else(|| "SPEC_PIN has no tag".to_string())?;
    let vector_sha256 = vector.ok_or_else(|| "SPEC_PIN has no vector-sha256".to_string())?;
    Ok(SpecPin { tag, vector_sha256 })
}

/// SHA-256 of the manifest file bytes, lowercase hex.
pub fn manifest_sha256(bytes: &[u8]) -> String {
    hex_encode(&sha256(bytes))
}

/// SPEC Rule I5. The manifest bytes must hash to `vector-sha256`.
pub fn check_pin(pin_text: &str, manifest_bytes: &[u8]) -> Result<SpecPin, String> {
    let pin = read_pin(pin_text)?;
    let got = manifest_sha256(manifest_bytes);
    if got != pin.vector_sha256 {
        return Err(format!(
            "manifest sha256 {got} does not equal vector-sha256 {}",
            pin.vector_sha256
        ));
    }
    Ok(pin)
}
