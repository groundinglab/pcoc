//! Waypoint accept and path serving.
//!
//! `POST /waypoint` and `GET /path/{leaf}` are this process.
//! `GET /index` and `GET /index/{height}` are not this process.
//! This process writes the index cache and does not serve those GETs.
//! Accept and seal rules are `services/bari/pool-handoff/REQUIREMENTS.md`.

#![deny(dead_code)]

pub mod bitcoin_rpc;
pub mod clock;
mod fee;
mod gossip;
pub mod hashcash;
pub mod header_source;
pub mod http;
pub mod merkle;
pub mod operator;
pub(crate) mod ops;
pub mod pool;
pub mod proof;
pub mod storage;
pub mod synth_commit;
mod utxo;

pub type Hash32 = [u8; 32];

/// Production admission boundary. Heights below this are testing.
///
/// Equal to `born_height` in `policy/groundinglab-admission/gl-admit-v10.toml`.
/// It is policy, not a SPEC rule. This process does not serve a path below it.
/// An index blob is served down through `born_height - MTP_LOOKBACK`.
/// Files already stored stay where they are.
pub const BORN_HEIGHT: u32 = 972685;

/// Headers this process still returns below [`BORN_HEIGHT`].
///
/// Equal to [`pcoc::constants::MTP_LOOKBACK`], which is
/// [`pcoc::constants::MTP_WINDOW`] minus one. These headers are not
/// production heights and are not an input to `hike_span`.
pub const MTP_LOOKBACK: u32 = pcoc::constants::MTP_LOOKBACK as u32;

/// Decode exactly 32 natural-order bytes from lowercase hexadecimal.
pub fn decode_hash32(encoded: &str) -> Result<Hash32, String> {
    if encoded.len() != 64
        || encoded
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err("expected exactly 64 lowercase hexadecimal characters".to_owned());
    }
    let decoded = hex::decode(encoded).map_err(|error| error.to_string())?;
    decoded
        .try_into()
        .map_err(|_| "expected exactly 32 decoded bytes".to_owned())
}

pub fn encode_hash32(value: &Hash32) -> String {
    hex::encode(value)
}

/// Render a Bitcoin transaction or block identifier in the conventional
/// Bitcoin Core/explorer display order. Carried and persisted bytes remain in
/// their natural order; only this presentation boundary reverses them.
pub fn encode_bitcoin_id(value: &Hash32) -> String {
    let mut displayed = *value;
    displayed.reverse();
    hex::encode(displayed)
}

/// Inverse of [`encode_bitcoin_id`]. Display order in, natural order out.
pub fn decode_bitcoin_id(display: &str) -> Result<Hash32, String> {
    if display.len() != 64 || !display.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("expected 64 hexadecimal characters".to_owned());
    }
    let mut decoded = hex::decode(display).map_err(|error| error.to_string())?;
    decoded.reverse();
    decoded
        .try_into()
        .map_err(|_| "expected exactly 32 decoded bytes".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bitcoin_identifier_presentation_reverses_natural_bytes() {
        let natural: Hash32 = std::array::from_fn(|index| index as u8);
        assert_eq!(
            encode_hash32(&natural),
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        );
        assert_eq!(
            encode_bitcoin_id(&natural),
            "1f1e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100"
        );
        assert_eq!(
            decode_bitcoin_id(&encode_bitcoin_id(&natural)).unwrap(),
            natural
        );
    }
}
