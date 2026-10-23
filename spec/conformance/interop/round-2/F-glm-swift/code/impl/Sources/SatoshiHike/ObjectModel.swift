// SECTION 8 hike object (Rule A3) and its section 7.3 deterministic encoding,
// plus the section 7.3 decode order used by verify (and Rule A5).

public struct HikeObjectStage {
    public var beacon: [UInt8]
    public var beaconHeight: UInt64
    public var waypoint: [UInt8]
    public var path: [UInt8]?   // nil on stage 0 (the key is omitted); [] for a skip
    public var legCount: Int { checkpoints.count }
    public var checkpoints: [[UInt8]]
    public var legResults: [[UInt8]]   // le64 per leg

    public init(beacon: [UInt8], beaconHeight: UInt64, waypoint: [UInt8],
                path: [UInt8]?, checkpoints: [[UInt8]], legResults: [[UInt8]]) {
        self.beacon = beacon
        self.beaconHeight = beaconHeight
        self.waypoint = waypoint
        self.path = path
        self.checkpoints = checkpoints
        self.legResults = legResults
    }
}

public struct HikeObject {
    public var publicKey: [UInt8]
    public var seedBeacon: [UInt8]
    public var stages: [HikeObjectStage]   // 18, stage 0 first
    public var lastInterrupt: [UInt8]
    public var lastInterruptHeight: UInt64
    public var slotBytes: UInt64
    public var nSlots: UInt64
    public var sampleBytes: UInt64
    public var stepsPerLeg: UInt64

    public init(profile: Profile, publicKey: [UInt8], seedBeacon: [UInt8],
                stages: [HikeObjectStage], lastInterrupt: [UInt8],
                lastInterruptHeight: UInt64) {
        self.slotBytes = UInt64(profile.slotBytes)
        self.nSlots = UInt64(profile.nSlots)
        self.sampleBytes = UInt64(profile.sampleBytes)
        self.stepsPerLeg = UInt64(profile.stepsPerLeg)
        self.publicKey = publicKey
        self.seedBeacon = seedBeacon
        self.stages = stages
        self.lastInterrupt = lastInterrupt
        self.lastInterruptHeight = lastInterruptHeight
    }
}

public enum ObjectEncode {
    /// SECTION 7.3 core deterministic encoding of one hike object
    /// (definite lengths, shortest uints, sorted map keys).
    public static func encode(_ obj: HikeObject) -> [UInt8] {
        var out: [UInt8] = []
        let topKeys = CBOR.sortedKeys(["fill", "kind", "stages", "slot_bytes", "n_slots",
                                       "public_key", "seed_beacon", "sample_bytes",
                                       "steps_per_leg", "last_interrupt", "last_interrupt_height"])
        out.append(0xA0 | UInt8(topKeys.count)) // map head, 11 pairs
        for key in topKeys {
            out += CBOR.encodeText(key)
            switch key {
            case "fill": out += CBOR.encodeText(Spec.fill)
            case "kind": out += CBOR.encodeText(Spec.kind)
            case "stages":
                out += CBOR.head(major: 4, length: obj.stages.count)
                for (g, st) in obj.stages.enumerated() {
                    out += encodeStage(st, stageZero: g == 0)
                }
            case "slot_bytes": out += CBOR.encodeUint(obj.slotBytes)
            case "n_slots": out += CBOR.encodeUint(obj.nSlots)
            case "public_key": out += CBOR.encodeBytes(obj.publicKey)
            case "seed_beacon": out += CBOR.encodeBytes(obj.seedBeacon)
            case "sample_bytes": out += CBOR.encodeUint(obj.sampleBytes)
            case "steps_per_leg": out += CBOR.encodeUint(obj.stepsPerLeg)
            case "last_interrupt": out += CBOR.encodeBytes(obj.lastInterrupt)
            case "last_interrupt_height": out += CBOR.encodeUint(obj.lastInterruptHeight)
            default: fatalError("unreachable")
            }
        }
        return out
    }

    static func encodeStage(_ st: HikeObjectStage, stageZero: Bool) -> [UInt8] {
        var keys = ["beacon", "waypoint", "leg_count", "checkpoints", "leg_results", "beacon_height"]
        if !stageZero { keys.insert("path", at: 0) } // stage 0 has no path key (Rule A3.3)
        let sorted = CBOR.sortedKeys(keys)
        var out: [UInt8] = [0xA0 | UInt8(sorted.count)]
        for key in sorted {
            out += CBOR.encodeText(key)
            switch key {
            case "path": out += CBOR.encodeBytes(st.path ?? [])
            case "beacon": out += CBOR.encodeBytes(st.beacon)
            case "waypoint": out += CBOR.encodeBytes(st.waypoint)
            case "leg_count": out += CBOR.encodeUint(UInt64(st.legCount))
            case "checkpoints":
                out += CBOR.head(major: 4, length: st.checkpoints.count)
                for c in st.checkpoints { out += CBOR.encodeBytes(c) }
            case "leg_results":
                out += CBOR.head(major: 4, length: st.legResults.count)
                for r in st.legResults { out += CBOR.encodeBytes(r) }
            case "beacon_height": out += CBOR.encodeUint(st.beaconHeight)
            default: fatalError("unreachable")
            }
        }
        return out
    }
}

/// SECTION 7.3 decode order (with erratum 1):
///  1. Input that is not one definite-length CBOR data item under these rules
///     is a malformed CBOR value -> shape `mismatch`, shape_item exactly
///     ["malformed cbor"] (section 14), and the CLI still emits a record.
///  2. One well-formed data item that is not a map, or a map with no `kind`
///     key whose value is a text string, is a decode failure: no record,
///     exit code 1.
///  3. A map whose `kind` text is not exactly KIND is Rule A5.
///  4. Everything else is measured by section 14.
public enum ObjectDecodeOutcome {
    case malformed
    case decodeFailure
    case unknownKind(String)   // Rule A5
    case hikeObject(CBORItem)  // kind == KIND; the generic decoded map
}

public enum ObjectDecode {
    public static func decode(_ bytes: [UInt8]) -> ObjectDecodeOutcome {
        switch CBOR.decode(bytes) {
        case .malformed:
            return .malformed
        case .item(let item):
            guard case .map = item else { return .decodeFailure }
            guard case .text(let kind)? = CBOR.field(item, "kind") else { return .decodeFailure }
            if kind != Spec.kind { return .unknownKind(kind) }
            return .hikeObject(item)
        }
    }
}

extension ObjectDecode {
    /// Rule A5 record bytes: {"kind":"unknown kind","kind_text":"<text>"},
    /// keys in that order, no insignificant whitespace, kind_text spelled as
    /// RFC 8785 spells a JSON string. The CLI's trailing newline is not part
    /// of the record.
    public static func ruleA5Record(kindText: String) -> [UInt8] {
        var out = Array("{\"kind\":\"unknown kind\",\"kind_text\":\"".utf8)
        out += JSONWriter.escapeString(kindText)
        out += Array("\"}".utf8)
        return out
    }
}
