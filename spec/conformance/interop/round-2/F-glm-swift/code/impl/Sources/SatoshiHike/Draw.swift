// SECTION 14.6 draw (Rule A4.4) and SECTION 15.5 thorough-round draw, each from
// a supplied byte source. Production verify reads the operating-system CSPRNG;
// a rig may supply draw bytes (section 16.1 "Draw bytes", never a production
// input).

#if canImport(Glibc)
import Glibc
import CSatoshiKernels
#endif

public protocol ByteSource {
    /// The next 8 bytes, or nil when the source is exhausted (section 14.6
    /// "not read").
    mutating func next8() -> [UInt8]?
}

/// Operating-system CSPRNG (getrandom on Linux; section 14.6).
public struct CSPRNGSource: ByteSource {
    public init() {}

    public mutating func next8() -> [UInt8]? {
        var buf = [UInt8](repeating: 0, count: 8)
        let ok = buf.withUnsafeMutableBytes { bp in
            shk_os_getrandom(bp.baseAddress, 8) == 0
        }
        return ok ? buf : nil
    }
}

/// The section 16.1 Draw bytes chain from a stated ASCII label:
/// b_0 = SHA-256(label), b_(n+1) = SHA-256(b_n), concatenated. Each draw
/// reads the chain 8 bytes at a time, always from b_0 (no draw continues
/// where another stopped).
public struct ChainSource: ByteSource {
    let label: String
    var blocks: [[UInt8]] = []
    var pos = 0

    public init(label: String) {
        self.label = label
        blocks.append(SHK.sha256(Array(label.utf8)))
    }

    mutating func extend() {
        if let last = blocks.last {
            blocks.append(SHK.sha256(last))
        }
    }

    public mutating func next8() -> [UInt8]? {
        while pos + 8 > blocks.count * 32 {
            extend()
            if blocks.count > 1_000_000 { return nil } // unreachable in practice
        }
        let block = pos / 32
        let off = pos % 32
        let b = blocks[block]
        let v = Array(b[off..<off + 8])
        pos += 8
        return v
    }

    public static func make(label: String) -> ChainSource { ChainSource(label: label) }
}

/// A fixed byte source (a rig's injected draw bytes).
public struct FixedSource: ByteSource {
    let bytes: [UInt8]
    var pos = 0
    public init(_ bytes: [UInt8]) { self.bytes = bytes }
    public mutating func next8() -> [UInt8]? {
        guard pos + 8 <= bytes.count else { return nil }
        let v = Array(bytes[pos..<pos + 8])
        pos += 8
        return v
    }
}

public enum Draw {
    /// SECTION 14.6 one unbiased draw: M = 2^64, limit = M − (M mod n);
    /// read 8 bytes; u = u64_le; if u < limit return u mod n; otherwise the
    /// bytes are consumed and the draw repeats. When n is a power of two,
    /// limit = M and the first draw is used.
    public static func oneDraw(n: UInt64, source: inout some ByteSource) -> UInt64? {
        if n == 0 { return nil }
        let mod = modulus2_64(n)                 // (2^64) mod n
        let powerOfTwo = mod == 0
        // limit = 2^64 - mod, computed in wrapping UInt64.
        let lim = 0 &- mod
        while true {
            guard let b = source.next8() else { return nil }
            let u = LE.u64(b)
            if powerOfTwo || u < lim {
                return u % n
            }
        }
    }

    static func modulus2_64(_ n: UInt64) -> UInt64 {
        // (2^64) mod n without a 128-bit type: 2^64 mod n = (2^32 mod n * 2^32) mod n.
        let lo = powmod(2, 32, n)
        return (lo &* lo) % n
    }

    static func powmod(_ base: UInt64, _ exp: UInt64, _ mod: UInt64) -> UInt64 {
        var result: UInt64 = 1
        var b = base % mod
        var e = exp
        while e > 0 {
            if e & 1 == 1 { result = mulmod(result, b, mod) }
            b = mulmod(b, b, mod)
            e >>= 1
        }
        return result
    }

    static func mulmod(_ a: UInt64, _ b: UInt64, _ m: UInt64) -> UInt64 {
        if m == 0 { return 0 }
        var a = a % m
        var bb = b
        var result: UInt64 = 0
        while bb > 0 {
            if bb & 1 == 1 { result = addmod(result, a, m) }
            a = addmod(a, a, m)
            bb >>= 1
        }
        return result
    }

    static func addmod(_ a: UInt64, _ b: UInt64, _ m: UInt64) -> UInt64 {
        if m == 0 { return 0 }
        let (s, overflow) = a.addingReportingOverflow(b)
        if overflow {
            // a + b = 2^64 + (s); (2^64 + s) mod m = (0 + s + (2^64 mod m)) mod m
            return (s % m &+ modulus2_64(m)) % m
        }
        return s % m
    }

    /// SECTION 14.6 six-leg draw: repeat the unbiased draw, discarding an
    /// index already chosen, until 6 distinct indices. nil = not read.
    public static func sixLeg(legTotal: UInt64, source: inout some ByteSource) -> [UInt64]? {
        guard legTotal >= 6 else { return nil }
        var chosen: [UInt64] = []
        var seen = Set<UInt64>()
        while chosen.count < 6 {
            guard let q = oneDraw(n: legTotal, source: &source) else { return nil }
            if seen.insert(q).inserted { chosen.append(q) }
        }
        return chosen
    }

    /// SECTION 15.5 thorough draw: D = 6 × rounds drawn as one sequence, split
    /// in draw order into rounds of 6. Indices stay distinct while an unused
    /// index remains; a candidate already in the current group of 6 is
    /// discarded.
    public static func thorough(legTotal: UInt64, rounds: Int,
                                source: inout some ByteSource) -> [UInt64]? {
        guard legTotal >= 6 else { return nil }
        let D = 6 * rounds
        var phase: [UInt64] = []
        var inPhase = Set<UInt64>()
        while phase.count < D {
            guard let q = oneDraw(n: legTotal, source: &source) else { return nil }
            let completeGroups = phase.count / 6
            let groupStart = 6 * completeGroups
            let currentGroup = Set(phase[min(groupStart, phase.count)...])
            if currentGroup.contains(q) { continue }
            if inPhase.contains(q) && inPhase.count < Int(legTotal) { continue }
            phase.append(q)
            inPhase.insert(q)
        }
        return phase
    }
}
