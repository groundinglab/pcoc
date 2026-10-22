//! SPEC §9. Portable-index JSON body. One object, no duplicate names, no BOM.

use crate::hexutil::{hex_decode_exact, reverse32};

#[derive(Clone, Debug)]
pub enum Json {
    Null,
    Bool(bool),
    /// `integer` is set only for a canonical integer literal in `i128` range.
    Number { integer: Option<i128> },
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

pub fn parse_document(bytes: &[u8]) -> Option<Json> {
    if bytes.len() >= 3 && bytes[0] == 0xef && bytes[1] == 0xbb && bytes[2] == 0xbf {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    let v = p.value().ok()?;
    p.skip();
    if p.i != p.s.len() {
        return None;
    }
    Some(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn skip(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Result<Json, ()> {
        let c = self.peek().ok_or(())?;
        match c {
            b'n' => self.lit(b"null", Json::Null),
            b't' => self.lit(b"true", Json::Bool(true)),
            b'f' => self.lit(b"false", Json::Bool(false)),
            b'"' => Ok(Json::String(self.string()?)),
            b'[' => self.array(),
            b'{' => self.object(),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(()),
        }
    }

    fn lit(&mut self, word: &[u8], v: Json) -> Result<Json, ()> {
        self.skip();
        if self.s[self.i..].starts_with(word) {
            self.i += word.len();
            Ok(v)
        } else {
            Err(())
        }
    }

    fn string(&mut self) -> Result<String, ()> {
        if !self.eat(b'"') {
            return Err(());
        }
        let mut out = String::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.s.get(self.i).copied().ok_or(())?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            if self.i + 4 > self.s.len() {
                                return Err(());
                            }
                            let hex = std::str::from_utf8(&self.s[self.i..self.i + 4]).map_err(|_| ())?;
                            let u = u32::from_str_radix(hex, 16).map_err(|_| ())?;
                            self.i += 4;
                            let ch = char::from_u32(u).ok_or(())?;
                            out.push(ch);
                        }
                        _ => return Err(()),
                    }
                }
                c if c < 0x20 => return Err(()),
                c => out.push(c as char),
            }
        }
        Err(())
    }

    fn number(&mut self) -> Result<Json, ()> {
        self.skip();
        let start = self.i;
        if self.eat(b'-') {}
        if self.peek() == Some(b'0') {
            self.i += 1;
        } else if matches!(self.peek(), Some(b'1'..=b'9')) {
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        } else {
            return Err(());
        }
        let mut fractional = false;
        if self.peek() == Some(b'.') {
            fractional = true;
            self.i += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(());
            }
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            fractional = true;
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(());
            }
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| ())?;
        let integer = if fractional {
            None
        } else {
            text.parse::<i128>().ok()
        };
        Ok(Json::Number { integer })
    }

    fn array(&mut self) -> Result<Json, ()> {
        if !self.eat(b'[') {
            return Err(());
        }
        let mut xs = Vec::new();
        if self.eat(b']') {
            return Ok(Json::Array(xs));
        }
        loop {
            xs.push(self.value()?);
            if self.eat(b']') {
                return Ok(Json::Array(xs));
            }
            if !self.eat(b',') {
                return Err(());
            }
        }
    }

    fn object(&mut self) -> Result<Json, ()> {
        if !self.eat(b'{') {
            return Err(());
        }
        let mut pairs = Vec::new();
        if self.eat(b'}') {
            return Ok(Json::Object(pairs));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(());
            }
            let k = self.string()?;
            if pairs.iter().any(|(ek, _)| ek == &k) {
                return Err(());
            }
            if !self.eat(b':') {
                return Err(());
            }
            let v = self.value()?;
            pairs.push((k, v));
            if self.eat(b'}') {
                return Ok(Json::Object(pairs));
            }
            if !self.eat(b',') {
                return Err(());
            }
        }
    }
}

pub fn obj_get<'a>(v: &'a Json, key: &str) -> Option<&'a Json> {
    if let Json::Object(pairs) = v {
        pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    } else {
        None
    }
}

fn json_u64_bounded(v: &Json, max: u64) -> Option<u64> {
    if let Json::Number { integer: Some(n) } = v {
        if *n >= 0 && (*n as u128) <= max as u128 {
            return Some(*n as u64);
        }
    }
    None
}

/// A successful header read: `header.hash` and `header.height` follow section 9.
#[derive(Clone, Debug)]
pub struct HeaderDoc {
    pub natural: [u8; 32],
    pub display: [u8; 32],
    pub height: u64,
    pub time: Option<u64>,
}

/// SPEC §9. Header measurement fields. `None` is an unsuccessful header read.
pub fn read_header(doc: &Json) -> Option<HeaderDoc> {
    let header = obj_get(doc, "header")?;
    let hash_s = match obj_get(header, "hash")? {
        Json::String(s) if s.len() == 64 && !s.starts_with("0x") => s,
        _ => return None,
    };
    let display: [u8; 32] = hex_decode_exact(hash_s)?;
    let height = json_u64_bounded(obj_get(header, "height")?, (1_u64 << 53) - 1)?;
    let time = obj_get(header, "time").and_then(|t| json_u64_bounded(t, u32::MAX as u64));
    // Erratum E26. The portable index has no previous-hash member. A carried
    // `previous_hash` changes nothing (§9) and is not read.
    Some(HeaderDoc {
        natural: reverse32(&display),
        display,
        height,
        time,
    })
}

#[derive(Clone, Debug)]
pub struct PlantFact {
    pub root: [u8; 32],
    pub txid: [u8; 32],
    pub vout: u32,
    pub block_hash: [u8; 32],
    pub block_height: u32,
}

fn parse_plant(v: &Json) -> Option<PlantFact> {
    let Json::Object(_) = v else {
        return None;
    };
    let root_s = match obj_get(v, "root")? {
        Json::String(s) if s.len() == 64 => s,
        _ => return None,
    };
    let tx_s = match obj_get(v, "txid")? {
        Json::String(s) if s.len() == 64 => s,
        _ => return None,
    };
    let bh_s = match obj_get(v, "block_hash")? {
        Json::String(s) if s.len() == 64 => s,
        _ => return None,
    };
    let vout = json_u64_bounded(obj_get(v, "vout")?, u32::MAX as u64)? as u32;
    let block_height = json_u64_bounded(obj_get(v, "block_height")?, u32::MAX as u64)? as u32;
    let root = hex_decode_exact(root_s)?;
    let txid_disp: [u8; 32] = hex_decode_exact(tx_s)?;
    let bh_disp: [u8; 32] = hex_decode_exact(bh_s)?;
    Some(PlantFact {
        root,
        txid: reverse32(&txid_disp),
        vout,
        block_hash: reverse32(&bh_disp),
        block_height,
    })
}

/// SPEC §14.3. Plants on a returned document, and whether `network` is `main` or `synthetic`.
pub struct PlantDoc {
    pub network_ok: bool,
    pub plants: Vec<PlantFact>,
}

pub fn read_plants(doc: &Json) -> Option<PlantDoc> {
    let Json::Object(_) = doc else {
        return None;
    };
    let network_ok = matches!(obj_get(doc, "network"), Some(Json::String(s)) if s == "main" || s == "synthetic");
    let mut plants = Vec::new();
    if let Some(Json::Array(xs)) = obj_get(doc, "plants") {
        for x in xs {
            if let Some(p) = parse_plant(x) {
                plants.push(p);
            }
        }
    }
    if let Some(p) = obj_get(doc, "plant") {
        if let Some(parsed) = parse_plant(p) {
            plants.push(parsed);
        }
    }
    Some(PlantDoc { network_ok, plants })
}
