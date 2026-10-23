// The test-plane producer: builds the hike over the CASES.md fixture
// beacons, encodes the object (section 7.3), and writes the Rule A3.4 stderr
// lines at the close. Offline: there is no Bari, so every waypoint POST is a
// miss, every W_1 … W_16 path is a skip and W_17 is not asked — the exact
// fixture outcome CASES.md fixes ("the empty path bytes of §13.3
// (path_skip) for every stage").

public enum Producer {
    /// Build one fixture hike and its encoded object bytes.
    public static func buildObject(profile: Profile, legsPerStage: [Int],
                                   publicKey: [UInt8]? = nil,
                                   seedBeacon: [UInt8]? = nil) throws -> [UInt8] {
        let pk = publicKey ?? Fixture.pkA
        let sb = seedBeacon ?? Fixture.seedBeacon
        let hike = Hike(publicKey: pk, seedBeacon: sb,
                        seedBeaconHeight: Fixture.seedBeaconHeight,
                        interrupts: Fixture.interrupts,
                        interruptHeights: Fixture.interruptHeights,
                        legsPerStage: legsPerStage, profile: profile)
        var objStages: [HikeObjectStage] = []
        for (g, st) in hike.stages.enumerated() {
            objStages.append(HikeObjectStage(
                beacon: st.beacon, beaconHeight: st.beaconHeight, waypoint: st.waypoint,
                path: g == 0 ? nil : [],   // stage 0 has no path key; skips are empty bytes
                checkpoints: st.checkpoints,
                legResults: st.legResults.map { LE.le64($0) }))
        }
        let obj = HikeObject(profile: profile, publicKey: pk, seedBeacon: sb,
                             stages: objStages,
                             lastInterrupt: hike.lastInterrupt,
                             lastInterruptHeight: hike.lastInterruptHeight)
        return ObjectEncode.encode(obj)
    }

    /// The object file names CASES.md fixes, for the test plane.
    public static let o1Name = "O1-S1.cbor"
    public static let o2Name = "O2-S2.cbor"
    public static let o3Name = "O3-PROD.cbor"

    /// Re-encode an object's decoded map with only the `kind` value replaced
    /// by `newKind` (one well-formed CBOR map; section 7.3 sorts it).
    public static func replaceKind(object: [UInt8], newKind: String) -> [UInt8] {
        guard case .hikeObject(let root) = ObjectDecode.decode(object) else {
            preconditionFailure("replaceKind expects a decoded hike object")
        }
        return reencode(root, replacedKey: "kind", replacedText: newKind)
    }

    /// Deterministic re-encoding of a decoded item (shortest uints, sorted map
    /// keys), with one text value replaced.
    static func reencode(_ item: CBORItem, replacedKey: String? = nil,
                         replacedText: String? = nil) -> [UInt8] {
        switch item {
        case .uint(let v):
            return CBOR.encodeUint(v)
        case .byteString(let b):
            return CBOR.encodeBytes(b)
        case .text(let s):
            return CBOR.encodeText(s)
        case .array(let items):
            var out = CBOR.head(major: 4, length: items.count)
            for it in items { out += reencode(it) }
            return out
        case .map(let entries):
            var out = CBOR.head(major: 5, length: entries.count)
            let sorted = entries.sorted { a, b in
                Array(CBOR.encodeText(a.0)).lexicographicallyPrecedes(Array(CBOR.encodeText(b.0)))
            }
            for (k, v) in sorted {
                out += CBOR.encodeText(k)
                if k == replacedKey, case .text = v, let t = replacedText {
                    out += CBOR.encodeText(t)
                } else {
                    out += reencode(v)
                }
            }
            return out
        }
    }

    /// Rule A3.4 produce stderr at the close: `path_skip <g>` for each empty
    /// path among W_1 … W_16 in ascending g, then
    /// `not fetched by design 17`.
    public static func closeStderrLines() -> [String] {
        var lines: [String] = []
        for g in 1...16 { lines.append("path_skip \(g)") }
        lines.append("not fetched by design 17")
        return lines
    }
}
