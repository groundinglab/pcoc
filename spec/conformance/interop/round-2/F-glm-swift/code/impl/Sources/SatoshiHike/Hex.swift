// Section 3 interpretation helpers and section 7 encodings.

public enum LE {
    /// le64(x): 8 little-endian bytes of an unsigned integer below 2^64 (section 3).
    public static func le64(_ x: UInt64) -> [UInt8] {
        [UInt8(x & 0xff), UInt8((x >> 8) & 0xff), UInt8((x >> 16) & 0xff), UInt8((x >> 24) & 0xff),
         UInt8((x >> 32) & 0xff), UInt8((x >> 40) & 0xff), UInt8((x >> 48) & 0xff), UInt8((x >> 56) & 0xff)]
    }

    /// u64_le(b): the unsigned integer of 8 little-endian bytes (section 3).
    public static func u64(_ b: [UInt8]) -> UInt64 {
        precondition(b.count == 8)
        var v: UInt64 = 0
        for i in 0..<8 { v |= UInt64(b[i]) << (8 * UInt64(i)) }
        return v
    }

    public static func u64(_ b: UnsafeRawPointer) -> UInt64 {
        var v: UInt64 = 0
        for i in 0..<8 { v |= UInt64(b.load(fromByteOffset: i, as: UInt8.self)) << (8 * UInt64(i)) }
        return v
    }
}

public enum Hex {
    /// Lowercase hex, byte 0 first (section 7.2).
    public static func encode(_ bytes: [UInt8]) -> String {
        let table: [Character] = Array("0123456789abcdef")
        var s = ""
        s.reserveCapacity(bytes.count * 2)
        for b in bytes {
            s.append(table[Int(b >> 4)])
            s.append(table[Int(b & 0xf)])
        }
        return s
    }

    public static func encode(_ bytes: ArraySlice<UInt8>) -> String { encode(Array(bytes)) }

    /// A 64-digit spelling denotes 32 bytes, first two digits byte 0 (section 7.2).
    /// Accepts 0-9 a-f A-F, no spaces, no 0x prefix. Returns nil when malformed
    /// or when the length is odd or over the expected byte length.
    public static func decode(_ s: String, expectBytes: Int? = nil) -> [UInt8]? {
        let chars = Array(s.unicodeScalars)
        if expectBytes != nil && chars.count != expectBytes! * 2 { return nil }
        if chars.count % 2 != 0 { return nil }
        var out: [UInt8] = []
        out.reserveCapacity(chars.count / 2)
        var i = 0
        while i < chars.count {
            guard let hi = hexVal(chars[i]), let lo = hexVal(chars[i + 1]) else { return nil }
            out.append(UInt8(hi << 4 | lo))
            i += 2
        }
        return out
    }

    static func hexVal(_ c: Unicode.Scalar) -> Int? {
        switch c {
        case "0"..."9": return Int(c.value - 48)
        case "a"..."f": return Int(c.value - 87)
        case "A"..."F": return Int(c.value - 55)
        default: return nil
        }
    }

    /// The u64 key spelling: 16 lowercase hex digits, big-endian spelling of the
    /// number, not the wire bytes (CASES.md output format).
    public static func u64Key(_ v: UInt64) -> String {
        let table: [Character] = Array("0123456789abcdef")
        var s = String()
        for shift in stride(from: 60, through: 0, by: -4) {
            s.append(table[Int((v >> UInt64(shift)) & 0xf)])
        }
        return s
    }
}

extension [UInt8] {
    /// Raw concatenation (section 3).
    public static func concat(_ parts: [[UInt8]]) -> [UInt8] {
        var out: [UInt8] = []
        for p in parts { out.append(contentsOf: p) }
        return out
    }

    /// le64 leg result wire bytes as lowercase hex (legs_le_hex).
    public var legsLeHex: String { Hex.encode(self) }
}

/// mix64, section 10.1, scalar Swift (reference used by tests and by the
/// generator-only Negatives module). Identical to the C kernels.
public func mix64(_ x: UInt64) -> UInt64 {
    var y = x &+ Spec.mixAdd
    y = (y ^ (y >> 30)) &* Spec.mixMul1
    y = (y ^ (y >> 27)) &* Spec.mixMul2
    return y ^ (y >> 31)
}

/// mix64 without its last line (the value y just before `return y ^ (y >> 31)`),
/// used only by the generator-only final_stage_hoist negative.
public func mix64Prelast(_ x: UInt64) -> UInt64 {
    var y = x &+ Spec.mixAdd
    y = (y ^ (y >> 30)) &* Spec.mixMul1
    y = (y ^ (y >> 27)) &* Spec.mixMul2
    return y
}

public func rotl64(_ x: UInt64, _ r: UInt64) -> UInt64 {
    r == 0 ? x : (x << r) | (x >> (64 - r))
}

public func reverse32(_ v: UInt32) -> UInt32 {
    var x = v
    x = ((x & 0x55555555) << 1) | ((x >> 1) & 0x55555555)
    x = ((x & 0x33333333) << 2) | ((x >> 2) & 0x33333333)
    x = ((x & 0x0F0F0F0F) << 4) | ((x >> 4) & 0x0F0F0F0F)
    x = ((x & 0x00FF00FF) << 8) | ((x >> 8) & 0x00FF00FF)
    return (x << 16) | (x >> 16)
}

/// floor_log2(n) for n >= 1 (section 10.3).
public func floorLog2(_ n: UInt64) -> Int {
    n == 0 ? 0 : 63 - n.leadingZeroBitCount
}
