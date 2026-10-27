//! The window accepts an nsec only. A hex secret is rejected here, even
//! though the core parser still accepts one for the W1 harness.

use pcoc_windows_core::keys::{KeyError, SecretKey, NSEC_PREFIX};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiKeyError {
    Empty,
    HexSecret,
    PublicKey,
    NotAnNsec,
}

impl UiKeyError {
    pub fn sentence(self) -> &'static str {
        match self {
            UiKeyError::Empty => "Paste an nsec, or press Generate.",
            UiKeyError::HexSecret => "Hex private keys are not accepted. Paste an nsec.",
            UiKeyError::PublicKey => "This is a public key. Paste an nsec.",
            UiKeyError::NotAnNsec => "This is not an nsec. Paste an nsec, or press Generate.",
        }
    }
}

/// Parse the key box. Hex scalars, `0x` hex, and npubs are refused.
pub fn accept_ui_secret(input: &str) -> Result<SecretKey, UiKeyError> {
    let text = input.trim_matches(|c: char| c.is_ascii_whitespace());
    if text.is_empty() {
        return Err(UiKeyError::Empty);
    }
    if text.starts_with("npub1") {
        return Err(UiKeyError::PublicKey);
    }
    if looks_like_hex_secret(text) {
        return Err(UiKeyError::HexSecret);
    }
    if !text.starts_with(NSEC_PREFIX) {
        return Err(UiKeyError::NotAnNsec);
    }
    SecretKey::from_nsec(text).map_err(|err| match err {
        KeyError::NotAPrivateKey | KeyError::NotAPublicKey => UiKeyError::NotAnNsec,
    })
}

fn looks_like_hex_secret(text: &str) -> bool {
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Write the nsec. Callers invoke this only from the Save key control.
pub fn save_key_file(path: &Path, nsec: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, nsec.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_secret_is_rejected_and_nsec_is_accepted() {
        let secret = SecretKey::generate();
        let nsec = secret.to_nsec();
        let hex = secret.to_hex();
        assert_eq!(
            accept_ui_secret(nsec.as_str()).unwrap().public_key(),
            secret.public_key()
        );
        assert_eq!(
            accept_ui_secret(&format!("  {}\n", nsec.as_str()))
                .unwrap()
                .public_key(),
            secret.public_key()
        );
        assert_eq!(
            accept_ui_secret(hex.as_str()).unwrap_err(),
            UiKeyError::HexSecret
        );
        assert_eq!(
            accept_ui_secret(&format!("0x{}", hex.as_str())).unwrap_err(),
            UiKeyError::HexSecret
        );
        assert!(UiKeyError::HexSecret.sentence().contains("not accepted"));
        assert_eq!(accept_ui_secret("").unwrap_err(), UiKeyError::Empty);
        assert_eq!(
            accept_ui_secret(&secret.public_key().npub()).unwrap_err(),
            UiKeyError::PublicKey
        );
        assert_eq!(
            accept_ui_secret("not-a-key").unwrap_err(),
            UiKeyError::NotAnNsec
        );
    }

    #[test]
    fn save_writes_only_when_asked() {
        let dir = std::env::temp_dir().join(format!("pcoc-windows-key-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pcoc.nsec");
        let secret = SecretKey::generate();
        assert!(!path.exists());
        save_key_file(&path, secret.to_nsec().as_str()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            accept_ui_secret(&text).unwrap().public_key(),
            secret.public_key()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
