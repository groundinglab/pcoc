//! SPEC §16 permanent negatives. This module is the only place they exist.
//! Nothing on the hike, verify, or produce path imports it.

const std = @import("std");
const hike = @import("hike");
const profile = hike.profile;
const terrain = hike.terrain;
const mix = hike.mix;
const leg = hike.leg;
const hex = hike.hex;
const fixture = hike.fixture;

pub const Error = error{ NegativeEqualsHonest, HonestLegDoesNotRevisit };

const Engine = profile.Engine;

fn mixBeforeFinal(x: u64) u64 {
    var y = x +% profile.MIX_ADD;
    y = (y ^ (y >> 30)) *% profile.MIX_MUL1;
    y = (y ^ (y >> 27)) *% profile.MIX_MUL2;
    return y;
}

fn leHex(v: u64) [16]u8 {
    var b: [8]u8 = undefined;
    std.mem.writeInt(u64, &b, v, .little);
    return hex.of(8, &b);
}

fn slotOf(words: []const u64, words_n: u64, n_slots: u64, S: u64) []const u64 {
    const i = S % n_slots;
    const start: usize = @intCast(i * words_n);
    const n: usize = @intCast(words_n);
    return words[start .. start + n];
}

fn run(words: []const u64, prof: profile.Profile, start: u64, comptime fold: fn ([]const u64, u64) u64, engine: Engine) u64 {
    var S = start;
    const wn = prof.wordsPerSlot();
    const ns = prof.nSlots();
    const steps: usize = @intCast(prof.steps_per_leg);
    for (0..steps) |_| {
        const acc = fold(slotOf(words, wn, ns, S), S);
        S = leg.seal(acc, engine);
    }
    return S;
}

fn honestFold(slot: []const u64, S: u64) u64 {
    return mix.foldSlot(slot, S, .fast);
}

fn slotSummary(slot: []const u64, S: u64) u64 {
    var x: u64 = 0;
    for (slot, 0..) |word, w| x ^= word ^ @as(u64, w);
    return S ^ x;
}

fn xorAccumulator(slot: []const u64, S: u64) u64 {
    var acc: u64 = 0;
    for (slot, 0..) |word, w| acc ^= mix.mix64(word ^ @as(u64, w) ^ S);
    return acc;
}

fn xorW(slot: []const u64, S: u64) u64 {
    var acc: u64 = 0;
    for (slot, 0..) |word, w| acc +%= mix.mix64(word ^ @as(u64, w) ^ S);
    return acc;
}

fn sPlusW(slot: []const u64, S: u64) u64 {
    var acc: u64 = 0;
    for (slot, 0..) |word, w| acc +%= mix.mix64(word ^ (S +% @as(u64, w)));
    return acc;
}

fn partial0(slot: []const u64, S: u64) u64 {
    return mix.mix64(slot[0] ^ S);
}

fn partialButLast(slot: []const u64, S: u64) u64 {
    var acc: u64 = 0;
    if (slot.len == 0) return 0;
    for (slot[0 .. slot.len - 1]) |word| acc +%= mix.mix64(word ^ S);
    return acc;
}

fn finalStage(slot: []const u64, S: u64) u64 {
    var x: u64 = 0;
    for (slot) |word| x ^= mixBeforeFinal(word ^ S);
    return x ^ (x >> 31);
}

fn prefoldLeg(words: []const u64, pre: []const u64, prof: profile.Profile, start: u64, engine: Engine) u64 {
    // Prefold never reads slot words. `words.len` is observed only so the
    // parameter stays in the signature that mirrors an honest leg.
    std.debug.assert(words.len == words.len);
    var S = start;
    const ns = prof.nSlots();
    const steps: usize = @intCast(prof.steps_per_leg);
    for (0..steps) |_| {
        const i = S % ns;
        const acc = pre[@intCast(i)] +% S;
        S = leg.seal(acc, engine);
    }
    return S;
}

fn cachedLeg(a: std.mem.Allocator, words: []const u64, prof: profile.Profile, start: u64, engine: Engine) !u64 {
    const ns: usize = @intCast(prof.nSlots());
    const cache = try a.alloc(?u64, ns);
    defer a.free(cache);
    @memset(cache, null);
    var S = start;
    const wn = prof.wordsPerSlot();
    const steps: usize = @intCast(prof.steps_per_leg);
    for (0..steps) |_| {
        const i = S % prof.nSlots();
        const iu: usize = @intCast(i);
        const acc = if (cache[iu]) |c| c else blk: {
            const folded = mix.foldSlot(slotOf(words, wn, prof.nSlots(), S), S, engine);
            cache[iu] = folded;
            break :blk folded;
        };
        S = leg.seal(acc, engine);
    }
    return S;
}

fn honestRevisits(words: []const u64, prof: profile.Profile, start: u64, engine: Engine) bool {
    const ns: usize = @intCast(prof.nSlots());
    var seen = std.heap.page_allocator.alloc(bool, ns) catch return false;
    defer std.heap.page_allocator.free(seen);
    @memset(seen, false);
    var S = start;
    var hit = false;
    const steps: usize = @intCast(prof.steps_per_leg);
    const wn = prof.wordsPerSlot();
    for (0..steps) |_| {
        const i: usize = @intCast(S % prof.nSlots());
        if (seen[i]) {
            hit = true;
        } else seen[i] = true;
        S = leg.step(words, wn, prof.nSlots(), S, engine);
    }
    return hit;
}

fn oneLane(i: u64, j: u64, lanes: [4]u64) u64 {
    const c = (i << 32) ^ j ^ profile.WORD_XOR;
    return mix.mix64(lanes[j % 4] ^ c);
}

fn all4(i: u64, j: u64, lanes: [4]u64) u64 {
    const c = (i << 32) ^ j ^ profile.WORD_XOR;
    const x = lanes[j % 4] ^
        std.math.rotl(u64, lanes[(j + 1) % 4], 17) ^
        std.math.rotl(u64, lanes[(j + 2) % 4], 31) ^
        std.math.rotl(u64, lanes[(j + 3) % 4], 47) ^
        c;
    return mix.mix64(x);
}

fn fillAlt(
    a: std.mem.Allocator,
    lanes: [4]u64,
    prof: profile.Profile,
    comptime base: fn (u64, u64, [4]u64) u64,
    engine: Engine,
) ![]align(64) u64 {
    const words: usize = @intCast(prof.wordsPerSlot());
    const slots: usize = @intCast(prof.nSlots());
    const terr = try a.alignedAlloc(u64, .@"64", slots * words);
    errdefer a.free(terr);
    var i: usize = 0;
    while (i < slots) : (i += 1) {
        const slot = terr[i * words ..][0..words];
        for (slot, 0..) |*w, j| w.* = base(@intCast(i), @intCast(j), lanes);
        if (i > 0) {
            switch (engine) {
                .scalar => terrain.applyParents(slot, @intCast(i), terr, @intCast(words)),
                .fast => terrain.applyParentsFast(slot, @intCast(i), terr, @intCast(words)),
            }
        }
    }
    return terr;
}

pub const Row = struct { key: []const u8, val: []const u8 };

fn put(a: std.mem.Allocator, rows: *std.ArrayList(Row), key: []const u8, val: []const u8) !void {
    try rows.append(a, .{ .key = try a.dupe(u8, key), .val = try a.dupe(u8, val) });
}

fn putLeg(a: std.mem.Allocator, rows: *std.ArrayList(Row), name: []const u8, v: u64, honest: u64) !void {
    const same = v == honest;
    const tok: []const u8 = if (same) "true" else "false";
    if (same) return error.NegativeEqualsHonest;
    var key: [96]u8 = undefined;
    const k1 = std.fmt.bufPrint(&key, "negatives.{s}.legs_le_hex", .{name}) catch unreachable;
    const hx = leHex(v);
    try put(a, rows, k1, &hx);
    const k2 = std.fmt.bufPrint(&key, "negatives.{s}.matches_honest", .{name}) catch unreachable;
    try put(a, rows, k2, tok);
}

fn putTerr(a: std.mem.Allocator, rows: *std.ArrayList(Row), name: []const u8, dig: [32]u8, honest: [32]u8) !void {
    const same = std.mem.eql(u8, &dig, &honest);
    const tok: []const u8 = if (same) "true" else "false";
    if (same) return error.NegativeEqualsHonest;
    var key: [96]u8 = undefined;
    const k1 = std.fmt.bufPrint(&key, "negatives.{s}.terrain_sha256", .{name}) catch unreachable;
    const hx = hex.of(32, &dig);
    try put(a, rows, k1, &hx);
    const k2 = std.fmt.bufPrint(&key, "negatives.{s}.matches_honest", .{name}) catch unreachable;
    try put(a, rows, k2, tok);
}

/// S1, pk_A, first stage-0 leg, plus the three terrain negatives. `f3` uses pk_B.
pub fn append(a: std.mem.Allocator, rows: *std.ArrayList(Row), engine: Engine) !void {
    const prof = profile.s1;
    const seed_a = terrain.derive(fixture.pkA(), fixture.seedBeacon(), engine);
    const words = try terrain.fill(a, seed_a.p, prof, engine);
    defer a.free(words);
    const start = std.mem.readInt(u64, seed_a.d[0..8], .little);
    if (!honestRevisits(words, prof, start, engine)) return error.HonestLegDoesNotRevisit;

    const honest = leg.legResult(words, prof, &seed_a.d, engine);
    const honest_dig = terrain.digest(words, engine);
    const hh = leHex(honest);
    try put(a, rows, "negatives.honest.legs_le_hex", &hh);
    const hd = hex.of(32, &honest_dig);
    try put(a, rows, "negatives.honest.terrain_sha256", &hd);

    try putLeg(a, rows, "slot_summary_fold", run(words, prof, start, slotSummary, engine), honest);
    try putLeg(a, rows, "xor_accumulator_fold", run(words, prof, start, xorAccumulator, engine), honest);
    try putLeg(a, rows, "xor_w_fold", run(words, prof, start, xorW, engine), honest);
    try putLeg(a, rows, "s_plus_w_fold", run(words, prof, start, sPlusW, engine), honest);
    try putLeg(a, rows, "partial_slot_read", run(words, prof, start, partial0, engine), honest);
    try putLeg(a, rows, "partial_slot_read_2047", run(words, prof, start, partialButLast, engine), honest);
    try putLeg(a, rows, "final_stage_hoist", run(words, prof, start, finalStage, engine), honest);

    const ns: usize = @intCast(prof.nSlots());
    const wn: usize = @intCast(prof.wordsPerSlot());
    const pre = try a.alloc(u64, ns);
    defer a.free(pre);
    for (0..ns) |i| {
        var acc: u64 = 0;
        const slot = words[i * wn ..][0..wn];
        for (slot, 0..) |word, w| acc +%= mix.mix64(word ^ @as(u64, w));
        pre[i] = acc;
    }
    try putLeg(a, rows, "prefold_hoist", prefoldLeg(words, pre, prof, start, engine), honest);
    try putLeg(a, rows, "cached_leg", try cachedLeg(a, words, prof, start, engine), honest);

    const seed_b = terrain.derive(fixture.pkB(), fixture.seedBeacon(), engine);
    const terr_b = try terrain.fill(a, seed_b.p, prof, engine);
    defer a.free(terr_b);
    const f3 = leg.legResult(terr_b, prof, &seed_a.d, engine);
    try putLeg(a, rows, "f3_shared_terrain", f3, honest);

    const one = try fillAlt(a, seed_a.p, prof, oneLane, engine);
    defer a.free(one);
    try putTerr(a, rows, "one_lane_flip", terrain.digest(one, engine), honest_dig);

    const lin = try fillAlt(a, seed_a.p, prof, all4, engine);
    defer a.free(lin);
    try putTerr(a, rows, "all4_kernel", terrain.digest(lin, engine), honest_dig);

    const x = seed_a.p[0] ^ seed_a.p[1] ^ seed_a.p[2] ^ seed_a.p[3];
    const squeezed = [4]u64{ x, x, x, x };
    const sq = try terrain.fill(a, squeezed, prof, engine);
    defer a.free(sq);
    try putTerr(a, rows, "squeezed_seed_terrain", terrain.digest(sq, engine), honest_dig);
}
