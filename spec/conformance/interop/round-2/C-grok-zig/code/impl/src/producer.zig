//! Offline test-plane producer. SPEC §13 with the §16.1 rig tip schedule:
//! exactly `legs_per_stage` legs, then the CASES interrupt. Paths are empty.

const std = @import("std");
const profile = @import("profile.zig");
const terrain = @import("terrain.zig");
const leg = @import("leg.zig");
const object = @import("object.zig");
const fixture = @import("fixture.zig");
const Engine = profile.Engine;

pub fn deinit(a: std.mem.Allocator, obj: *object.Object) void {
    for (&obj.stages) |*st| {
        a.free(st.checkpoints);
        a.free(st.leg_results);
        if (st.path) |p| if (p.len != 0) a.free(p);
    }
}

pub fn build(
    a: std.mem.Allocator,
    pk: [32]u8,
    prof: profile.Profile,
    legs_per_stage: [18]u32,
    engine: Engine,
) !object.Object {
    const seed_beacon = fixture.seedBeacon();
    const seed = terrain.derive(pk, seed_beacon, engine);
    const words_mem = try terrain.fill(a, seed.p, prof, engine);
    defer a.free(words_mem);
    const bytes = std.mem.sliceAsBytes(words_mem);

    var stages: [18]object.Stage = undefined;
    var waypoint = seed.d;
    var beacon = seed_beacon;
    var beacon_height: u64 = fixture.seed_height;

    for (0..18) |g| {
        const nlegs: usize = legs_per_stage[g];
        const cps = try a.alloc([32]u8, nlegs);
        const lrs = try a.alloc([8]u8, nlegs);
        var previous = waypoint;
        for (0..nlegs) |j| {
            const result = leg.legResult(words_mem, prof, &previous, engine);
            const samp = leg.sample(bytes, &previous, prof);
            const cp = leg.checkpoint(&previous, &waypoint, samp, result, engine);
            cps[j] = cp;
            std.mem.writeInt(u64, &lrs[j], result, .little);
            previous = cp;
        }
        const path: ?[]const u8 = if (g == 0) null else "";
        stages[g] = .{
            .beacon = beacon,
            .beacon_height = beacon_height,
            .waypoint = waypoint,
            .path = path,
            .checkpoints = cps,
            .leg_results = lrs,
        };
        const ig = fixture.interrupt(g);
        const ih: u64 = 1001 + @as(u64, g);
        if (g < 17) {
            const fold = leg.checkpointFold(cps, engine);
            const slot = leg.slotSample(words_mem, prof.wordsPerSlot(), &ig, prof.nSlots());
            waypoint = leg.waypoint(&ig, slot, &stages[g].waypoint, &fold, engine);
            beacon = ig;
            beacon_height = ih;
        } else {
            beacon = ig;
            beacon_height = ih;
        }
    }

    return .{
        .slot_bytes = prof.slot_bytes,
        .n_slots = prof.nSlots(),
        .sample_bytes = prof.sample_bytes,
        .steps_per_leg = prof.steps_per_leg,
        .public_key = pk,
        .seed_beacon = seed_beacon,
        .stages = stages,
        .last_interrupt = beacon,
        .last_interrupt_height = beacon_height,
    };
}

pub fn produce(
    a: std.mem.Allocator,
    pk: [32]u8,
    prof: profile.Profile,
    legs_per_stage: [18]u32,
    engine: Engine,
) ![]u8 {
    var obj = try build(a, pk, prof, legs_per_stage, engine);
    defer deinit(a, &obj);
    return object.encode(a, &obj);
}

pub fn writeFixtures(a: std.mem.Allocator, dir: std.fs.Dir, engine: Engine, stderr: std.fs.File) !void {
    const specs = [_]struct { name: []const u8, prof: profile.Profile, legs: u32 }{
        .{ .name = "O1-S1.cbor", .prof = profile.s1, .legs = 1 },
        .{ .name = "O2-S2.cbor", .prof = profile.s2, .legs = 3 },
        .{ .name = "O3-PROD.cbor", .prof = profile.production, .legs = 1 },
    };
    for (specs) |sp| {
        var legs: [18]u32 = undefined;
        @memset(&legs, sp.legs);
        const bytes = try produce(a, fixture.pkA(), sp.prof, legs, engine);
        defer a.free(bytes);
        var f = try dir.createFile(sp.name, .{ .truncate = true });
        defer f.close();
        try f.writeAll(bytes);
        try object.writePathSkip(stderr);
    }
}
