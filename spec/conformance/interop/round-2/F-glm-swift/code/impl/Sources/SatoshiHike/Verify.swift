// SECTION 14 verify: the full measurement record. Verify records measurements
// against the object and the portable index and does not judge them
// (Rule A4). Exit codes and CLI shape live in section 18 / the CLI.

public struct VerifyOptions {
    /// The profile in force (section 6 constants, or the conformance-small
    /// four integers as verifier inputs).
    public var profile: Profile
    /// The portable index ({index-url} stand-in). The offline test plane uses
    /// the synthetic fixture index CASES.md defines.
    public var index: IndexProvider
    /// A factory for the section 14.6 byte source (OS CSPRNG in production;
    /// a rig may supply draw bytes).
    public var makeDrawSource: () -> any ByteSource

    public init(profile: Profile = .production,
                index: IndexProvider = FixtureIndex(),
                makeDrawSource: @escaping () -> any ByteSource = { CSPRNGSource() }) {
        self.profile = profile
        self.index = index
        self.makeDrawSource = makeDrawSource
    }
}

public enum VerifyOutcome {
    /// The section 14 record (or, for malformed CBOR input, the section 14
    /// record with shape mismatch and shape_item exactly ["malformed cbor"]).
    case record(JVal)
    /// Rule A5: the kind text of an unknown kind.
    case unknownKind(String)
    /// Section 7.3 decode failure (no record; CLI exits 1).
    case decodeFailure
}

public enum Verify {
    public static func run(objectBytes: [UInt8], options: VerifyOptions) -> VerifyOutcome {
        switch ObjectDecode.decode(objectBytes) {
        case .malformed:
            return .record(malformedRecord(profile: options.profile, index: options.index))
        case .decodeFailure:
            return .decodeFailure
        case .unknownKind(let kind):
            return .unknownKind(kind)
        case .hikeObject(let root):
            return .record(measure(root: root, options: options))
        }
    }

    // ------------------------------------------------------------ helpers

    /// One stored header measurement (Rule A4.2).
    struct HeaderMeas {
        var storedHash: [UInt8]?
        var storedHeight: UInt64?
        var heightOutOfRange = false   // > 2^53 - 1 in the object's own bytes
        var comparison = Outcome.notRead
        var T: String = Outcome.notRead
        var theight: Int64? = nil
    }

    /// The verify record holds the integer T when it was taken; the string is
    /// only the `not read` token.
    static func headerValue(_ v: String?, _ i: Int64?) -> JVal {
        if let n = i { return .int(n) }
        if let s = v { return .str(s) }
        return .str(Outcome.notRead)
    }

    /// Read one stored header from the index (section 14.2, one section 9 GET).
    static func readHeader(index: IndexProvider, storedHash: [UInt8]?, storedHeight: UInt64?,
                           heightDefined: Bool, height: UInt64?) -> HeaderMeas {
        var m = HeaderMeas()
        m.storedHash = storedHash
        m.storedHeight = storedHeight
        guard storedHash != nil, heightDefined, let h = height else {
            // stored_hash or stored_height is `not defined` on the record, and
            // the index comparison and T are `not read` (section 14).
            m.comparison = Outcome.notRead
            m.T = Outcome.notRead
            return m
        }
        guard let body = index.get(height: h), let doc = IndexDoc(body: Array(body.utf8)) else {
            m.comparison = Outcome.notRead
            m.T = Outcome.notRead
            return m
        }
        guard doc.usableHeader else {
            m.comparison = Outcome.notRead
            m.T = Outcome.notRead
            return m
        }
        let naturalDoc = doc.headerHashNatural!
        let heightOk = doc.headerHeight == storedHeight
        if naturalDoc == storedHash! && heightOk {
            m.comparison = Outcome.match
            if let t = doc.headerTime {
                m.T = String(t)
                m.theight = Int64(t)
            } else {
                m.T = Outcome.notRead
            }
        } else {
            m.comparison = Outcome.mismatch
            m.T = Outcome.notRead
        }
        return m
    }

    /// Median-time-past of a beacon at height h (Rule A4.10). The beacon's own
    /// header is the Rule A4.2 read; the other heights in the window are
    /// additional reads. Returns nil when MTP is `not read`.
    static func medianTimePast(beaconHeight: UInt64, beaconMatch: Bool, beaconTime: Int64?,
                               index: IndexProvider) -> Int64? {
        let n = Int(min(11, beaconHeight + 1))
        guard n >= 1 else { return nil }
        var times: [Int64] = []
        if beaconMatch, let t = beaconTime {
            times.append(t)
        } else {
            return nil
        }
        let low = beaconHeight - UInt64(n) + 1
        for i in 1..<n {
            let h = low + UInt64(i)
            guard let body = index.get(height: h),
                  let doc = IndexDoc(body: Array(body.utf8)),
                  doc.usableHeader,
                  doc.headerHeight == h,
                  let t = doc.headerTime else { return nil }
            times.append(Int64(t))
        }
        times.sort()
        return times[n / 2]
    }

    // -------------------------------------------------------- malformed

    static func malformedRecord(profile: Profile, index: IndexProvider) -> JVal {
        var top: [(String, JVal)] = []
        top.append(("profile", .str(profile.name)))
        if profile.name == "conformance-small" {
            top.append(("sample_bytes", .uint(UInt64(profile.sampleBytes))))
            top.append(("slot_bytes", .uint(UInt64(profile.slotBytes))))
            top.append(("terrain_bytes", .uint(UInt64(profile.terrainBytes))))
            top.append(("steps_per_leg", .uint(UInt64(profile.stepsPerLeg))))
        }
        top.append(("shape", .str(Outcome.mismatch)))
        top.append(("shape_item", .arr([.str("malformed cbor")])))
        var headers: [JVal] = []
        for _ in 0..<19 {
            headers.append(.obj([
                ("stored_hash", .str(Outcome.notDefined)),
                ("stored_height", .str(Outcome.notDefined)),
                ("index_comparison", .str(Outcome.notRead)),
                ("T", .str(Outcome.notRead)),
            ]))
        }
        top.append(("headers", .arr(headers)))
        top.append(("seed_value", .str(Outcome.notDefined)))
        var cps: [JVal] = []
        for g in 0..<18 { cps.append(.obj([("g", .uint(UInt64(g))), ("legs", .str(Outcome.notDefined))])) }
        top.append(("checkpoints", .arr(cps)))
        var wps: [JVal] = []
        for g in 1...17 { wps.append(.obj([("g", .uint(UInt64(g))), ("result", .str(Outcome.notDefined))])) }
        top.append(("waypoints", .arr(wps)))
        top.append(("paths", pathsForAbsentStages()))
        top.append(("anchors", .arr(Array(repeating: .str(Outcome.notAnchored), count: 16))))
        top.append(("anchored_total", .uint(0)))
        top.append(("skip_list", .arr((1...16).map { .uint(UInt64($0)) })))
        for k in ["a", "b", "N", "S", "delta_h", "P_b", "height_P_b", "index_comparison_P_b",
                  "T_P_b", "height_I_a_minus_1", "T_I_a_minus_1"] {
            top.append((k, .str(Outcome.notDefined)))
        }
        top.append(("fixed_start", .obj([
            ("N", .str(Outcome.notDefined)),
            ("S", .str(Outcome.notDefined)),
            ("delta_h", .str(Outcome.notDefined)),
        ])))
        top.append(("six_leg", .str(Outcome.notDefined)))
        top.append(("six_leg_runs", .arr([])))
        var perStage: [JVal] = []
        for g in 0..<18 {
            perStage.append(.obj([
                ("g", .uint(UInt64(g))),
                ("checkpoint_count", .str(Outcome.notDefined)),
                ("span", .str(Outcome.notRead)),
            ]))
        }
        top.append(("analysis", .obj([
            ("whole_hike_legs", .str(Outcome.notDefined)),
            ("whole_hike_span", .str(Outcome.notRead)),
            ("per_stage", .arr(perStage)),
            ("legs_outside", .str(Outcome.notDefined)),
            ("hike_legs", .str(Outcome.notDefined)),
            ("hike_span", .str(Outcome.notRead)),
        ])))
        return .obj(top)
    }

    /// The paths array when stages are absent or their path bytes are absent.
    /// A present-and-empty stages[17].path is `not fetched by design`; every
    /// other absent case records `not defined` for w17_path. W_1 … W_16 with
    /// no usable bytes record `skip` (the shape measurement already carries
    /// the violation). Documented as a SPEC question in RESULTS.md; the
    /// shape/decode edge objects come in a later round.
    static func pathsForAbsentStages(w17PathEmpty: Bool = false) -> JVal {
        var paths: [JVal] = []
        for g in 1...16 {
            paths.append(.obj([
                ("g", .uint(UInt64(g))),
                ("path_state", .str(Outcome.skip)),
                ("plant", .str(Outcome.na)),
                ("P_g", .str(Outcome.notDefined)),
                ("height_P_g", .str(Outcome.notDefined)),
                ("d_g", .str(Outcome.notDefined)),
                ("claimed_block", .str(Outcome.notDefined)),
            ]))
        }
        paths.append(.obj([
            ("g", .uint(17)),
            ("w17_path", .str(w17PathEmpty ? Outcome.notFetchedByDesign : Outcome.notDefined)),
            ("claimed_block", .str(Outcome.notDefined)),
        ]))
        return .arr(paths)
    }

    // ---------------------------------------------------------- measure

    static func measure(root: CBORItem, options: VerifyOptions) -> JVal {
        let profile = options.profile
        let index = options.index

        // ---------------- field extraction ----------------
        let kindText = CBOR.asText(CBOR.field(root, "kind"))
        let fillText = CBOR.asText(CBOR.field(root, "fill"))
        let slotBytes = CBOR.asUint(CBOR.field(root, "slot_bytes"))
        let nSlots = CBOR.asUint(CBOR.field(root, "n_slots"))
        let sampleBytes = CBOR.asUint(CBOR.field(root, "sample_bytes"))
        let stepsPerLeg = CBOR.asUint(CBOR.field(root, "steps_per_leg"))
        let publicKey = CBOR.asBytes(CBOR.field(root, "public_key"))
        let seedBeacon = CBOR.asBytes(CBOR.field(root, "seed_beacon"))
        let lastInterrupt = CBOR.asBytes(CBOR.field(root, "last_interrupt"))
        let lastInterruptHeightRaw = CBOR.asUint(CBOR.field(root, "last_interrupt_height"))
        let stagesArr = CBOR.asArray(CBOR.field(root, "stages"))

        var stages: [CBORItem?] = Array(repeating: nil, count: 18)
        if let arr = stagesArr {
            for (i, s) in arr.enumerated() where i < 18 { stages[i] = s }
        }

        // Per-stage field accessors.
        func stageBytes(_ g: Int, _ key: String) -> [UInt8]? {
            guard case .map? = stages[g] else { return nil }
            return CBOR.asBytes(CBOR.field(stages[g]!, key))
        }
        func stageUint(_ g: Int, _ key: String) -> UInt64? {
            guard case .map? = stages[g] else { return nil }
            return CBOR.asUint(CBOR.field(stages[g]!, key))
        }

        // ---------------- shape (Rule A4.1) ----------------
        var shapeMatch = true
        var shapeItems: [String] = []
        func violate(_ s: String) { shapeMatch = false; shapeItems.append(s) }

        let topKeys: Set<String> = {
            guard case .map(let entries) = root else { return [] }
            return Set(entries.map { $0.0 })
        }()

        // field set: the top-level map and each present stage map have
        // exactly the section 8 keys and value types.
        func typeOK(_ item: CBORItem?, _ want: String) -> Bool {
            switch (item, want) {
            case (.uint?, "uint"), (.text?, "text"), (.byteString?, "bytes"),
                 (.array?, "array"), (.map?, "map"): return true
            default: return false
            }
        }
        let wantTop: [(String, String)] = [
            ("kind", "text"), ("fill", "text"), ("slot_bytes", "uint"), ("n_slots", "uint"),
            ("sample_bytes", "uint"), ("steps_per_leg", "uint"), ("public_key", "bytes"),
            ("seed_beacon", "bytes"), ("stages", "array"), ("last_interrupt", "bytes"),
            ("last_interrupt_height", "uint"),
        ]
        var fieldSetOK = topKeys.count == wantTop.count
        for (k, t) in wantTop {
            if topKeys.contains(k) != true || !typeOK(CBOR.field(root, k), t) {
                fieldSetOK = false
            }
        }
        if !fieldSetOK { violate("field set") }

        // Stage maps: stage 0 has exactly 6 keys (no path); stages 1-17 have 7.
        for g in 0..<18 {
            guard case .map(let entries)? = stages[g] else { continue } // absent stage: per-field conditions below
            let keys = Set(entries.map { $0.0 })
            let want: [(String, String)] = g == 0
                ? [("beacon", "bytes"), ("waypoint", "bytes"), ("leg_count", "uint"),
                   ("checkpoints", "array"), ("leg_results", "array"), ("beacon_height", "uint")]
                : [("path", "bytes"), ("beacon", "bytes"), ("waypoint", "bytes"),
                   ("leg_count", "uint"), ("checkpoints", "array"), ("leg_results", "array"),
                   ("beacon_height", "uint")]
            var ok = keys.count == want.count
            for (k, t) in want {
                if !keys.contains(k) || !typeOK(CBOR.field(stages[g]!, k), t) { ok = false }
            }
            if !ok { violate("field set") }
        }

        // stage-0 path key.
        if case .map(let entries)? = stages[0], entries.contains(where: { $0.0 == "path" }) {
            violate("stage-0 path key")
        }

        // W_17 path.
        let w17Path = stageBytes(17, "path")
        if w17Path == nil || w17Path!.count != 0 { violate("W_17 path") }

        // constant (the profile in force).
        if kindText != Spec.kind || fillText != Spec.fill
            || slotBytes != UInt64(profile.slotBytes)
            || nSlots != UInt64(profile.nSlots)
            || sampleBytes != UInt64(profile.sampleBytes)
            || stepsPerLeg != UInt64(profile.stepsPerLeg) {
            violate("constant")
        }

        // stages length.
        if stagesArr?.count != 18 { violate("stages length") }

        // height_range: an integer above 2^53 - 1 in the object's own bytes.
        if let lih = lastInterruptHeightRaw, lih > Spec.heightMax { violate("height_range") }
        for g in 0..<18 {
            if let h = stageUint(g, "beacon_height"), h > Spec.heightMax { violate("height_range") }
        }

        // leg_count / widths.
        var checkpointsByStage: [[[UInt8]]] = []
        var legResultsByStage: [[[UInt8]]] = []
        var legCounts: [UInt64?] = []
        for g in 0..<18 {
            let cpsItems = CBOR.asArray(CBOR.field(stages[g] ?? .uint(0), "checkpoints"))
            let lrsItems = CBOR.asArray(CBOR.field(stages[g] ?? .uint(0), "leg_results"))
            let lc = stageUint(g, "leg_count")
            legCounts.append(lc)
            var cplist: [[UInt8]] = []
            var lrlist: [[UInt8]] = []
            var stageOK = true
            if let cpsArr = cpsItems {
                for c in cpsArr { if case .byteString(let b) = c { cplist.append(b) } else { stageOK = false } }
            }
            if let lrsArr = lrsItems {
                for r in lrsArr { if case .byteString(let b) = r { lrlist.append(b) } else { stageOK = false } }
            }
            checkpointsByStage.append(cplist)
            legResultsByStage.append(lrlist)
            let n = cplist.count
            let usable = stageOK && stages[g] != nil
            if !usable || lc == nil || lc! < 1 || lc! != UInt64(n) || n != lrlist.count {
                violate("leg_count")
            }
            for c in cplist where c.count != 32 { violate("checkpoint width") }
            for r in lrlist where r.count != 8 { violate("leg result width") }
        }

        // seed beacon.
        if stageBytes(0, "beacon") == nil || stageBytes(0, "beacon") != seedBeacon {
            violate("seed beacon")
        }

        // last_interrupt.
        var lastInterruptOK = lastInterrupt != nil && lastInterrupt!.count == 32
        let s17beacon = stageBytes(17, "beacon")
        if !lastInterruptOK || s17beacon == nil || lastInterrupt! == s17beacon! { lastInterruptOK = false }
        // height strictly greater; an out-of-range height is treated as absent.
        let s17height = stageUint(17, "beacon_height")
        let lihDefined = lastInterruptHeightRaw != nil && lastInterruptHeightRaw! <= Spec.heightMax
        if lihDefined {
            if s17height == nil || s17height! > Spec.heightMax || lastInterruptHeightRaw! <= s17height! {
                lastInterruptOK = false
            }
        } else {
            lastInterruptOK = false
        }
        if !lastInterruptOK { violate("last_interrupt") }

        // Stored headers, in order: height order and hash differ. A height
        // above 2^53 − 1 is treated as absent by every other shape condition.
        var storedHashes: [[UInt8]?] = []
        var storedHeights: [UInt64?] = []
        for g in 0..<18 { storedHashes.append(stageBytes(g, "beacon")); storedHeights.append(stageUint(g, "beacon_height")) }
        storedHashes.append(lastInterrupt)
        storedHeights.append(lihDefined ? lastInterruptHeightRaw : nil)
        for i in 1..<storedHashes.count {
            let prevH = storedHeights[i - 1]
            let thisH = storedHeights[i]
            let prevOK = prevH != nil && prevH! <= Spec.heightMax
            let thisOK = thisH != nil && thisH! <= Spec.heightMax
            if !prevOK || !thisOK || thisH! <= prevH! { violate("header height order") }
            let prevHash = storedHashes[i - 1]
            let thisHash = storedHashes[i]
            if prevHash == nil || thisHash == nil || prevHash! == thisHash! { violate("header hash differ") }
        }

        // Order the violated strings per the shape table (1…14), each once.
        let shapeTableOrder = [
            "malformed cbor", "field set", "stage-0 path key", "W_17 path", "constant",
            "stages length", "leg_count", "checkpoint width", "leg result width",
            "seed beacon", "last_interrupt", "header height order", "header hash differ",
            "height_range",
        ]
        var seenShape = Set<String>()
        var shapeItemOrdered: [String] = []
        for s in shapeTableOrder where shapeItems.contains(s) && seenShape.insert(s).inserted {
            shapeItemOrdered.append(s)
        }

        // ---------------- terrain ----------------
        var terrain: Terrain? = nil
        if let pk = publicKey, let sb = seedBeacon {
            terrain = Terrain(publicKey: pk, seedBeacon: sb, profile: profile)
        }
        let seedComputed: [UInt8]? = (publicKey != nil && seedBeacon != nil)
            ? Seed.seed(publicKey: publicKey!, seedBeacon: seedBeacon!) : nil

        // ---------------- headers (Rule A4.2) ----------------
        var headerMeas: [HeaderMeas] = []
        for i in 0..<19 {
            let hDefined = storedHeights[i] != nil && storedHeights[i]! <= Spec.heightMax
            headerMeas.append(readHeader(index: index, storedHash: storedHashes[i],
                                         storedHeight: storedHeights[i],
                                         heightDefined: hDefined,
                                         height: storedHeights[i]))
        }

        // ---------------- seed_value ----------------
        var seedValue = Outcome.mismatch
        if let s0 = stageBytes(0, "waypoint"), let computed = seedComputed {
            seedValue = s0 == computed ? Outcome.match : Outcome.mismatch
        } else {
            seedValue = Outcome.mismatch // absent inputs cannot equal the seed
        }

        // ---------------- checkpoints (Rule A4.3) ----------------
        var checkpointsRecord: [JVal] = []
        for g in 0..<18 {
            var legs: [JVal] = []
            let cps = checkpointsByStage[g]
            let lrs = legResultsByStage[g]
            if stages[g] == nil || terrain == nil {
                // Unusable stage bytes: that stage's legs is `not defined`.
                checkpointsRecord.append(.obj([
                    ("g", .uint(UInt64(g))),
                    ("legs", .str(Outcome.notDefined)),
                ]))
                continue
            }
            let limb = g == 0 ? seedComputed! : stageBytes(g, "waypoint")
            for (j, cp) in cps.enumerated() {
                if limb == nil {
                    legs.append(.obj([("j", .uint(UInt64(j))), ("result", .str(Outcome.notDefined))]))
                    continue
                }
                guard j < lrs.count else {
                    // A checkpoint with no leg result is `mismatch` (Rule A4.3).
                    legs.append(.obj([("j", .uint(UInt64(j))), ("result", .str(Outcome.mismatch))]))
                    continue
                }
                let previous: [UInt8]? = j == 0 ? limb : cps[j - 1]
                if previous == nil {
                    legs.append(.obj([("j", .uint(UInt64(j))), ("result", .str(Outcome.mismatch))]))
                    continue
                }
                let sample = terrain!.sample(at: previous!)
                let digest = SHK.sha256(concat: [previous!, limb!, sample, lrs[j]])
                legs.append(.obj([("j", .uint(UInt64(j))),
                                  ("result", .str(digest == cp ? Outcome.match : Outcome.mismatch))]))
            }
            checkpointsRecord.append(.obj([("g", .uint(UInt64(g))), ("legs", .arr(legs))]))
        }

        // ---------------- waypoints (Rule A4.3) ----------------
        var waypointsRecord: [JVal] = []
        for gPlus in 1...17 {
            let g = gPlus - 1 // stage pair (g, g+1)
            let B = stageBytes(gPlus, "beacon")
            let W = stageBytes(g, "waypoint")
            let storedWaypoint = stageBytes(gPlus, "waypoint")
            var result = Outcome.notDefined
            if B != nil && W != nil && storedWaypoint != nil && stages[g] != nil && terrain != nil {
                let recomputed = Waypoint.build(B: B!, terrain: terrain!, stageWaypoint: W!,
                                                checkpoints: checkpointsByStage[g])
                result = recomputed == storedWaypoint! ? Outcome.match : Outcome.mismatch
            } else if storedWaypoint != nil && B != nil && W != nil && stages[g] != nil && terrain == nil {
                result = Outcome.notDefined
            }
            waypointsRecord.append(.obj([("g", .uint(UInt64(gPlus))), ("result", .str(result))]))
        }

        // ---------------- paths (Rule A4.5) and anchors ----------------
        func claimedBlockJVal(_ body: CanonicalPathBody) -> JVal {
            .obj([
                ("block_hash", .str(Hex.encode(Array(body.blockHash.reversed())))),
                ("block_height", .uint(UInt64(body.blockHeight))),
            ])
        }

        func plantMatches(_ p: IndexPlant, _ body: CanonicalPathBody) -> Bool {
            p.root == body.root && p.txid == body.txid && p.vout == body.vout &&
            p.blockHash == body.blockHash && p.blockHeight == body.blockHeight
        }

        struct PathMeas {
            var state: String = Outcome.skip
            var plant: String = Outcome.na
            var Pg: JVal = .str(Outcome.notDefined)
            var heightPg: JVal = .str(Outcome.notDefined)
            var dg: JVal = .str(Outcome.notDefined)
            var claimed: JVal = .str(Outcome.notDefined)
            var anchor = Outcome.notAnchored
            var body: CanonicalPathBody? = nil
            var validPath = false
            var issuedPlantGet = false
            var plantReadNotReturned = false
        }
        var pathMeas: [PathMeas] = []
        var claimedByG: [UInt64: CanonicalPathBody] = [:]
        for g in 1...16 {
            var pm = PathMeas()
            let pathBytes = stageBytes(g, "path")
            if pathBytes == nil || pathBytes!.isEmpty {
                pm.state = Outcome.skip
                pm.plant = Outcome.na
            } else {
                pm.state = Outcome.present
                let wp = stageBytes(g, "waypoint")
                if let body = CanonicalPathBody(pathBytes!), let w = wp, body.validate(for: w) {
                    pm.body = body
                    pm.validPath = true
                    let H = stageUint(g, "beacon_height") // W_g posted at H
                    let inWindow = H != nil
                        && Spec.anchorWindowOffsets.contains { UInt64(body.blockHeight) == H! &+ $0 }
                    // That inclusion header must be an interrupt stored on the
                    // object: same height and the same 32 natural bytes.
                    var interruptMatch: [UInt8]? = nil
                    if inWindow {
                        for k in 0..<19 where storedHeights[k] == UInt64(body.blockHeight) {
                            if let sh = storedHashes[k], sh == body.blockHash {
                                interruptMatch = sh
                            }
                        }
                    }
                    if inWindow && interruptMatch != nil {
                        pm.issuedPlantGet = true
                        let doc: IndexDoc?
                        if let bodyText = index.get(height: UInt64(body.blockHeight)) {
                            doc = IndexDoc(body: Array(bodyText.utf8))
                        } else {
                            doc = nil
                        }
                        if doc == nil {
                            pm.plant = Outcome.notRead
                            pm.plantReadNotReturned = true
                            pm.claimed = claimedBlockJVal(body)
                        } else {
                            let d = doc!
                            if let net = d.network, net == "main" || net == "synthetic" {
                                let plants = d.hasPlantSet ? d.plants : []
                                var matched = false
                                for p in plants { if plantMatches(p, body) { matched = true } }
                                if !d.hasPlantSet, let p = d.plantMember, plantMatches(p, body) { matched = true }
                                if matched {
                                    pm.plant = Outcome.match
                                    pm.Pg = .obj([
                                        ("block_hash", .str(Hex.encode(Array(body.blockHash.reversed())))),
                                        ("block_height", .uint(UInt64(body.blockHeight))),
                                    ])
                                    pm.heightPg = .uint(UInt64(body.blockHeight))
                                    // d_g only when the index comparison of I_{g-1}
                                    // is also match (Rule A4.5); the subtraction uses
                                    // header.height from that matching document.
                                    if headerMeas[g].comparison == Outcome.match,
                                       let ha = headerMeas[g].storedHeight, ha <= Spec.heightMax {
                                        pm.dg = .int(Int64(UInt64(body.blockHeight)) - Int64(ha))
                                    }
                                    pm.anchor = Outcome.anchored
                                } else {
                                    pm.plant = Outcome.mismatch
                                }
                            } else {
                                pm.plant = Outcome.mismatch
                            }
                            pm.claimed = claimedBlockJVal(body)
                        }
                    } else {
                        // The read was not issued: plant `not read` (14.3).
                        pm.plant = Outcome.notRead
                        pm.claimed = claimedBlockJVal(body)
                    }
                } else {
                    pm.plant = Outcome.mismatch
                    pm.claimed = .str(Outcome.notDefined)
                }
            }
            pathMeas.append(pm)
        }

        // W_17.
        let w17J: JVal
        if let p17 = w17Path, p17.isEmpty {
            w17J = .obj([("g", .uint(17)),
                         ("w17_path", .str(Outcome.notFetchedByDesign)),
                         ("claimed_block", .str(Outcome.notDefined))])
        } else {
            var claimed: JVal = .str(Outcome.notDefined)
            if let p17 = w17Path, !p17.isEmpty {
                // Decode only far enough to read block_hash and block_height,
                // using the canonical path layout (14.3). No validate_for.
                if let (bh, bht) = CanonicalPathBody.lenientBlockLocator(p17) {
                    claimed = .obj([
                        ("block_hash", .str(Hex.encode(Array(bh.reversed())))),
                        ("block_height", .uint(UInt64(bht))),
                    ])
                }
            }
            w17J = .obj([("g", .uint(17)), ("w17_path", .str(Outcome.notDefined)),
                         ("claimed_block", claimed)])
        }

        let anchors = pathMeas.map { $0.anchor }
        let anchoredTotal = anchors.filter { $0 == Outcome.anchored }.count
        let skipList = (1...16).filter { pathMeas[$0 - 1].state == Outcome.skip }.map { UInt64($0) }

        // ---------------- bookends (Rules A4.6–A4.11) ----------------
        // Global leg indices (section 14.6): start_0 = 0; the index q with
        // start_g ≤ q < start_{g+1} is leg j = q − start_g of stage g.
        var starts: [Int] = []
        var totalLegs = 0
        var allStagesUsable = true
        for g in 0..<18 {
            starts.append(totalLegs)
            if stages[g] == nil { allStagesUsable = false; continue }
            totalLegs += checkpointsByStage[g].count
        }
        if !allStagesUsable { totalLegs = -1 }

        func notDefAll() -> [(String, JVal)] {
            [("a", .str(Outcome.notDefined)), ("b", .str(Outcome.notDefined)),
             ("N", .str(Outcome.notDefined)), ("S", .str(Outcome.notDefined)),
             ("delta_h", .str(Outcome.notDefined)), ("P_b", .str(Outcome.notDefined)),
             ("height_P_b", .str(Outcome.notDefined)), ("index_comparison_P_b", .str(Outcome.notDefined)),
             ("T_P_b", .str(Outcome.notDefined)), ("height_I_a_minus_1", .str(Outcome.notDefined)),
             ("T_I_a_minus_1", .str(Outcome.notDefined))]
        }

        var bookends: [(String, JVal)] = []
        var fixedStart: JVal = .obj([
            ("N", .str(Outcome.notDefined)), ("S", .str(Outcome.notDefined)),
            ("delta_h", .str(Outcome.notDefined)),
        ])
        var legsOutside: JVal = .str(Outcome.notDefined)

        let validPaths = pathMeas.filter { $0.validPath }.count
        let anchoredCount = anchors.filter { $0 == Outcome.anchored }.count

        if validPaths < 2 {
            bookends = notDefAll()
        } else if pathMeas.contains(where: { $0.issuedPlantGet && $0.plantReadNotReturned }) {
            // Step 2: an issued plant GET that does not return a document
            // records every bookend field `not read`.
            bookends = notDefAll().map { key, _ in (key, .str(Outcome.notRead)) }
            fixedStart = .obj([
                ("N", .str(Outcome.notRead)), ("S", .str(Outcome.notRead)),
                ("delta_h", .str(Outcome.notRead)),
            ])
            legsOutside = .str(Outcome.notRead)
        } else if anchoredCount < 2 {
            bookends = notDefAll()
        } else {
            // Step 4: a, b, N integers; then the endpoint reads.
            let aIdx = anchors.firstIndex(of: Outcome.anchored)!
            let bIdx = anchors.lastIndex(of: Outcome.anchored)!
            let a = aIdx + 1, b = bIdx + 1   // W_a … W_b
            var N = 0
            for g in a...(b - 1) { N += checkpointsByStage[g].count }

            let headA = headerMeas[a]           // I_{a-1} = headers[a]
            let bMeas = pathMeas[bIdx]
            let bodyB = bMeas.body!
            let pbHeight = UInt64(bodyB.blockHeight)

            // The P_b header read reuses the same height (Rule A4.8).
            var pbDoc: IndexDoc? = nil
            if let text = index.get(height: pbHeight) {
                pbDoc = IndexDoc(body: Array(text.utf8))
            }
            var cmpPb = Outcome.notRead
            var TPb: JVal = .str(Outcome.notRead)
            var heightPb: JVal = .str(Outcome.notDefined)
            var Pb: JVal = .str(Outcome.notDefined)
            if let d = pbDoc {
                if d.usableHeader {
                    let nat = d.headerHashNatural!
                    if nat == bodyB.blockHash && d.headerHeight == pbHeight {
                        cmpPb = Outcome.match
                        heightPb = .uint(pbHeight)
                        if let t = d.headerTime { TPb = .int(Int64(t)) }
                    } else {
                        cmpPb = Outcome.mismatch
                    }
                } else {
                    cmpPb = Outcome.notRead
                }
            }
            // P_b is the block encoding for W_b when bookends exist.
            Pb = .obj([
                ("block_hash", .str(Hex.encode(Array(bodyB.blockHash.reversed())))),
                ("block_height", .uint(pbHeight)),
            ])

            if headA.comparison == Outcome.notRead || cmpPb == Outcome.notRead {
                // Every bookend field `not read` (Rule A4.11 step 4).
                bookends = notDefAll().map { key, _ in (key, .str(Outcome.notRead)) }
                fixedStart = .obj([
                    ("N", .str(Outcome.notRead)), ("S", .str(Outcome.notRead)),
                    ("delta_h", .str(Outcome.notRead)),
                ])
                legsOutside = .str(Outcome.notRead)
            } else {
                // A returned `mismatch` is not the all-not-read case: a, b and N
                // stay integers, S is `not read` when a T was not taken, and
                // delta_h is `not defined` when a height is not index-confirmed.
                let S: JVal
                if let tp = TPb.int64Value(), let ta = headA.theight {
                    S = .int(tp - ta)
                } else {
                    S = .str(Outcome.notRead)
                }
                let deltaH: JVal
                if case .uint(let hpb) = heightPb,
                   headA.comparison == Outcome.match,
                   let ha = headA.storedHeight, ha <= Spec.heightMax {
                    deltaH = .int(Int64(hpb) - Int64(ha))
                } else {
                    deltaH = .str(Outcome.notDefined)
                }
                let heightIa1: JVal = headA.comparison == Outcome.match && headA.storedHeight != nil && headA.storedHeight! <= Spec.heightMax
                    ? .uint(headA.storedHeight!) : .str(Outcome.notDefined)
                bookends = [
                    ("a", .uint(UInt64(a))), ("b", .uint(UInt64(b))), ("N", .uint(UInt64(N))),
                    ("S", S), ("delta_h", deltaH), ("P_b", Pb), ("height_P_b", heightPb),
                    ("index_comparison_P_b", .str(cmpPb)), ("T_P_b", TPb),
                    ("height_I_a_minus_1", heightIa1),
                    ("T_I_a_minus_1", headerValue(headA.T, headA.theight)),
                ]
                // fixed_start: N over stages 1 … b−1, S = T_P_b − headers[1].T.
                var fsN = 0
                for g in 1...(b - 1) { fsN += checkpointsByStage[g].count }
                let head1 = headerMeas[1]
                var fsS: JVal = .str(Outcome.notRead)
                if let tp = TPb.int64Value(), let t1 = head1.theight { fsS = .int(tp - t1) }
                var fsDh: JVal = .str(Outcome.notDefined)
                if case .uint(let hpb) = heightPb, head1.comparison == Outcome.match,
                   let h1 = head1.storedHeight, h1 <= Spec.heightMax {
                    fsDh = .int(Int64(hpb) - Int64(h1))
                } else if head1.comparison == Outcome.notRead || cmpPb == Outcome.notRead {
                    fsDh = .str(Outcome.notRead)
                }
                fixedStart = .obj([("N", .uint(UInt64(fsN))), ("S", fsS), ("delta_h", fsDh)])
                var outside = 0
                for g in 0..<18 where g < a || g >= b { outside += checkpointsByStage[g].count }
                legsOutside = .uint(UInt64(outside))
            }
        }

        // ---------------- six_leg (Rule A4.4) ----------------
        var sixLeg: JVal = .str(Outcome.notDefined)
        var sixLegRuns: JVal = .arr([])
        if totalLegs >= 6 {
            var source = options.makeDrawSource()
            if let indices = Draw.sixLeg(legTotal: UInt64(totalLegs), source: &source) {
                var results: [JVal] = []
                for q in indices {
                    // Find the stage g and leg j for global index q.
                    var g = 0
                    while g + 1 < 18 && UInt64(q) >= UInt64(starts[g + 1]) { g += 1 }
                    let j = Int(q) - starts[g]
                    let lrs = legResultsByStage[g]
                    var result = Outcome.mismatch
                    if terrain != nil && j < lrs.count {
                        let limb = g == 0 ? seedComputed : stageBytes(g, "waypoint")
                        let previous: [UInt8]? = j == 0 ? limb
                            : (j - 1 < checkpointsByStage[g].count ? checkpointsByStage[g][j - 1] : nil)
                        if let prev = previous, let lim = limb {
                            let lr = Leg.result(terrain: terrain!, previousCheckpoint: prev)
                            result = lr == LE.u64(lrs[j]) ? Outcome.match : Outcome.mismatch
                        }
                    }
                    results.append(.str(result))
                }
                let drawObj = JVal.obj([
                    ("indices", .arr(indices.map { .uint($0) })),
                    ("results", .arr(results)),
                ])
                sixLegRuns = .arr([drawObj])
                sixLeg = drawObj
            } else {
                sixLeg = .str(Outcome.notRead)
                sixLegRuns = .str(Outcome.notRead)
            }
        }

        // ---------------- analysis (Rule A4.10) ----------------
        var wholeHikeLegs: JVal = .str(Outcome.notDefined)
        if allStagesUsable {
            var n = 0
            for g in 1...17 { n += checkpointsByStage[g].count }
            wholeHikeLegs = .uint(UInt64(n))
        }
        var wholeHikeSpan: JVal = .str(Outcome.notRead)
        if let t18 = headerMeas[18].theight, let t1 = headerMeas[1].theight {
            wholeHikeSpan = .int(t18 - t1)
        }
        var perStage: [JVal] = []
        for g in 0..<18 {
            var span: JVal = .str(Outcome.notRead)
            // Stage 0: T(I_0) − T(seed_beacon); stages 1-16: T(I_g) − T(I_{g-1});
            // stage 17: T(I_17) − T(I_16). All from the Rule A4.2 reads.
            let prevT = g == 0 ? headerMeas[0].theight : headerMeas[g].theight
            let thisT = g == 17 ? headerMeas[18].theight : headerMeas[g + 1].theight
            if let p = prevT, let t = thisT { span = .int(t - p) }
            let count: JVal = stages[g] == nil ? .str(Outcome.notDefined)
                : .uint(UInt64(checkpointsByStage[g].count))
            perStage.append(.obj([
                ("g", .uint(UInt64(g))),
                ("checkpoint_count", count),
                ("span", span),
            ]))
        }
        var hikeLegs: JVal = .str(Outcome.notDefined)
        if allStagesUsable { hikeLegs = .uint(UInt64(totalLegs)) }
        var hikeSpan: JVal = .str(Outcome.notRead)
        if allStagesUsable,
           let mtp0 = medianTimePast(beaconHeight: storedHeights[0] ?? 0,
                                     beaconMatch: headerMeas[0].comparison == Outcome.match,
                                     beaconTime: headerMeas[0].theight, index: index),
           let mtp18 = medianTimePast(beaconHeight: lastInterruptHeightRaw ?? 0,
                                      beaconMatch: headerMeas[18].comparison == Outcome.match,
                                      beaconTime: headerMeas[18].theight, index: index) {
            hikeSpan = .int(mtp18 - mtp0)
        }

        // ---------------- record assembly ----------------
        var top: [(String, JVal)] = []
        top.append(("profile", .str(profile.name)))
        if profile.name == "conformance-small" {
            top.append(("sample_bytes", .uint(UInt64(profile.sampleBytes))))
            top.append(("slot_bytes", .uint(UInt64(profile.slotBytes))))
            top.append(("terrain_bytes", .uint(UInt64(profile.terrainBytes))))
            top.append(("steps_per_leg", .uint(UInt64(profile.stepsPerLeg))))
        }
        top.append(("shape", .str(shapeMatch ? Outcome.match : Outcome.mismatch)))
        top.append(("shape_item", .arr(shapeItemOrdered.map { .str($0) })))
        var headersRecord: [JVal] = []
        for m in headerMeas {
            let shJ: JVal
            if let sh = m.storedHeight, sh <= Spec.heightMax {
                shJ = .uint(sh)
            } else {
                shJ = .str(Outcome.notDefined)
            }
            headersRecord.append(.obj([
                ("stored_hash", .str(m.storedHash.map { Hex.encode($0) } ?? Outcome.notDefined)),
                ("stored_height", shJ),
                ("index_comparison", .str(m.comparison)),
                ("T", headerValue(m.T, m.theight)),
            ]))
        }
        top.append(("headers", .arr(headersRecord)))
        top.append(("seed_value", .str(seedValue)))
        top.append(("checkpoints", .arr(checkpointsRecord)))
        top.append(("waypoints", .arr(waypointsRecord)))
        var pathsRecord: [JVal] = []
        for (i, pm) in pathMeas.enumerated() {
            pathsRecord.append(.obj([
                ("g", .uint(UInt64(i + 1))),
                ("path_state", .str(pm.state)),
                ("plant", .str(pm.plant)),
                ("P_g", pm.Pg),
                ("height_P_g", pm.heightPg),
                ("d_g", pm.dg),
                ("claimed_block", pm.claimed),
            ]))
        }
        pathsRecord.append(w17J)
        top.append(("paths", .arr(pathsRecord)))
        top.append(("anchors", .arr(anchors.map { .str($0) })))
        top.append(("anchored_total", .uint(UInt64(anchoredTotal))))
        top.append(("skip_list", .arr(skipList.map { .uint($0) })))
        for (k, v) in bookends {
            if k == "a" || k == "b" || k == "N" { top.append((k, v)) }
        }
        top.append(("S", bookends.first(where: { $0.0 == "S" })?.1 ?? .str(Outcome.notDefined)))
        top.append(("delta_h", bookends.first(where: { $0.0 == "delta_h" })?.1 ?? .str(Outcome.notDefined)))
        top.append(("fixed_start", fixedStart))
        for k in ["P_b", "height_P_b", "index_comparison_P_b", "T_P_b",
                  "height_I_a_minus_1", "T_I_a_minus_1"] {
            top.append((k, bookends.first(where: { $0.0 == k })?.1 ?? .str(Outcome.notDefined)))
        }
        top.append(("six_leg", sixLeg))
        top.append(("six_leg_runs", sixLegRuns))
        top.append(("analysis", .obj([
            ("whole_hike_legs", wholeHikeLegs),
            ("whole_hike_span", wholeHikeSpan),
            ("per_stage", .arr(perStage)),
            ("legs_outside", legsOutside),
            ("hike_legs", hikeLegs),
            ("hike_span", hikeSpan),
        ])))
        return .obj(top)
    }
}

extension JVal {
    func int64Value() -> Int64? {
        if case .int(let i) = self { return i }
        return nil
    }
    func uint64Value() -> UInt64? {
        if case .uint(let u) = self { return u }
        return nil
    }
}
