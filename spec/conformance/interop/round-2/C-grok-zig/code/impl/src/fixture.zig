//! CASES.md fixed beacons, heights, and public keys. Test-plane inputs only.

const sha = @import("sha.zig");

pub const pk_a_hex = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
pub const pk_b_hex = "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";

pub fn pkA() [32]u8 {
    const hex = @import("hex.zig");
    return hex.parse32(pk_a_hex) catch unreachable;
}
pub fn pkB() [32]u8 {
    const hex = @import("hex.zig");
    return hex.parse32(pk_b_hex) catch unreachable;
}

fn hashStr(s: []const u8) [32]u8 {
    var out: [32]u8 = undefined;
    sha.hashFast(s, &out);
    return out;
}

pub fn seedBeacon() [32]u8 {
    return hashStr("pcoc-pack-2.2/beacon/seed");
}

/// I_k for k = 0 … 17.
pub fn interrupt(k: usize) [32]u8 {
    var buf: [64]u8 = undefined;
    const prefix = "pcoc-pack-2.2/beacon/";
    @memcpy(buf[0..prefix.len], prefix);
    const n = stdFmt(buf[prefix.len..], k);
    return hashStr(buf[0 .. prefix.len + n]);
}

pub fn filler(h: u64) [32]u8 {
    var buf: [80]u8 = undefined;
    const prefix = "pcoc-pack-2.2/index/filler/";
    @memcpy(buf[0..prefix.len], prefix);
    const n = stdFmtU(buf[prefix.len..], h);
    return hashStr(buf[0 .. prefix.len + n]);
}

/// Natural 32 bytes served at height h. Seed is 1000. I_k is at 1001 + k.
pub fn naturalAt(h: u64) [32]u8 {
    if (h == 1000) return seedBeacon();
    if (h >= 1001 and h <= 1018) return interrupt(@intCast(h - 1001));
    return filler(h);
}

pub fn timeAt(h: u64) u64 {
    return 1700000000 + 600 * h;
}

pub const index_lo: u64 = 989;
pub const index_hi: u64 = 1018;
pub const seed_height: u64 = 1000;

fn stdFmt(buf: []u8, k: usize) usize {
    return stdFmtU(buf, k);
}
fn stdFmtU(buf: []u8, v: u64) usize {
    var tmp: [32]u8 = undefined;
    var x = v;
    var n: usize = 0;
    if (x == 0) {
        buf[0] = '0';
        return 1;
    }
    while (x > 0) {
        tmp[n] = @intCast('0' + x % 10);
        n += 1;
        x /= 10;
    }
    for (0..n) |i| buf[i] = tmp[n - 1 - i];
    return n;
}

const std = @import("std");

test "beacon heights line up" {
    const s = seedBeacon();
    const int0 = interrupt(0);
    try std.testing.expect(!std.mem.eql(u8, &s, &int0));
    try std.testing.expectEqual(@as(u64, 1700600000), timeAt(1000));
}
