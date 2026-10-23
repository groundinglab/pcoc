// SECTION 11 leg: the step (11.1), the leg result (11.2), the checkpoint
// (11.4). SECTION 12.2 waypoint. SECTION 13.2 drive (one hike over the
// fixture beacons).

import CSatoshiKernels

public enum Leg {

    /// SECTION 11.1 step, with the measurements the fold vector needs.
    public struct StepTrace {
        public let S: UInt64          // input state
        public let slotIndex: UInt64  // slot read: S mod N_SLOTS
        public let acc: UInt64        // section 11.1 accumulator after the full slot
        public let next: UInt64       // u64_le(E[0:8])
    }

    public static func step(terrain: Terrain, S: UInt64, opt: Int32 = FoldOpt.auto) -> UInt64 {
        terrain.withRaw { raw in
            shk_leg_run(raw, UInt64(terrain.nSlots), terrain.wordsPerSlot, 1, S, opt)
        }
    }

    public static func stepTrace(terrain: Terrain, S: UInt64, opt: Int32 = FoldOpt.auto) -> StepTrace {
        var slotIndex: UInt64 = 0
        var acc: UInt64 = 0
        let next = terrain.withRaw { raw in
            shk_step_measure(raw, UInt64(terrain.nSlots), terrain.wordsPerSlot, S, &slotIndex, &acc, opt)
        }
        return StepTrace(S: S, slotIndex: slotIndex, acc: acc, next: next)
    }

    /// SECTION 11.2 leg result: STEPS_PER_LEG steps from the first eight bytes
    /// of the previous checkpoint. The leg result is the u64 end state.
    public static func result(terrain: Terrain, previousCheckpoint: [UInt8],
                              steps: Int? = nil, opt: Int32 = FoldOpt.auto) -> UInt64 {
        let n = steps ?? terrain.profile.stepsPerLeg
        let S0 = LE.u64(Array(previousCheckpoint[0..<8]))
        return terrain.withRaw { raw in
            shk_leg_run(raw, UInt64(terrain.nSlots), terrain.wordsPerSlot, UInt64(n), S0, opt)
        }
    }

    /// SECTION 11.4 checkpoint (Rule A2.3):
    /// H(previous ‖ stage limb ‖ terrain sample(at previous) ‖ le64(leg result)).
    public static func checkpoint(previous: [UInt8], stageLimb: [UInt8], sample: [UInt8],
                                  legResult: UInt64) -> [UInt8] {
        SHK.sha256(concat: [previous, stageLimb, sample, LE.le64(legResult)])
    }
}

extension Terrain {
    /// Run a raw leg over the terrain buffer without copying it.
    public func withRaw<T>(_ body: (UnsafeRawPointer) -> T) -> T {
        body(rawPointer)
    }
}

public enum Waypoint {
    /// SECTION 12.2 fold: F_0 = 0^32; F_{j+1} = SHA-256(F_j ‖ C[j]); fold = F_n.
    public static func fold(_ checkpoints: [[UInt8]]) -> [UInt8] {
        var f = [UInt8](repeating: 0, count: 32)
        for c in checkpoints {
            f = SHK.sha256(concat: [f, c])
        }
        return f
    }

    /// SECTION 12.2 (Rule A1.3):
    /// W_{g+1} = SHA-256(B ‖ slot_sample(B) ‖ W ‖ fold)
    /// B is the interrupt I_g in natural order; W is stages[g].waypoint
    /// (the seed when g = 0).
    public static func build(B: [UInt8], terrain: Terrain, stageWaypoint: [UInt8],
                             checkpoints: [[UInt8]]) -> [UInt8] {
        let sample = terrain.slotSample(beacon: B)
        return SHK.sha256(concat: [B, sample, stageWaypoint, fold(checkpoints)])
    }
}

/// One produced stage (SECTION 8 Rule A3.2).
public struct StageRecord {
    public let beacon: [UInt8]
    public let beaconHeight: UInt64
    public let waypoint: [UInt8]   // seed on stage 0
    public let checkpoints: [[UInt8]]
    public let legResults: [UInt64]

    public init(beacon: [UInt8], beaconHeight: UInt64, waypoint: [UInt8],
                checkpoints: [[UInt8]], legResults: [UInt64]) {
        self.beacon = beacon
        self.beaconHeight = beaconHeight
        self.waypoint = waypoint
        self.checkpoints = checkpoints
        self.legResults = legResults
    }
}

/// SECTION 13.2 procedure, driven by a fixture: fixed beacons (seed_beacon and
/// I_0 … I_17 with their heights) and a legs_per_stage list. The interrupt
/// schedule and the leg counts are world events the test plane fixes; the
/// computation of every leg, checkpoint and waypoint is the SPEC's.
public struct Hike {
    public let profile: Profile
    public let publicKey: [UInt8]
    public let seedBeacon: [UInt8]
    public let stages: [StageRecord]           // 18, stage 0 first
    public let lastInterrupt: [UInt8]
    public let lastInterruptHeight: UInt64

    public init(publicKey: [UInt8], seedBeacon: [UInt8], seedBeaconHeight: UInt64,
                interrupts: [[UInt8]], interruptHeights: [UInt64],
                legsPerStage: [Int], profile: Profile,
                terrain: Terrain? = nil, opt: Int32 = FoldOpt.auto) {
        precondition(interrupts.count == Spec.stages && interruptHeights.count == Spec.stages)
        precondition(legsPerStage.count == Spec.stages)
        self.profile = profile
        self.publicKey = publicKey
        self.seedBeacon = seedBeacon

        let terr = terrain ?? Terrain(publicKey: publicKey, seedBeacon: seedBeacon, profile: profile)
        var stageRecords: [StageRecord] = []
        var waypoint = terr.seed   // stages[0].waypoint = the seed (Rule A2.2)
        var beacon = seedBeacon
        var beaconHeight = seedBeaconHeight

        for g in 0..<Spec.stages {
            var checkpoints: [[UInt8]] = []
            var legResults: [UInt64] = []
            var previous = waypoint // leg 0 start: seed on stage 0, W_g on g >= 1
            for _ in 0..<legsPerStage[g] {
                // Section 13.2 repeat body.
                let lr = Leg.result(terrain: terr, previousCheckpoint: previous, opt: opt)
                let sample = terr.sample(at: previous)
                let cp = Leg.checkpoint(previous: previous, stageLimb: waypoint,
                                        sample: sample, legResult: lr)
                checkpoints.append(cp)
                legResults.append(lr)
                previous = cp
            }
            stageRecords.append(StageRecord(beacon: beacon, beaconHeight: beaconHeight,
                                            waypoint: waypoint, checkpoints: checkpoints,
                                            legResults: legResults))
            if g < Spec.stages - 1 {
                // Rule A1.3: build W_{g+1} from I_g, its slot sample, the stage
                // waypoint and the fold of this stage's checkpoints.
                // I_g is stored as stages[g+1].beacon.
                waypoint = Waypoint.build(B: interrupts[g], terrain: terr,
                                           stageWaypoint: stageRecords[g].waypoint,
                                           checkpoints: checkpoints)
                beacon = interrupts[g]
                beaconHeight = interruptHeights[g]
            }
        }
        self.stages = stageRecords
        self.lastInterrupt = interrupts[17]
        self.lastInterruptHeight = interruptHeights[17]
    }
}
