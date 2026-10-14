//! An ephemeral nsec.
//!
//! The user pastes an `nsec1…` string or generates one. The secret stays in
//! memory and is wiped on drop. The npub and the hex public key are derived
//! from it. Nothing here writes a key file, a password, or an ncryptsec.

use bech32::{FromBase32, ToBase32, Variant};
use secp256k1::{Keypair, Message, Secp256k1, XOnlyPublicKey};
use std::fmt;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyError {
    NotAnNsec,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("This is not an nsec.")
    }
}

/// The 32-byte x-only public key.
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
        let text = input.trim_matches(|c: char| c.is_ascii_whitespace());
        let bytes = if text.starts_with("npub1") {
            decode_bech32_32("npub", text).ok_or(KeyError::NotAnNsec)?
        } else {
            let hex = text
                .strip_prefix("0x")
                .or_else(|| text.strip_prefix("0X"))
                .unwrap_or(text);
            crate::hex::decode32(hex).ok_or(KeyError::NotAnNsec)?
        };
        XOnlyPublicKey::from_slice(&bytes).map_err(|_| KeyError::NotAnNsec)?;
        Ok(PublicKey(bytes))
    }

    pub fn npub(&self) -> String {
        bech32::encode("npub", self.0.to_base32(), Variant::Bech32).expect("npub encodes")
    }

    pub fn hex(&self) -> String {
        crate::hex::encode(&self.0)
    }

    /// First ten characters, an ellipsis, and the last six. For the header.
    pub fn short_npub(&self) -> String {
        let npub = self.npub();
        format!("{}…{}", &npub[..10], &npub[npub.len() - 6..])
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

    /// An `nsec1…` string. Whitespace at the ends is ignored.
    pub fn from_nsec(input: &str) -> Result<Self, KeyError> {
        let text = input.trim_matches(|c: char| c.is_ascii_whitespace());
        let bytes = Zeroizing::new(decode_bech32_32("nsec", text).ok_or(KeyError::NotAnNsec)?);
        Self::from_scalar(bytes)
    }

    /// A 32-byte scalar. Refused when it is not a valid secret key.
    pub(crate) fn from_scalar(bytes: Zeroizing<[u8; 32]>) -> Result<Self, KeyError> {
        secp256k1::SecretKey::from_slice(bytes.as_ref()).map_err(|_| KeyError::NotAnNsec)?;
        Ok(Self { bytes })
    }

    /// The `nsec1…` spelling. Shown so the user can copy it. Not stored.
    pub fn to_nsec(&self) -> Zeroizing<String> {
        let mut data = self.bytes.as_ref().to_base32();
        let out = bech32::encode("nsec", &data, Variant::Bech32).expect("nsec encodes");
        for u in data.iter_mut() {
            *u = bech32::u5::try_from_u8(0).unwrap();
        }
        Zeroizing::new(out)
    }

    pub fn public_key(&self) -> PublicKey {
        let secp = Secp256k1::signing_only();
        let mut keypair =
            Keypair::from_seckey_slice(&secp, self.bytes.as_ref()).expect("valid scalar");
        let (xonly, _) = keypair.x_only_public_key();
        keypair.non_secure_erase();
        PublicKey(xonly.serialize())
    }

    /// BIP-340 over a 32-byte message. The message is signed as given.
    /// It is not hashed again.
    pub fn sign_digest(&self, message: &[u8; 32], aux: &[u8; 32]) -> [u8; 64] {
        let secp = Secp256k1::signing_only();
        let mut keypair =
            Keypair::from_seckey_slice(&secp, self.bytes.as_ref()).expect("valid scalar");
        let sig = secp.sign_schnorr_with_aux_rand(&Message::from_digest(*message), &keypair, aux);
        keypair.non_secure_erase();
        *sig.as_ref()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const NIP19_NSEC: &str = "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5";
    const NIP19_NSEC_HEX: &str = "67dea2ed018072d675f5415ecfaed7d2597555e202d85b3d65ea4e58d2d92ffa";
    const NIP19_NPUB: &str = "npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg";

    #[test]
    fn nsec_round_trips_and_derives_the_npub() {
        let secret = SecretKey::from_nsec(NIP19_NSEC).unwrap();
        assert_eq!(crate::hex::encode(secret.bytes.as_ref()), NIP19_NSEC_HEX);
        assert_eq!(secret.to_nsec().as_str(), NIP19_NSEC);
        assert_eq!(format!("{secret:?}"), "SecretKey([redacted])");
        let again = SecretKey::from_nsec(&format!("  {NIP19_NSEC}\n")).unwrap();
        assert_eq!(again.public_key(), secret.public_key());
        let npub = secret.public_key().npub();
        assert!(npub.starts_with("npub1"));
        assert_eq!(secret.public_key().hex().len(), 64);
        assert_eq!(npub.chars().take(10).count(), 10);
        assert!(secret.public_key().short_npub().starts_with(&npub[..10]));
    }

    #[test]
    fn generated_key_derives_the_same_public_key_from_its_nsec() {
        let secret = SecretKey::generate();
        let nsec = secret.to_nsec();
        let opened = SecretKey::from_nsec(&nsec).unwrap();
        assert_eq!(opened.public_key(), secret.public_key());
        assert!(!nsec.contains(&secret.public_key().hex()));
    }

    #[test]
    fn a_bad_nsec_is_refused() {
        assert!(SecretKey::from_nsec("").is_err());
        assert!(SecretKey::from_nsec("not a key").is_err());
        assert!(SecretKey::from_nsec(NIP19_NSEC_HEX).is_err());
        assert!(SecretKey::from_nsec(NIP19_NPUB).is_err());
        assert_eq!(
            SecretKey::from_nsec("nsec1qqqq").unwrap_err().to_string(),
            "This is not an nsec."
        );
        let mut broken = NIP19_NSEC.to_owned();
        broken.pop();
        broken.push('7');
        assert!(SecretKey::from_nsec(&broken).is_err());
    }

    #[test]
    fn npub_parse_accepts_bech32_and_hex() {
        let key = PublicKey::parse(NIP19_NPUB).unwrap();
        assert_eq!(PublicKey::parse(&key.hex()).unwrap(), key);
        assert_eq!(PublicKey::parse(&format!("0x{}", key.hex())).unwrap(), key);
        assert!(PublicKey::parse(NIP19_NSEC).is_err());
        assert!(PublicKey::parse("npub1qqqq").is_err());
    }
}
