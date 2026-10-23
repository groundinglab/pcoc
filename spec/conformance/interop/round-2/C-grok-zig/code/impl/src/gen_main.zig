const std = @import("std");
const candidates = @import("candidates.zig");

pub fn main() !void {
    var gpa_state = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa_state.deinit();
    const gpa = gpa_state.allocator();
    const args = try std.process.argsAlloc(gpa);
    defer std.process.argsFree(gpa, args);
    var out_path: []const u8 = "candidates";
    var i: usize = 1;
    while (i < args.len) : (i += 1) {
        if (std.mem.eql(u8, args[i], "--out")) {
            i += 1;
            if (i >= args.len) std.process.exit(2);
            out_path = args[i];
        } else std.process.exit(2);
    }
    var dir = try std.fs.cwd().makeOpenPath(out_path, .{});
    defer dir.close();
    try candidates.writeAll(gpa, dir, .fast);
}

fn openCommitted() !std.fs.Dir {
    const paths = [_][]const u8{ "../candidates", "candidates" };
    for (paths) |p| {
        return std.fs.cwd().openDir(p, .{}) catch continue;
    }
    return error.CandidatesMissing;
}

test "committed candidates are unchanged" {
    const a = std.testing.allocator;
    var tmp = std.testing.tmpDir(.{});
    defer tmp.cleanup();
    try candidates.writeAll(a, tmp.dir, .fast);
    var committed = try openCommitted();
    defer committed.close();
    const names = [_][]const u8{
        "seed.txt",     "nl4.txt",      "terrain.txt", "fold.txt",
        "kat-small.txt", "test-3leg.txt", "prod.txt",   "wes.txt",
        "rate.txt",     "anchor.txt",   "draw.txt",    "unknown-kind.txt",
        "hashcash.txt", "negatives.txt",
    };
    for (names) |name| {
        const got = try tmp.dir.readFileAlloc(a, name, 8 << 20);
        defer a.free(got);
        const exp = try committed.readFileAlloc(a, name, 8 << 20);
        defer a.free(exp);
        if (!std.mem.eql(u8, got, exp)) {
            std.debug.print("candidate changed: {s}\n", .{name});
            return error.CandidatesChanged;
        }
    }
}
