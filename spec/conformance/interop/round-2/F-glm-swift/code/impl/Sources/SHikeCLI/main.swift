// shike — the Satoshi Hike CLI.
//
//   shike verify <object> [--profile production|conformance-small]
//        [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N]
//        [--draw-source csp|chain:<label>|file:<path>]
//        [--out <path>]
//   Section 18: stdout is the one section 14 record (keys in section 14
//   order, no insignificant whitespace, one trailing newline), or the Rule A5
//   record. Exit 0 = a record was produced; 1 = unreadable file or a
//   section 7.3 decode failure; 2 = usage error. Stdout and stderr carry no
//   pass/fail/admission wording.
//
//   shike produce [--out objects]   — writes O1-S1.cbor, O2-S2.cbor and
//        O3-PROD.cbor (production profile, one leg per stage) and the Rule
//        A3.4 stderr lines at the close.
//
//   shike bench                     — timings for RESULTS.md.
//   shike policy-check <file>       — section 15.4 policy file validation.

import Foundation
import SatoshiHike

func usage() -> Never {
    FileHandle.standardError.write(Data("""
    usage: shike verify <object> [options]
           shike produce [--out <dir>]
           shike bench
           shike policy-check <file>
    options:
      --profile production|conformance-small   (conformance-small requires all four integers)
      --sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N
      --draw-source csp|chain:<label>|file:<path>
      --out <path>     write the record bytes to this path too
    """.utf8))
    exit(2)
}

func failUsage(_ msg: String) -> Never {
    FileHandle.standardError.write(Data("shike: \(msg)\n".utf8))
    exit(2)
}

var args = Array(CommandLine.arguments.dropFirst())
guard !args.isEmpty else { usage() }
let cmd = args.removeFirst()

func writeOut(_ bytes: [UInt8]) {
    FileHandle.standardOutput.write(Data(bytes))
}

switch cmd {
case "verify":
    guard !args.isEmpty else { usage() }
    let path = args.removeFirst()
    guard let bytes = FS.read(path) else {
        FileHandle.standardError.write(Data("shike: cannot read \(path)\n".utf8))
        exit(1)
    }
    var profileName = "production"
    var csSample: Int? = nil
    var csSlot: Int? = nil
    var csTerrain: Int? = nil
    var csSteps: Int? = nil
    var drawSpec = "csp"
    var outPath: String? = nil

    var i = 0
    while i < args.count {
        let a = args[i]
        switch a {
        case "--profile":
            guard i + 1 < args.count else { failUsage("--profile needs a value") }
            profileName = args[i + 1]
            i += 2
        case "--sample-bytes":
            guard i + 1 < args.count, let v = Int(args[i + 1]) else { failUsage("bad --sample-bytes") }
            csSample = v; i += 2
        case "--slot-bytes":
            guard i + 1 < args.count, let v = Int(args[i + 1]) else { failUsage("bad --slot-bytes") }
            csSlot = v; i += 2
        case "--terrain-bytes":
            guard i + 1 < args.count, let v = Int(args[i + 1]) else { failUsage("bad --terrain-bytes") }
            csTerrain = v; i += 2
        case "--steps-per-leg":
            guard i + 1 < args.count, let v = Int(args[i + 1]) else { failUsage("bad --steps-per-leg") }
            csSteps = v; i += 2
        case "--draw-source":
            guard i + 1 < args.count else { failUsage("--draw-source needs a value") }
            drawSpec = args[i + 1]; i += 2
        case "--out":
            guard i + 1 < args.count else { failUsage("--out needs a value") }
            outPath = args[i + 1]; i += 2
        default:
            failUsage("unknown option \(a)")
        }
    }

    guard profileName == "production" || profileName == "conformance-small" else {
        failUsage("--profile must be production or conformance-small")
    }
    let profile: Profile
    if profileName == "conformance-small" {
        guard let sb = csSample, let sl = csSlot, let tb = csTerrain, let sp = csSteps else {
            failUsage("conformance-small requires --sample-bytes, --slot-bytes, --terrain-bytes and --steps-per-leg (section 18)")
        }
        profile = Profile(sampleBytes: sb, slotBytes: sl, terrainBytes: tb, stepsPerLeg: sp)
        guard profile.isValidConformanceSmall else {
            failUsage("profile integers violate the section 16.1 constraints")
        }
    } else {
        profile = .production
    }

    let makeSource: () -> any ByteSource
    if drawSpec == "csp" {
        makeSource = { CSPRNGSource() }
    } else if drawSpec.hasPrefix("chain:") {
        let label = String(drawSpec.dropFirst("chain:".count))
        makeSource = { ChainSource(label: label) }
    } else if drawSpec.hasPrefix("file:") {
        let p = String(drawSpec.dropFirst("file:".count))
        guard let b = FS.read(p) else { failUsage("cannot read draw bytes file") }
        makeSource = { FixedSource(b) }
    } else {
        failUsage("bad --draw-source")
    }

    let options = VerifyOptions(profile: profile, index: FixtureIndex(),
                                makeDrawSource: makeSource)
    switch Verify.run(objectBytes: bytes, options: options) {
    case .record(let record):
        var out = JSONWriter.encodeBytes(record)
        out.append(0x0A) // the CLI's one trailing newline
        writeOut(out)
        if let op = outPath { FS.write(op, out) }
        exit(0)
    case .unknownKind(let kindText):
        var out = ObjectDecode.ruleA5Record(kindText: kindText)
        out.append(0x0A)
        writeOut(out)
        if let op = outPath { FS.write(op, out) }
        exit(0)
    case .decodeFailure:
        FileHandle.standardError.write(Data("shike: section 7.3 decode failure\n".utf8))
        exit(1)
    }

case "produce":
    var outDir = "objects"
    var i = 0
    while i < args.count {
        if args[i] == "--out" && i + 1 < args.count { outDir = args[i + 1]; i += 2 } else { i += 1 }
    }
    FS.mkdirp(outDir)
    do {
        let t0 = Date()
        let o1 = try Producer.buildObject(profile: Fixture.S1, legsPerStage: Fixture.legsPerStageS1)
        FS.write("\(outDir)/\(Producer.o1Name)", o1)
        print("O1 (S1, 18x1 legs)  \(o1.count) bytes  \(Date().timeIntervalSince(t0).formatted(.number.precision(.fractionLength(1))))s")
        let t1 = Date()
        let o2 = try Producer.buildObject(profile: Fixture.S2, legsPerStage: Fixture.legsPerStageS2)
        FS.write("\(outDir)/\(Producer.o2Name)", o2)
        print("O2 (S2, 18x3 legs)  \(o2.count) bytes  \(Date().timeIntervalSince(t1).formatted(.number.precision(.fractionLength(1))))s")
        let t2 = Date()
        let o3 = try Producer.buildObject(profile: .production, legsPerStage: Fixture.legsPerStageS1)
        FS.write("\(outDir)/\(Producer.o3Name)", o3)
        print("O3 (PROD, 18x1 legs)  \(o3.count) bytes  \(Date().timeIntervalSince(t2).formatted(.number.precision(.fractionLength(1))))s")
        // Rule A3.4: the close writes the stderr step at once.
        for line in Producer.closeStderrLines() {
            FileHandle.standardError.write(Data((line + "\n").utf8))
        }
    } catch {
        FileHandle.standardError.write(Data("shike produce: error: \(error)\n".utf8))
        exit(1)
    }

case "bench":
    Bench.run()

case "policy-check":
    guard args.count == 1 else { usage() }
    guard let text = String(bytes: FS.read(args[0]) ?? [], encoding: .utf8) else {
        FileHandle.standardError.write(Data("shike: cannot read \(args[0])\n".utf8))
        exit(1)
    }
    switch PolicyLoad.load(text: text) {
    case .success(let levers):
        print("policy file accepted; effective levers:")
        print("  min_anchored_paths = \(levers.minAnchoredPaths!)")
        print("  leg_rate_floor = \(levers.legRateFloorP!)/\(levers.legRateFloorQ!)")
        print("  submission_deadline_blocks = \(levers.submissionDeadlineBlocks!)")
        print("  held_retry_seconds = \(levers.heldRetrySeconds!)")
        print("  reverify_max_attempts = \(levers.reverifyMaxAttempts.map(String.init) ?? "unset")")
        print("  max_held_blocks = \(levers.maxHeldBlocks.map(String.init) ?? "unset")")
        print("  thorough_rounds = \(levers.thoroughRounds!)")
        print("  provisional = \(levers.provisional ?? "unset")")
        print("  fast_accept_bound_ms = \(levers.fastAcceptBoundMs.map(String.init) ?? "unset")")
        print("  async_catch_budget_ms = \(levers.asyncCatchBudgetMs.map(String.init) ?? "unset")")
        print("  verify_cap_per_object = \(levers.verifyCapPerObject.map(String.init) ?? "unset")")
        print("  verify_cap_per_door_session = \(levers.verifyCapPerDoorSession.map(String.init) ?? "unset")")
        print("  fake_leg_ignore_seconds = \(levers.fakeLegIgnoreSeconds.map(String.init) ?? "unset")")
        print("  challenge_lifetime_seconds = \(levers.challengeLifetimeSeconds!)")
        print("  challenge_clock_skew_seconds = \(levers.challengeClockSkewSeconds!)")
        print("  result_retention_seconds = \(levers.resultRetentionSeconds!)")
    case .failure(let f):
        print("policy file REFUSED: \(f.reason)")
        exit(1)
    }

default:
    usage()
}
