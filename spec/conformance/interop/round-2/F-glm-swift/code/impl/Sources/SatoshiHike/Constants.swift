// SPEC 2.2 section 6 constants and the profile model (sections 6, 16.1).

public enum Spec {
    public static let kind = "v2.2-pcoc-object"            // P6.15 KIND
    public static let fill = "dag2"                        // P6.16 FILL
    public static let sealDomain: [UInt8] = Array("pcoc-walk-v1".utf8) // P6.13, 12 bytes
    public static let stages = 18                         // P6.8 STAGES
    public static let hashBytes = 32                      // P6.9
    public static let legResultBytes = 8                  // P6.10
    public static let publicKeyBytes = 32                 // P6.11
    public static let beaconBytes = 32                    // P6.12

    public static let mixAdd: UInt64 = 0x9E3779B97F4A7C15
    public static let mixMul1: UInt64 = 0xBF58476D1CE4E5B9
    public static let mixMul2: UInt64 = 0x94D049BB133111EB
    public static let wordXor: UInt64 = 0xC0FFEE
    public static let parentMul: UInt64 = 0xD1B54A32D192ED03
    public static let slotMul: UInt64 = 0xA24BAED4963EE407

    public static let anchorWindowOffsets: [UInt64] = [2, 3, 4] // section 13.1, P13.2

    /// The height bound of section 9 / section 14 (2^53 - 1).
    public static let heightMax: UInt64 = (1 << 53) - 1
}

/// The profile in force (section 14, section 16.1). `production` is the section 6
/// constants. `conformance-small` differs only in the four integers, which QA picks.
public struct Profile: Equatable, Sendable {
    public let name: String // "production" or "conformance-small"
    public let sampleBytes: Int
    public let slotBytes: Int
    public let terrainBytes: Int
    public let stepsPerLeg: Int

    public init(production: ()) {
        name = "production"
        slotBytes = 16384
        sampleBytes = 1024
        terrainBytes = 268_435_456
        stepsPerLeg = 2_000_000
    }

    public init(sampleBytes: Int, slotBytes: Int, terrainBytes: Int, stepsPerLeg: Int) {
        name = "conformance-small"
        self.sampleBytes = sampleBytes
        self.slotBytes = slotBytes
        self.terrainBytes = terrainBytes
        self.stepsPerLeg = stepsPerLeg
    }

    public static let production = Profile(production: ())

    // Section 6 formulas (P6.23, P6.24, P6.25) applied to the profile values.
    public var wordsPerSlot: Int { slotBytes / 8 }
    public var nSlots: Int { terrainBytes / slotBytes }
    public var offsetModulus: UInt64 { UInt64(terrainBytes - sampleBytes + 1) }

    /// Section 16.1 constraints for a conformance-small profile.
    public var isValidConformanceSmall: Bool {
        name == "production" || (
            slotBytes >= 8 && slotBytes % 8 == 0 && slotBytes <= 16384 &&
            stepsPerLeg >= 1 && stepsPerLeg <= 2_000_000 &&
            sampleBytes >= 1 && sampleBytes <= 1024 &&
            terrainBytes <= 268_435_456 &&
            terrainBytes % slotBytes == 0 &&
            terrainBytes >= sampleBytes
        )
    }
}

/// SECTION 14 outcome tokens (section 4, closed lowercase set).
public enum Outcome {
    public static let match = "match"
    public static let mismatch = "mismatch"
    public static let present = "present"
    public static let skip = "skip"
    public static let notDefined = "not defined"
    public static let notRead = "not read"
    public static let notFetchedByDesign = "not fetched by design"
    public static let na = "n/a"
    public static let anchored = "anchored"
    public static let notAnchored = "not anchored"
    public static let unknownKind = "unknown kind"
}
