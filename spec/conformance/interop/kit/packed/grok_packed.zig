//! Thin packed-run driver over Grok's UNCHANGED `hike` library (src/root.zig).
//! run:   one independent hike: own pk (sha256 of a label), own 256 MiB terrain,
//!        chained 2M-step legs (SPEC 11.2-11.4, stage 0), until <stopfile> exists.
//! check: recompute one leg (result + checkpoint) for <pk_hex> from <prev_hex>.
const std = @import("std");
const hike = @import("hike");
const terrain = hike.terrain;
const leg = hike.leg;
const prof = hike.profile.production;

fn eng(s: []const u8) hike.profile.Engine {
    return if (std.mem.eql(u8, s, "scalar")) .scalar else .fast;
}
fn out(comptime fmt: []const u8, args: anytype) void {
    var buf: [512]u8 = undefined;
    const s = std.fmt.bufPrint(&buf, fmt, args) catch unreachable;
    std.fs.File.stdout().writeAll(s) catch {};
}
pub fn main() !void {
    const a = std.heap.page_allocator;
    const args = try std.process.argsAlloc(a);
    const mode = args[1];
    if (std.mem.eql(u8, mode, "run")) {
        // run <label> <engine> <stopfile>
        var pk: [32]u8 = undefined;
        std.crypto.hash.sha2.Sha256.hash(args[2], &pk, .{});
        const e = eng(args[3]);
        out("pk,{s}\n", .{std.fmt.bytesToHex(pk, .lower)});
        const t0 = std.time.nanoTimestamp();
        const seed = terrain.derive(pk, hike.fixture.seedBeacon(), e);
        const words = try terrain.fill(a, seed.p, prof, e);
        const bytes = std.mem.sliceAsBytes(words);
        out("fill,{d},{d}\n", .{ t0, std.time.nanoTimestamp() });
        const waypoint = seed.d;
        var previous = waypoint;
        var j: usize = 0;
        while (j < 100000) : (j += 1) {
            const ts = std.time.nanoTimestamp();
            const r = leg.legResult(words, prof, &previous, e);
            const samp = leg.sample(bytes, &previous, prof);
            const cp = leg.checkpoint(&previous, &waypoint, samp, r, e);
            const te = std.time.nanoTimestamp();
            var le: [8]u8 = undefined;
            std.mem.writeInt(u64, &le, r, .little);
            out("leg,{d},{d},{d},{s},{s},{s}\n", .{ j, ts, te, std.fmt.bytesToHex(le, .lower), std.fmt.bytesToHex(previous, .lower), std.fmt.bytesToHex(cp, .lower) });
            previous = cp;
            std.fs.cwd().access(args[4], .{}) catch continue;
            break;
        }
    } else {
        // check <pk_hex> <prev_hex> <engine>
        var pk: [32]u8 = undefined;
        var prev: [32]u8 = undefined;
        _ = try std.fmt.hexToBytes(&pk, args[2]);
        _ = try std.fmt.hexToBytes(&prev, args[3]);
        const e = eng(args[4]);
        const seed = terrain.derive(pk, hike.fixture.seedBeacon(), e);
        const words = try terrain.fill(a, seed.p, prof, e);
        const r = leg.legResult(words, prof, &prev, e);
        const cp = leg.checkpoint(&prev, &seed.d, leg.sample(std.mem.sliceAsBytes(words), &prev, prof), r, e);
        var le: [8]u8 = undefined;
        std.mem.writeInt(u64, &le, r, .little);
        out("{s},{s}\n", .{ std.fmt.bytesToHex(le, .lower), std.fmt.bytesToHex(cp, .lower) });
    }
}
