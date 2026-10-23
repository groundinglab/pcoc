// Ordered compact JSON writer for the section 14 verify record and the Rule
// A5 record. The verify record is written with keys in the order section 14
// lists, no insignificant whitespace, one trailing newline (that newline is
// the CLI's, not part of the record).

public enum JVal {
    case int(Int64)
    case uint(UInt64)
    case str(String)
    case arr([JVal])
    case obj([(String, JVal)])
}

extension JVal: Equatable {
    public static func == (lhs: JVal, rhs: JVal) -> Bool {
        switch (lhs, rhs) {
        case (.int(let a), .int(let b)): return a == b
        case (.uint(let a), .uint(let b)): return a == b
        case (.str(let a), .str(let b)): return a == b
        case (.arr(let a), .arr(let b)): return a == b
        case (.obj(let a), .obj(let b)):
            guard a.count == b.count else { return false }
            for (x, y) in zip(a, b) {
                if x.0 != y.0 || x.1 != y.1 { return false }
            }
            return true
        default:
            return false
        }
    }
}

public enum JSONWriter {
    /// Compact encoding, keys in insertion order, integers in plain decimal
    /// (no exponent, no leading zeros, no -0).
    public static func encode(_ v: JVal) -> String {
        var out = ""
        write(v, into: &out)
        return out
    }

    public static func encodeBytes(_ v: JVal) -> [UInt8] {
        Array(encode(v).utf8)
    }

    static func write(_ v: JVal, into out: inout String) {
        switch v {
        case .int(let i):
            out += String(i)
        case .uint(let u):
            out += String(u)
        case .str(let s):
            out += "\""
            out += escaped(s)
            out += "\""
        case .arr(let items):
            out += "["
            for (i, it) in items.enumerated() {
                if i > 0 { out += "," }
                write(it, into: &out)
            }
            out += "]"
        case .obj(let entries):
            out += "{"
            for (i, (k, val)) in entries.enumerated() {
                if i > 0 { out += "," }
                out += "\""
                out += escaped(k)
                out += "\":"
                write(val, into: &out)
            }
            out += "}"
        }
    }

    /// RFC 8785 string escaping: ", \, and control characters < 0x20, with the
    /// short forms \b \t \n \f \r and \u00xx (lowercase hex) otherwise.
    public static func escapeString(_ s: String) -> [UInt8] {
        var out: [UInt8] = []
        for c in s.unicodeScalars {
            switch c {
            case "\"": out += Array("\\\"".utf8)
            case "\\": out += Array("\\\\".utf8)
            case "\u{08}": out += Array("\\b".utf8)
            case "\u{09}": out += Array("\\t".utf8)
            case "\u{0A}": out += Array("\\n".utf8)
            case "\u{0C}": out += Array("\\f".utf8)
            case "\u{0D}": out += Array("\\r".utf8)
            default:
                if c.value < 0x20 {
                    let table: [Character] = Array("0123456789abcdef")
                    var hex = "\\u"
                    for shift in stride(from: 12, through: 0, by: -4) {
                        hex.append(table[Int((c.value >> UInt32(shift)) & 0xf)])
                    }
                    out += Array(hex.utf8)
                } else {
                    out += Array(String(c).utf8)
                }
            }
        }
        return out
    }

    static func escaped(_ s: String) -> String {
        String(decoding: escapeString(s), as: UTF8.self)
    }
}
