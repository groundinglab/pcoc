// SECTION 13.1 Hashcash: the challenge, the digest, the difficulty, the solve.
// Produce and Bari use exactly this puzzle.

public enum Hashcash {
    public static let challengePrefix: [UInt8] = Array("bari-hc-v2".utf8) // ten ASCII bytes
    public static let difficultyBits = 21                                    // P13.5

    /// bari_origin: the ASCII serialization of a web origin under RFC 6454
    /// section 6.2 — lowercase scheme, ://, lowercase host (A-label form),
    /// and :port only when it is not the scheme's default.
    public static func webOrigin(scheme: String, host: String, port: Int?) -> String {
        let s = scheme.lowercased()
        let h = host.lowercased()
        let defaultPort = (s == "https") ? 443 : (s == "http") ? 80 : nil
        if let p = port, p != defaultPort {
            return "\(s)://\(h):\(p)"
        }
        return "\(s)://\(h)"
    }

    /// challenge = SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash).
    public static func challenge(bariOrigin: String, headerHash: [UInt8]) -> [UInt8] {
        SHK.sha256(concat: [challengePrefix, Array(bariOrigin.utf8), headerHash])
    }

    /// digest = SHA-256(challenge ‖ waypoint ‖ le64(nonce)).
    public static func digest(challenge: [UInt8], waypoint: [UInt8], nonce: UInt64) -> [UInt8] {
        SHK.sha256(concat: [challenge, waypoint, LE.le64(nonce)])
    }

    /// Leading zero bits counted from the first digest byte; inside a byte the
    /// high bit is first. A zero byte contributes eight bits and the count
    /// continues (section 13.1).
    public static func leadingZeroBits(_ digest: [UInt8]) -> Int {
        var bits = 0
        for b in digest {
            if b == 0 {
                bits += 8
                continue
            }
            var v = b
            while v & 0x80 == 0 {
                bits += 1
                v <<= 1
            }
            break
        }
        return bits
    }

    public struct Solution {
        public let nonce: UInt64
        public let digest: [UInt8]
        public let leadingZeroBits: Int
    }

    /// The nonce search starts at 0 (P13.6): the first u64 in the order
    /// 0, 1, 2, … whose digest has at least 21 leading zero bits.
    public static func solve(challenge: [UInt8], waypoint: [UInt8],
                             difficulty: Int = difficultyBits) -> Solution {
        var nonce: UInt64 = 0
        while true {
            let d = digest(challenge: challenge, waypoint: waypoint, nonce: nonce)
            if leadingZeroBits(d) >= difficulty {
                return Solution(nonce: nonce, digest: d,
                                leadingZeroBits: leadingZeroBits(d))
            }
            nonce &+= 1
        }
    }
}
