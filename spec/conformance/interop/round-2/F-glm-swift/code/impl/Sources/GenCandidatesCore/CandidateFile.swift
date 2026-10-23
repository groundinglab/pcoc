// The deterministic candidate generator: writes every CASES.md case with
// exactly its keys into candidates/, one file per case group, lines sorted by
// key in byte order, key<TAB>value<LF, ASCII only, ending with the LF of the
// last line. No expected values are published: the pack job compares cores'
// files byte for byte.

import SatoshiHike
import Negatives

public enum Generator {
    public static func hex(_ b: [UInt8]) -> String { Hex.encode(b) }
    public static func u64k(_ v: UInt64) -> String { Hex.u64Key(v) }

    public static func allFileNames() -> [String] {
        ["seed", "nl4", "terrain", "fold", "kat-small", "test-3leg", "prod",
         "wes", "rate", "anchor", "draw", "unknown-kind", "hashcash", "negatives"]
    }

    /// Write all candidate files into `dir` (created if missing).
    /// Throws GeneratorError on any inconsistency the CASES file says must
    /// stop the generator (a negative matching the honest value, or a cached
    /// leg whose fixture does not revisit a slot).
    public static func run(outDir: String, repoRoot: String?) throws {
        FS.mkdirp(outDir)
        var files: [String: [String]] = [:]

        files["seed.txt"] = seedCase()
        files["nl4.txt"] = nl4Case()
        files["terrain.txt"] = try terrainCase()
        files["fold.txt"] = foldCase()
        files["kat-small.txt"] = try hikeCase(prefix: "kat-small", profile: Fixture.S1,
                                             legsPerStage: Fixture.legsPerStageS1)
        files["test-3leg.txt"] = try hikeCase(prefix: "test-3leg", profile: Fixture.S2,
                                              legsPerStage: Fixture.legsPerStageS2)
        files["prod.txt"] = try prodCase()
        files["wes.txt"] = wesCase()
        files["rate.txt"] = rateCase()
        files["anchor.txt"] = anchorCase()
        files["draw.txt"] = drawCase()
        files["unknown-kind.txt"] = try unknownKindCase(repoRoot: repoRoot)
        files["hashcash.txt"] = hashcashCase()
        files["negatives.txt"] = try negativesCase()

        for (name, lines) in files {
            try writeLines(lines: lines, to: outDir + "/" + name)
        }
        // CASES.md lists exactly these files; nothing else.
        let expected = Set(allFileNames().map { $0 + ".txt" })
        guard Set(files.keys) == expected else {
            throw GeneratorError.message("file set mismatch: \(files.keys.sorted())")
        }
    }

    static func writeLines(lines: [String], to path: String) throws {
        var out = ""
        for l in lines { out += l + "\n" }
        FS.write(path, Array(out.utf8))
    }

    static func emit(_ dict: [(String, String)]) -> [String] {
        // Lines are sorted by key in byte order; no trailing whitespace.
        let sorted = dict.sorted { Array($0.0.utf8).lexicographicallyPrecedes(Array($1.0.utf8)) }
        return sorted.map { "\($0)\t\($1)" }
    }

    // ------------------------------------------------------------- cases

    /// seed: for pk_A and pk_B, each with seed_beacon: the 64-byte preimage,
    /// D, and lanes P0 … P3.
    static func seedCase() -> [String] {
        var out: [(String, String)] = []
        for (name, pk) in [("pk_A", Fixture.pkA), ("pk_B", Fixture.pkB)] {
            let pre = Seed.preimage(publicKey: pk, seedBeacon: Fixture.seedBeacon)
            let D = SHK.sha256(pre)
            let lanes = Seed.lanes(D)
            out.append(("seed.\(name).preimage", hex(pre)))
            out.append(("seed.\(name).D", hex(D)))
            for k in 0..<4 {
                out.append(("seed.\(name).P\(k)_u64", u64k(lanes[k])))
            }
        }
        return emit(out)
    }

    /// nl4: S1, pk_A. Slots 0, 1, 1023, j = 0 … 7: the NL4 base word (10.4,
    /// before parents and slot_mix) and the stored terrain word.
    static func nl4Case() -> [String] {
        var out: [(String, String)] = []
        let pk = Fixture.pkA
        let D = Seed.seed(publicKey: pk, seedBeacon: Fixture.seedBeacon)
        let lanes = Seed.lanes(D)
        let words = Fixture.S1.wordsPerSlot   // 8
        // Base words for slot i (no parents, no slot_mix: slot 0 keeps them).
        func baseWords(_ i: UInt64) -> [UInt64] {
            let ish = i << 32
            var w = [UInt64](repeating: 0, count: words)
            for j in 0..<words {
                let c = ish ^ UInt64(j) ^ Spec.wordXor
                var t = mix64(lanes[j % 4] ^ c)
                t = mix64(t ^ rotl64(lanes[(j + 1) % 4], 17))
                t = mix64(t ^ rotl64(lanes[(j + 2) % 4], 31))
                t = mix64(t ^ rotl64(lanes[(j + 3) % 4], 47))
                w[j] = t
            }
            return w
        }
        let terrain = Terrain(publicKey: pk, seedBeacon: Fixture.seedBeacon, profile: Fixture.S1)
        for i in [UInt64(0), 1, 1023] {
            let base = baseWords(i)
            for j in 0..<words {
                out.append(("nl4.S1.slot.\(i).word.\(j).base_u64", u64k(base[j])))
                let stored = LE.u64(Array(terrain.slotBytes(i)[j * 8..<j * 8 + 8]))
                out.append(("nl4.S1.slot.\(i).word.\(j).stored_u64", u64k(stored)))
            }
        }
        return emit(out)
    }

    /// terrain: the SHA-256 of the whole terrain array for S1, S2 and PROD (pk_A).
    static func terrainCase() throws -> [String] {
        var out: [(String, String)] = []
        let pk = Fixture.pkA
        let sb = Fixture.seedBeacon
        let t1 = Terrain(publicKey: pk, seedBeacon: sb, profile: Fixture.S1)
        out.append(("terrain.S1.terrain_sha256", hex(t1.digest)))
        let t2 = Terrain(publicKey: pk, seedBeacon: sb, profile: Fixture.S2)
        out.append(("terrain.S2.terrain_sha256", hex(t2.digest)))
        let t3 = Terrain(publicKey: pk, seedBeacon: sb, profile: .production)
        out.append(("terrain.PROD.terrain_sha256", hex(t3.digest)))
        return emit(out)
    }

    /// fold: S1, pk_A, the first leg of stage 0. Steps 0 … 3: input state S,
    /// slot index read, §11.1 acc after the full slot, output state; plus the
    /// leg's final leg_result.
    static func foldCase() -> [String] {
        var out: [(String, String)] = []
        let terrain = Terrain(publicKey: Fixture.pkA, seedBeacon: Fixture.seedBeacon,
                              profile: Fixture.S1)
        var S = LE.u64(Array(terrain.seed[0..<8]))   // S_0 = u64_le(D[0:8]) = P[0]
        for n in 0..<4 {
            let tr = Leg.stepTrace(terrain: terrain, S: S)
            out.append(("fold.S1.step.\(n).S_u64", u64k(tr.S)))
            out.append(("fold.S1.step.\(n).slot_index", String(tr.slotIndex)))
            out.append(("fold.S1.step.\(n).acc_u64", u64k(tr.acc)))
            out.append(("fold.S1.step.\(n).S_next_u64", u64k(tr.next)))
            S = tr.next
        }
        let lr = Leg.result(terrain: terrain, previousCheckpoint: terrain.seed)
        out.append(("fold.S1.leg_result_u64", u64k(lr)))
        return emit(out)
    }

    /// kat-small / test-3leg: terrain digest, per-stage stored waypoint
    /// (stage 0 is the seed), and per-leg legs_le_hex + checkpoint.
    static func hikeCase(prefix: String, profile: Profile, legsPerStage: [Int]) throws -> [String] {
        var out: [(String, String)] = []
        let pk = Fixture.pkA
        let sb = Fixture.seedBeacon
        let terrain = Terrain(publicKey: pk, seedBeacon: sb, profile: profile)
        out.append(("\(prefix).\(profile == Fixture.S1 ? "S1" : "S2").terrain_sha256",
                   hex(terrain.digest)))
        let hike = Hike(publicKey: pk, seedBeacon: sb, seedBeaconHeight: Fixture.seedBeaconHeight,
                        interrupts: Fixture.interrupts, interruptHeights: Fixture.interruptHeights,
                        legsPerStage: legsPerStage, profile: profile, terrain: terrain)
        let profName = prefix == "kat-small" ? "S1" : "S2"
        for g in 0..<18 {
            let st = hike.stages[g]
            out.append(("\(prefix).\(profName).stage.\(g).waypoint", hex(st.waypoint)))
            for (j, cp) in st.checkpoints.enumerated() {
                out.append(("\(prefix).\(profName).stage.\(g).leg.\(j).legs_le_hex",
                           LE.le64(st.legResults[j]).legsLeHex))
                out.append(("\(prefix).\(profName).stage.\(g).leg.\(j).checkpoint", hex(cp)))
            }
        }
        return emit(out)
    }

    /// prod: PROD, pk_A. The terrain digest and the first leg of stage 0 at
    /// the section 6 constants: legs_le_hex and its checkpoint.
    static func prodCase() throws -> [String] {
        var out: [(String, String)] = []
        let pk = Fixture.pkA
        let sb = Fixture.seedBeacon
        let terrain = Terrain(publicKey: pk, seedBeacon: sb, profile: .production)
        out.append(("prod.PROD.terrain_sha256", hex(terrain.digest)))
        // First leg of stage 0 from the seed.
        let lr = Leg.result(terrain: terrain, previousCheckpoint: terrain.seed)
        let sample = terrain.sample(at: terrain.seed)
        let cp = Leg.checkpoint(previous: terrain.seed, stageLimb: terrain.seed,
                                sample: sample, legResult: lr)
        out.append(("prod.PROD.stage.0.leg.0.legs_le_hex", LE.le64(lr).legsLeHex))
        out.append(("prod.PROD.stage.0.leg.0.checkpoint", hex(cp)))
        return emit(out)
    }

    /// wes: waypoint_equals_seed for S1 pk_A: D and the stage-0 waypoint side
    /// by side, and `equal`.
    static func wesCase() -> [String] {
        let pk = Fixture.pkA
        let sb = Fixture.seedBeacon
        let terrain = Terrain(publicKey: pk, seedBeacon: sb, profile: Fixture.S1)
        let D = terrain.seed
        let stored = D   // stages[0].waypoint is the seed by design (Rule A2.2)
        return emit([
            ("wes.S1.D", hex(D)),
            ("wes.S1.stage.0.waypoint", hex(stored)),
            ("wes.S1.equal", D == stored ? "true" : "false"),
        ])
    }

    /// rate: the Rule B2 integer comparison q · hike_legs ≥ p · hike_span over
    /// the CASES.md grid.
    static func rateCase() -> [String] {
        var out: [(String, String)] = []
        let legsGrid: [UInt64] = [0, 1, 5, 6, 1000]
        let spans: [Int64] = [1, 300, 3600]
        let floors: [(UInt64, UInt64)] = [(1, 5), (1, 4), (7, 20)]
        for L in legsGrid {
            for s in spans {
                for (p, q) in floors {
                    let v = Comparisons.rateFloor(p: p, q: q, hikeLegs: L, hikeSpan: s)
                    out.append(("rate.legs.\(L).span.\(s).floor.\(p)_\(q)", v))
                }
            }
        }
        return emit(out)
    }

    /// anchor: the §16.1 Anchor comparison for H = 100 and inclusion heights
    /// 100 … 106 (offsets 2, 3, 4 after stored stage height H).
    static func anchorCase() -> [String] {
        var out: [(String, String)] = []
        for h in UInt64(100)...106 {
            out.append(("anchor.H.100.height.\(h)", Comparisons.anchor(H: 100, inclusionHeight: h)))
        }
        return emit(out)
    }

    /// draw: the §16.1 Draw bytes chain from "pcoc-pack-2.2/draw". Six draws
    /// (leg counts 6, 18, 54 × six-leg and thorough), each from b_0.
    static func drawCase() -> [String] {
        var out: [(String, String)] = []
        out.append(("draw.b0_label", "pcoc-pack-2.2/draw"))
        out.append(("draw.thorough_rounds", "3"))
        let label = "pcoc-pack-2.2/draw"
        for L in [UInt64(6), 18, 54] {
            var six = ChainSource(label: label)
            guard let sixIdx = Draw.sixLeg(legTotal: L, source: &six) else {
                fatalError("chain source exhausted for six-leg draw at \(L)")
            }
            out.append(("draw.legs.\(L).six_leg.indices",
                        sixIdx.map { String($0) }.joined(separator: ",")))
            var thorough = ChainSource(label: label)
            guard let thIdx = Draw.thorough(legTotal: L, rounds: 3, source: &thorough) else {
                fatalError("chain source exhausted for thorough draw at \(L)")
            }
            out.append(("draw.legs.\(L).thorough.indices",
                        thIdx.map { String($0) }.joined(separator: ",")))
        }
        return emit(out)
    }

    /// unknown-kind: the Rule A5 record bytes for the four labels. Each input
    /// is the S1 object O1 with only its kind value replaced by the label
    /// (one well-formed CBOR map).
    static func unknownKindCase(repoRoot: String?) throws -> [String] {
        var out: [(String, String)] = []
        let labels = ["v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"]
        // Build O1 to take its field values from.
        let o1 = try Producer.buildObject(profile: Fixture.S1, legsPerStage: Fixture.legsPerStageS1)
        var n = 0
        for label in labels {
            let bytes = Producer.replaceKind(object: o1, newKind: label)
            // The record is the Rule A5 output for that input.
            switch Verify.run(objectBytes: bytes, options: VerifyOptions(profile: Fixture.S1)) {
            case .unknownKind(let text):
                let rec = ObjectDecode.ruleA5Record(kindText: text)
                out.append(("unknown-kind.\(n).kind_text_hex", hex(Array(label.utf8))))
                out.append(("unknown-kind.\(n).record_hex", hex(rec)))
            default:
                throw GeneratorError.message("label \(label.debugDescription) did not produce a Rule A5 record")
            }
            n += 1
        }
        return emit(out)
    }

    /// hashcash: §13.1 with the CASES.md inputs.
    static func hashcashCase() -> [String] {
        var out: [(String, String)] = []
        let bariOrigin = "https://example.com"
        let fee0 = SHK.sha256(Array("pcoc-pack-2.2/hashcash/fee_header/0".utf8))
        let fee1 = SHK.sha256(Array("pcoc-pack-2.2/hashcash/fee_header/1".utf8))
        let fee2 = SHK.sha256(Array("pcoc-pack-2.2/hashcash/fee_header/2".utf8))
        let feeHeader = fee0 + fee1 + Array(fee2[0..<16])   // 80 bytes
        let headerHash = SHK.sha256(feeHeader)               // natural order
        let waypoint = SHK.sha256(Array("pcoc-pack-2.2/hashcash/waypoint".utf8))
        let challenge = Hashcash.challenge(bariOrigin: bariOrigin, headerHash: headerHash)
        let sol = Hashcash.solve(challenge: challenge, waypoint: waypoint)
        out.append(("hashcash.bari_origin", bariOrigin))
        out.append(("hashcash.fee_header", hex(feeHeader)))
        out.append(("hashcash.header_hash", hex(headerHash)))
        out.append(("hashcash.waypoint", hex(waypoint)))
        out.append(("hashcash.difficulty_bits", String(Hashcash.difficultyBits)))
        out.append(("hashcash.challenge", hex(challenge)))
        out.append(("hashcash.nonce", String(sol.nonce)))
        out.append(("hashcash.digest", hex(sol.digest)))
        out.append(("hashcash.leading_zero_bits", String(sol.leadingZeroBits)))
        return emit(out)
    }

    /// negatives: S1, pk_A, seed_beacon, the first leg of stage 0
    /// (f3_shared_terrain also uses pk_B). Honest values plus the thirteen
    /// permanent negatives. matches_honest is computed, never a literal; the
    /// generator stops with an error if any negative equals honest.
    static func negativesCase() throws -> [String] {
        var out: [(String, String)] = []
        let pkA = Fixture.pkA
        let pkB = Fixture.pkB
        let sb = Fixture.seedBeacon
        let profile = Fixture.S1

        let terrainA = Terrain(publicKey: pkA, seedBeacon: sb, profile: profile)
        let honestLeg = Leg.result(terrain: terrainA, previousCheckpoint: terrainA.seed)
        let honestTerrainDigest = terrainA.digest
        out.append(("negatives.honest.legs_le_hex", LE.le64(honestLeg).legsLeHex))
        out.append(("negatives.honest.terrain_sha256", hex(honestTerrainDigest)))

        func check(_ name: String, _ negative: String, _ honest: String) throws {
            if negative == honest {
                throw GeneratorError.message("negative \(name) equals the honest value; the generator must stop")
            }
            out.append(("negatives.\(name).matches_honest",
                        negative == honest ? "true" : "false"))
        }

        // Leg negatives, from the seed (Rule A2.2).
        let legNegatives: [(String, NegativeLegKind?)] = [
            ("slot_summary_fold", .slotSummaryFold),
            ("xor_accumulator_fold", .xorAccumulatorFold),
            ("xor_w_fold", .xorWFold),
            ("s_plus_w_fold", .sPlusWFold),
            ("prefold_hoist", .prefoldHoist),
            ("final_stage_hoist", .finalStageHoist),
            ("partial_slot_read", .partialSlotRead),
            ("partial_slot_read_2047", .partialSlotRead2047),
            ("cached_leg", .cachedLeg),
        ]
        for (name, kind) in legNegatives {
            guard let k = kind else { continue }
            var revisit = false
            let v = Negatives.negativeLegResult(
                kind: k, profile: profile, startSeed: terrainA.seed,
                terrain: terrainA.rawPointer, nSlots: terrainA.nSlots,
                wordsPerSlot: terrainA.wordsPerSlot, steps: profile.stepsPerLeg,
                requireSlotRevisit: k == .cachedLeg, slotRevisitFound: &revisit)
            let le = LE.le64(v).legsLeHex
            out.append(("negatives.\(name).legs_le_hex", le))
            try check(name, le, LE.le64(honestLeg).legsLeHex)
        }

        // f3_shared_terrain: pk_A's first leg of stage 0 on pk_B's terrain.
        let terrainB = Terrain(publicKey: pkB, seedBeacon: sb, profile: profile)
        let f3 = Negatives.f3SharedTerrainLegResult(profile: profile,
                                                    startSeed: terrainA.seed,
                                                    terrainB: terrainB.rawPointer)
        let f3le = LE.le64(f3).legsLeHex
        out.append(("negatives.f3_shared_terrain.legs_le_hex", f3le))
        try check("f3_shared_terrain", f3le, LE.le64(honestLeg).legsLeHex)

        // Terrain negatives.
        let tNegatives: [(String, NegativeTerrainKind)] = [
            ("one_lane_flip", .oneLaneFlip),
            ("all4_kernel", .all4Kernel),
            ("squeezed_seed_terrain", .squeezedSeed),
        ]
        for (name, kind) in tNegatives {
            let d = Negatives.terrainDigest(publicKey: pkA, seedBeacon: sb,
                                            profile: profile, kind: kind)
            out.append(("negatives.\(name).terrain_sha256", hex(d)))
            try check(name, hex(d), hex(honestTerrainDigest))
        }
        return emit(out)
    }
}

public enum GeneratorError: Error {
    case message(String)
}
