//! SPEC §10.1 mix64 and the §11.1 slot fold.
//! Fast fold issues one load of every word and the same mix64 arithmetic,
//! eight (and four groups of eight) words at a time.

const std = @import("std");
const profile = @import("profile.zig");
const Engine = profile.Engine;

pub fn mix64(x: u64) u64 {
    var y = x +% profile.MIX_ADD;
    y = (y ^ (y >> 30)) *% profile.MIX_MUL1;
    y = (y ^ (y >> 27)) *% profile.MIX_MUL2;
    return y ^ (y >> 31);
}

fn mix64x8(x: @Vector(8, u64)) @Vector(8, u64) {
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

fn load8(ptr: [*]const u64) @Vector(8, u64) {
    var v: @Vector(8, u64) = undefined;
    @memcpy(@as([*]u8, @ptrCast(&v))[0..64], @as([*]const u8, @ptrCast(ptr))[0..64]);
    return v;
}

fn hsum(v: @Vector(8, u64)) u64 {
    return v[0] +% v[1] +% v[2] +% v[3] +% v[4] +% v[5] +% v[6] +% v[7];
}

/// SPEC §11.1 scalar fold. A plain word loop. LLVM may still autovectorise it.
pub fn foldScalar(words: []const u64, S: u64) u64 {
    var acc: u64 = 0;
    for (words) |w| acc +%= mix64(w ^ S);
    return acc;
}

/// SPEC §11.1. Eight-wide mix64, 32 words in flight, then a horizontal sum.
/// Every word of the slot is still mixed with `S`.
pub fn foldFast(words: []const u64, S: u64) u64 {
    var acc: @Vector(8, u64) = @splat(0);
    const sv: @Vector(8, u64) = @splat(S);
    var i: usize = 0;
    while (i + 32 <= words.len) : (i += 32) {
        const m0 = mix64x8(load8(words.ptr + i) ^ sv);
        const m1 = mix64x8(load8(words.ptr + i + 8) ^ sv);
        const m2 = mix64x8(load8(words.ptr + i + 16) ^ sv);
        const m3 = mix64x8(load8(words.ptr + i + 24) ^ sv);
        acc +%= (m0 +% m1) +% (m2 +% m3);
    }
    while (i + 8 <= words.len) : (i += 8) {
        acc +%= mix64x8(load8(words.ptr + i) ^ sv);
    }
    var sum = hsum(acc);
    while (i < words.len) : (i += 1) sum +%= mix64(words[i] ^ S);
    return sum;
}

/// SPEC §11.1. `words` is the full slot. Every word is mixed with `S`.
pub fn foldSlot(words: []const u64, S: u64, engine: Engine) u64 {
    return switch (engine) {
        .scalar => foldScalar(words, S),
        .fast => foldFast(words, S),
    };
}

test "vector fold matches scalar" {
    var words: [32]u64 = undefined;
    for (&words, 0..) |*w, i| w.* = @as(u64, i) *% 0x9E3779B97F4A7C15 ^ 0xC0FFEE;
    const s: u64 = 0x545dfe297dc78951;
    try std.testing.expectEqual(foldSlot(&words, s, .scalar), foldSlot(&words, s, .fast));
}
