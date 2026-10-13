//! Compact JSON for the decision log.
//!
//! Log lines are not RFC 8785. Their key order is the order the encoder writes.
//! `jcs` remains for the draft vectors checked below. The hash domain
//! `pcoc-gate-log-v1` is frozen. The feed id is `pcoc-gate-feed-v2`. This
//! module does not escape `/`, does not escape U+007F, and writes U+0000
//! through U+001F as `\u00xx` with lowercase hex.

use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

pub const GENESIS_HEX: &str = "b927019a3b2ca1a67b5e7d3a09bd14f5ba2b762abccd3c6ab2da3dbeafd4e671";
pub const LOG_DOMAIN: &[u8] = b"pcoc-gate-log-v1\n";
pub const MAX_JSON_INT: u64 = (1 << 53) - 1;

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn hex32(bytes: &[u8; 32]) -> String {
    hex::encode(bytes)
}

/// SHA-256 of `pcoc-gate-log-v1 genesis` plus one newline. No key.
pub fn genesis_hash() -> [u8; 32] {
    sha256(b"pcoc-gate-log-v1 genesis\n")
}

/// SHA-256 of the domain, a newline, and the line bytes. The separator
/// newline after the decision object is not part of `line_bytes`. A section
/// 14 record's newline inside `verify_summary` is.
pub fn event_hash(line_bytes: &[u8]) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(LOG_DOMAIN.len() + line_bytes.len());
    preimage.extend_from_slice(LOG_DOMAIN);
    preimage.extend_from_slice(line_bytes);
    sha256(&preimage)
}

/// RFC 8785 for the JSON values this door publishes. Keys sort by UTF-16 code
/// units. A shorter key that is a prefix of a longer key comes first.
pub fn jcs(value: &Value) -> Result<Vec<u8>, String> {
    let mut out = String::new();
    write_jcs(value, &mut out)?;
    Ok(out.into_bytes())
}

fn write_jcs(value: &Value, out: &mut String) -> Result<(), String> {
    match value {
        Value::Null => Err("JSON null is not a canonical value".to_owned()),
        Value::Bool(true) => {
            out.push_str("true");
            Ok(())
        }
        Value::Bool(false) => {
            out.push_str("false");
            Ok(())
        }
        Value::Number(number) => {
            out.push_str(&canonical_number(number)?);
            Ok(())
        }
        Value::String(text) => {
            out.push_str(&escape(text));
            Ok(())
        }
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_jcs(item, out)?;
            }
            out.push(']');
            Ok(())
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|left, right| utf16_units(left).cmp(&utf16_units(right)));
            out.push('{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&escape(key));
                out.push(':');
                write_jcs(&map[*key], out)?;
            }
            out.push('}');
            Ok(())
        }
    }
}

fn utf16_units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn canonical_number(number: &Number) -> Result<String, String> {
    if let Some(value) = number.as_u64() {
        return Ok(value.to_string());
    }
    if let Some(value) = number.as_i64() {
        if value == 0 {
            return Err("negative zero is not a canonical number".to_owned());
        }
        return Ok(value.to_string());
    }
    Err(format!("a fractional number is not canonical: {number}"))
}

pub fn escape(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) <= 0x1f => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Compact object. Keys stay in insertion order. This is the log-line encoding.
pub fn compact_object(fields: &[(String, Value)]) -> Result<Vec<u8>, String> {
    let mut map = Map::new();
    for (key, value) in fields {
        if value.is_null() {
            return Err(format!("log field {key} is null"));
        }
        map.insert(key.clone(), value.clone());
    }
    serde_json::to_vec(&Value::Object(map)).map_err(|err| err.to_string())
}

pub fn in_json_bound(value: u64) -> bool {
    value <= MAX_JSON_INT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_matches_the_locked_pin() {
        assert_eq!(hex::encode(genesis_hash()), GENESIS_HEX);
    }
}
