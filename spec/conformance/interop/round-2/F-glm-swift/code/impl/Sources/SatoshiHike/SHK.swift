// Bridge to the C kernels (CSatoshiKernels) plus the small-hash conveniences
// the construction needs. Every SHA-256 goes through the C dispatcher, which
// uses SHA-NI on this host (SPEC 2.2 section 11.1 normative requirement).

import CSatoshiKernels

public enum SHK {
    public static func sha256(_ bytes: [UInt8]) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: 32)
        bytes.withUnsafeBufferPointer { bp in
            shk_sha256(bp.baseAddress, bytes.count, &out)
        }
        return out
    }

    /// H(part0 ‖ part1 ‖ ...) — raw concatenation (section 3).
    public static func sha256(parts: [UInt8]...) -> [UInt8] {
        sha256(concat: parts)
    }

    public static func sha256(concat parts: [[UInt8]]) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: 32)
        var storage: [[UInt8]] = []
        var ptrs: [UnsafePointer<UInt8>?] = []
        for p in parts { storage.append(p) }
        for p in storage { p.withUnsafeBufferPointer { ptrs.append($0.baseAddress) } }
        var lens = parts.map { $0.count }
        ptrs.withUnsafeBufferPointer { pb in
            lens.withUnsafeMutableBufferPointer { lb in
                shk_sha256_parts(pb.baseAddress, lb.baseAddress, ptrs.count, &out)
            }
        }
        return out
    }

    public static func sha256(buffer: UnsafeRawPointer, count: Int) -> [UInt8] {
        var out = [UInt8](repeating: 0, count: 32)
        shk_sha256(buffer, count, &out)
        return out
    }

    public static var cpuHasShaNI: Bool { shk_cpu_has_sha_ni() != 0 }
    public static var cpuHasAVX2: Bool { shk_cpu_has_avx2() != 0 }
    public static var cpuHasAVX512: Bool { shk_cpu_has_avx512dq() != 0 }

    public static func cpuBrand() -> String {
        var buf = [CChar](repeating: 0, count: 64)
        shk_cpu_brand(&buf, buf.count)
        return String(cString: buf)
    }
}

/// Fold-option selection shared by the library callers.
/// opt: -1 auto (best SIMD), 0 scalar, 1 AVX-2, 2 AVX-512.
public enum FoldOpt {
    public static let auto = Int32(-1)
    public static let scalar = Int32(0)
    public static let avx2 = Int32(1)
    public static let avx512 = Int32(2)
}
