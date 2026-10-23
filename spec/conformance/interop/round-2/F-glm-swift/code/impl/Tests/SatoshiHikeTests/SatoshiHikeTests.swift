// Tests: the SPEC 7.4 checks, kernel differentials, CBOR round-trips, the
// fixture hike self-consistency through the section 14 verifier, the Rule A5
// records, and the committed-candidates check (re-run the generator and
// compare with the committed files, CASES.md output format).

import Testing
import Foundation
import CSatoshiKernels
@testable import SatoshiHike
import GenCandidatesCore
import Negatives

@Suite("SPEC 7.4 checks") struct Spec74Checks {
    @Test func le64Checks() {
        #expect(LE.le64(1) == [0x01, 0, 0, 0, 0, 0, 0, 0])
        #expect(LE.le64(0x0102030405060708) == [8, 7, 6, 5, 4, 3, 2, 1])
        #expect(LE.u64(LE.le64(0xdeadbeefcafebabe)) == 0xdeadbeefcafebabe)
    }

    @Test func sha256Abc() {
        #expect(Hex.encode(SHK.sha256(Array("abc".utf8)))
                == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    }

    @Test func sealZero() {
        // SHA-256(SEAL_DOMAIN ‖ le64(0)) = 4816bdaf…; S' = 0x76042423afbd1648.
        let d = SHK.sha256(concat: [Spec.sealDomain, LE.le64(0)])
        #expect(Hex.encode(d) == "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533")
        #expect(LE.u64(Array(d[0..<8])) == 0x76042423afbd1648)
    }

    @Test func seedExample() {
        // SPEC 2.2 section 7.4: seed preimage 32×0x11 ‖ 32×0x22.
        // The SPEC's printed digest is 5189c77d…ec4 6 8 668527…, but the correct
        // FIPS 180-4 digest (confirmed independently by Python hashlib) is
        // 5189c77d…ec469868527…, and the SPEC's own lane values
        // (P[1] = 0x27858669c45e046a etc.) are consistent only with the
        // correct digest. Recorded as a SPEC example typo in RESULTS.md.
        var pre = [UInt8](repeating: 0x11, count: 32)
        pre += [UInt8](repeating: 0x22, count: 32)
        let D = SHK.sha256(pre)
        #expect(Hex.encode(D) == "5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c")
        let lanes = Seed.lanes(D)
        #expect(Hex.u64Key(lanes[0]) == "545dfe297dc78951")
        #expect(Hex.u64Key(lanes[1]) == "27858669c45e046a")
        #expect(Hex.u64Key(lanes[2]) == "9cdac73ac1a5fe85")
        #expect(Hex.u64Key(lanes[3]) == "7c81df6efbf55f11")
        // NL4 base word at i = 0, j = 0.
        let c = Spec.wordXor
        var w = mix64(lanes[0] ^ c)
        w = mix64(w ^ rotl64(lanes[1], 17))
        w = mix64(w ^ rotl64(lanes[2], 31))
        w = mix64(w ^ rotl64(lanes[3], 47))
        #expect(Hex.u64Key(w) == "c7582082a632c2c8")
    }
}

@Suite("Kernel differentials") struct KernelDifferentials {
    @Test func terrainScalarSwiftEqualsC() {
        // Full S1 and S2 terrains: the obviously-correct Swift scalar fill
        // must equal the C kernels (both dispatch paths).
        for profile in [Fixture.S1, Fixture.S2] {
            let tC = Terrain(publicKey: Fixture.pkA, seedBeacon: Fixture.seedBeacon, profile: profile)
            let buf = UnsafeMutableRawPointer.allocate(byteCount: profile.terrainBytes, alignment: 64)
            defer { buf.deallocate() }
            let lanes = Seed.lanes(Seed.seed(publicKey: Fixture.pkA, seedBeacon: Fixture.seedBeacon))
            TerrainFill.scalarReference(buf, nSlots: profile.nSlots,
                                        wordsPerSlot: profile.wordsPerSlot, P: lanes)
            let dSwift = SHK.sha256(buffer: UnsafeRawPointer(buf), count: profile.terrainBytes)
            #expect(dSwift == tC.digest, "Swift scalar terrain differs from C fill at \(profile)")
        }
    }

    @Test func prodTerrainScalarEqualsAvx512() {
        // The full 256 MiB production terrain: scalar C == AVX-512 C.
        let pk = Fixture.pkA, sb = Fixture.seedBeacon
        let lanes = Seed.lanes(Seed.seed(publicKey: pk, seedBeacon: sb))
        let size = 268_435_456
        let bufA = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 64)
        let bufB = UnsafeMutableRawPointer.allocate(byteCount: size, alignment: 64)
        defer { bufA.deallocate(); bufB.deallocate() }
        var P = lanes
        P.withUnsafeBufferPointer { shk_terrain_fill_scalar(bufA, 16384, 2048, $0.baseAddress) }
        P.withUnsafeBufferPointer { shk_terrain_fill_avx512(bufB, 16384, 2048, $0.baseAddress) }
        #expect(memcmp(bufA, bufB, size) == 0)
    }

    @Test func foldKernelsAgree() {
        let t = Terrain(publicKey: Fixture.pkA, seedBeacon: Fixture.seedBeacon, profile: Fixture.S2)
        for k in 0..<32 {
            let slot = UInt64(k * 131) % UInt64(t.nSlots)
            let S: UInt64 = UInt64(k) &* 0x9E3779B97F4A7C15 &+ 12345
            var ref: UInt64 = 0
            let bytes = t.slotBytes(slot)
            for j in 0..<t.wordsPerSlot {
                ref = ref &+ mix64(LE.u64(Array(bytes[j * 8..<j * 8 + 8])) ^ S)
            }
            #expect(t.fold(slotIndex: slot, S: S) == ref)
            #expect(t.foldScalar(slotIndex: slot, S: S) == ref)
        }
    }

    @Test func s2LegScalarEqualsOptimized() {
        let t = Terrain(publicKey: Fixture.pkA, seedBeacon: Fixture.seedBeacon, profile: Fixture.S2)
        let a = Leg.result(terrain: t, previousCheckpoint: t.seed, opt: FoldOpt.scalar)
        let b = Leg.result(terrain: t, previousCheckpoint: t.seed, opt: FoldOpt.avx512)
        #expect(a == b)
    }
}

@Suite("CBOR") struct CBORTests {
    @Test func roundTripO1() throws {
        let o1 = try Producer.buildObject(profile: Fixture.S1, legsPerStage: Fixture.legsPerStageS1)
        guard case .hikeObject(let root) = ObjectDecode.decode(o1) else {
            Issue.record("O1 did not decode as a hike object")
            return
        }
        #expect(CBOR.asUint(CBOR.field(root, "steps_per_leg")) == 100)
        // Re-encode deterministically: byte-identical.
        #expect(Producer.reencode(root) == o1)
    }

    @Test func decodeRules() throws {
        let o1 = try Producer.buildObject(profile: Fixture.S1, legsPerStage: Fixture.legsPerStageS1)
        // Trailing byte → malformed.
        if case .hikeObject = ObjectDecode.decode(o1 + [0]) { Issue.record("trailing byte accepted") }
        // Empty → malformed.
        if case .hikeObject = ObjectDecode.decode([]) { Issue.record("empty accepted") }
        // Not a map → decode failure.
        if case .hikeObject = ObjectDecode.decode([0x01]) { Issue.record("non-map accepted") }
        // Map without kind text → decode failure.
        if case .hikeObject = ObjectDecode.decode([0xa1, 0x61, 0x61, 0x01]) { Issue.record("no-kind map accepted") }
        // Kind not KIND → Rule A5. "ab" is a 2-byte text value: 0x62 61 62.
        if case .unknownKind("ab") = ObjectDecode.decode(
            [0xa1, 0x64, 0x6b, 0x69, 0x6e, 0x64, 0x62, 0x61, 0x62]) { } else {
            Issue.record("unknown kind not detected")
        }
        // Truncated text value (declared length 3, only 2 bytes) → malformed.
        if case .hikeObject = ObjectDecode.decode(
            [0xa1, 0x64, 0x6b, 0x69, 0x6e, 0x64, 0x63, 0x61, 0x62]) {
            Issue.record("truncated text value accepted")
        }
        // A map whose kind is exactly KIND is a hike object measured by
        // section 14, not Rule A5.
        let minimalKind: [UInt8] = [0xa1, 0x64, 0x6b, 0x69, 0x6e, 0x64, 0x70] + Array(Spec.kind.utf8)
        if case .hikeObject = ObjectDecode.decode(minimalKind) { } else {
            Issue.record("bare kind==KIND map not measured")
        }
    }

    @Test func keyOrderInsensitivity() throws {
        // Reordering top-level keys is not a shape mismatch; decoding works.
        let o1 = try Producer.buildObject(profile: Fixture.S1, legsPerStage: Fixture.legsPerStageS1)
        guard case .hikeObject(let root) = ObjectDecode.decode(o1) else { return }
        guard case .map(let entries) = root else { return }
        let reversed = CBORItem.map(entries.reversed())
        #expect(CBOR.asBytes(CBOR.field(reversed, "public_key")) == Fixture.pkA)
    }
}

@Suite("Verify records") struct VerifyTests {
    func verifyOptions(_ profile: Profile, drawLabel: String? = nil) -> VerifyOptions {
        VerifyOptions(profile: profile, index: FixtureIndex(),
                      makeDrawSource: { ChainSource(label: drawLabel ?? "pcoc-pack-2.2/draw") })
    }

    func recordObject(_ v: JVal) -> [(String, JVal)] {
        if case .obj(let o) = v { return o }
        Issue.record("expected an object record")
        return []
    }

    @Test func verifyO1() throws {
        let o1 = try Producer.buildObject(profile: Fixture.S1, legsPerStage: Fixture.legsPerStageS1)
        guard case .record(let rec) = Verify.run(objectBytes: o1, options: verifyOptions(Fixture.S1)) else {
            Issue.record("no record"); return
        }
        let top = recordObject(rec)
        let dict = Dictionary(uniqueKeysWithValues: top.map { ($0.0, $0.1) })
        #expect(dict["profile"] == .str("conformance-small"))
        #expect(dict["shape"] == .str("match"))
        #expect(dict["shape_item"] == .arr([]))
        #expect(dict["seed_value"] == .str("match"))
        #expect(dict["anchored_total"] == .uint(0))
        #expect(dict["skip_list"] == .arr((1...16).map { .uint(UInt64($0)) }))
        // analysis carries hike_legs, hike_span, whole_hike_legs, whole_hike_span.
        guard case .obj(let anFields) = dict["analysis"] else {
            Issue.record("missing analysis"); return
        }
        let an = Dictionary(uniqueKeysWithValues: anFields.map { ($0.0, $0.1) })
        #expect(an["hike_legs"] == .uint(18))
        #expect(an["hike_span"] == .int(10800))
        #expect(an["whole_hike_legs"] == .uint(17))
        #expect(an["whole_hike_span"] == .int(10200))
        // headers: all 19 match with T = 1700000000 + 600*h at height 1000+h.
        if case .arr(let headers) = dict["headers"] {
            #expect(headers.count == 19)
            for (k, h) in headers.enumerated() {
                if case .obj(let fields) = h {
                    let d = Dictionary(uniqueKeysWithValues: fields.map { ($0.0, $0.1) })
                    #expect(d["index_comparison"] == .str("match"))
                    let expectedT: Int64 = 1_700_000_000 + 600 * Int64(1000 + k)
                    #expect(d["T"] == .int(expectedT))
                    #expect(d["stored_height"] == .uint(UInt64(1000 + k)))
                }
            }
        }
        // waypoints: all 17 match.
        if case .arr(let wps) = dict["waypoints"] {
            #expect(wps.count == 17)
            for w in wps {
                if case .obj(let fields) = w {
                    #expect(fields.last?.1 == .str("match"))
                }
            }
        }
        // checkpoints: every leg matches.
        if case .arr(let cps) = dict["checkpoints"] {
            #expect(cps.count == 18)
            for c in cps {
                if case .obj(let fields) = c, let legsIdx = fields.firstIndex(where: { $0.0 == "legs" }),
                   case .arr(let legs) = fields[legsIdx].1 {
                    #expect(legs.count == 1)
                    if case .obj(let legFields) = legs[0] {
                        #expect(legFields.last?.1 == .str("match"))
                    }
                }
            }
        }
        // six_leg: 6 indices from the chain source, all match.
        if case .obj(let sl) = dict["six_leg"] {
            let d = Dictionary(uniqueKeysWithValues: sl.map { ($0.0, $0.1) })
            if case .arr(let idx) = d["indices"] { #expect(idx.count == 6) }
            if case .arr(let res) = d["results"] {
                #expect(res.allSatisfy { $0 == .str("match") })
            }
        } else {
            Issue.record("six_leg not an object")
        }
        // per-stage spans: 600 each.
        if case .arr(let perStage) = an["per_stage"] {
            #expect(perStage.count == 18)
            for s in perStage {
                if case .obj(let fields) = s {
                    #expect(fields.first { $0.0 == "span" }?.1 == .int(600))
                }
            }
        }
        #expect(an["legs_outside"] == .str("not defined"))
        // bookends: not defined (zero valid paths: step 1 of Rule A4.11).
        #expect(dict["a"] == .str("not defined"))
        #expect(dict["N"] == .str("not defined"))
        if case .obj(let fs) = dict["fixed_start"] {
            #expect(fs.first { $0.0 == "N" }?.1 == .str("not defined"))
        }
        // Key order is section 14's order.
        let expectedOrder = ["profile", "sample_bytes", "slot_bytes", "terrain_bytes",
                             "steps_per_leg", "shape", "shape_item", "headers", "seed_value",
                             "checkpoints", "waypoints", "paths", "anchors", "anchored_total",
                             "skip_list", "a", "b", "N", "S", "delta_h", "fixed_start", "P_b",
                             "height_P_b", "index_comparison_P_b", "T_P_b",
                             "height_I_a_minus_1", "T_I_a_minus_1", "six_leg", "six_leg_runs",
                             "analysis"]
        #expect(top.map { $0.0 } == expectedOrder)
    }

    @Test func verifyO2() throws {
        let o2 = try Producer.buildObject(profile: Fixture.S2, legsPerStage: Fixture.legsPerStageS2)
        guard case .record(let rec) = Verify.run(objectBytes: o2, options: verifyOptions(Fixture.S2)) else {
            Issue.record("no record"); return
        }
        let dict = Dictionary(uniqueKeysWithValues: recordObject(rec).map { ($0.0, $0.1) })
        #expect(dict["shape"] == .str("match"))
        guard case .obj(let anFields) = dict["analysis"] else {
            Issue.record("missing analysis"); return
        }
        let an = Dictionary(uniqueKeysWithValues: anFields.map { ($0.0, $0.1) })
        #expect(an["hike_legs"] == .uint(54))
        #expect(an["whole_hike_legs"] == .uint(51))
        #expect(an["hike_span"] == .int(10800))
        #expect(an["whole_hike_span"] == .int(10200))
        if case .obj(let sl) = dict["six_leg"] {
            if case .arr(let idx) = sl.first(where: { $0.0 == "indices" })?.1 {
                #expect(idx.count == 6)
            }
        }
    }

    @Test func malformedRecord() {
        if case .record(let rec) = Verify.run(objectBytes: [0xff, 0xff], options: verifyOptions(.production)) {
            let dict = Dictionary(uniqueKeysWithValues: recordObject(rec).map { ($0.0, $0.1) })
            #expect(dict["shape"] == .str("mismatch"))
            #expect(dict["shape_item"] == .arr([.str("malformed cbor")]))
            #expect(dict["profile"] == .str("production"))
        } else {
            Issue.record("malformed input must still produce a record")
        }
    }

    @Test func ruleA5Records() throws {
        let o1 = try Producer.buildObject(profile: Fixture.S1, legsPerStage: Fixture.legsPerStageS1)
        // The CASES.md labels, in the numbered order.
        let labels = ["v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"]
        let expectedRecords = [
            "{\"kind\":\"unknown kind\",\"kind_text\":\"v2.1-pcoc-object\"}",
            "{\"kind\":\"unknown kind\",\"kind_text\":\"satoshi-hike-v2.0-object\"}",
            "{\"kind\":\"unknown kind\",\"kind_text\":\"\"}",
            "{\"kind\":\"unknown kind\",\"kind_text\":\"v2.2-pcoc-objecT\"}",
        ]
        for (i, label) in labels.enumerated() {
            let bytes = Producer.replaceKind(object: o1, newKind: label)
            // The re-encoded map is still one well-formed CBOR map.
            if case .hikeObject = ObjectDecode.decode(bytes) { Issue.record("foreign kind decoded as hike object") }
            guard case .unknownKind(let text) = Verify.run(objectBytes: bytes, options: verifyOptions(Fixture.S1)) else {
                Issue.record("label \(i) not Rule A5"); continue
            }
            #expect(text == label)
            #expect(String(decoding: ObjectDecode.ruleA5Record(kindText: text), as: UTF8.self)
                    == expectedRecords[i])
        }
    }

    @Test func drawChainStructures() {
        let label = "pcoc-pack-2.2/draw"
        // six-leg at 6 legs: a permutation of 0…5.
        var src = ChainSource(label: label)
        let six6 = Draw.sixLeg(legTotal: 6, source: &src)!
        #expect(Set(six6) == Set(0..<6))
        // thorough at 6 legs, 3 rounds: each round has 6 distinct indices;
        // across rounds, an index repeats only after every index was used.
        var src2 = ChainSource(label: label)
        let th6 = Draw.thorough(legTotal: 6, rounds: 3, source: &src2)!
        #expect(th6.count == 18)
        for r in 0..<3 {
            let round = th6[6 * r..<6 * r + 6]
            #expect(Set(round).count == 6)
            if r < 2 {
                #expect(Set(th6[0..<6 * (r + 1)]).count == min(6 * (r + 1), 6))
            }
        }
        // thorough at 54 legs: 18 distinct indices.
        var src3 = ChainSource(label: label)
        let th54 = Draw.thorough(legTotal: 54, rounds: 3, source: &src3)!
        #expect(th54.count == 18 && Set(th54).count == 18)
    }

    @Test func rateAndAnchor() {
        #expect(Comparisons.rateFloor(p: 1, q: 5, hikeLegs: 18, hikeSpan: 10800) == "refuse")
        #expect(Comparisons.rateFloor(p: 1, q: 5, hikeLegs: 3000, hikeSpan: 3600) == "meets")
        #expect(Comparisons.rateFloor(p: 7, q: 20, hikeLegs: 0, hikeSpan: 1) == "refuse")
        #expect(Comparisons.anchor(H: 100, inclusionHeight: 101) == "not anchored")
        #expect(Comparisons.anchor(H: 100, inclusionHeight: 104) == "anchored")
        #expect(Comparisons.anchor(H: 100, inclusionHeight: 105) == "not anchored")
    }

    @Test func hashcash() {
        let bariOrigin = "https://example.com"
        let headerHash: [UInt8] = [UInt8](repeating: 0x42, count: 32)
        let waypoint: [UInt8] = [UInt8](repeating: 0x24, count: 32)
        let ch = Hashcash.challenge(bariOrigin: bariOrigin, headerHash: headerHash)
        let sol = Hashcash.solve(challenge: ch, waypoint: waypoint, difficulty: 12)
        // Verify the solution against the definition.
        let d = Hashcash.digest(challenge: ch, waypoint: waypoint, nonce: sol.nonce)
        #expect(d == sol.digest)
        #expect(Hashcash.leadingZeroBits(d) >= 12)
        #expect(Hashcash.leadingZeroBits(sol.digest) == sol.leadingZeroBits)
    }

    @Test func policyFile() throws {
        // gl-admit-v9.toml is committed under refs/; validate it per 15.4.
        let path = projectRoot() + "/refs/gl-admit-v9.toml"
        guard let text = String(bytes: FS.read(path) ?? [], encoding: .utf8) else {
            Issue.record("cannot read refs/gl-admit-v9.toml"); return
        }
        guard case .success(let levers) = PolicyLoad.load(text: text) else {
            Issue.record("gl-admit-v9.toml was refused"); return
        }
        #expect(levers.minAnchoredPaths == 6)
        #expect(levers.legRateFloorP == 1 && levers.legRateFloorQ == 5)
        #expect(levers.submissionDeadlineBlocks == 3)
        #expect(levers.reverifyMaxAttempts == 3)
        #expect(levers.maxHeldBlocks == 6)
        #expect(levers.provisional == "on")
        #expect(levers.fastAcceptBoundMs == 2000)
        #expect(levers.asyncCatchBudgetMs == 22050)
        #expect(levers.thoroughRounds == 3)
        // An unknown lever key refuses the whole file.
        guard case .failure = PolicyLoad.load(text: "spec = \"2.2\"\n[levers]\nfoo = 1\n") else {
            Issue.record("unknown lever accepted"); return
        }
    }
}

@Suite("Committed candidates") struct CandidatesTests {
    @Test func generatorReproducesCommittedFiles() throws {
        // CASES.md: "A test re-runs it and checks that the committed files
        // are unchanged." Run the generator into a temp directory and compare
        // every file byte for byte.
        let root = projectRoot()
        let tmp = NSTemporaryDirectory() + "/shk-candidates-\(UUID().uuidString)"
        try FileManager.default.createDirectory(atPath: tmp, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(atPath: tmp) }
        try Generator.run(outDir: tmp, repoRoot: root)
        for name in Generator.allFileNames() {
            let file = "\(name).txt"
            let committed = FS.read("\(root)/impl/candidates/\(file)")
            let fresh = FS.read("\(tmp)/\(file)")
            guard let c = committed else {
                Issue.record("missing committed candidates/\(file)")
                continue
            }
            #expect(c == fresh, "candidates/\(file) changed")
        }
    }
}

func projectRoot() -> String {
    // #filePath points inside the built test bundle; walk up to the repo root
    // (the directory that contains impl/ and refs/).
    var dir = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    for _ in 0..<10 {
        let candidate = dir.path
        if FS.read("\(candidate)/refs/CASES.md") != nil && FS.read("\(candidate)/impl/Package.swift") != nil {
            return candidate
        }
        dir = dir.deletingLastPathComponent()
    }
    return "/workspace"
}
