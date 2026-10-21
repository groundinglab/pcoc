//! DEMO key derivation for the desktop POC.
//!
//! Pins, not a Lock amend of the hike:
//! BIP-39 English, 12 words, empty passphrase only.
//! Path `m/44'/1237'/0'/0/0` (NIP-06, account 0).
//! secp256k1 BIP-340 x-only `public_key`.
//! `nsec` / `npub` are NIP-19 bech32 clothing of those 32-byte strings.
//!
//! This crate does not read or write a hike object. The hike's
//! `production_seed` is terrain (`public_key` concatenated with the seed
//! beacon). It is not this key.
#![forbid(unsafe_code)]

use bech32::ToBase32;
use bip39::{Language, Mnemonic};
use hmac::{Hmac, Mac};
use secp256k1::{Keypair, Message, PublicKey, Scalar, Secp256k1, SecretKey, XOnlyPublicKey};
use serde::Serialize;
use sha2::Sha512;
use zeroize::Zeroize;

/// Published NIP-06 12-word vector. Empty passphrase. Account 0.
pub const VECTOR_MNEMONIC: &str =
    "leader monkey parrot ring guide accident before fence cannon height naive bean";
pub const VECTOR_SECRET_HEX: &str =
    "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a";
pub const VECTOR_PUBLIC_HEX: &str =
    "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917";
#[path = "vectors-allowlist/nip06_vector.rs"]
mod nip06_vector;
pub use nip06_vector::VECTOR_NSEC;
pub const VECTOR_NPUB: &str = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";

pub const PATH: &str = "m/44'/1237'/0'/0/0";
pub const DEMO_MNEMONIC_SCHEMA: &str = "pcoc-demo-mnemonic-v0";

const HARDENED: u32 = 0x8000_0000;
const PATH_INDEXES: [u32; 5] = [44 | HARDENED, 1237 | HARDENED, 0 | HARDENED, 0, 0];

type HmacSha512 = Hmac<Sha512>;

/// A demo identity. The mnemonic, secret and nsec are zeroed on drop.
///
/// The secret parts (`mnemonic`, `secret`, `nsec`) are private fields. Read them
/// only through [`DemoKey::mnemonic`], [`DemoKey::secret_key`] and [`DemoKey::nsec`].
/// `Debug` prints the public key and npub only. There is no `Display`.
pub struct DemoKey {
    mnemonic: String,
    secret: [u8; 32],
    pub public_key: [u8; 32],
    nsec: String,
    pub npub: String,
}

impl Clone for DemoKey {
    fn clone(&self) -> Self {
        Self {
            mnemonic: self.mnemonic.clone(),
            secret: self.secret,
            public_key: self.public_key,
            nsec: self.nsec.clone(),
            npub: self.npub.clone(),
        }
    }
}

impl Drop for DemoKey {
    fn drop(&mut self) {
        self.mnemonic.zeroize();
        self.secret.zeroize();
        self.nsec.zeroize();
    }
}

impl std::fmt::Debug for DemoKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DemoKey")
            .field("public_key", &encode_hex(&self.public_key))
            .field("npub", &self.npub)
            .finish()
    }
}

impl DemoKey {
    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    pub fn secret_key(&self) -> &[u8; 32] {
        &self.secret
    }

    /// NIP-19 `nsec` spelling of the secret key. This is secret material: do not log it.
    pub fn nsec(&self) -> &str {
        &self.nsec
    }

    pub fn public_key_hex(&self) -> String {
        encode_hex(&self.public_key)
    }
}

/// Twelve English words, empty passphrase, NIP-06 account 0.
pub fn from_mnemonic(phrase: &str) -> Result<DemoKey, String> {
    let words: Vec<&str> = phrase.split_whitespace().collect();
    if words.len() != 12 {
        return Err(
            "DEMO mnemonic must be exactly 12 English BIP-39 words; the passphrase is empty"
                .to_owned(),
        );
    }
    let joined = words.join(" ");
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, &joined)
        .map_err(|err| format!("DEMO mnemonic was not accepted as English BIP-39 ({err})"))?;
    let seed = mnemonic.to_seed("");
    let secret = derive_nip06(&seed)?;
    Ok(dress(mnemonic.to_string(), secret))
}

/// Draw 128 bits from the OS CSPRNG and encode a 12-word mnemonic.
pub fn create_demo_key() -> Result<DemoKey, String> {
    let mut entropy = [0u8; 16];
    getrandom::getrandom(&mut entropy).map_err(|err| format!("OS CSPRNG: {err}"))?;
    let mnemonic = Mnemonic::from_entropy(&entropy).map_err(|err| format!("BIP-39: {err}"))?;
    let phrase = mnemonic.to_string();
    from_mnemonic(&phrase)
}

/// A fresh random secp256k1 key. The secret is not a mnemonic and has no `nsec` spelling.
///
/// Dropping the value zeroes the secret. Callers may write [`FreshTestKey::secret_bytes`]
/// to a mode `0600` file. They must not print it.
pub struct FreshTestKey {
    secret: zeroize::Zeroizing<[u8; 32]>,
    public_key: [u8; 32],
}

impl FreshTestKey {
    pub fn secret_bytes(&self) -> &[u8; 32] {
        &self.secret
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.public_key
    }

    pub fn public_key_hex(&self) -> String {
        encode_hex(&self.public_key)
    }
}

/// Draw a new secret from the OS CSPRNG. Not the demo mnemonic.
pub fn fresh_test_key() -> Result<FreshTestKey, String> {
    let secp = Secp256k1::new();
    for _ in 0..64 {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).map_err(|err| format!("OS CSPRNG: {err}"))?;
        if let Ok(key) = SecretKey::from_slice(&bytes) {
            let keypair = Keypair::from_secret_key(&secp, &key);
            let public_key = keypair.x_only_public_key().0.serialize();
            bytes.zeroize();
            let mut secret = [0u8; 32];
            secret.copy_from_slice(&key.secret_bytes());
            return Ok(FreshTestKey {
                secret: zeroize::Zeroizing::new(secret),
                public_key,
            });
        }
        bytes.zeroize();
    }
    Err("could not draw a secret key".to_owned())
}

pub fn random_32() -> Result<[u8; 32], String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|err| format!("OS CSPRNG: {err}"))?;
    Ok(bytes)
}

/// BIP-340 Schnorr over `message` as the 32-byte digest.
///
/// `Message::from_digest` does not hash `message`. Callers that already hold
/// a digest must not hash it again before this function.
pub fn sign_bip340_message(
    secret: &[u8; 32],
    message: &[u8; 32],
    aux: [u8; 32],
) -> Result<[u8; 64], String> {
    let secp = Secp256k1::new();
    let key =
        SecretKey::from_slice(secret).map_err(|_| "secret key was not accepted".to_owned())?;
    let keypair = Keypair::from_secret_key(&secp, &key);
    let message = Message::from_digest(*message);
    let signature = secp.sign_schnorr_with_aux_rand(&message, &keypair, &aux);
    Ok(signature.serialize())
}

/// Verify a BIP-340 signature over `message` as the 32-byte digest.
pub fn verify_bip340_message(
    public_key: &[u8; 32],
    message: &[u8; 32],
    signature: &[u8; 64],
) -> bool {
    let secp = Secp256k1::new();
    let Ok(xonly) = XOnlyPublicKey::from_slice(public_key) else {
        return false;
    };
    let Ok(signature) = secp256k1::schnorr::Signature::from_slice(signature) else {
        return false;
    };
    let message = Message::from_digest(*message);
    secp.verify_schnorr(&signature, &message, &xonly).is_ok()
}

/// stdout document for `--demo-mnemonic` with no produce output path.
pub fn demo_mnemonic_report(key: &DemoKey) -> String {
    let doc = DemoMnemonicDoc {
        schema: DEMO_MNEMONIC_SCHEMA,
        public_key: key.public_key_hex(),
        npub: key.npub.clone(),
    };
    let mut bytes = serde_json::to_vec(&doc).expect("demo mnemonic json");
    bytes.push(b'\n');
    String::from_utf8(bytes).expect("utf-8")
}

pub fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

pub fn decode_hex32(spelling: &str) -> Result<[u8; 32], String> {
    let spelling = spelling
        .trim()
        .strip_prefix("0x")
        .unwrap_or(spelling.trim());
    let bytes = decode_hex(spelling)?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| "expected 32 bytes of hex".to_owned())
}

/// NIP-19 human-readable part of a secret key.
const NSEC_HRP: &str = "nsec";

fn dress(mnemonic: String, secret: [u8; 32]) -> DemoKey {
    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(&secret).expect("derived secret is on the curve");
    let keypair = Keypair::from_secret_key(&secp, &key);
    let (xonly, _) = XOnlyPublicKey::from_keypair(&keypair);
    let public_key = xonly.serialize();
    DemoKey {
        nsec: nip19(NSEC_HRP, &secret),
        npub: nip19("npub", &public_key),
        mnemonic,
        secret,
        public_key,
    }
}

fn derive_nip06(seed: &[u8; 64]) -> Result<[u8; 32], String> {
    let i = hmac_sha512(b"Bitcoin seed", seed);
    let mut secret = SecretKey::from_slice(&i[..32]).map_err(|_| "BIP-32 master key invalid")?;
    let mut chain = [0u8; 32];
    chain.copy_from_slice(&i[32..]);
    for index in PATH_INDEXES {
        let (child, next_chain) = ckd(secret, chain, index)?;
        secret = child;
        chain = next_chain;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(secret.as_ref());
    Ok(out)
}

fn ckd(key: SecretKey, chain: [u8; 32], index: u32) -> Result<(SecretKey, [u8; 32]), String> {
    let secp = Secp256k1::new();
    let mut data = Vec::with_capacity(37);
    if index >= HARDENED {
        data.push(0x00);
        data.extend_from_slice(key.as_ref());
    } else {
        let public = PublicKey::from_secret_key(&secp, &key);
        data.extend_from_slice(&public.serialize());
    }
    data.extend_from_slice(&index.to_be_bytes());
    let i = hmac_sha512(&chain, &data);
    let mut il = [0u8; 32];
    il.copy_from_slice(&i[..32]);
    let tweak = Scalar::from_be_bytes(il)
        .map_err(|_| format!("BIP-32 child tweak out of range at index {index}"))?;
    let child = key
        .add_tweak(&tweak)
        .map_err(|_| format!("BIP-32 child key invalid at index {index}"))?;
    let mut next = [0u8; 32];
    next.copy_from_slice(&i[32..]);
    Ok((child, next))
}

fn hmac_sha512(key: &[u8], data: &[u8]) -> [u8; 64] {
    let mut mac = HmacSha512::new_from_slice(key).expect("hmac accepts any key length");
    mac.update(data);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 64];
    out.copy_from_slice(&bytes);
    out
}

pub fn npub_of(public_key: &[u8; 32]) -> String {
    nip19("npub", public_key)
}

/// x-only public key of a 32-byte secret. `Err` when the secret is not on the curve.
pub fn public_key_of_secret(secret: &[u8; 32]) -> Result<[u8; 32], String> {
    let secp = Secp256k1::new();
    let key = SecretKey::from_slice(secret).map_err(|_| "secret key was not accepted".to_owned())?;
    let keypair = Keypair::from_secret_key(&secp, &key);
    Ok(keypair.x_only_public_key().0.serialize())
}

fn nip19(hrp: &str, bytes: &[u8; 32]) -> String {
    bech32::encode(hrp, bytes.to_base32(), bech32::Variant::Bech32).expect("nip-19 bech32")
}

fn decode_hex(spelling: &str) -> Result<Vec<u8>, String> {
    if spelling.is_empty() || spelling.len() % 2 != 0 {
        return Err("hex length must be even and non-empty".to_owned());
    }
    if !spelling.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("hex digits only".to_owned());
    }
    let mut out = Vec::with_capacity(spelling.len() / 2);
    let bytes = spelling.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = (bytes[i] as char).to_digit(16).unwrap();
        let lo = (bytes[i + 1] as char).to_digit(16).unwrap();
        out.push(((hi << 4) | lo) as u8);
    }
    Ok(out)
}

#[derive(Serialize)]
struct DemoMnemonicDoc {
    schema: &'static str,
    public_key: String,
    npub: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_test_key_is_a_new_public_key() {
        let first = fresh_test_key().unwrap();
        let second = fresh_test_key().unwrap();
        assert_ne!(first.public_key(), second.public_key());
        assert_eq!(first.public_key_hex().len(), 64);
        assert_ne!(first.secret_bytes(), second.secret_bytes());
    }

    #[test]
    fn gold_vector_matches_nip06() {
        let key = from_mnemonic(VECTOR_MNEMONIC).unwrap();
        assert_eq!(key.public_key_hex(), VECTOR_PUBLIC_HEX);
        assert_eq!(encode_hex(key.secret_key()), VECTOR_SECRET_HEX);
        assert_eq!(key.nsec(), VECTOR_NSEC);
        assert_eq!(key.npub, VECTOR_NPUB);
        assert_eq!(key.mnemonic(), VECTOR_MNEMONIC);
    }

    #[test]
    fn twenty_four_words_are_refused() {
        let phrase = "what bleak badge arrange retreat wolf trade produce cricket blur garlic valid proud rude strong choose busy staff weather area salt hollow arm fade";
        let err = from_mnemonic(phrase).unwrap_err();
        assert!(err.contains("12"));
    }

    #[test]
    fn bad_checksum_is_refused() {
        let phrase =
            "leader monkey parrot ring guide accident before fence cannon height naive abandon";
        assert!(from_mnemonic(phrase).is_err());
    }

    #[test]
    fn bip340_signs_the_32_byte_message_and_does_not_hash_it_again() {
        use sha2::{Digest, Sha256};

        let key = from_mnemonic(VECTOR_MNEMONIC).unwrap();
        let message = [0x42u8; 32];
        let signature = sign_bip340_message(key.secret_key(), &message, [7u8; 32]).unwrap();
        assert!(verify_bip340_message(&key.public_key, &message, &signature));
        let mut hashed = [0u8; 32];
        hashed.copy_from_slice(&Sha256::digest(message));
        assert!(!verify_bip340_message(&key.public_key, &hashed, &signature));
    }

    #[test]
    fn demo_report_has_the_xonly_and_not_the_secret() {
        let key = from_mnemonic(VECTOR_MNEMONIC).unwrap();
        let report = demo_mnemonic_report(&key);
        assert!(report.contains(VECTOR_PUBLIC_HEX));
        assert!(report.contains(VECTOR_NPUB));
        assert!(!report.contains(VECTOR_SECRET_HEX));
        assert!(!report.contains(VECTOR_NSEC));
        assert!(report.ends_with('\n'));
    }

    /// `DemoKey` must not implement `Display` (inherent-const probe: the inherent
    /// const only exists when `T: Display`, otherwise the trait default is used).
    #[test]
    fn demo_key_has_no_display() {
        use std::marker::PhantomData;
        trait NoDisplay {
            const IMPLS_DISPLAY: bool = false;
        }
        struct Probe<T: ?Sized>(PhantomData<T>);
        impl<T: ?Sized> NoDisplay for Probe<T> {}
        #[allow(dead_code)]
        impl<T: ?Sized + std::fmt::Display> Probe<T> {
            const IMPLS_DISPLAY: bool = true;
        }
        // Evaluated at compile time: a Display impl on DemoKey fails the test build.
        const { assert!(<Probe<String>>::IMPLS_DISPLAY, "probe self-check") };
        const {
            assert!(
                !<Probe<DemoKey>>::IMPLS_DISPLAY,
                "DemoKey must not implement Display"
            )
        };
    }

    #[test]
    fn debug_output_contains_no_secret() {
        let key = from_mnemonic(VECTOR_MNEMONIC).unwrap();
        for shown in [
            format!("{key:?}"),
            format!("{key:#?}"),
            format!("{:?}", key.clone()),
        ] {
            assert!(shown.contains(VECTOR_NPUB), "{shown}");
            assert!(!shown.contains(VECTOR_NSEC), "nsec leaked: {shown}");
            assert!(
                !shown.contains(VECTOR_SECRET_HEX),
                "secret hex leaked: {shown}"
            );
            assert!(!shown.contains(VECTOR_MNEMONIC), "mnemonic leaked: {shown}");
            for word in VECTOR_MNEMONIC.split_whitespace() {
                assert!(
                    !shown.contains(word),
                    "mnemonic word {word} leaked: {shown}"
                );
            }
            assert!(
                !shown.contains(&format!("{NSEC_HRP}1")),
                "nsec prefix leaked: {shown}"
            );
        }
    }
}
