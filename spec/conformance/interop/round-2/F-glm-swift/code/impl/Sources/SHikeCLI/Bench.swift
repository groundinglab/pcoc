// Benchmarks for RESULTS.md: terrain fill and one production leg, both the
// optimised and the plain scalar path, plus SHA-256 throughput. Every bench
// run verifies the optimized bytes equal the scalar bytes before reporting.

import CSatoshiKernels
import Foundation
import SatoshiHike

enum Bench {
    static func now() -> Double {
        Double(DispatchTime.now().uptimeNanoseconds) / 1_000_000_000
    }

    static func run() {
        let brand = SHK.cpuBrand()
        print("cpu: \(brand)")
        print("sha_ni=\(SHK.cpuHasShaNI) avx2=\(SHK.cpuHasAVX2) avx512dq=\(SHK.cpuHasAVX512)")

        let pk = Fixture.pkA
        let sb = Fixture.seedBeacon

        // ---------------- SHA-256 throughput ----------------
        do {
            let buf = [UInt8](repeating: 0x61, count: 1 << 24) // 16 MiB
            var out = [UInt8](repeating: 0, count: 32)
            let t0 = now()
            buf.withUnsafeBufferPointer { shk_sha256($0.baseAddress, buf.count, &out) }
            let tAuto = now() - t0
            buf.withUnsafeBufferPointer { shk_sha256_mode($0.baseAddress, buf.count, &out, 1) }
            let tSw = now() - t0 - tAuto
            let t1 = now()
            buf.withUnsafeBufferPointer { shk_sha256_mode($0.baseAddress, buf.count, &out, 1) }
            let tSoft = now() - t1
            let t2 = now()
            buf.withUnsafeBufferPointer { shk_sha256_mode($0.baseAddress, buf.count, &out, 2) }
            let tNi = now() - t2
            print(String(format: "sha256 16MiB: auto %.3fs software %.3fs sha-ni %.3fs", tAuto, tSoft, tNi))
        }

        // ---------------- terrain fill, production ----------------
        do {
            var P = Seed.lanes(Seed.seed(publicKey: pk, seedBeacon: sb))
            let size = 268_435_456
            let bufA = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 64)
            let bufB = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 64)
            defer { bufA.deallocate(); bufB.deallocate() }
            // Touch both buffers first: a cold 256 MiB allocation pays page
            // faults that would otherwise land inside the first timed run.
            memset(bufA, 0, size)
            memset(bufB, 0, size)

            let t0 = now()
            P.withUnsafeBufferPointer { shk_terrain_fill_scalar(bufA, 16384, 2048, $0.baseAddress) }
            let tScalar = now() - t0
            let t1 = now()
            P.withUnsafeBufferPointer { shk_terrain_fill_avx512(bufB, 16384, 2048, $0.baseAddress) }
            let tAvx = now() - t1
            let same = memcmp(bufA, bufB, size) == 0
            print(String(format: "terrain fill 256MiB: scalar %.3fs avx512 %.3fs identical=%d",
                         tScalar, tAvx, same ? 1 : 0))
            if !same { print("BENCH FAIL: terrain bytes differ"); exit(1) }
            let dA = SHK.sha256(buffer: UnsafeRawPointer(bufA), count: size)
            print("terrain digest (scalar path) = \(Hex.encode(dA))")
        }

        // ---------------- one production leg ----------------
        do {
            var P = Seed.lanes(Seed.seed(publicKey: pk, seedBeacon: sb))
            let size = 268_435_456
            let buf = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 64)
            defer { buf.deallocate() }
            P.withUnsafeBufferPointer { shk_terrain_fill(buf, 16384, 2048, $0.baseAddress) }
            let D = Seed.seed(publicKey: pk, seedBeacon: sb)
            let S0 = LE.u64(Array(D[0..<8]))

            let t0 = now()
            let rScalar = shk_leg_run(buf, 16384, 2048, 2_000_000, S0, 0)
            let tScalar = now() - t0
            let t1 = now()
            let rAvx2 = shk_leg_run(buf, 16384, 2048, 2_000_000, S0, 1)
            let tAvx2 = now() - t1
            let t2 = now()
            let rAvx512 = shk_leg_run(buf, 16384, 2048, 2_000_000, S0, 2)
            let tAvx512 = now() - t2
            print(String(format: "one production leg (2,000,000 steps x 2048 words): scalar %.3fs avx2 %.3fs avx512 %.3fs",
                         tScalar, tAvx2, tAvx512))
            if rScalar != rAvx512 || rScalar != rAvx2 {
                print(String(format: "BENCH FAIL: leg results differ %016llx %016llx %016llx",
                             rScalar, rAvx2, rAvx512))
                exit(1)
            }
            print(String(format: "leg result = %016llx", rScalar))
        }
    }
}
