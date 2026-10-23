// SECTION 15.4 admission levers and the policy-file encoding. A policy sets
// the SPEC's levers to values inside their ranges; it never changes a SPEC
// rule. The loader validates a TOML policy file exactly as section 15.4
// "Policy file encoding" states and refuses the whole file on any error.

public struct Levers {
    public var minAnchoredPaths: Int?      // default 6
    public var legRateFloorP: UInt64?       // default p = 1
    public var legRateFloorQ: UInt64?       // default q = 5
    public var submissionDeadlineBlocks: Int? // K, default 3
    public var heldRetrySeconds: Int?       // default 600
    public var reverifyMaxAttempts: Int?   // none
    public var maxHeldBlocks: Int?          // none
    public var thoroughRounds: Int?          // default 3
    public var provisional: String?          // "on"/"off", none
    public var fastAcceptBoundMs: Int?      // seconds lever, none
    public var asyncCatchBudgetMs: Int?      // seconds lever, none
    public var verifyCapPerObject: Int?     // unset
    public var verifyCapPerDoorSession: Int? // unset
    public var fakeLegIgnoreSeconds: Int?   // unset
    public var challengeLifetimeSeconds: Int? // default 120
    public var challengeClockSkewSeconds: Int? // default 5
    public var resultRetentionSeconds: Int?   // default 600
}

public enum PolicyLoad {
        public struct Failure: Error {
            public let reason: String
        }

    /// Parse and validate a TOML v1.0.0 policy file per section 15.4.
    /// Returns the effective levers (SPEC defaults applied where absent) or a
    /// refusal reason. Keys outside [levers], other than spec, belong to the
    /// policy and are not validated here.
    public static func load(text: String) -> Result<Levers, Failure> {
        var inLevers = false
        var sawSpec = false
        var levers = Levers()
        var seenLevers = Set<String>()

        for rawLine in text.split(separator: "\n", omittingEmptySubsequences: true) {
            var line = String(rawLine)
            if let hash = line.firstIndex(of: "#") {
                // strip comments (crude but sufficient for this file shape)
                let before = line[line.startIndex..<hash]
                line = String(before)
            }
            line = line.trimmingWhitespace()
            if line.isEmpty { continue }
            if line.hasPrefix("[") && line.hasSuffix("]") {
                let section = String(line.dropFirst().dropLast()).trimmingWhitespace()
                inLevers = (section == "levers")
                continue
            }
            guard let eq = line.firstIndex(of: "=") else {
                return .failure(Failure(reason: "not a key/value line: \(line)"))
            }
            let key = String(line[line.startIndex..<eq]).trimmingWhitespace()
            let valueText = String(line[line.index(after: eq)...]).trimmingWhitespace()
            if !inLevers {
                if key == "spec" {
                    guard parseString(valueText) == "2.2" else {
                        return .failure(Failure(reason: "spec does not name SPEC 2.2"))
                    }
                    sawSpec = true
                }
                continue // other top-level keys and [policy_rules] belong to the policy
            }
            guard seenLevers.insert(key).inserted else {
                return .failure(Failure(reason: "duplicate lever key \(key)"))
            }
            switch key {
            case "min_anchored_paths":
                guard let v = parseInt(valueText), v >= 2, v <= 16 else {
                    return .failure(Failure(reason: "min_anchored_paths out of range 2…16"))
                }
                levers.minAnchoredPaths = v
            case "leg_rate_floor":
                guard let (p, q) = parsePQ(valueText), p >= 1, q >= 1 else {
                    return .failure(Failure(reason: "leg_rate_floor must be { p ≥ 1, q ≥ 1 }"))
                }
                levers.legRateFloorP = p
                levers.legRateFloorQ = q
            case "submission_deadline_blocks":
                guard let v = parseInt(valueText), v >= 0, v <= 6 else {
                    return .failure(Failure(reason: "submission_deadline_blocks out of range 0…6"))
                }
                levers.submissionDeadlineBlocks = v
            case "held_retry_seconds":
                guard let v = parseInt(valueText), v >= 1 else {
                    return .failure(Failure(reason: "held_retry_seconds must be ≥ 1"))
                }
                levers.heldRetrySeconds = v
            case "reverify_max_attempts":
                guard let v = parseInt(valueText), v >= 1 else {
                    return .failure(Failure(reason: "reverify_max_attempts must be ≥ 1"))
                }
                levers.reverifyMaxAttempts = v
            case "max_held_blocks":
                guard let v = parseInt(valueText), v >= 1 else {
                    return .failure(Failure(reason: "max_held_blocks must be ≥ 1"))
                }
                levers.maxHeldBlocks = v
            case "thorough_rounds":
                guard let v = parseInt(valueText), v >= 1 else {
                    return .failure(Failure(reason: "thorough_rounds must be ≥ 1"))
                }
                levers.thoroughRounds = v
            case "provisional":
                let s = parseString(valueText)
                guard s == "on" || s == "off" else {
                    return .failure(Failure(reason: "provisional must be \"on\" or \"off\""))
                }
                levers.provisional = s
            case "fast_accept_bound", "async_catch_budget":
                guard let ms = parseSecondsMs(valueText), ms > 0 else {
                    return .failure(Failure(reason: "\(key) must be positive seconds (≤ 3 fractional digits)"))
                }
                if key == "fast_accept_bound" { levers.fastAcceptBoundMs = ms }
                else { levers.asyncCatchBudgetMs = ms }
            case "verify_cap_per_object":
                guard let v = parseInt(valueText), v >= 1 else {
                    return .failure(Failure(reason: "verify_cap_per_object must be ≥ 1"))
                }
                levers.verifyCapPerObject = v
            case "verify_cap_per_door_session":
                guard let v = parseInt(valueText), v >= 1 else {
                    return .failure(Failure(reason: "verify_cap_per_door_session must be ≥ 1"))
                }
                levers.verifyCapPerDoorSession = v
            case "fake_leg_ignore_seconds":
                guard let v = parseInt(valueText), v >= 1 else {
                    return .failure(Failure(reason: "fake_leg_ignore_seconds must be ≥ 1"))
                }
                levers.fakeLegIgnoreSeconds = v
            case "challenge_lifetime_seconds":
                guard let v = parseInt(valueText), v >= 1 else {
                    return .failure(Failure(reason: "challenge_lifetime_seconds must be ≥ 1"))
                }
                levers.challengeLifetimeSeconds = v
            case "challenge_clock_skew_seconds":
                guard let v = parseInt(valueText), v >= 0 else {
                    return .failure(Failure(reason: "challenge_clock_skew_seconds must be ≥ 0"))
                }
                levers.challengeClockSkewSeconds = v
            case "result_retention_seconds":
                guard let v = parseInt(valueText), v >= 0 else {
                    return .failure(Failure(reason: "result_retention_seconds must be ≥ 0"))
                }
                levers.resultRetentionSeconds = v
            default:
                return .failure(Failure(reason: "unknown lever key \(key)"))
            }
        }

        guard sawSpec else {
            return .failure(Failure(reason: "no top-level spec string"))
        }

        // Apply labelled defaults where absent.
        if levers.minAnchoredPaths == nil { levers.minAnchoredPaths = 6 }
        if levers.legRateFloorP == nil { levers.legRateFloorP = 1 }
        if levers.legRateFloorQ == nil { levers.legRateFloorQ = 5 }
        if levers.submissionDeadlineBlocks == nil { levers.submissionDeadlineBlocks = 3 }
        if levers.heldRetrySeconds == nil { levers.heldRetrySeconds = 600 }
        if levers.thoroughRounds == nil { levers.thoroughRounds = 3 }
        if levers.challengeLifetimeSeconds == nil { levers.challengeLifetimeSeconds = 120 }
        if levers.challengeClockSkewSeconds == nil { levers.challengeClockSkewSeconds = 5 }
        if levers.resultRetentionSeconds == nil { levers.resultRetentionSeconds = 600 }
        return .success(levers)
    }

    static func parseString(_ s: String) -> String? {
        guard s.hasPrefix("\""), s.hasSuffix("\""), s.count >= 2 else { return nil }
        return String(s.dropFirst().dropLast())
    }

    static func parseInt(_ s: String) -> Int? {
        guard let v = Int(s), v >= 0 else { return nil }
        return v
    }

    /// A positive seconds lever: TOML integer or float with at most 3
    /// fractional digits; read as integer milliseconds, round(x*1000).
    static func parseSecondsMs(_ s: String) -> Int? {
        if let v = Int(s) { return v * 1000 }
        guard let v = Double(s) else { return nil }
        let text = s
        if let dot = text.firstIndex(of: ".") {
            let frac = text[text.index(after: dot)...]
            if frac.count > 3 { return nil }
        }
        if text.lowercased().contains(where: { $0 == "e" }) { return nil }
        return Int((v * 1000).rounded())
    }

    /// leg_rate_floor = { p = 1, q = 5 }
    static func parsePQ(_ s: String) -> (UInt64, UInt64)? {
        guard s.hasPrefix("{"), s.hasSuffix("}") else { return nil }
        let inner = String(s.dropFirst().dropLast())
        var p: UInt64? = nil
        var q: UInt64? = nil
        for part in inner.split(separator: ",") {
            let kv = part.split(separator: "=").map { String($0).trimmingWhitespace() }
            guard kv.count == 2 else { return nil }
            if kv[0] == "p" { p = UInt64(kv[1]) } else if kv[0] == "q" { q = UInt64(kv[1]) }
            else { return nil }
        }
        guard let pv = p, let qv = q else { return nil }
        return (pv, qv)
    }
}

extension String {
    func trimmingWhitespace() -> String {
        var s = self
        while let f = s.first, f == " " || f == "\t" || f == "\r" { s.removeFirst() }
        while let l = s.last, l == " " || l == "\t" || l == "\r" { s.removeLast() }
        return s
    }
}
