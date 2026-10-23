// swift-tools-version:6.0
// Satoshi Hike SPEC 2.2 (with erratum 1) — independent Swift implementation.
// Targets:
//   CSatoshiKernels  — C kernels: SHA-256 (SHA-NI + scalar), slot fold (AVX-512/AVX-2/scalar),
//                      terrain fill (AVX-512/scalar), the §11.2 leg driver.
//   SatoshiHike      — the SPEC construction (sections 6–12), CBOR (7.3), object (8),
//                      verify record (14), draw (14.6/15.5), comparisons, hashcash (13.1),
//                      fixture index, policy (15.4 encoding).
//   Negatives        — the §16 permanent negatives. Generator-only module; never linked into
//                      the SDK code paths (verify/produce do not depend on it).
//   GenCandidatesCore — the deterministic candidate generator (writes candidates/).
//   SHikeCLI         — `shike verify|produce|bench|policy-check`.
//   GenCandidatesCLI — `gen-candidates` (the only executable that links Negatives).
import PackageDescription

let package = Package(
    name: "SatoshiHike",
    targets: [
        .target(name: "CSatoshiKernels"),
        .target(name: "SatoshiHike", dependencies: ["CSatoshiKernels"]),
        .target(name: "Negatives", dependencies: ["SatoshiHike"]),
        .target(name: "GenCandidatesCore", dependencies: ["SatoshiHike", "Negatives"]),
        .executableTarget(name: "SHikeCLI", dependencies: ["SatoshiHike"]),
        .executableTarget(name: "GenCandidatesCLI", dependencies: ["GenCandidatesCore"]),
        .testTarget(name: "SatoshiHikeTests", dependencies: ["SatoshiHike", "GenCandidatesCore", "Negatives"]),
    ],
    swiftLanguageModes: [.v5]
)
