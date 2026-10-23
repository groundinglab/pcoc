// SECTION 7.3 CBOR. Produce emits the core deterministic encoding; verify
// decodes any definite-length encoding of the same values, including a
// non-shortest unsigned integer. Key order is not a shape mismatch.

public enum CBORItem {
    case uint(UInt64)
    case byteString([UInt8])
    case text(String)
    case array([CBORItem])
    /// Map entries in wire order, keys as decoded strings.
    case map([(String, CBORItem)])
}

public enum CBORDecodeOutcome {
    case malformed
    case item(CBORItem)
}

public enum CBOR {
    // ------------------------------------------------------------- encode

    public static func head(major: UInt8, length: Int) -> [UInt8] {
        let m = major << 5
        if length < 24 { return [m | UInt8(length)] }
        if length < 256 { return [m | 24, UInt8(length)] }
        if length < 65536 { return [m | 25, UInt8(length >> 8), UInt8(length & 0xff)] }
        if length < 1 << 32 {
            let v = UInt32(length)
            return [m | 26, UInt8(v >> 24 & 0xff), UInt8(v >> 16 & 0xff),
                    UInt8(v >> 8 & 0xff), UInt8(v & 0xff)]
        }
        let v = UInt64(length)
        return [m | 27] + LE.le64(v).reversed()
    }

    public static func encodeUint(_ v: UInt64) -> [UInt8] {
        if v < 24 { return [UInt8(v)] }
        if v < 256 { return [24, UInt8(v)] }
        if v < 65536 { return [25, UInt8(v >> 8), UInt8(v & 0xff)] }
        if v < 1 << 32 { return [26, UInt8(v >> 24 & 0xff), UInt8(v >> 16 & 0xff),
                                UInt8(v >> 8 & 0xff), UInt8(v & 0xff)] }
        return [27] + LE.le64(v).reversed()
    }

    public static func encodeText(_ s: String) -> [UInt8] {
        head(major: 3, length: s.utf8.count) + Array(s.utf8)
    }

    public static func encodeBytes(_ b: [UInt8]) -> [UInt8] {
        head(major: 2, length: b.count) + b
    }

    /// Map keys sorted by the bytewise lexicographic order of their
    /// definite-length encodings (section 7.3). Every field name in this
    /// object is at most 23 bytes, so the encoding is the head byte
    /// (0x60 + length) followed by the UTF-8 bytes.
    public static func sortedKeys(_ keys: [String]) -> [String] {
        func encoded(_ s: String) -> [UInt8] {
            let u = Array(s.utf8)
            return [UInt8(0x60 + u.count)] + u
        }
        return keys.sorted { Array(encoded($0)).lexicographicallyPrecedes(Array(encoded($1))) }
    }

    // ------------------------------------------------------------- decode

    public static func decode(_ bytes: [UInt8]) -> CBORDecodeOutcome {
        var pos = 0

        // One definite-length data item only; any byte after it is malformed.
        guard let item = readItem(bytes, &pos), pos == bytes.count else {
            return .malformed
        }
        return .item(item)
    }

    static func readItem(_ b: [UInt8], _ pos: inout Int) -> CBORItem? {
        guard pos < b.count else { return nil }
        let head = b[pos]
        pos += 1
        let major = head >> 5
        let info = head & 0x1f

        // Indefinite lengths, in any major type, are a malformed CBOR value
        // under this file's decoding rules (section 7.3).
        if info == 31 { return nil }

        var length: Int
        switch info {
        case 0...23: length = Int(info)
        case 24:
            guard pos < b.count else { return nil }
            length = Int(b[pos]); pos += 1
        case 25:
            guard pos + 2 <= b.count else { return nil }
            length = Int(b[pos]) << 8 | Int(b[pos + 1]); pos += 2
        case 26:
            guard pos + 4 <= b.count else { return nil }
            var v = 0
            for i in 0..<4 { v = v << 8 | Int(b[pos + i]) }
            pos += 4; length = v
        case 27:
            guard pos + 8 <= b.count else { return nil }
            var v: UInt64 = 0
            for i in 0..<8 { v = v << 8 | UInt64(b[pos + i]) }
            pos += 8
            if v > UInt64(Int.max) { return nil }
            length = Int(v)
        default:
            return nil // info 28-30 are not valid in this object
        }

        switch major {
        case 0:
            return .uint(UInt64(length))
        case 1:
            return nil // negative integers are malformed here
        case 2:
            guard pos + length <= b.count else { return nil }
            let v = Array(b[pos..<pos + length]); pos += length
            return .byteString(v)
        case 3:
            guard pos + length <= b.count else { return nil }
            let v = Array(b[pos..<pos + length]); pos += length
            guard let s = String(validating: v, as: UTF8.self) else { return nil }
            return .text(s)
        case 4:
            var items: [CBORItem] = []
            for _ in 0..<length {
                guard let it = readItem(b, &pos) else { return nil }
                items.append(it)
            }
            return .array(items)
        case 5:
            var entries: [(String, CBORItem)] = []
            var seenKeys = Set<Array<UInt8>>()
            for _ in 0..<length {
                guard pos < b.count else { return nil }
                let keyStart = pos
                guard let keyItem = readItem(b, &pos) else { return nil }
                // A map key that is not a text string is malformed.
                guard case .text(let key) = keyItem else { return nil }
                // Duplicate key (by encoded key bytes) is malformed.
                let keyBytes = Array(b[keyStart..<pos])
                if !seenKeys.insert(keyBytes).inserted { return nil }
                guard let value = readItem(b, &pos) else { return nil }
                entries.append((key, value))
            }
            return .map(entries)
        case 6:
            return nil // tags are malformed here
        default:
            return nil // simple values and floating-point are malformed here
        }
    }

    // --------------------------------------------------------- accessors

    public static func field(_ item: CBORItem, _ key: String) -> CBORItem? {
        guard case .map(let entries) = item else { return nil }
        for (k, v) in entries where k == key { return v }
        return nil
    }

    public static func asBytes(_ item: CBORItem?) -> [UInt8]? {
        guard case .byteString(let b)? = item else { return nil }
        return b
    }

    public static func asText(_ item: CBORItem?) -> String? {
        guard case .text(let s)? = item else { return nil }
        return s
    }

    public static func asUint(_ item: CBORItem?) -> UInt64? {
        guard case .uint(let v)? = item else { return nil }
        return v
    }

    public static func asArray(_ item: CBORItem?) -> [CBORItem]? {
        guard case .array(let a)? = item else { return nil }
        return a
    }

    public static func asMap(_ item: CBORItem?) -> [(String, CBORItem)]? {
        guard case .map(let m)? = item else { return nil }
        return m
    }
}
