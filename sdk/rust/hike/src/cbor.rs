//! Definite-length CBOR (RFC 8949) for one hike object.
//!
//! Produce uses the core deterministic encoding: shortest unsigned integers
//! and map keys in encoded-key byte order. Verification accepts any
//! definite-length encoding of the same values, including non-shortest
//! unsigned integers and any key order.

use std::collections::HashSet;

/// Implementation resource limit: the most arrays and maps one item may sit
/// inside, counting itself. A section 8 hike object nests at most 4 deep
/// (top-level map, `stages` array, stage map, `checkpoints`/`leg_results`
/// array; their elements are byte strings). 64 is 16 times that and is the
/// same value in the Rust, Go and Java SDKs. A deeper item is malformed CBOR,
/// so decoding work and stack depth stay bounded before any signature check.
pub const MAX_CBOR_NESTING: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Cbor {
    Uint(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Cbor>),
    Map(Vec<(String, Cbor)>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fail(pub &'static str);

struct Cur<'a> {
    data: &'a [u8],
    i: usize,
    /// Arrays and maps open around the item being read.
    depth: usize,
}

impl<'a> Cur<'a> {
    fn rest(&self) -> &'a [u8] {
        &self.data[self.i..]
    }

    fn byte(&mut self) -> Result<u8, Fail> {
        let b = *self.rest().first().ok_or(Fail("truncated"))?;
        self.i += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Fail> {
        if self.rest().len() < n {
            return Err(Fail("truncated"));
        }
        let start = self.i;
        self.i += n;
        Ok(&self.data[start..self.i])
    }

    fn read_len(&mut self, ai: u8) -> Result<u64, Fail> {
        match ai {
            0..=23 => Ok(u64::from(ai)),
            24 => Ok(u64::from(self.byte()?)),
            25 => {
                let b = self.take(2)?;
                Ok(u64::from(u16::from_be_bytes(
                    b.try_into().map_err(|_| Fail("truncated"))?,
                )))
            }
            26 => {
                let b = self.take(4)?;
                Ok(u64::from(u32::from_be_bytes(
                    b.try_into().map_err(|_| Fail("truncated"))?,
                )))
            }
            27 => {
                let b = self.take(8)?;
                Ok(u64::from_be_bytes(
                    b.try_into().map_err(|_| Fail("truncated"))?,
                ))
            }
            _ => Err(Fail("unsupported additional info")),
        }
    }

    fn item(&mut self) -> Result<Cbor, Fail> {
        let head = self.byte()?;
        let major = head >> 5;
        let ai = head & 0x1f;
        match major {
            0 => Ok(Cbor::Uint(self.read_len(ai)?)),
            1 => Err(Fail("negative integer")),
            2 => self.bytes_item(ai),
            3 => self.text_item(ai),
            4 => self.array_item(ai),
            5 => self.map_item(ai),
            6 => Err(Fail("tag")),
            _ => Err(Fail("simple or float")),
        }
    }

    fn count(&mut self, ai: u8) -> Result<usize, Fail> {
        if ai == 31 {
            return Err(Fail("indefinite length"));
        }
        let n = self.read_len(ai)?;
        let n = usize::try_from(n).map_err(|_| Fail("length"))?;
        // Each remaining element occupies at least one byte.
        if n > self.rest().len() {
            return Err(Fail("length exceeds input"));
        }
        Ok(n)
    }

    fn bytes_item(&mut self, ai: u8) -> Result<Cbor, Fail> {
        let n = self.count(ai)?;
        Ok(Cbor::Bytes(self.take(n)?.to_vec()))
    }

    fn text_item(&mut self, ai: u8) -> Result<Cbor, Fail> {
        let n = self.count(ai)?;
        let bytes = self.take(n)?;
        let text = std::str::from_utf8(bytes).map_err(|_| Fail("text is not utf-8"))?;
        Ok(Cbor::Text(text.to_owned()))
    }

    fn enter(&mut self) -> Result<(), Fail> {
        if self.depth >= MAX_CBOR_NESTING {
            return Err(Fail("nesting depth"));
        }
        self.depth += 1;
        Ok(())
    }

    fn array_item(&mut self, ai: u8) -> Result<Cbor, Fail> {
        self.enter()?;
        let n = self.count(ai)?;
        let mut items = Vec::with_capacity(n);
        for _ in 0..n {
            items.push(self.item()?);
        }
        self.depth -= 1;
        Ok(Cbor::Array(items))
    }

    fn map_item(&mut self, ai: u8) -> Result<Cbor, Fail> {
        self.enter()?;
        let n = self.count(ai)?;
        let mut pairs = Vec::with_capacity(n);
        let mut seen = HashSet::with_capacity(n);
        for _ in 0..n {
            let key = self.item()?;
            let Cbor::Text(key) = key else {
                return Err(Fail("map key is not text"));
            };
            if !seen.insert(key.clone()) {
                return Err(Fail("duplicate key"));
            }
            pairs.push((key, self.item()?));
        }
        self.depth -= 1;
        Ok(Cbor::Map(pairs))
    }
}

/// One CBOR data item, plus whether any byte follows it.
pub(crate) fn decode_head(data: &[u8]) -> Result<(Cbor, bool), Fail> {
    if data.is_empty() {
        return Err(Fail("empty"));
    }
    let mut cur = Cur { data, i: 0, depth: 0 };
    let value = cur.item()?;
    Ok((value, cur.i != data.len()))
}

pub(crate) fn decode_one(data: &[u8]) -> Result<Cbor, Fail> {
    let (value, trailing) = decode_head(data)?;
    if trailing {
        return Err(Fail("trailing bytes"));
    }
    Ok(value)
}

#[derive(Clone, Copy)]
enum UintWidth {
    Shortest,
    #[cfg(test)]
    Eight,
}

fn put_uint(out: &mut Vec<u8>, major: u8, n: u64, width: UintWidth) {
    match width {
        #[cfg(test)]
        UintWidth::Eight => {
            out.push((major << 5) | 27);
            out.extend_from_slice(&n.to_be_bytes());
        }
        UintWidth::Shortest => {
            if n < 24 {
                out.push((major << 5) | n as u8);
            } else if n < 256 {
                out.push((major << 5) | 24);
                out.push(n as u8);
            } else if n < 65536 {
                out.push((major << 5) | 25);
                out.extend_from_slice(&(n as u16).to_be_bytes());
            } else if n < 0x1_0000_0000 {
                out.push((major << 5) | 26);
                out.extend_from_slice(&(n as u32).to_be_bytes());
            } else {
                out.push((major << 5) | 27);
                out.extend_from_slice(&n.to_be_bytes());
            }
        }
    }
}

fn encode_into(out: &mut Vec<u8>, value: &Cbor, width: UintWidth) {
    match value {
        Cbor::Uint(n) => put_uint(out, 0, *n, width),
        Cbor::Bytes(b) => {
            put_uint(out, 2, b.len() as u64, width);
            out.extend_from_slice(b);
        }
        Cbor::Text(s) => {
            put_uint(out, 3, s.len() as u64, width);
            out.extend_from_slice(s.as_bytes());
        }
        Cbor::Array(items) => {
            put_uint(out, 4, items.len() as u64, width);
            for item in items {
                encode_into(out, item, width);
            }
        }
        Cbor::Map(pairs) => {
            put_uint(out, 5, pairs.len() as u64, width);
            for (key, item) in pairs {
                put_uint(out, 3, key.len() as u64, width);
                out.extend_from_slice(key.as_bytes());
                encode_into(out, item, width);
            }
        }
    }
}

pub(crate) fn encode(value: &Cbor) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(&mut out, value, UintWidth::Shortest);
    out
}

#[cfg(test)]
pub(crate) fn encode_wide_uints(value: &Cbor) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(&mut out, value, UintWidth::Eight);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_arrays(depth: usize) -> Vec<u8> {
        let mut out = vec![0x81; depth.saturating_sub(1)];
        out.push(0x80);
        out
    }

    #[test]
    fn nesting_up_to_the_limit_decodes() {
        assert!(decode_one(&nested_arrays(MAX_CBOR_NESTING)).is_ok());
        // Maps count the same as arrays.
        let mut maps = Vec::new();
        for _ in 0..MAX_CBOR_NESTING - 1 {
            maps.extend_from_slice(&[0xa1, 0x61, b'k']);
        }
        maps.push(0xa0);
        assert!(decode_one(&maps).is_ok());
    }

    #[test]
    fn nesting_past_the_limit_is_malformed_not_a_crash() {
        assert_eq!(decode_one(&nested_arrays(MAX_CBOR_NESTING + 1)), Err(Fail("nesting depth")));
        // About 100 000 deep, run on a small thread: no stack overflow.
        let deep = nested_arrays(100_000);
        let got = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || decode_head(&deep).map(|_| ()))
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(got, Err(Fail("nesting depth")));
        // Deep and truncated, deep inside a map: still a typed failure.
        let mut inner = vec![0xa1, 0x63, b'z', b'z', b'z'];
        inner.extend(vec![0x81; 100_000]);
        assert!(decode_head(&inner).is_err());
    }

    /// Sibling containers do not add up: only the open ones count.
    fn siblings(count: usize) -> (Vec<u8>, Vec<u8>) {
        let mut arrays = vec![0x98, count as u8];
        for _ in 0..count {
            arrays.extend_from_slice(&[0x81, 0x80]);
        }
        let mut maps = vec![0xb8, count as u8];
        for n in 0..count {
            maps.extend_from_slice(&[0x62, b'a' + (n / 26) as u8, b'a' + (n % 26) as u8, 0xa1, 0x61, b'v', 0xa0]);
        }
        (arrays, maps)
    }

    #[test]
    fn sibling_containers_do_not_count_toward_the_limit() {
        let (arrays, maps) = siblings(200);
        assert!(decode_one(&arrays).is_ok());
        assert!(decode_one(&maps).is_ok());
    }

    #[test]
    fn a_valid_hike_nests_four_deep() {
        // top map > stages array > stage map > checkpoints array > bytes.
        let object = Cbor::Map(vec![(
            "stages".to_owned(),
            Cbor::Array(vec![Cbor::Map(vec![(
                "checkpoints".to_owned(),
                Cbor::Array(vec![Cbor::Bytes(vec![0; 32])]),
            )])]),
        )]);
        let mut cur = Cur { data: &encode(&object), i: 0, depth: 0 };
        assert!(cur.item().is_ok());
        assert!(4 * 4 <= MAX_CBOR_NESTING);
    }
}
