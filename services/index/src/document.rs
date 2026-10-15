//! Portable-index document this process serves and rebuilds.
//!
//! Same JSON grammar Bari writes (`StoredPortableIndex` in `pcoc-bari`):
//! `{version, network, header, plant}` and `plants` when that array is
//! non-empty. `header.time` and `header.timestamp_unix_ms` are omitted when
//! absent. `plant.tree_size` is optional here: absent decodes as `None` and
//! is omitted on write. Bari always writes it. The verifier does not compare
//! it (specification P14.5).

use serde::{Deserialize, Serialize};

/// `OP_RETURN` `OP_PUSHBYTES_37` payload: `BARI` ‖ version `0x02` ‖ 32-byte root.
pub const BARI_TAG_PREFIX: &[u8] = b"BARI\x02";
/// Whole script: `OP_RETURN` ‖ push-37 ‖ tag ‖ root. 39 bytes.
pub const BARI_SCRIPT_LEN: usize = 39;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDocument {
    pub version: u8,
    pub network: String,
    pub header: IndexHeader,
    pub plant: Option<IndexPlant>,
    /// Every plant included in this block. Omitted when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plants: Vec<IndexPlant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexHeader {
    pub hash: String,
    pub height: u32,
    /// Bitcoin header `nTime`, unix seconds. Omitted when not read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<u64>,
    /// Bari's wall-clock observation. Off-chain. Omitted when absent.
    /// Rebuild does not write this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexPlant {
    pub root: String,
    /// Leaf count of the Merkle tree. Off-chain: the 39-byte tag carries the
    /// root and not the size. Absent on a chain rebuild. Omitted when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_size: Option<u32>,
    pub txid: String,
    pub vout: u32,
    pub block_hash: String,
    pub block_height: u32,
}

impl IndexDocument {
    /// Compact JSON, the bytes `Json<T>` serves and `GET /index/since` lines use.
    pub fn to_json_vec(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
}

/// `6a25` ‖ `BARI` ‖ `02` ‖ root. Returns the root hex (natural order) or `None`.
pub fn bari_root_from_script(script: &[u8]) -> Option<String> {
    if script.len() != BARI_SCRIPT_LEN {
        return None;
    }
    if script[0] != 0x6a || script[1] != 0x25 {
        return None;
    }
    if &script[2..7] != BARI_TAG_PREFIX {
        return None;
    }
    Some(hex_encode(&script[7..39]))
}

/// Hex form of [`bari_root_from_script`].
pub fn bari_root_hex(script_hex: &str) -> Option<String> {
    let bytes = decode_hex(script_hex)?;
    bari_root_from_script(&bytes)
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Some(out)
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_size_absent_round_trips_and_is_omitted() {
        let raw = r#"{"version":2,"network":"main","header":{"hash":"aa","height":1,"time":9},"plant":{"root":"bb","txid":"cc","vout":1,"block_hash":"dd","block_height":1},"plants":[{"root":"bb","txid":"cc","vout":1,"block_hash":"dd","block_height":1}]}"#;
        let doc: IndexDocument = serde_json::from_str(raw).unwrap();
        assert!(doc.plant.as_ref().unwrap().tree_size.is_none());
        assert!(doc.plants[0].tree_size.is_none());
        let again = serde_json::to_string(&doc).unwrap();
        assert!(!again.contains("tree_size"));
        assert!(!again.contains("timestamp_unix_ms"));
        let parsed: IndexDocument = serde_json::from_str(&again).unwrap();
        assert_eq!(parsed, doc);
    }

    #[test]
    fn tree_size_present_is_kept() {
        let raw = r#"{"version":2,"network":"main","header":{"hash":"aa","height":1},"plant":null,"plants":[{"root":"bb","tree_size":2,"txid":"cc","vout":0,"block_hash":"dd","block_height":1}]}"#;
        let doc: IndexDocument = serde_json::from_str(raw).unwrap();
        assert_eq!(doc.plants[0].tree_size, Some(2));
        let again = serde_json::to_string(&doc).unwrap();
        assert!(again.contains("\"tree_size\":2"));
    }

    #[test]
    fn bari_tag_is_the_39_byte_script() {
        let root = "ab".repeat(32);
        let script = format!("6a254241524902{root}");
        assert_eq!(script.len(), 78);
        assert_eq!(bari_root_hex(&script).as_deref(), Some(root.as_str()));
        assert!(bari_root_hex("6a254241524901aabb").is_none());
        assert!(bari_root_hex(&script[..script.len() - 2]).is_none());
    }
}
