//! Minimal JSON writer for the section 14 record. No insignificant whitespace.

pub fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A JSON number or a measurement token.
#[derive(Clone, Debug)]
pub enum J {
    Str(String),
    Num(i128),
    Arr(Vec<J>),
    Obj(Vec<(&'static str, J)>),
}

impl J {
    pub fn s(t: &str) -> Self {
        J::Str(t.to_string())
    }

    pub fn render(&self) -> String {
        match self {
            J::Str(t) => escape_string(t),
            J::Num(n) => {
                if *n == 0 {
                    "0".to_string()
                } else {
                    n.to_string()
                }
            }
            J::Arr(xs) => {
                let mut o = String::from("[");
                for (i, x) in xs.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    o.push_str(&x.render());
                }
                o.push(']');
                o
            }
            J::Obj(pairs) => {
                let mut o = String::from("{");
                for (i, (k, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    o.push_str(&escape_string(k));
                    o.push(':');
                    o.push_str(&v.render());
                }
                o.push('}');
                o
            }
        }
    }
}

/// SPEC Rule A5. The record bytes, with no trailing newline.
pub fn unknown_kind_record(kind_text: &str) -> String {
    format!(
        "{{\"kind\":\"unknown kind\",\"kind_text\":{}}}",
        escape_string(kind_text)
    )
}
