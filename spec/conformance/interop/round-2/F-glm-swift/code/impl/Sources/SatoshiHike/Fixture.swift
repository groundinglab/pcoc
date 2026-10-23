// The CASES.md fixed inputs (round 2): keys, beacons, heights, profiles,
// and the synthetic test-plane index.

public enum Fixture {
    public static let pkAHex = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"
    public static let pkBHex = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"

    public static var pkA: [UInt8] { Hex.decode(pkAHex, expectBytes: 32)! }
    public static var pkB: [UInt8] { Hex.decode(pkBHex, expectBytes: 32)! }

    public static var seedBeacon: [UInt8] {
        SHK.sha256(Array("pcoc-pack-2.2/beacon/seed".utf8))
    }

    /// I_k = SHA-256(ASCII "pcoc-pack-2.2/beacon/" ‖ decimal k), k = 0 … 17.
    public static func interrupt(_ k: Int) -> [UInt8] {
        SHK.sha256(Array("pcoc-pack-2.2/beacon/\(k)".utf8))
    }

    public static var interrupts: [[UInt8]] { (0...17).map { interrupt($0) } }

    /// Beacon heights: seed_beacon is at height 1000; I_k is at 1001 + k, so
    /// stages[g].beacon_height = 1000 + g and last_interrupt_height = 1018.
    public static let seedBeaconHeight: UInt64 = 1000
    public static func interruptHeight(_ k: Int) -> UInt64 { 1001 + UInt64(k) }
    public static var interruptHeights: [UInt64] { (0...17).map { interruptHeight($0) } }
    public static let lastInterruptHeight: UInt64 = 1018

    /// Profiles (CASES.md "Fixed inputs" table; each meets the section 16.1
    /// constraints). These are fixture inputs, not SPEC parameters.
    public static let S1 = Profile(sampleBytes: 32, slotBytes: 64,
                                   terrainBytes: 65_536, stepsPerLeg: 100)
    public static let S2 = Profile(sampleBytes: 64, slotBytes: 256,
                                   terrainBytes: 1_048_576, stepsPerLeg: 1000)
    public static let legsPerStageS1 = [Int](repeating: 1, count: 18)
    public static let legsPerStageS2 = [Int](repeating: 3, count: 18)

    /// The index plant body filler for non-beacon heights (CASES.md objects).
    public static func indexFiller(_ h: UInt64) -> [UInt8] {
        SHK.sha256(Array("pcoc-pack-2.2/index/filler/\(h)".utf8))
    }
}
