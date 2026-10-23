// Small file and output helpers.

import Glibc

public enum FS {
    public static func read(_ path: String) -> [UInt8]? {
        guard let f = fopen(path, "rb") else { return nil }
        defer { fclose(f) }
        var out: [UInt8] = []
        var buf = [UInt8](repeating: 0, count: 1 << 20)
        while true {
            let n = fread(&buf, 1, buf.count, f)
            if n <= 0 { break }
            out.append(contentsOf: buf[0..<n])
            if n < buf.count { break }
        }
        return out
    }

    @discardableResult
    public static func write(_ path: String, _ bytes: [UInt8]) -> Bool {
        guard let f = fopen(path, "wb") else { return false }
        defer { fclose(f) }
        var ok = true
        bytes.withUnsafeBufferPointer { bp in
            if !bp.isEmpty {
                ok = fwrite(bp.baseAddress, 1, bp.count, f) == bp.count
            }
        }
        return ok
    }

    public static func mkdirp(_ path: String) {
        _ = mkdir(path, 0o755)
    }
}
