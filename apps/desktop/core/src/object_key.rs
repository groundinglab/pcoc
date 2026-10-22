//! A 32-byte `public_key` from one CBOR item.
//!
//! `pcoc::cbor::decode_one` recurses and has no depth cap. Nesting deeper
//! than 63 is rejected here, before that decoder runs. Depth 63 still yields
//! a key.

use pcoc::cbor::{self, Cbor};

/// Containers at this depth are still decoded. One more is rejected.
const MAX_NESTING: usize = 63;

/// The first 32-byte `public_key` byte string, or nothing.
pub fn public_key(bytes: &[u8]) -> Option<[u8; 32]> {
    if nesting_too_deep(bytes) {
        return None;
    }
    let value = cbor::decode_one(bytes).ok()?;
    find_key(&value)
}

pub fn nesting_too_deep(bytes: &[u8]) -> bool {
    !nesting_within_limit(bytes)
}

fn find_key(value: &Cbor) -> Option<[u8; 32]> {
    match value {
        Cbor::Map(pairs) => {
            if let Some(Cbor::Bytes(raw)) = cbor::map_get(pairs, "public_key") {
                if raw.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(raw);
                    return Some(key);
                }
            }
            for (_, child) in pairs {
                if let Some(key) = find_key(child) {
                    return Some(key);
                }
            }
            None
        }
        Cbor::Array(items) => items.iter().find_map(find_key),
        _ => None,
    }
}

struct Header {
    major: u8,
    arg: u64,
    header: usize,
}

fn read_header(data: &[u8]) -> Option<Header> {
    let ib = *data.first()?;
    let major = ib >> 5;
    let ai = ib & 0x1f;
    if ai == 31 {
        return None;
    }
    if ai < 24 {
        return Some(Header {
            major,
            arg: u64::from(ai),
            header: 1,
        });
    }
    let nbytes = match ai {
        24 => 1,
        25 => 2,
        26 => 4,
        27 => 8,
        _ => return None,
    };
    if data.len() < 1 + nbytes {
        return None;
    }
    let mut arg = 0u64;
    for byte in &data[1..1 + nbytes] {
        arg = (arg << 8) | u64::from(*byte);
    }
    Some(Header {
        major,
        arg,
        header: 1 + nbytes,
    })
}

/// `false` when an array or map would nest deeper than 63.
/// A header this walk cannot bound is left to the decoder, which rejects it
/// without recursing (indefinite lengths, tags, truncated headers).
fn nesting_within_limit(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let mut i = 0usize;
    let mut depth = 0usize;
    let mut open: Vec<u64> = Vec::new();
    loop {
        if i >= data.len() {
            return true;
        }
        let Some(step) = read_header(&data[i..]) else {
            return true;
        };
        if step.header > data.len() - i {
            return true;
        }
        match step.major {
            4 | 5 => {
                let unread = data.len() - i - step.header;
                if step.arg > unread as u64 {
                    return true;
                }
                let children = if step.major == 5 {
                    step.arg.saturating_mul(2)
                } else {
                    step.arg
                };
                i += step.header;
                if children == 0 {
                    if !finish_item(&mut open, &mut depth) {
                        return true;
                    }
                } else {
                    depth += 1;
                    if depth > MAX_NESTING {
                        return false;
                    }
                    open.push(children);
                }
            }
            2 | 3 => {
                let unread = data.len() - i - step.header;
                if step.arg > unread as u64 {
                    return true;
                }
                i += step.header + step.arg as usize;
                if !finish_item(&mut open, &mut depth) {
                    return true;
                }
            }
            0 | 1 | 7 => {
                i += step.header;
                if !finish_item(&mut open, &mut depth) {
                    return true;
                }
            }
            _ => return true,
        }
    }
}

fn finish_item(open: &mut Vec<u64>, depth: &mut usize) -> bool {
    if open.is_empty() {
        return false;
    }
    loop {
        let left = open.last_mut().expect("open container");
        *left -= 1;
        if *left > 0 {
            return true;
        }
        open.pop();
        *depth -= 1;
        if open.is_empty() {
            return false;
        }
    }
}

/// `depth` containers, the innermost a map holding `public_key`.
fn nest_key(depth: usize, key: &[u8; 32]) -> Vec<u8> {
    let mut bytes = cbor::encode_map(&[("public_key".into(), cbor::encode_bytes(key))]);
    for _ in 1..depth {
        bytes = cbor::encode_array(&[bytes]);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_63_yields_the_key_and_64_is_rejected() {
        let key = [0x42u8; 32];
        assert_eq!(public_key(&nest_key(1, &key)), Some(key));
        assert_eq!(public_key(&nest_key(63, &key)), Some(key));
        assert!(nesting_too_deep(&nest_key(64, &key)));
        assert_eq!(public_key(&nest_key(64, &key)), None);
    }

    #[test]
    fn a_short_byte_string_is_not_the_key() {
        let bytes = cbor::encode_map(&[("public_key".into(), cbor::encode_bytes(&[0u8; 31]))]);
        assert_eq!(public_key(&bytes), None);
    }
}
