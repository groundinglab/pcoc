// SECTION 9 body rules and the portable index. The offline test plane serves
// the synthetic index CASES.md defines: GET /index/<h> for h = 989 … 1018,
// each document compact JSON
//   {"network":"synthetic","header":{"hash":…,"height":h,"time":…}}
// with hash the display-order hex (byte-reversed) of the natural bytes at
// that height, and time = 1700000000 + 600*h. The natural bytes are the
// beacon stored at that height, or the filler hash at any other height.

public enum JSON {
    /// A parsed JSON value. Integers are tracked separately from floats; the
    /// section 9 rules reject fractional/exponent spellings where an integer
    /// is required.
    public enum Value {
        case object([(String, Value)])
        case array([Value])
        case string(String)
        case int(Int64)
        case float(String)   // any number with a fraction or exponent
        case bool(Bool)
        case null
    }

    public enum ParseResult {
        case ok(Value)
        case fail
    }

    /// Parse one UTF-8 JSON object with no BOM, no second value, and no
    /// duplicate member name anywhere (section 9).
    public static func parseObjectOnly(_ bytes: [UInt8]) -> ParseResult {
        if bytes.count >= 3, bytes[0] == 0xEF, bytes[1] == 0xBB, bytes[2] == 0xBF {
            return .fail // BOM is not a successful read
        }
        guard let str = String(validating: bytes, as: UTF8.self) else { return .fail }
        var parser = Parser(scalars: Array(str.unicodeScalars))
        parser.skipWs()
        guard let v = parser.parseValue(dup: &parser.dup) else { return .fail }
        parser.skipWs()
        guard parser.pos == parser.scalars.count else { return .fail } // no second value
        guard case .object = v else { return .fail }
        return .ok(v)
    }

    final class Parser {
        let scalars: [Unicode.Scalar]
        var pos = 0
        var dup = false

        init(scalars: [Unicode.Scalar]) { self.scalars = scalars }

        func skipWs() {
            while pos < scalars.count {
                let c = scalars[pos]
                if c == " " || c == "\t" || c == "\n" || c == "\r" { pos += 1 } else { break }
            }
        }

        func peek() -> Unicode.Scalar? { pos < scalars.count ? scalars[pos] : nil }

        func parseValue(dup: inout Bool) -> Value? {
            guard let c = peek() else { return nil }
            switch c {
            case "{": return parseObject(dup: &dup)
            case "[": return parseArray(dup: &dup)
            case "\"": return parseString().map { .string($0) }
            case "t":
                return expect("true") ? .bool(true) : nil
            case "f":
                return expect("false") ? .bool(false) : nil
            case "n":
                return expect("null") ? Value.null : nil
            case "-", "0"..."9":
                return parseNumber()
            default:
                return nil
            }
        }

        func expect(_ word: String) -> Bool {
            let w = Array(word.unicodeScalars)
            guard pos + w.count <= scalars.count else { return false }
            for (i, c) in w.enumerated() where scalars[pos + i] != c { return false }
            pos += w.count
            return true
        }

        func parseObject(dup: inout Bool) -> Value? {
            pos += 1 // {
            var entries: [(String, Value)] = []
            var seen: Set<String> = []
            skipWs()
            if peek() == "}" { pos += 1; return .object(entries) }
            while true {
                skipWs()
                guard peek() == "\"" else { return nil }
                guard let key = parseString() else { return nil }
                if !seen.insert(key).inserted { dup = true; return nil }
                skipWs()
                guard peek() == ":" else { return nil }
                pos += 1
                skipWs()
                guard let v = parseValue(dup: &dup) else { return nil }
                entries.append((key, v))
                skipWs()
                guard let c = peek() else { return nil }
                if c == "," { pos += 1; continue }
                if c == "}" { pos += 1; return .object(entries) }
                return nil
            }
        }

        func parseArray(dup: inout Bool) -> Value? {
            pos += 1 // [
            var items: [Value] = []
            skipWs()
            if peek() == "]" { pos += 1; return .array(items) }
            while true {
                skipWs()
                guard let v = parseValue(dup: &dup) else { return nil }
                items.append(v)
                skipWs()
                guard let c = peek() else { return nil }
                if c == "," { pos += 1; continue }
                if c == "]" { pos += 1; return .array(items) }
                return nil
            }
        }

        func parseString() -> String? {
            pos += 1 // opening quote
            var out = String.UnicodeScalarView()
            while pos < scalars.count {
                let c = scalars[pos]
                pos += 1
                if c == "\"" {
                    return String(out)
                }
                if c == "\\" {
                    guard pos < scalars.count else { return nil }
                    let e = scalars[pos]
                    pos += 1
                    switch e {
                    case "\"": out.append("\"")
                    case "\\": out.append("\\")
                    case "/": out.append("/")
                    case "b": out.append("\u{08}")
                    case "f": out.append("\u{0C}")
                    case "n": out.append("\n")
                    case "r": out.append("\r")
                    case "t": out.append("\t")
                    case "u":
                        guard pos + 4 <= scalars.count else { return nil }
                        var v: UInt32 = 0
                        for _ in 0..<4 {
                            guard let d = Hex.hexVal(scalars[pos]) else { return nil }
                            v = v << 4 | UInt32(d)
                            pos += 1
                        }
                        out.append(Unicode.Scalar(v)!)
                    default:
                        return nil
                    }
                    continue
                }
                if c.value < 0x20 { return nil } // unescaped control
                out.append(c)
            }
            return nil
        }

        func parseNumber() -> Value? {
            let start = pos
            var isFloat = false
            if peek() == "-" { pos += 1 }
            var digits = 0
            while let c = peek(), c.value >= 48, c.value <= 57 {
                pos += 1; digits += 1
            }
            if digits == 0 { return nil }
            if peek() == "." {
                isFloat = true
                pos += 1
                var fd = 0
                while let c = peek(), c.value >= 48, c.value <= 57 { pos += 1; fd += 1 }
                if fd == 0 { return nil }
            }
            if peek() == "e" || peek() == "E" {
                isFloat = true
                pos += 1
                if peek() == "+" || peek() == "-" { pos += 1 }
                var ed = 0
                while let c = peek(), c.value >= 48, c.value <= 57 { pos += 1; ed += 1 }
                if ed == 0 { return nil }
            }
            let s = String(String.UnicodeScalarView(scalars[start..<pos]))
            if isFloat { return .float(s) }
            guard let v = Int64(s) else { return .int(Int64.max) } // too large: not an integer
            return .int(v)
        }
    }

    public static func object(_ v: Value) -> [(String, Value)]? {
        guard case .object(let o) = v else { return nil }
        return o
    }

    public static func member(_ v: Value, _ key: String) -> Value? {
        guard case .object(let o) = v else { return nil }
        for (k, val) in o where k == key { return val }
        return nil
    }

    public static func asInt(_ v: Value?) -> Int64? {
        guard case .int(let i)? = v else { return nil }
        return i
    }

    public static func asString(_ v: Value?) -> String? {
        guard case .string(let s)? = v else { return nil }
        return s
    }
}

/// One portable-index document as section 9 / section 14.3 use it.
public struct IndexDoc {
    public let raw: JSON.Value
    public let network: String?
    public let headerHashHex: String?   // display order, 64 hex digits, or nil
    public let headerHeight: UInt64?    // integer 0 … 2^53-1, or nil
    public let headerTime: UInt32?      // integer 0 … 2^32-1, or nil
    public let usableHeader: Bool        // header object with valid hash+height
    public let plants: [IndexPlant]
    public let plantMember: IndexPlant?  // top-level `plant` object
    public let hasPlantSet: Bool         // a `plants` array was present

    public init?(body: [UInt8]) {
        guard case .ok(let v) = JSON.parseObjectOnly(body) else { return nil }
        self.raw = v
        self.network = JSON.asString(JSON.member(v, "network"))
        var uHash: String? = nil
        var uHeight: UInt64? = nil
        var uTime: UInt32? = nil
        var usable = false
        if let h = JSON.member(v, "header"), case .object = h {
            // A successful header read needs header.hash (64 hex digits) and
            // header.height (integer 0 … 2^53 − 1). `time` is read separately.
            if let hs = JSON.asString(JSON.member(h, "hash")),
               let bytes = Hex.decode(hs, expectBytes: 32) {
                uHash = hs
                if let ht = JSON.asInt(JSON.member(h, "height")), ht >= 0,
                   UInt64(ht) <= Spec.heightMax {
                    uHeight = UInt64(ht)
                    usable = true
                    if let t = JSON.asInt(JSON.member(h, "time")), t >= 0, t <= Int64(UInt32.max) {
                        uTime = UInt32(t)
                    }
                }
            }
        }
        self.headerHashHex = uHash
        self.headerHeight = uHeight
        self.headerTime = uTime
        self.usableHeader = usable

        var plants: [IndexPlant] = []
        self.hasPlantSet = JSON.member(v, "plants") != nil
        if let arr = JSON.member(v, "plants"), case .array(let items) = arr {
            for it in items {
                if let p = IndexPlant(value: it) { plants.append(p) }
            }
        }
        self.plants = plants
        if let pm = JSON.member(v, "plant"), case .object = pm {
            self.plantMember = IndexPlant(value: pm)
        } else {
            self.plantMember = nil
        }
    }

    /// The natural-order beacon bytes of header.hash (the single reversal of
    /// section 9): natural[i] = display[31 − i].
    public var headerHashNatural: [UInt8]? {
        guard let hs = headerHashHex, let d = Hex.decode(hs, expectBytes: 32) else { return nil }
        return Array(d.reversed())
    }
}

/// A recorded plant (section 14.3 step 2): root natural hex, txid and
/// block_hash Bitcoin display hex, vout and block_height integers.
public struct IndexPlant: Equatable {
    public let root: [UInt8]        // natural
    public let txid: [UInt8]         // natural (already reversed from display hex)
    public let vout: UInt32
    public let blockHash: [UInt8]    // natural (already reversed)
    public let blockHeight: UInt32

    public init?(value: JSON.Value) {
        guard case .object = value else { return nil }
        guard let rootHex = JSON.asString(JSON.member(value, "root")),
              let root = Hex.decode(rootHex, expectBytes: 32) else { return nil }
        guard let txidHex = JSON.asString(JSON.member(value, "txid")),
              let txidD = Hex.decode(txidHex, expectBytes: 32) else { return nil }
        guard let bhHex = JSON.asString(JSON.member(value, "block_hash")),
              let bhD = Hex.decode(bhHex, expectBytes: 32) else { return nil }
        guard let vout = JSON.asInt(JSON.member(value, "vout")), vout >= 0, vout <= Int64(UInt32.max) else { return nil }
        guard let bht = JSON.asInt(JSON.member(value, "block_height")), bht >= 0, bht <= Int64(UInt32.max) else { return nil }
        self.root = root
        self.txid = Array(txidD.reversed())
        self.vout = UInt32(vout)
        self.blockHash = Array(bhD.reversed())
        self.blockHeight = UInt32(bht)
    }
}

/// The provider of index documents. Each `get` is one section 9 read: the
/// fixture provider serializes the compact JSON document and the caller
/// parses it per read.
public protocol IndexProvider {
    func get(height: UInt64) -> String?
}

/// The synthetic test-plane index of CASES.md.
public struct FixtureIndex: IndexProvider {
    public let minHeight: UInt64 = 989
    public let maxHeight: UInt64 = 1018

    public init() {}

    /// Natural bytes at that height: the beacon stored there, or the filler.
    public func naturalBytes(at h: UInt64) -> [UInt8] {
        if h == Fixture.seedBeaconHeight { return Fixture.seedBeacon }
        if h > Fixture.seedBeaconHeight && h <= Fixture.lastInterruptHeight {
            return Fixture.interrupt(Int(h - Fixture.seedBeaconHeight) - 1)
        }
        return Fixture.indexFiller(h)
    }

    public func get(height: UInt64) -> String? {
        guard height >= minHeight && height <= maxHeight else { return nil }
        let natural = naturalBytes(at: height)
        let display = Hex.encode(Array(natural.reversed()))
        let time = 1_700_000_000 + 600 * UInt64(height)
        return "{\"network\":\"synthetic\",\"header\":{\"hash\":\"\(display)\",\"height\":\(height),\"time\":\(time)}}"
    }
}
