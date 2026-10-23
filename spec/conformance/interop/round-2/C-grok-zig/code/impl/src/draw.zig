//! SPEC §14.6 unbiased draw and §15.5 thorough-round draw.
//! A label expands to b_0 = SHA-256(label), b_(n+1) = SHA-256(b_n), concatenated.
//! Each draw reads that chain from the first byte.

const std = @import("std");
const sha = @import("sha.zig");

pub const Chain = struct {
    prev: [32]u8,
    buf: [32]u8,
    off: usize = 0,
    started: bool = false,

    pub fn init(label: []const u8) Chain {
        var h: [32]u8 = undefined;
        sha.hashFast(label, &h);
        return .{ .prev = h, .buf = h, .off = 0, .started = true };
    }

    pub fn read8(self: *Chain) [8]u8 {
        var out: [8]u8 = undefined;
        var n: usize = 0;
        while (n < 8) {
            if (self.off == 32) {
                var nxt: [32]u8 = undefined;
                sha.hashFast(&self.prev, &nxt);
                self.prev = nxt;
                self.buf = nxt;
                self.off = 0;
            }
            const take = @min(8 - n, 32 - self.off);
            @memcpy(out[n..][0..take], self.buf[self.off .. self.off + take]);
            self.off += take;
            n += take;
        }
        return out;
    }
};

pub const Short = error{Short};

fn one(read8: *const fn (*anyopaque) Short![8]u8, ctx: *anyopaque, leg_total: u64) Short!u64 {
    if (leg_total == 0) return error.Short;
    const m: u128 = @as(u128, 1) << 64;
    const rem: u128 = m % leg_total;
    if (rem == 0) {
        const b = try read8(ctx);
        const u = std.mem.readInt(u64, &b, .little);
        return u % leg_total;
    }
    const limit: u64 = @intCast(m - rem);
    while (true) {
        const b = try read8(ctx);
        const u = std.mem.readInt(u64, &b, .little);
        if (u < limit) return u % leg_total;
    }
}

fn chainRead(ctx: *anyopaque) Short![8]u8 {
    const c: *Chain = @ptrCast(@alignCast(ctx));
    return c.read8();
}

fn contains(xs: []const u64, q: u64) bool {
    for (xs) |x| if (x == q) return true;
    return false;
}

fn allUsed(xs: []const u64, leg_total: u64) bool {
    if (xs.len < leg_total) return false;
    var i: u64 = 0;
    while (i < leg_total) : (i += 1) {
        if (!contains(xs, i)) return false;
    }
    return true;
}

/// SPEC §14.6. Six distinct global indices. `leg_total` < 6 is the caller's `not defined`.
pub fn sixLeg(label: []const u8, leg_total: u64) [6]u64 {
    var chain = Chain.init(label);
    var out: [6]u64 = undefined;
    var n: usize = 0;
    while (n < 6) {
        const q = one(chainRead, @ptrCast(&chain), leg_total) catch unreachable;
        if (contains(out[0..n], q)) continue;
        out[n] = q;
        n += 1;
    }
    return out;
}

/// SPEC §15.5. Returns D = 6 * thorough_rounds indices in draw order.
pub fn thorough(label: []const u8, leg_total: u64, rounds: u64, a: std.mem.Allocator) ![]u64 {
    const D: usize = @intCast(6 * rounds);
    var phase: std.ArrayList(u64) = .empty;
    errdefer phase.deinit(a);
    var chain = Chain.init(label);
    while (phase.items.len < D) {
        const q = one(chainRead, @ptrCast(&chain), leg_total) catch return error.Short;
        const group_start = (phase.items.len / 6) * 6;
        if (contains(phase.items[group_start..], q)) continue;
        if (contains(phase.items, q) and !allUsed(phase.items, leg_total)) continue;
        try phase.append(a, q);
    }
    return phase.toOwnedSlice(a);
}

pub fn formatIndices(a: std.mem.Allocator, xs: []const u64) ![]u8 {
    var list: std.ArrayList(u8) = .empty;
    errdefer list.deinit(a);
    for (xs, 0..) |x, i| {
        if (i != 0) try list.append(a, ',');
        var tmp: [32]u8 = undefined;
        const s = std.fmt.bufPrint(&tmp, "{d}", .{x}) catch unreachable;
        try list.appendSlice(a, s);
    }
    return list.toOwnedSlice(a);
}

const Os = struct {
    fn read(ctx: *anyopaque) Short![8]u8 {
        _ = ctx;
        var b: [8]u8 = undefined;
        std.posix.getrandom(&b) catch return error.Short;
        return b;
    }
};

pub fn sixLegOs(leg_total: u64) ![6]u64 {
    var out: [6]u64 = undefined;
    var n: usize = 0;
    var guard: usize = 0;
    var dummy: u8 = 0;
    while (n < 6) {
        guard += 1;
        if (guard > 1000000) return error.Short;
        const q = try one(Os.read, @ptrCast(&dummy), leg_total);
        if (contains(out[0..n], q)) continue;
        out[n] = q;
        n += 1;
    }
    return out;
}

test "power of two draw uses the first bytes" {
    const xs = sixLeg("pcoc-pack-2.2/draw", 8);
    for (xs) |x| try std.testing.expect(x < 8);
    // distinct
    for (xs, 0..) |x, i| {
        for (xs[0..i]) |y| try std.testing.expect(x != y);
    }
}

test "thorough groups of 6 are distinct at 6 legs" {
    const a = std.testing.allocator;
    const xs = try thorough("pcoc-pack-2.2/draw", 6, 3, a);
    defer a.free(xs);
    try std.testing.expectEqual(@as(usize, 18), xs.len);
    var r: usize = 0;
    while (r < 3) : (r += 1) {
        const g = xs[r * 6 .. r * 6 + 6];
        var seen = [_]bool{false} ** 6;
        for (g) |q| {
            try std.testing.expect(q < 6);
            try std.testing.expect(!seen[@intCast(q)]);
            seen[@intCast(q)] = true;
        }
    }
}
