pub fn encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

/// 64 hex digits, either case.
pub fn decode32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in text.as_bytes().chunks(2).enumerate() {
        out[i] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(out)
}

/// 64 lowercase hex digits only (section 15.1 grammar).
pub fn decode32_lower(text: &str) -> Option<[u8; 32]> {
    if !text
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    decode32(text)
}

pub fn decode(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    text.as_bytes()
        .chunks(2)
        .map(|pair| Some((nibble(pair[0])? << 4) | nibble(pair[1])?))
        .collect()
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Bitcoin display order is the natural header hash reversed by bytes (section 9).
pub fn display_order(natural: &[u8; 32]) -> [u8; 32] {
    let mut out = *natural;
    out.reverse();
    out
}

/// `e7b3a1c04d9f…e7a4f18b`: twelve leading and eight trailing digits.
pub fn short(hex: &str) -> String {
    if hex.len() <= 20 {
        return hex.to_owned();
    }
    format!("{}…{}", &hex[..12], &hex[hex.len() - 8..])
}

/// Leading display-order digits of a header hash. Long enough that a
/// mainnet tip's prefix is not only the zero run.
pub fn prefix(natural: &[u8; 32]) -> String {
    encode(&display_order(natural)).chars().take(24).collect()
}

/// `00000000…547f22`: the status-bar form of a display-order tip.
pub fn short_tip(hex: &str) -> String {
    if hex.len() <= 14 {
        return hex.to_owned();
    }
    format!("{}…{}", &hex[..8], &hex[hex.len() - 6..])
}
