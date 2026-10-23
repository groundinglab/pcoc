//! SPEC §10 seed and §10.4 dag2 terrain fill.

const std = @import("std");
const profile = @import("profile.zig");
const sha = @import("sha.zig");
const mix = @import("mix.zig");
const Engine = profile.Engine;

pub const Seed = struct {
    preimage: [64]u8,
    d: [32]u8,
    p: [4]u64,
};

/// SPEC §10.
pub fn derive(public_key: [32]u8, seed_beacon: [32]u8, engine: Engine) Seed {
    var pre: [64]u8 = undefined;
    @memcpy(pre[0..32], &public_key);
    @memcpy(pre[32..64], &seed_beacon);
    var d: [32]u8 = undefined;
    sha.hash(&pre, &d, engine);
    var p: [4]u64 = undefined;
    for (0..4) |k| p[k] = std.mem.readInt(u64, d[k * 8 ..][0..8], .little);
    return .{ .preimage = pre, .d = d, .p = p };
}

pub fn floorLog2(n: u64) u6 {
    return @intCast(63 - @clz(n));
}

pub fn reverse32(v: u64) u64 {
    const x: u32 = @truncate(v);
    return @bitReverse(x);
}

/// SPEC §10.3. `i` is in 1 … N_SLOTS−1.
pub fn bitrevParent(i: u64) u64 {
    const bits: u6 = @max(floorLog2(i), 1);
    const shifted = reverse32(i) >> @intCast(32 - @as(u32, bits));
    return shifted % i;
}

pub fn mulParent(i: u64) u64 {
    const h = i *% profile.MIX_ADD;
    return (h >> 32) % i;
}

/// SPEC §10.4 NL4 base word, before parents and slot_mix.
pub fn nl4Base(i: u64, j: u64, lanes: [4]u64) u64 {
    const c = (i << 32) ^ j ^ profile.WORD_XOR;
    var t = mix.mix64(lanes[j % 4] ^ c);
    t = mix.mix64(t ^ std.math.rotl(u64, lanes[(j + 1) % 4], 17));
    t = mix.mix64(t ^ std.math.rotl(u64, lanes[(j + 2) % 4], 31));
    t = mix.mix64(t ^ std.math.rotl(u64, lanes[(j + 3) % 4], 47));
    return t;
}

fn nl4Base8(i: u64, j: usize, lanes: [4]u64) [8]u64 {
    var out: [8]u64 = undefined;
    const i_hi = i << 32;
    var p0: @Vector(8, u64) = undefined;
    var r17: @Vector(8, u64) = undefined;
    var r31: @Vector(8, u64) = undefined;
    var r47: @Vector(8, u64) = undefined;
    var c: @Vector(8, u64) = undefined;
    for (0..8) |lane| {
        const jj: u64 = j + lane;
        c[lane] = i_hi ^ jj ^ profile.WORD_XOR;
        p0[lane] = lanes[jj % 4];
        r17[lane] = std.math.rotl(u64, lanes[(jj + 1) % 4], 17);
        r31[lane] = std.math.rotl(u64, lanes[(jj + 2) % 4], 31);
        r47[lane] = std.math.rotl(u64, lanes[(jj + 3) % 4], 47);
    }
    var t = mixVec(p0 ^ c);
    t = mixVec(t ^ r17);
    t = mixVec(t ^ r31);
    t = mixVec(t ^ r47);
    for (0..8) |lane| out[lane] = t[lane];
    return out;
}

fn mixVec(x: @Vector(8, u64)) @Vector(8, u64) {
    const add: @Vector(8, u64) = @splat(profile.MIX_ADD);
    const m1: @Vector(8, u64) = @splat(profile.MIX_MUL1);
    const m2: @Vector(8, u64) = @splat(profile.MIX_MUL2);
    const s30: @Vector(8, u6) = @splat(30);
    const s27: @Vector(8, u6) = @splat(27);
    const s31: @Vector(8, u6) = @splat(31);
    var y = x +% add;
    y = (y ^ (y >> s30)) *% m1;
    y = (y ^ (y >> s27)) *% m2;
    return y ^ (y >> s31);
}

fn store8(dst: []u64, v: [8]u64) void {
    @memcpy(dst[0..8], &v);
}

/// Parents and slot_mix for slot `i > 0`. `slot` is the base words, updated in place.
/// `terrain` holds already-stored earlier slots as native little-endian words.
pub fn applyParents(slot: []u64, i: u64, terrain: []const u64, words: u64) void {
    const parents = [_]u64{ i - 1, bitrevParent(i), mulParent(i) };
    for (parents, 0..) |pidx, ki| {
        const k: u64 = ki + 1;
        const kmul = k *% profile.PARENT_MUL;
        const pw = terrain[pidx * words ..][0..words];
        var j: usize = 0;
        while (j < words) : (j += 1) {
            const rot: u32 = @intCast((@as(u64, j) + k) % 64);
            const mixed = (slot[j] ^ std.math.rotl(u64, pw[j], rot)) ^ kmul;
            slot[j] = mix.mix64(mixed);
        }
    }
    const sm = i *% profile.SLOT_MUL;
    for (slot) |*w| w.* = mix.mix64(w.* ^ sm);
}

pub fn applyParentsFast(slot: []u64, i: u64, terrain: []const u64, words: u64) void {
    const parents = [_]u64{ i - 1, bitrevParent(i), mulParent(i) };
    for (parents, 0..) |pidx, ki| {
        const k: u64 = ki + 1;
        const kmul = k *% profile.PARENT_MUL;
        const pw = terrain[pidx * words ..][0..words];
        var j: usize = 0;
        while (j + 8 <= words) : (j += 8) {
            var x: [8]u64 = undefined;
            for (0..8) |lane| {
                const rot: u32 = @intCast((@as(u64, j + lane) + k) % 64);
                x[lane] = (slot[j + lane] ^ std.math.rotl(u64, pw[j + lane], rot)) ^ kmul;
            }
            var xv: @Vector(8, u64) = x;
            xv = mixVec(xv);
            for (0..8) |lane| slot[j + lane] = xv[lane];
        }
        while (j < words) : (j += 1) {
            const rot: u32 = @intCast((@as(u64, j) + k) % 64);
            slot[j] = mix.mix64((slot[j] ^ std.math.rotl(u64, pw[j], rot)) ^ kmul);
        }
    }
    const sm = i *% profile.SLOT_MUL;
    const smv: @Vector(8, u64) = @splat(sm);
    var j: usize = 0;
        while (j + 8 <= words) : (j += 8) {
        var tmp: [8]u64 = undefined;
        @memcpy(&tmp, slot[j..][0..8]);
        var xv: @Vector(8, u64) = tmp;
        xv = mixVec(xv ^ smv);
        for (0..8) |lane| slot[j + lane] = xv[lane];
    }
    while (j < words) : (j += 1) slot[j] = mix.mix64(slot[j] ^ sm);
}

/// SPEC §10.4. Returns N_SLOTS × WORDS_PER_SLOT little-endian words.
pub fn fill(allocator: std.mem.Allocator, lanes: [4]u64, p: profile.Profile, engine: Engine) ![]align(64) u64 {
    const words: usize = @intCast(p.wordsPerSlot());
    const slots: usize = @intCast(p.nSlots());
    const nbytes = slots * words * @sizeOf(u64);
    const huge = nbytes >= (2 << 20);
    const terrain = if (huge)
        try allocator.alignedAlloc(u64, @as(std.mem.Alignment, @enumFromInt(21)), slots * words)
    else
        try allocator.alignedAlloc(u64, .@"64", slots * words);
    errdefer allocator.free(terrain);
    if (huge) {
        const raw = std.mem.sliceAsBytes(terrain);
        std.posix.madvise(@alignCast(raw.ptr), raw.len, std.os.linux.MADV.HUGEPAGE) catch {};
    }
    @memset(terrain, 0);
    var i: usize = 0;
    while (i < slots) : (i += 1) {
        const slot = terrain[i * words ..][0..words];
        switch (engine) {
            .scalar => {
                for (slot, 0..) |*w, j| w.* = nl4Base(@intCast(i), @intCast(j), lanes);
                if (i > 0) applyParents(slot, @intCast(i), terrain, @intCast(words));
            },
            .fast => {
                var j: usize = 0;
                while (j + 8 <= words) : (j += 8) {
                    const chunk = nl4Base8(@intCast(i), j, lanes);
                    store8(slot[j..], chunk);
                }
                while (j < words) : (j += 1) slot[j] = nl4Base(@intCast(i), @intCast(j), lanes);
                if (i > 0) applyParentsFast(slot, @intCast(i), terrain, @intCast(words));
            },
        }
    }
    return terrain;
}

pub fn digest(terrain: []const u64, engine: Engine) [32]u8 {
    var out: [32]u8 = undefined;
    sha.hash(std.mem.sliceAsBytes(terrain), &out, engine);
    return out;
}

test "section 7.4 nl4 base word" {
    var pre: [64]u8 = undefined;
    @memset(pre[0..32], 0x11);
    @memset(pre[32..64], 0x22);
    var d: [32]u8 = undefined;
    sha.hash(&pre, &d, .fast);
    const hex = @import("hex.zig");
    const spelled = hex.of(32, &d);
    try std.testing.expectEqualStrings("5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c", &spelled);
    var lanes: [4]u64 = undefined;
    for (0..4) |k| lanes[k] = std.mem.readInt(u64, d[k * 8 ..][0..8], .little);
    try std.testing.expectEqual(@as(u64, 0x545dfe297dc78951), lanes[0]);
    try std.testing.expectEqual(@as(u64, 0x27858669c45e046a), lanes[1]);
    try std.testing.expectEqual(@as(u64, 0x9cdac73ac1a5fe85), lanes[2]);
    try std.testing.expectEqual(@as(u64, 0x7c81df6efbf55f11), lanes[3]);
    try std.testing.expectEqual(@as(u64, 0xc7582082a632c2c8), nl4Base(0, 0, lanes));
}

test "s1 scalar and fast terrain agree" {
    const fx = @import("fixture.zig");
    const seed = derive(fx.pkA(), fx.seedBeacon(), .fast);
    const a = std.testing.allocator;
    const t1 = try fill(a, seed.p, profile.s1, .scalar);
    defer a.free(t1);
    const t2 = try fill(a, seed.p, profile.s1, .fast);
    defer a.free(t2);
    try std.testing.expectEqualSlices(u64, t1, t2);
}
