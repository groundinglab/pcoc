// gen-candidates: the deterministic candidate generator (CASES.md).
// Writes the 13 candidate files into impl/candidates/ (or --out <dir>).
// Negatives live only in this command's dependency tree.

import Foundation
import GenCandidatesCore
import SatoshiHike

let args = CommandLine.arguments
var outDir = "candidates"
var i = 1
while i < args.count {
    if args[i] == "--out" && i + 1 < args.count {
        outDir = args[i + 1]
        i += 2
    } else {
        i += 1
    }
}

do {
    let started = Date()
    try Generator.run(outDir: outDir, repoRoot: nil)
    let dt = Date().timeIntervalSince(started)
    FileHandle.standardError.write(Data("gen-candidates: wrote \(Generator.allFileNames().count) files to \(outDir) in \(String(format: "%.1f", dt))s\n".utf8))
} catch {
    FileHandle.standardError.write(Data("gen-candidates: error: \(error)\n".utf8))
    exit(1)
}
