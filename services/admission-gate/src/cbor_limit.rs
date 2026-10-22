//! Iterative CBOR nesting cap.
//!
//! The SDK decoder walks arrays and maps by recursion and has no depth cap.
//! A deep array posted at sign-in must be refused before that walk, or a
//! 2 MiB worker stack overflows (the F1 regression). The cap is 64 containers
//! from the top, counting the outer map. Depth 63 in the sign-in fixture
//! (outer map plus 63 arrays) is still readable. One more is not.

/// `true` when a well-formed prefix nests containers deeper than `max`.
///
/// A truncated or non-container header returns `false` so the caller can
/// treat the bytes as undecodable without recursing.
pub fn nesting_exceeds(bytes: &[u8], max: usize) -> bool {
    let mut pos = 0usize;
    // Remaining direct children of each open container, outer first.
    let mut open: Vec<u64> = Vec::new();
    let mut top = true;
    loop {
        if !top {
            loop {
                match open.last() {
                    Some(0) => {
                        open.pop();
                    }
                    Some(_) => break,
                    None => return false,
                }
            }
            if let Some(left) = open.last_mut() {
                *left -= 1;
            }
        }
        top = false;
        if pos >= bytes.len() {
            return false;
        }
        let major = bytes[pos] >> 5;
        let ai = bytes[pos] & 0x1f;
        pos += 1;
        // Indefinite length is not a container this gate will walk.
        if ai == 31 {
            return false;
        }
        let Some(arg) = read_arg(bytes, &mut pos, ai) else {
            return false;
        };
        match major {
            0 | 1 => {}
            2 | 3 => {
                let end = pos.saturating_add(arg as usize);
                if end > bytes.len() || arg > isize::MAX as u64 {
                    return false;
                }
                pos = end;
            }
            4 | 5 => {
                let depth = open.len() + 1;
                if depth > max {
                    return true;
                }
                let children = if major == 5 {
                    arg.saturating_mul(2)
                } else {
                    arg
                };
                open.push(children);
            }
            _ => return false,
        }
    }
}

fn read_arg(bytes: &[u8], pos: &mut usize, ai: u8) -> Option<u64> {
    match ai {
        0..=23 => Some(u64::from(ai)),
        24 => {
            let b = *bytes.get(*pos)?;
            *pos += 1;
            Some(u64::from(b))
        }
        25 => {
            let s = bytes.get(*pos..*pos + 2)?;
            *pos += 2;
            Some(u64::from(u16::from_be_bytes([s[0], s[1]])))
        }
        26 => {
            let s = bytes.get(*pos..*pos + 4)?;
            *pos += 4;
            Some(u64::from(u32::from_be_bytes([s[0], s[1], s[2], s[3]])))
        }
        27 => {
            let s = bytes.get(*pos..*pos + 8)?;
            *pos += 8;
            Some(u64::from_be_bytes([
                s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
            ]))
        }
        _ => None,
    }
}

/// The sign-in cap. One past this is not decoded.
pub const MAX_CBOR_NESTING: usize = 64;
