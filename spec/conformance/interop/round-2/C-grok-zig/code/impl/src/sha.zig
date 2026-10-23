//! SPEC §3 SHA-256. The fast path is std Sha256, which emits SHA-NI on an
//! x86_64 target that has SHA and AVX2 (SPEC §11.1). `scalar` is the plain
//! FIPS 180-4 compression, used for the scalar timing comparison.

const std = @import("std");
const profile = @import("profile.zig");
const Engine = profile.Engine;

const K = [64]u32{
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
};

const iv = [8]u32{
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
};

fn compress(state: *[8]u32, block: *const [64]u8) void {
    var w: [64]u32 = undefined;
    for (0..16) |i| {
        w[i] = std.mem.readInt(u32, block[i * 4 ..][0..4], .big);
    }
    for (16..64) |i| {
        const s0 = std.math.rotr(u32, w[i - 15], 7) ^ std.math.rotr(u32, w[i - 15], 18) ^ (w[i - 15] >> 3);
        const s1 = std.math.rotr(u32, w[i - 2], 17) ^ std.math.rotr(u32, w[i - 2], 19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16] +% s0 +% w[i - 7] +% s1;
    }
    var a = state[0];
    var b = state[1];
    var c = state[2];
    var d = state[3];
    var e = state[4];
    var f = state[5];
    var g = state[6];
    var h = state[7];
    for (0..64) |i| {
        const S1 = std.math.rotr(u32, e, 6) ^ std.math.rotr(u32, e, 11) ^ std.math.rotr(u32, e, 25);
        const ch = (e & f) ^ (~e & g);
        const t1 = h +% S1 +% ch +% K[i] +% w[i];
        const S0 = std.math.rotr(u32, a, 2) ^ std.math.rotr(u32, a, 13) ^ std.math.rotr(u32, a, 22);
        const maj = (a & b) ^ (a & c) ^ (b & c);
        const t2 = S0 +% maj;
        h = g;
        g = f;
        f = e;
        e = d +% t1;
        d = c;
        c = b;
        b = a;
        a = t1 +% t2;
    }
    state[0] +%= a;
    state[1] +%= b;
    state[2] +%= c;
    state[3] +%= d;
    state[4] +%= e;
    state[5] +%= f;
    state[6] +%= g;
    state[7] +%= h;
}

pub const Scalar = struct {
    s: [8]u32 = iv,
    buf: [64]u8 = undefined,
    buf_len: usize = 0,
    total: u64 = 0,

    pub fn update(self: *Scalar, data: []const u8) void {
        var off: usize = 0;
        if (self.buf_len != 0) {
            const n = @min(64 - self.buf_len, data.len);
            @memcpy(self.buf[self.buf_len..][0..n], data[0..n]);
            self.buf_len += n;
            off += n;
            if (self.buf_len == 64) {
                compress(&self.s, &self.buf);
                self.buf_len = 0;
            }
        }
        while (off + 64 <= data.len) : (off += 64) {
            compress(&self.s, data[off..][0..64]);
        }
        if (off < data.len) {
            const n = data.len - off;
            @memcpy(self.buf[0..n], data[off..]);
            self.buf_len = n;
        }
        self.total += data.len;
    }

    pub fn final(self: *Scalar, out: *[32]u8) void {
        var block: [64]u8 = undefined;
        @memcpy(block[0..self.buf_len], self.buf[0..self.buf_len]);
        block[self.buf_len] = 0x80;
        if (self.buf_len + 1 <= 56) {
            @memset(block[self.buf_len + 1 .. 56], 0);
            std.mem.writeInt(u64, block[56..64], self.total * 8, .big);
            compress(&self.s, &block);
        } else {
            @memset(block[self.buf_len + 1 .. 64], 0);
            compress(&self.s, &block);
            @memset(block[0..56], 0);
            std.mem.writeInt(u64, block[56..64], self.total * 8, .big);
            compress(&self.s, &block);
        }
        for (0..8) |i| std.mem.writeInt(u32, out[i * 4 ..][0..4], self.s[i], .big);
    }
};

pub const Hasher = union(enum) {
    fast: std.crypto.hash.sha2.Sha256,
    scalar: Scalar,

    pub fn init(engine: Engine) Hasher {
        return switch (engine) {
            .fast => .{ .fast = std.crypto.hash.sha2.Sha256.init(.{}) },
            .scalar => .{ .scalar = .{} },
        };
    }
    pub fn update(self: *Hasher, data: []const u8) void {
        switch (self.*) {
            .fast => |*h| h.update(data),
            .scalar => |*h| h.update(data),
        }
    }
    pub fn final(self: *Hasher, out: *[32]u8) void {
        switch (self.*) {
            .fast => |*h| h.final(out),
            .scalar => |*h| h.final(out),
        }
    }
};

/// SPEC §3 H.
pub fn hash(data: []const u8, out: *[32]u8, engine: Engine) void {
    var h = Hasher.init(engine);
    h.update(data);
    h.final(out);
}

pub fn hashFast(data: []const u8, out: *[32]u8) void {
    hash(data, out, .fast);
}

/// One SHA-256 of a 20-byte seal message, specialised so the leg loop inlines it.
/// `fast` is std Sha256 (SHA-NI on this host). `scalar` is the FIPS compressor.
pub fn hash20(msg: *const [20]u8, out: *[32]u8, comptime engine: Engine) void {
    switch (engine) {
        .fast => std.crypto.hash.sha2.Sha256.hash(msg, out, .{}),
        .scalar => {
            var block: [64]u8 = [_]u8{0} ** 64;
            @memcpy(block[0..20], msg);
            block[20] = 0x80;
            block[63] = 160;
            var st = iv;
            compress(&st, &block);
            for (0..8) |i| std.mem.writeInt(u32, out[i * 4 ..][0..4], st[i], .big);
        },
    }
}

test "fips abc and section 7.4 seal" {
    var out: [32]u8 = undefined;
    hash("abc", &out, .scalar);
    var fast: [32]u8 = undefined;
    hash("abc", &fast, .fast);
    const expect = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    var got: [64]u8 = undefined;
    const hex = "0123456789abcdef";
    for (out, 0..) |b, i| {
        got[i * 2] = hex[b >> 4];
        got[i * 2 + 1] = hex[b & 0xf];
    }
    try std.testing.expectEqualStrings(expect, &got);
    try std.testing.expectEqualSlices(u8, &out, &fast);

    var msg: [20]u8 = undefined;
    @memcpy(msg[0..12], profile.SEAL_DOMAIN);
    @memset(msg[12..20], 0);
    hash(&msg, &out, .fast);
    hash(&msg, &fast, .scalar);
    try std.testing.expectEqualSlices(u8, &out, &fast);
    const seal = "4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533";
    for (out, 0..) |b, i| {
        got[i * 2] = hex[b >> 4];
        got[i * 2 + 1] = hex[b & 0xf];
    }
    try std.testing.expectEqualStrings(seal, &got);
    const s_next = std.mem.readInt(u64, out[0..8], .little);
    try std.testing.expectEqual(@as(u64, 0x76042423afbd1648), s_next);
}
