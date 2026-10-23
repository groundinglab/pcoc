// SECTION 14.3 canonical path body: decode, canonical_sibling_count, and
// validate_for. Round 2's objects carry empty paths (path_skip), so these are
// not exercised by the fixtures, but the section 14 verifier implements them.

public struct CanonicalPathBody {
    public let leaf: [UInt8]
    public let leafIndex: UInt32
    public let treeSize: UInt32
    public let siblings: [[UInt8]]
    public let root: [UInt8]
    public let txid: [UInt8]
    public let vout: UInt32
    public let blockHash: [UInt8]
    public let blockHeight: UInt32

    /// Decode one canonical path body. Version byte 0x01; length is
    /// 146 + 32 × sibling_count and no other length decodes; sibling_count at
    /// most 32; tree_size at least 1; leaf_index in 0 … tree_size − 1; the
    /// sibling count equals canonical_sibling_count(tree_size).
    public init?(_ bytes: [UInt8]) {
        guard bytes.count >= 147 else { return nil }
        guard bytes[0] == 0x01 else { return nil }
        let count = Int(bytes[145])
        guard bytes.count == 146 + 32 * count else { return nil }
        guard count <= 32 else { return nil }
        var p = 1
        func take(_ n: Int) -> [UInt8] {
            let v = Array(bytes[p..<p + n]); p += n; return v
        }
        func be32(_ b: [UInt8]) -> UInt32 {
            (UInt32(b[0]) << 24) | (UInt32(b[1]) << 16) | (UInt32(b[2]) << 8) | UInt32(b[3])
        }
        self.leaf = take(32)
        self.leafIndex = be32(take(4))
        self.treeSize = be32(take(4))
        p += 1 // sibling_count, already read at 145
        var sibs: [[UInt8]] = []
        for _ in 0..<count { sibs.append(take(32)) }
        self.siblings = sibs
        self.root = take(32)
        self.txid = take(32)
        self.vout = be32(take(4))
        self.blockHash = take(32)
        self.blockHeight = be32(take(4))
        guard treeSize >= 1 else { return nil }
        guard leafIndex < treeSize else { return nil }
        guard count == CanonicalPathBody.canonicalSiblingCount(Int(treeSize)) else { return nil }
    }

    /// canonical_sibling_count(tree_size): counts one level while width > 1,
    /// setting width = floor(width/2) + (width mod 2) after each level.
    public static func canonicalSiblingCount(_ treeSize: Int) -> Int {
        var width = treeSize
        var count = 0
        while width > 1 {
            width = width / 2 + (width % 2)
            count += 1
        }
        return count
    }

    /// For a non-empty stages[17].path: read block_hash and block_height only,
    /// far enough into the canonical layout (section 14.3). No validate_for.
    public static func lenientBlockLocator(_ bytes: [UInt8]) -> (blockHash: [UInt8], blockHeight: UInt32)? {
        guard bytes.count >= 147, bytes[0] == 0x01 else { return nil }
        let count = Int(bytes[41])
        guard bytes.count == 146 + 32 * count else { return nil }
        func be32(_ o: Int) -> UInt32 {
            (UInt32(bytes[o]) << 24) | (UInt32(bytes[o + 1]) << 16) |
            (UInt32(bytes[o + 2]) << 8) | UInt32(bytes[o + 3])
        }
        let bhOff = 110 + 32 * count   // block_hash
        let bhtOff = 142 + 32 * count  // block_height
        guard bhtOff + 4 <= bytes.count else { return nil }
        return (Array(bytes[bhOff..<bhOff + 32]), be32(bhtOff))
    }

    /// validate_for(waypoint): leaf equals the waypoint and the Merkle path
    /// resolves to root. Leaf node: SHA-256(0x1A ‖ leaf). Parent node:
    /// SHA-256(0x1B ‖ left ‖ right).
    public func validate(for waypoint: [UInt8]) -> Bool {
        guard leaf == waypoint else { return false }
        var node = SHK.sha256(concat: [[0x1A], leaf])
        var index = Int(leafIndex)
        var width = Int(treeSize)
        for sib in siblings {
            if width % 2 == 1 && index == width - 1 {
                guard sib == node else { return false }
                node = SHK.sha256(concat: [[0x1B], node, node])
            } else if index % 2 == 0 {
                node = SHK.sha256(concat: [[0x1B], node, sib])
            } else {
                node = SHK.sha256(concat: [[0x1B], sib, node])
            }
            index = index / 2
            width = width / 2 + (width % 2)
        }
        return node == root
    }
}
