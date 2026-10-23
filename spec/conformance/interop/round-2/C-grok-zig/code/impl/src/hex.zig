const std = @import("std");

const digits = "0123456789abcdef";

pub fn write(out: []u8, bytes: []const u8) void {
    std.debug.assert(out.len == bytes.len * 2);
    for (bytes, 0..) |b, i| {
        out[i * 2] = digits[b >> 4];
        out[i * 2 + 1] = digits[b & 0xf];
    }
}

pub fn of(comptime n: usize, bytes: *const [n]u8) [n * 2]u8 {
    var out: [n * 2]u8 = undefined;
    write(&out, bytes);
    return out;
}

pub fn u64be(v: u64) [16]u8 {
    var out: [16]u8 = undefined;
    var x = v;
    var i: usize = 16;
    while (i > 0) {
        i -= 1;
        out[i] = digits[@as(u4, @intCast(x & 0xf))];
        x >>= 4;
    }
    return out;
}

pub fn parse(dst: []u8, text: []const u8) !void {
    if (text.len != dst.len * 2) return error.BadHex;
    for (dst, 0..) |*b, i| {
        const hi = val(text[i * 2]) orelse return error.BadHex;
        const lo = val(text[i * 2 + 1]) orelse return error.BadHex;
        b.* = (hi << 4) | lo;
    }
}

fn val(c: u8) ?u8 {
    return switch (c) {
        '0'...'9' => c - '0',
        'a'...'f' => c - 'a' + 10,
        'A'...'F' => c - 'A' + 10,
        else => null,
    };
}

pub fn parse32(text: []const u8) ![32]u8 {
    var out: [32]u8 = undefined;
    try parse(&out, text);
    return out;
}
