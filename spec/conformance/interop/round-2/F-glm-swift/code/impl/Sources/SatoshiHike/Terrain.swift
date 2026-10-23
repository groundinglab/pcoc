// SECTION 10 terrain: the seed, the lanes, the dag2 fill (10.4), and the
// terrain sample (11.3). Terrain is filled once per PCOC (10.4, 11.1).

import CSatoshiKernels

public enum Seed {
    /// SECTION 10. seed_preimage = public_key ‖ seed_beacon (64 bytes, no label);
    /// D = SHA-256(seed_preimage); P[k] = u64_le(D[8k:8k+8]).
    public static func preimage(publicKey: [UInt8], seedBeacon: [UInt8]) -> [UInt8] {
        publicKey + seedBeacon
    }

    public static func seed(publicKey: [UInt8], seedBeacon: [UInt8]) -> [UInt8] {
        SHK.sha256(preimage(publicKey: publicKey, seedBeacon: seedBeacon))
    }

    public static func lanes(_ D: [UInt8]) -> [UInt64] {
        (0..<4).map { k in LE.u64(Array(D[8 * k..<8 * k + 8])) }
    }
}

/// The terrain of one PCOC: TERRAIN_BYTES filled once by section 10.4 from the
/// seed lanes, then left unchanged. The step does not write it (10.4).
public final class Terrain {
    public let profile: Profile
    public let publicKey: [UInt8]
    public let seedBeacon: [UInt8]
    public let seed: [UInt8]        // D
    public let lanes: [UInt64]       // P[0..3]
    public let nSlots: Int
    public let wordsPerSlot: Int
    let buf: UnsafeMutableRawPointer
    let bufSize: Int

    public init(publicKey: [UInt8], seedBeacon: [UInt8], profile: Profile,
                scalar: Bool = false) {
        precondition(publicKey.count == 32 && seedBeacon.count == 32)
        self.profile = profile
        self.publicKey = publicKey
        self.seedBeacon = seedBeacon
        self.seed = Seed.seed(publicKey: publicKey, seedBeacon: seedBeacon)
        self.lanes = Seed.lanes(seed)
        self.nSlots = profile.nSlots
        self.wordsPerSlot = profile.wordsPerSlot
        bufSize = profile.terrainBytes
        buf = UnsafeMutableRawPointer.allocate(byteCount: bufSize, alignment: 64)
        var P = lanes
        if scalar {
            TerrainFill.scalarReference(buf, nSlots: nSlots, wordsPerSlot: wordsPerSlot, P: P)
        } else {
            P.withUnsafeBufferPointer { pb in
                shk_terrain_fill(buf, UInt64(nSlots), wordsPerSlot, pb.baseAddress)
            }
        }
    }

    deinit { buf.deallocate() }

    public var rawPointer: UnsafeRawPointer { UnsafeRawPointer(buf) }

    /// SHA-256 over the whole terrain array (the pack's terrain digest).
    public var digest: [UInt8] {
        SHK.sha256(buffer: rawPointer, count: bufSize)
    }

    /// A full slot as bytes (slot i is terrain[i*SLOT_BYTES : (i+1)*SLOT_BYTES]).
    public func slotBytes(_ index: UInt64) -> [UInt8] {
        let off = Int(index) * profile.slotBytes
        return [UInt8](UnsafeRawBufferPointer(start: rawPointer + off, count: profile.slotBytes))
    }

    /// SECTION 11.1 fold over the whole slot, mixed with S (dispatched to the
    /// best kernel; identical bytes to the sequential loop).
    public func fold(slotIndex: UInt64, S: UInt64, opt: Int32 = FoldOpt.auto) -> UInt64 {
        let off = Int(slotIndex) * profile.slotBytes
        return shk_fold(rawPointer + off, wordsPerSlot, S)
    }

    public func foldScalar(slotIndex: UInt64, S: UInt64) -> UInt64 {
        let off = Int(slotIndex) * profile.slotBytes
        return shk_fold_scalar(rawPointer + off, wordsPerSlot, S)
    }

    /// SECTION 11.3 terrain sample at a previous checkpoint: SAMPLE_BYTES at
    /// offset u64_le(previous[0:8]) mod OFFSET_MODULUS. Not the waypoint slot
    /// sample (11.3).
    public func sample(at previous: [UInt8]) -> [UInt8] {
        precondition(previous.count == 32)
        let offset = Int(LE.u64(Array(previous[0..<8])) % profile.offsetModulus)
        return [UInt8](UnsafeRawBufferPointer(start: rawPointer + offset,
                                              count: profile.sampleBytes))
    }

    /// SECTION 12.2 slot sample: the full slot selected by the interrupt hash.
    public func slotSample(beacon: [UInt8]) -> [UInt8] {
        precondition(beacon.count == 32)
        let slotIndex = LE.u64(Array(beacon[0..<8])) % UInt64(nSlots)
        return slotBytes(slotIndex)
    }
}

/// SECTION 10.4 dag2 fill, honest NL4 base word only. This is the obviously
/// correct scalar reference; the C kernels (scalar and AVX-512) must produce
/// identical bytes (differential-tested). The Negatives target carries the
/// forbidden fill variants itself and never touches this file.
public enum TerrainFill {
    public static func scalarReference(_ terrain: UnsafeMutableRawPointer,
                                      nSlots: Int, wordsPerSlot: Int, P: [UInt64]) {
        let slotBytes = wordsPerSlot * 8
        var w = [UInt64](repeating: 0, count: wordsPerSlot)
        for i in 0..<UInt64(nSlots) {
            let ish = i << 32
            for j in 0..<wordsPerSlot {
                let c = ish ^ UInt64(j) ^ Spec.wordXor
                var t = mix64(P[j % 4] ^ c)
                t = mix64(t ^ rotl64(P[(j + 1) % 4], 17))
                t = mix64(t ^ rotl64(P[(j + 2) % 4], 31))
                t = mix64(t ^ rotl64(P[(j + 3) % 4], 47))
                w[j] = t
            }
            if i > 0 {
                let bits = max(floorLog2(i), 1)
                let parents = [i - 1,
                               (UInt64(reverse32(UInt32(truncatingIfNeeded: i))) >> (32 - bits)) % i,
                               ((i &* Spec.mixAdd) >> 32) % i]
                for k in 1...3 {
                    let pw = parents[k - 1]
                    let kc = UInt64(k) &* Spec.parentMul
                    let base = terrain + Int(pw) * slotBytes
                    for j in 0..<wordsPerSlot {
                        let rot = UInt64((j + k) % 64)
                        var pv: UInt64 = 0
                        for b in 0..<8 {
                            pv |= UInt64((base + j * 8 + b).load(as: UInt8.self)) << (8 * UInt64(b))
                        }
                        w[j] = mix64((w[j] ^ rotl64(pv, rot)) ^ kc)
                    }
                }
                let sm = i &* Spec.slotMul
                for j in 0..<wordsPerSlot { w[j] = mix64(w[j] ^ sm) }
            }
            let dst = terrain + Int(i) * slotBytes
            for j in 0..<wordsPerSlot {
                let v = w[j]
                for b in 0..<8 {
                    (dst + j * 8 + b).storeByte(UInt8(truncatingIfNeeded: v >> (8 * UInt64(b))))
                }
            }
        }
    }
}

extension UnsafeMutableRawPointer {
    fileprivate func storeByte(_ v: UInt8) {
        storeBytes(of: v, as: UInt8.self)
    }
}
