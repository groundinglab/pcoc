//! A session key for the Windows producer.
//!
//! Ask A4: a public identifier path, plus an optional private key that
//! converts. The private key may be an nsec or 64 hex digits. The public
//! identifier is derived from it. The secret stays in memory and is wiped
//! on drop. `Debug` never prints it. Nothing here writes a key file.

use bech32::{FromBase32, ToBase32, Variant};
use secp256k1::{Keypair, Secp256k1, XOnlyPublicKey};
use std::fmt;
use zeroize::{Zeroize, Zeroizing};

/// Why a public or private key string was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyError {
    /// Not an npub, not 32 bytes of public hex, or not a valid x-only key.
    NotAPublicKey,
    /// Not an nsec and not a valid 32-byte secret scalar.
    NotAPrivateKey,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::NotAPublicKey => f.write_str("This is not a public key."),
            KeyError::NotAPrivateKey => f.write_str("This is not a private key."),
        }
    }
}

/// The 32-byte x-only public key. This is the identifier, not a secret.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicKey(pub [u8; 32]);

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", self.hex())
    }
}

impl PublicKey {
    /// `npub1…`, or 64 hex digits with an optional `0x`.
    pub fn parse(input: &str) -> Result<Self, KeyError> {
        let text = trim_ws(input);
        let bytes = if text.starts_with("npub1") {
            decode_bech32_32("npub", text).ok_or(KeyError::NotAPublicKey)?
        } else {
            let hex = text
                .strip_prefix("0x")
                .or_else(|| text.strip_prefix("0X"))
                .unwrap_or(text);
            crate::hex::decode32(hex).ok_or(KeyError::NotAPublicKey)?
        };
        XOnlyPublicKey::from_slice(&bytes).map_err(|_| KeyError::NotAPublicKey)?;
        Ok(PublicKey(bytes))
    }

    pub fn npub(&self) -> String {
        bech32::encode("npub", self.0.to_base32(), Variant::Bech32).expect("npub encodes")
    }

    pub fn hex(&self) -> String {
        crate::hex::encode(&self.0)
    }
}

/// A secp256k1 scalar in memory. Zeroed on drop. `Debug` never prints it.
pub struct SecretKey {
    bytes: Zeroizing<[u8; 32]>,
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey([redacted])")
    }
}

impl SecretKey {
    /// A fresh key from the OS CSPRNG.
    pub fn generate() -> Self {
        loop {
            let mut bytes = Zeroizing::new([0u8; 32]);
            getrandom::getrandom(bytes.as_mut()).expect("OS randomness");
            if secp256k1::SecretKey::from_slice(bytes.as_ref()).is_ok() {
                return Self { bytes };
            }
        }
    }

    /// An nsec (bech32, HRP `nsec`) or 64 hex digits of the scalar.
    /// Optional `0x`. Whitespace at the ends is ignored.
    pub fn parse(input: &str) -> Result<Self, KeyError> {
        let text = trim_ws(input);
        if text.starts_with("nsec1") {
            return Self::from_nsec(text);
        }
        let hex = text
            .strip_prefix("0x")
            .or_else(|| text.strip_prefix("0X"))
            .unwrap_or(text);
        let bytes = Zeroizing::new(crate::hex::decode32(hex).ok_or(KeyError::NotAPrivateKey)?);
        Self::from_scalar(bytes)
    }

    /// An nsec string (bech32, HRP `nsec`).
    pub fn from_nsec(input: &str) -> Result<Self, KeyError> {
        let text = trim_ws(input);
        let bytes = Zeroizing::new(decode_bech32_32("nsec", text).ok_or(KeyError::NotAPrivateKey)?);
        Self::from_scalar(bytes)
    }

    fn from_scalar(bytes: Zeroizing<[u8; 32]>) -> Result<Self, KeyError> {
        secp256k1::SecretKey::from_slice(bytes.as_ref()).map_err(|_| KeyError::NotAPrivateKey)?;
        Ok(Self { bytes })
    }

    /// The bech32 nsec spelling. For the user to copy. Not stored.
    pub fn to_nsec(&self) -> Zeroizing<String> {
        let mut data = self.bytes.as_ref().to_base32();
        let out = bech32::encode("nsec", &data, Variant::Bech32).expect("nsec encodes");
        for u in data.iter_mut() {
            *u = bech32::u5::try_from_u8(0).unwrap();
        }
        Zeroizing::new(out)
    }

    /// Lowercase hex of the scalar. For convert tests. Not stored. Not logged.
    pub fn to_hex(&self) -> Zeroizing<String> {
        Zeroizing::new(crate::hex::encode(self.bytes.as_ref()))
    }

    /// The public identifier derived from this scalar.
    pub fn public_key(&self) -> PublicKey {
        let secp = Secp256k1::signing_only();
        let mut keypair =
            Keypair::from_seckey_slice(&secp, self.bytes.as_ref()).expect("valid scalar");
        let (xonly, _) = keypair.x_only_public_key();
        keypair.non_secure_erase();
        PublicKey(xonly.serialize())
    }
}

fn trim_ws(input: &str) -> &str {
    input.trim_matches(|c: char| c.is_ascii_whitespace())
}

fn decode_bech32_32(expected_hrp: &str, text: &str) -> Option<[u8; 32]> {
    let (hrp, data, variant) = bech32::decode(text).ok()?;
    if hrp != expected_hrp || variant != Variant::Bech32 {
        return None;
    }
    let mut bytes = Vec::<u8>::from_base32(&data).ok()?;
    let out: Option<[u8; 32]> = bytes.as_slice().try_into().ok();
    bytes.zeroize();
    out
}

/// What the producer may hold for one run.
///
/// A public identifier is enough to name the hike. An optional private key
/// derives that identifier. Config never stores either secret form.
#[derive(Debug)]
pub enum KeyMaterial {
    /// Public identifier only. No scalar is present.
    Public(PublicKey),
    /// Scalar in memory. The identifier is derived, not supplied separately.
    Secret(SecretKey),
}

impl KeyMaterial {
    /// Parse a public identifier (`npub` or hex) or a private key (`nsec` or hex).
    /// A string that is a valid secret is taken as a secret so the public
    /// identifier can be derived. An npub is never a secret.
    pub fn parse(input: &str) -> Result<Self, KeyError> {
        let text = trim_ws(input);
        if text.starts_with("npub1") {
            return Ok(KeyMaterial::Public(PublicKey::parse(text)?));
        }
        if text.starts_with("nsec1") {
            return Ok(KeyMaterial::Secret(SecretKey::from_nsec(text)?));
        }
        if let Ok(secret) = SecretKey::parse(text) {
            return Ok(KeyMaterial::Secret(secret));
        }
        Ok(KeyMaterial::Public(PublicKey::parse(text)?))
    }

    pub fn public_key(&self) -> PublicKey {
        match self {
            KeyMaterial::Public(key) => *key,
            KeyMaterial::Secret(secret) => secret.public_key(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip19_vector::NIP19_NSEC;

    const NIP19_NSEC_HEX: &str = "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa";
    const NIP19_NPUB: &str = "npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg";

    #[test]
    fn generate_then_nsec_and_hex_derive_the_same_public_key() {
        let secret = SecretKey::generate();
        assert_eq!(format!("{secret:?}"), "SecretKey([redacted])");
        let nsec = secret.to_nsec();
        let hex = secret.to_hex();
        let from_nsec = SecretKey::parse(nsec.as_str()).unwrap();
        let from_hex = SecretKey::parse(hex.as_str()).unwrap();
        let from_prefixed = SecretKey::parse(&format!("0x{}", hex.as_str())).unwrap();
        assert_eq!(from_nsec.public_key(), secret.public_key());
        assert_eq!(from_hex.public_key(), secret.public_key());
        assert_eq!(from_prefixed.public_key(), secret.public_key());
        assert_eq!(from_nsec.public_key().hex().len(), 64);
        assert!(from_nsec.public_key().npub().starts_with("npub1"));
    }

    #[test]
    fn nip19_example_converts_between_nsec_and_hex() {
        let secret = SecretKey::parse(NIP19_NSEC).unwrap();
        assert_eq!(secret.to_hex().as_str(), NIP19_NSEC_HEX);
        let from_hex = SecretKey::parse(NIP19_NSEC_HEX).unwrap();
        assert_eq!(from_hex.to_nsec().as_str(), NIP19_NSEC);
        assert_eq!(from_hex.public_key().npub(), NIP19_NPUB);
        assert_eq!(PublicKey::parse(NIP19_NPUB).unwrap(), secret.public_key());
        assert_eq!(
            PublicKey::parse(&secret.public_key().hex()).unwrap(),
            secret.public_key()
        );
    }

    #[test]
    fn public_path_does_not_require_a_secret() {
        let key = PublicKey::parse(NIP19_NPUB).unwrap();
        let material = KeyMaterial::parse(&key.npub()).unwrap();
        assert!(matches!(material, KeyMaterial::Public(_)));
        assert_eq!(material.public_key(), key);
        assert!(SecretKey::parse(NIP19_NPUB).is_err());
        assert!(SecretKey::parse("not a key").is_err());
        assert!(PublicKey::parse(NIP19_NSEC).is_err());
    }

    #[test]
    fn debug_of_key_material_redacts_the_scalar() {
        let material = KeyMaterial::parse(NIP19_NSEC).unwrap();
        let rendered = format!("{material:?}");
        assert!(rendered.contains("redacted"));
        assert!(!rendered.contains("nsec1"));
        assert!(!rendered.contains(NIP19_NSEC_HEX));
    }
}
