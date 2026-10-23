// The SPEC 2.2 section 16 permanent negatives. This module lives only in the
// candidate generator (`gen-candidates`); the SDK paths (verify, produce,
// bench) never link it. Each negative is a construction this file forbids,
// computed here only so the pack can pin it as a non-match.

import SatoshiHike

public enum NegativeTerrainKind {
    case oneLaneFlip
    case all4Kernel
    case squeezedSeed
}

public enum NegativeLegKind {
    case slotSummaryFold
    case xorAccumulatorFold
    case xorWFold
    case sPlusWFold
    case partialSlotRead
    case partialSlotRead2047
    case prefoldHoist
    case finalStageHoist
    case cachedLeg
}

/// The §10.4 dag2 fill with a forbidden base word map (one_lane_flip,
/// all4_kernel) or with the four lanes replaced by P0⊕P1⊕P2⊕P3
/// (squeezed_seed_terrain, the §10.2 squeeze). Parents and slot_mix are
/// unchanged. Terrain is filled once and the digest is the SHA-256 of the
/// whole array.
public enum Negatives {
    public static func terrainDigest(publicKey: [UInt8], seedBeacon: [UInt8], profile: Profile,
                                     kind: NegativeTerrainKind) -> [UInt8] {
        var lanes = Seed.lanes(Seed.seed(publicKey: publicKey, seedBeacon: seedBeacon))
        if case .squeezedSeed = kind {
            let x = lanes[0] ^ lanes[1] ^ lanes[2] ^ lanes[3]
            lanes = [x, x, x, x]
        }
        let nSlots = profile.nSlots
        let words = profile.wordsPerSlot
        let slotBytes = profile.slotBytes
        let terrain = UnsafeMutableRawPointer.allocate(byteCount: profile.terrainBytes,
                                                       alignment: 64)
        defer { terrain.deallocate() }
        var w = [UInt64](repeating: 0, count: words)
        for i in 0..<UInt64(nSlots) {
            let ish = i << 32
            for j in 0..<words {
                let c = ish ^ UInt64(j) ^ Spec.wordXor
                var t: UInt64
                switch kind {
                case .oneLaneFlip:
                    t = mix64(lanes[j % 4] ^ c)
                case .all4Kernel:
                    t = mix64(lanes[j % 4] ^ rotl64(lanes[(j + 1) % 4], 17)
                              ^ rotl64(lanes[(j + 2) % 4], 31)
                              ^ rotl64(lanes[(j + 3) % 4], 47) ^ c)
                case .squeezedSeed:
                    t = mix64(lanes[j % 4] ^ c)
                    t = mix64(t ^ rotl64(lanes[(j + 1) % 4], 17))
                    t = mix64(t ^ rotl64(lanes[(j + 2) % 4], 31))
                    t = mix64(t ^ rotl64(lanes[(j + 3) % 4], 47))
                }
                w[j] = t
            }
            if i > 0 {
                let bits = max(floorLog2(i), 1)
                let parents = [i - 1,
                               (UInt64(reverse32(UInt32(truncatingIfNeeded: i))) >> (32 - bits)) % i,
                               ((i &* Spec.mixAdd) >> 32) % i]
                for k in 1...3 {
                    let pw = terrain + Int(parents[k - 1]) * slotBytes
                    let kc = UInt64(k) &* Spec.parentMul
                    for j in 0..<words {
                        let rot = UInt64((j + k) % 64)
                        var pv: UInt64 = 0
                        for b in 0..<8 {
                            pv |= UInt64((pw + j * 8 + b).load(as: UInt8.self)) << (8 * UInt64(b))
                        }
                        w[j] = mix64((w[j] ^ rotl64(pv, rot)) ^ kc)
                    }
                }
                let sm = i &* Spec.slotMul
                for j in 0..<words { w[j] = mix64(w[j] ^ sm) }
            }
            let dst = terrain + Int(i) * slotBytes
            for j in 0..<words {
                let v = w[j]
                for b in 0..<8 {
                    (dst + j * 8 + b).storeBytes(of: UInt8(truncatingIfNeeded: v >> (8 * UInt64(b))), as: UInt8.self)
                }
            }
        }
        return SHK.sha256(buffer: UnsafeRawPointer(terrain), count: profile.terrainBytes)
    }

    static func loadWord(_ ptr: UnsafeRawPointer, _ j: Int) -> UInt64 {
        var v: UInt64 = 0
        for b in 0..<8 {
            v |= UInt64((ptr + j * 8 + b).load(as: UInt8.self)) << (8 * UInt64(b))
        }
        return v
    }

    /// One negative step over the honest terrain with a forbidden fold, then
    /// the §11.1 seal (SHA-256(SEAL_DOMAIN ‖ le64(S_folded)) → u64_le(E[0:8])).
    static func seal(_ sFolded: UInt64) -> UInt64 {
        let e = SHK.sha256(concat: [Spec.sealDomain, LE.le64(sFolded)])
        return LE.u64(Array(e[0..<8]))
    }

    /// The first leg of stage 0 computed with a forbidden fold, starting from
    /// the seed (Rule A2.2). f3SharedTerrain uses the honest fold on another
    /// key's terrain, so it is driven by an explicit terrain argument.
    public static func negativeLegResult(kind: NegativeLegKind, profile: Profile,
                                         startSeed: [UInt8],
                                         terrain: UnsafeRawPointer,
                                         nSlots: Int, wordsPerSlot: Int,
                                         steps: Int,
                                         requireSlotRevisit: Bool = false,
                                         slotRevisitFound: inout Bool) -> UInt64 {
        var S = LE.u64(Array(startSeed[0..<8]))   // S_0 = u64_le(D[0:8])
        var prefoldCache: [UInt64: UInt64] = [:]  // prefold_hoist: per-slot pre(i)
        var cachedFold: [UInt64: UInt64] = [:]    // cached_leg: slot → S_folded
        let slotBytes = wordsPerSlot * 8
        for _ in 0..<steps {
            let idx = S % UInt64(nSlots)
            let slot = terrain + Int(idx) * slotBytes
            var sFolded: UInt64
            switch kind {
            case .slotSummaryFold:
                var x: UInt64 = 0
                for j in 0..<wordsPerSlot { x ^= loadWord(slot, j) ^ UInt64(j) }
                sFolded = S ^ x
            case .xorAccumulatorFold:
                var acc: UInt64 = 0
                for j in 0..<wordsPerSlot { acc ^= mix64(loadWord(slot, j) ^ UInt64(j) ^ S) }
                sFolded = acc
            case .xorWFold:
                var acc: UInt64 = 0
                for j in 0..<wordsPerSlot { acc = acc &+ mix64(loadWord(slot, j) ^ UInt64(j) ^ S) }
                sFolded = acc
            case .sPlusWFold:
                var acc: UInt64 = 0
                for j in 0..<wordsPerSlot { acc = acc &+ mix64(loadWord(slot, j) ^ ((S &+ UInt64(j)))) }
                sFolded = acc
            case .partialSlotRead:
                sFolded = mix64(loadWord(slot, 0) ^ S)
            case .partialSlotRead2047:
                var acc: UInt64 = 0
                for j in 0..<(wordsPerSlot - 1) { acc = acc &+ mix64(loadWord(slot, j) ^ S) }
                sFolded = acc
            case .prefoldHoist:
                if let pre = prefoldCache[idx] {
                    sFolded = pre &+ S
                } else {
                    var acc: UInt64 = 0
                    for j in 0..<wordsPerSlot { acc = acc &+ mix64(loadWord(slot, j) ^ UInt64(j)) }
                    prefoldCache[idx] = acc
                    sFolded = acc &+ S
                }
            case .finalStageHoist:
                var x: UInt64 = 0
                for j in 0..<wordsPerSlot { x ^= mix64Prelast(loadWord(slot, j) ^ S) }
                sFolded = x ^ (x >> 31)
            case .cachedLeg:
                if let cached = cachedFold[idx] {
                    slotRevisitFound = true
                    sFolded = cached
                } else {
                    var acc: UInt64 = 0
                    for j in 0..<wordsPerSlot { acc = acc &+ mix64(loadWord(slot, j) ^ S) }
                    cachedFold[idx] = acc
                    sFolded = acc
                }
            }
            S = seal(sFolded)
        }
        if requireSlotRevisit && kind == .cachedLeg && !slotRevisitFound {
            fatalError("cached_leg negative: the fixture leg does not revisit a slot; pick other fixture inputs")
        }
        return S
    }

    /// f3_shared_terrain: pk_A's first leg of stage 0, from pk_A's own start
    /// (the seed), run on pk_B's terrain (same seed_beacon). This is a leg
    /// value, not a terrain digest. It uses the honest §11.1 fold on the
    /// wrong terrain; it guards section 19 row F4 (shared terrain).
    public static func f3SharedTerrainLegResult(profile: Profile, startSeed: [UInt8],
                                                 terrainB: UnsafeRawPointer) -> UInt64 {
        var S = LE.u64(Array(startSeed[0..<8]))
        for _ in 0..<profile.stepsPerLeg {
            let idx = S % UInt64(profile.nSlots)
            let slot = terrainB + Int(idx) * profile.slotBytes
            var acc: UInt64 = 0
            for j in 0..<profile.wordsPerSlot {
                acc = acc &+ mix64(loadWord(slot, j) ^ S)
            }
            S = seal(acc)
        }
        return S
    }
}
