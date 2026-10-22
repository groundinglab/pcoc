//! SPEC §7.2 hexadecimal. Byte 0 is the first two digits.

/// SPEC §7.2. Encode bytes as lowercase hex, byte 0 first.
pub fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0xf) as usize] as char);
    }
    out
}

/// SPEC §7.2. Decode hex. Letter case does not change the value. No spaces.
pub fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Some(out)
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// SPEC §7.2. Decode exactly `n` bytes.
pub fn hex_decode_exact<const N: usize>(text: &str) -> Option<[u8; N]> {
    let v = hex_decode(text)?;
    if v.len() != N {
        return None;
    }
    let mut a = [0u8; N];
    a.copy_from_slice(&v);
    Some(a)
}

/// SPEC §9. `natural[i] = display[31 − i]`.
pub fn reverse32(bytes: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = bytes[31 - i];
    }
    out
}
