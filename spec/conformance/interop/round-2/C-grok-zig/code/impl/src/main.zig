const std = @import("std");
const hike = @import("hike");

const profile = hike.profile;
const producer = hike.producer;
const verify = hike.verify;
const terrain = hike.terrain;
const leg = hike.leg;
const hex = hike.hex;
const fixture = hike.fixture;
const sha = hike.sha;

fn usage() noreturn {
    const err = std.fs.File.stderr();
    err.writeAll(
        \\usage: hike verify <object> [--profile production|conformance-small|test|live]
        \\           [--sample-bytes N --slot-bytes N --terrain-bytes N --steps-per-leg N]
        \\           [--index-url URL | --no-index] [--draw-label LABEL]
        \\       hike produce-fixtures [--out dir]
        \\       hike bench
        \\
    ) catch {};
    std.process.exit(2);
}

fn need(args: [][:0]u8, i: *usize) [:0]u8 {
    i.* += 1;
    if (i.* >= args.len) usage();
    return args[i.*];
}

fn parseU(s: []const u8) u64 {
    return std.fmt.parseInt(u64, s, 10) catch usage();
}

pub fn main() !void {
    var gpa_state = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa_state.deinit();
    const gpa = gpa_state.allocator();
    const args = try std.process.argsAlloc(gpa);
    defer std.process.argsFree(gpa, args);
    if (args.len < 2) usage();
    if (std.mem.eql(u8, args[1], "bench")) {
        try bench();
        return;
    }
    if (std.mem.eql(u8, args[1], "produce-fixtures")) {
        var out_path: []const u8 = "objects";
        var i: usize = 2;
        while (i < args.len) : (i += 1) {
            if (std.mem.eql(u8, args[i], "--out")) {
                out_path = need(args, &i);
            } else usage();
        }
        var dir = try std.fs.cwd().makeOpenPath(out_path, .{});
        defer dir.close();
        try producer.writeFixtures(gpa, dir, .fast, std.fs.File.stderr());
        const names = [_][]const u8{ "O1-S1.cbor", "O2-S2.cbor", "O3-PROD.cbor" };
        var stdout = std.fs.File.stdout();
        for (names) |name| {
            const bytes = try dir.readFileAlloc(gpa, name, 64 << 20);
            defer gpa.free(bytes);
            var dig: [32]u8 = undefined;
            sha.hashFast(bytes, &dig);
            const h = hex.of(32, &dig);
            var line: [128]u8 = undefined;
            const s = std.fmt.bufPrint(&line, "{s} {s}\n", .{ name, h }) catch unreachable;
            try stdout.writeAll(s);
        }
        return;
    }
    if (!std.mem.eql(u8, args[1], "verify")) usage();

    var file: ?[]const u8 = null;
    var profile_tok: []const u8 = "production";
    var sample: ?u64 = null;
    var slot: ?u64 = null;
    var terr: ?u64 = null;
    var steps: ?u64 = null;
    var index_url: ?[]const u8 = null;
    var no_index = false;
    var draw_label: ?[]const u8 = null;
    var i: usize = 2;
    while (i < args.len) : (i += 1) {
        const ar = args[i];
        if (std.mem.eql(u8, ar, "--profile")) {
            profile_tok = need(args, &i);
        } else if (std.mem.eql(u8, ar, "--sample-bytes")) {
            sample = parseU(need(args, &i));
        } else if (std.mem.eql(u8, ar, "--slot-bytes")) {
            slot = parseU(need(args, &i));
        } else if (std.mem.eql(u8, ar, "--terrain-bytes")) {
            terr = parseU(need(args, &i));
        } else if (std.mem.eql(u8, ar, "--steps-per-leg")) {
            steps = parseU(need(args, &i));
        } else if (std.mem.eql(u8, ar, "--index-url")) {
            index_url = need(args, &i);
        } else if (std.mem.eql(u8, ar, "--no-index")) {
            no_index = true;
        } else if (std.mem.eql(u8, ar, "--draw-label")) {
            draw_label = need(args, &i);
        } else if (ar.len > 0 and ar[0] != '-' and file == null) {
            file = ar;
        } else usage();
    }
    const path = file orelse usage();
    const small = std.mem.eql(u8, profile_tok, "conformance-small");
    const known = small or std.mem.eql(u8, profile_tok, "production") or
        std.mem.eql(u8, profile_tok, "test") or std.mem.eql(u8, profile_tok, "live");
    if (!known) usage();
    if (draw_label != null and !small) usage();
    var prof = profile.production;
    if (small) {
        if (sample == null or slot == null or terr == null or steps == null) usage();
        prof = .{
            .sample_bytes = sample.?,
            .slot_bytes = slot.?,
            .terrain_bytes = terr.?,
            .steps_per_leg = steps.?,
        };
        if (!profile.profileOk(prof)) usage();
    }
    const mode: hike.index.Mode = if (no_index)
        .none
    else if (index_url) |u|
        .{ .url = u }
    else if (std.mem.eql(u8, profile_tok, "live"))
        .none
    else
        .fixture;
    const bytes = std.fs.cwd().readFileAlloc(gpa, path, 64 << 20) catch std.process.exit(1);
    defer gpa.free(bytes);
    const rec = verify.record(gpa, bytes, .{
        .profile_name = if (small) .conformance_small else .production,
        .prof = prof,
        .index_mode = mode,
        .draw_label = draw_label,
        .engine = .fast,
    }) catch |err| switch (err) {
        error.DecodeFailure => std.process.exit(1),
        error.InvalidProfile => usage(),
        else => |e| return e,
    };
    defer gpa.free(rec);
    var stdout = std.fs.File.stdout();
    try stdout.writeAll(rec);
    try stdout.writeAll("\n");
}

fn secs(t0: i128, t1: i128) f64 {
    return @as(f64, @floatFromInt(t1 - t0)) / 1_000_000_000.0;
}

fn bench() !void {
    const a = std.heap.smp_allocator;
    const seed = terrain.derive(fixture.pkA(), fixture.seedBeacon(), .fast);
    const prof = profile.production;
    var stdout = std.fs.File.stdout();
    var line: [128]u8 = undefined;

    var t0 = std.time.nanoTimestamp();
    const slow = try terrain.fill(a, seed.p, prof, .scalar);
    var t1 = std.time.nanoTimestamp();
    const fast = try terrain.fill(a, seed.p, prof, .fast);
    const t2 = std.time.nanoTimestamp();
    const same_t = std.mem.eql(u8, std.mem.sliceAsBytes(slow), std.mem.sliceAsBytes(fast));
    a.free(slow);
    var s = std.fmt.bufPrint(&line, "terrain_scalar_s={d:.3}\n", .{secs(t0, t1)}) catch unreachable;
    try stdout.writeAll(s);
    s = std.fmt.bufPrint(&line, "terrain_fast_s={d:.3}\n", .{secs(t1, t2)}) catch unreachable;
    try stdout.writeAll(s);
    s = std.fmt.bufPrint(&line, "terrain_equal={s}\n", .{if (same_t) "true" else "false"}) catch unreachable;
    try stdout.writeAll(s);

    t0 = std.time.nanoTimestamp();
    const leg_s = leg.legResult(fast, prof, &seed.d, .scalar);
    t1 = std.time.nanoTimestamp();
    const leg_f = leg.legResult(fast, prof, &seed.d, .fast);
    const t3 = std.time.nanoTimestamp();
    a.free(fast);
    s = std.fmt.bufPrint(&line, "leg_scalar_s={d:.3}\n", .{secs(t0, t1)}) catch unreachable;
    try stdout.writeAll(s);
    s = std.fmt.bufPrint(&line, "leg_fast_s={d:.3}\n", .{secs(t1, t3)}) catch unreachable;
    try stdout.writeAll(s);
    s = std.fmt.bufPrint(&line, "leg_equal={s}\n", .{if (leg_s == leg_f) "true" else "false"}) catch unreachable;
    try stdout.writeAll(s);
    var le: [8]u8 = undefined;
    std.mem.writeInt(u64, &le, leg_f, .little);
    const hx = hex.of(8, &le);
    s = std.fmt.bufPrint(&line, "leg_fast_le_hex={s}\n", .{hx}) catch unreachable;
    try stdout.writeAll(s);
}
