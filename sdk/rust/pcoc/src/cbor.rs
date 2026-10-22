//! SPEC §7.3. Deterministic CBOR encode, and a definite-length decoder.
//! Indefinite lengths, tags, floats, simple values, negative integers,
//! non-text keys, duplicate keys, and trailing bytes are malformed.

/// A decoded CBOR item. Unsigned integers keep their numeric value.
#[derive(Clone, Debug, PartialEq)]
pub enum Cbor {
    Uint(u64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<Cbor>),
    Map(Vec<(String, Cbor)>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;

fn head(major: u8, n: u64) -> Vec<u8> {
    let m = major << 5;
    if n < 24 {
        vec![m | (n as u8)]
    } else if n <= 0xff {
        vec![m | 24, n as u8]
    } else if n <= 0xffff {
        let x = n as u16;
        vec![m | 25, (x >> 8) as u8, x as u8]
    } else if n <= 0xffff_ffff {
        let x = n as u32;
        vec![
            m | 26,
            (x >> 24) as u8,
            (x >> 16) as u8,
            (x >> 8) as u8,
            x as u8,
        ]
    } else {
        vec![
            m | 27,
            (n >> 56) as u8,
            (n >> 48) as u8,
            (n >> 40) as u8,
            (n >> 32) as u8,
            (n >> 24) as u8,
            (n >> 16) as u8,
            (n >> 8) as u8,
            n as u8,
        ]
    }
}

/// SPEC §7.3. Shortest unsigned integer.
pub fn encode_uint(n: u64) -> Vec<u8> {
    head(0, n)
}

/// SPEC §7.3. Definite byte string.
pub fn encode_bytes(b: &[u8]) -> Vec<u8> {
    let mut v = head(2, b.len() as u64);
    v.extend_from_slice(b);
    v
}

/// SPEC §7.3. Definite text string.
pub fn encode_text(s: &str) -> Vec<u8> {
    let mut v = head(3, s.len() as u64);
    v.extend_from_slice(s.as_bytes());
    v
}

/// SPEC §7.3. Definite array of already-encoded items.
pub fn encode_array(items: &[Vec<u8>]) -> Vec<u8> {
    let mut v = head(4, items.len() as u64);
    for it in items {
        v.extend_from_slice(it);
    }
    v
}

/// SPEC §7.3. Definite map. Keys are sorted by their definite-length encodings.
pub fn encode_map(pairs: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut enc: Vec<(Vec<u8>, Vec<u8>)> = pairs
        .iter()
        .map(|(k, val)| (encode_text(k), val.clone()))
        .collect();
    enc.sort_by(|a, b| a.0.cmp(&b.0));
    let mut v = head(5, enc.len() as u64);
    for (k, val) in enc {
        v.extend_from_slice(&k);
        v.extend_from_slice(&val);
    }
    v
}

struct Cur<'a> {
    data: &'a [u8],
    i: usize,
}

impl<'a> Cur<'a> {
    fn rest(&self) -> usize {
        self.data.len().saturating_sub(self.i)
    }

    fn u8(&mut self) -> Result<u8, Malformed> {
        if self.i >= self.data.len() {
            return Err(Malformed);
        }
        let b = self.data[self.i];
        self.i += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        if self.rest() < n {
            return Err(Malformed);
        }
        let s = &self.data[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }

    fn arg(&mut self, ai: u8) -> Result<u64, Malformed> {
        if ai < 24 {
            Ok(ai as u64)
        } else if ai == 24 {
            Ok(self.u8()? as u64)
        } else if ai == 25 {
            let b = self.take(2)?;
            Ok(u16::from_be_bytes([b[0], b[1]]) as u64)
        } else if ai == 26 {
            let b = self.take(4)?;
            Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64)
        } else if ai == 27 {
            let b = self.take(8)?;
            Ok(u64::from_be_bytes([
                b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            ]))
        } else {
            Err(Malformed)
        }
    }

    fn item(&mut self) -> Result<Cbor, Malformed> {
        let ib = self.u8()?;
        let major = ib >> 5;
        let ai = ib & 0x1f;
        // Indefinite (ai == 31) is malformed for every major type we accept.
        if ai == 31 {
            return Err(Malformed);
        }
        match major {
            0 => Ok(Cbor::Uint(self.arg(ai)?)),
            1 => Err(Malformed),
            2 => {
                let n = self.arg(ai)?;
                if n > self.rest() as u64 {
                    return Err(Malformed);
                }
                let s = self.take(n as usize)?;
                Ok(Cbor::Bytes(s.to_vec()))
            }
            3 => {
                let n = self.arg(ai)?;
                if n > self.rest() as u64 {
                    return Err(Malformed);
                }
                let s = self.take(n as usize)?;
                let t = std::str::from_utf8(s).map_err(|_| Malformed)?;
                Ok(Cbor::Text(t.to_string()))
            }
            4 => {
                let n = self.arg(ai)?;
                // A definite element cannot be empty, so n cannot exceed the remaining bytes.
                if n > self.rest() as u64 {
                    return Err(Malformed);
                }
                let mut arr = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    arr.push(self.item()?);
                }
                Ok(Cbor::Array(arr))
            }
            5 => {
                let n = self.arg(ai)?;
                if n > self.rest() as u64 {
                    return Err(Malformed);
                }
                let mut map = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    let key = self.item()?;
                    let Cbor::Text(k) = key else {
                        return Err(Malformed);
                    };
                    if map.iter().any(|(ek, _)| ek == &k) {
                        return Err(Malformed);
                    }
                    let val = self.item()?;
                    map.push((k, val));
                }
                Ok(Cbor::Map(map))
            }
            _ => Err(Malformed),
        }
    }
}

/// SPEC §7.3. Decode one definite-length data item. Trailing bytes are malformed
/// when `whole` is true. The returned length is the item's end.
pub fn decode_item(data: &[u8]) -> Result<(Cbor, usize), Malformed> {
    let mut c = Cur { data, i: 0 };
    let v = c.item()?;
    Ok((v, c.i))
}

/// SPEC §7.3. One data item and no byte after it.
pub fn decode_one(data: &[u8]) -> Result<Cbor, Malformed> {
    let (v, n) = decode_item(data)?;
    if n != data.len() {
        return Err(Malformed);
    }
    Ok(v)
}

pub fn map_get<'a>(map: &'a [(String, Cbor)], key: &str) -> Option<&'a Cbor> {
    map.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}
