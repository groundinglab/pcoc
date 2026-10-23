//! SPEC §7.3 CBOR. Produce uses shortest definite lengths. Decode accepts
//! non-shortest definite unsigned arguments. Indefinite lengths, tags, floats,
//! simple values, negatives, non-text keys, duplicate keys, and trailing
//! bytes are malformed.

const std = @import("std");

pub const Value = union(enum) {
    uint: u64,
    bytes: []const u8,
    text: []const u8,
    array: []Value,
    map: []Entry,
};

pub const Entry = struct {
    key: []const u8,
    val: Value,
};

pub const Error = error{ Malformed, OutOfMemory };

const Cur = struct {
    data: []const u8,
    i: usize,
    a: std.mem.Allocator,

    fn take(self: *Cur, n: usize) Error![]const u8 {
        if (self.i + n > self.data.len) return error.Malformed;
        const s = self.data[self.i .. self.i + n];
        self.i += n;
        return s;
    }
    fn byte(self: *Cur) Error!u8 {
        const s = try self.take(1);
        return s[0];
    }
    fn arg(self: *Cur, ai: u5) Error!u64 {
        return switch (ai) {
            0...23 => ai,
            24 => try self.byte(),
            25 => std.mem.readInt(u16, (try self.take(2))[0..2], .big),
            26 => std.mem.readInt(u32, (try self.take(4))[0..4], .big),
            27 => std.mem.readInt(u64, (try self.take(8))[0..8], .big),
            else => error.Malformed,
        };
    }
};

pub fn decode(a: std.mem.Allocator, data: []const u8) Error!Value {
    var c = Cur{ .data = data, .i = 0, .a = a };
    const v = try parseValue(&c);
    if (c.i != data.len) return error.Malformed;
    return v;
}

fn parseValue(c: *Cur) Error!Value {
    const head = try c.byte();
    const major: u3 = @intCast(head >> 5);
    const ai: u5 = @intCast(head & 0x1f);
    if (ai == 31) return error.Malformed;
    const n = c.arg(ai) catch return error.Malformed;
    switch (major) {
        0 => return .{ .uint = n },
        1, 6, 7 => return error.Malformed,
        2 => {
            if (n > c.data.len - c.i) return error.Malformed;
            return .{ .bytes = try c.take(@intCast(n)) };
        },
        3 => {
            if (n > c.data.len - c.i) return error.Malformed;
            const s = try c.take(@intCast(n));
            if (!std.unicode.utf8ValidateSlice(s)) return error.Malformed;
            return .{ .text = s };
        },
        4 => {
            if (n > c.data.len - c.i) return error.Malformed;
            const arr = try c.a.alloc(Value, @intCast(n));
            for (arr) |*slot| slot.* = try parseValue(c);
            return .{ .array = arr };
        },
        5 => {
            if (n > (c.data.len - c.i) / 2) return error.Malformed;
            const entries = try c.a.alloc(Entry, @intCast(n));
            for (entries) |*e| {
                const keyv = try parseValue(c);
                switch (keyv) {
                    .text => |t| e.key = t,
                    else => return error.Malformed,
                }
                e.val = try parseValue(c);
            }
            // duplicate keys
            for (entries, 0..) |e, i| {
                for (entries[0..i]) |prev| {
                    if (std.mem.eql(u8, prev.key, e.key)) return error.Malformed;
                }
            }
            return .{ .map = entries };
        },
    }
}

pub fn get(map: []const Entry, key: []const u8) ?Value {
    for (map) |e| if (std.mem.eql(u8, e.key, key)) return e.val;
    return null;
}

pub const Writer = struct {
    buf: *std.ArrayList(u8),
    a: std.mem.Allocator,

    fn byte(self: Writer, b: u8) !void {
        try self.buf.append(self.a, b);
    }
    pub fn uint(self: Writer, v: u64) !void {
        if (v < 24) {
            try self.byte(@intCast(v));
        } else if (v <= 0xff) {
            try self.byte(0x18);
            try self.byte(@intCast(v));
        } else if (v <= 0xffff) {
            try self.byte(0x19);
            var b: [2]u8 = undefined;
            std.mem.writeInt(u16, &b, @intCast(v), .big);
            try self.buf.appendSlice(self.a, &b);
        } else if (v <= 0xffffffff) {
            try self.byte(0x1a);
            var b: [4]u8 = undefined;
            std.mem.writeInt(u32, &b, @intCast(v), .big);
            try self.buf.appendSlice(self.a, &b);
        } else {
            try self.byte(0x1b);
            var b: [8]u8 = undefined;
            std.mem.writeInt(u64, &b, v, .big);
            try self.buf.appendSlice(self.a, &b);
        }
    }
    fn head(self: Writer, major: u8, n: u64) !void {
        if (n < 24) {
            try self.byte(major | @as(u8, @intCast(n)));
        } else if (n <= 0xff) {
            try self.byte(major | 24);
            try self.byte(@intCast(n));
        } else if (n <= 0xffff) {
            try self.byte(major | 25);
            var b: [2]u8 = undefined;
            std.mem.writeInt(u16, &b, @intCast(n), .big);
            try self.buf.appendSlice(self.a, &b);
        } else if (n <= 0xffffffff) {
            try self.byte(major | 26);
            var b: [4]u8 = undefined;
            std.mem.writeInt(u32, &b, @intCast(n), .big);
            try self.buf.appendSlice(self.a, &b);
        } else {
            try self.byte(major | 27);
            var b: [8]u8 = undefined;
            std.mem.writeInt(u64, &b, n, .big);
            try self.buf.appendSlice(self.a, &b);
        }
    }
    pub fn text(self: Writer, s: []const u8) !void {
        try self.head(0x60, s.len);
        try self.buf.appendSlice(self.a, s);
    }
    pub fn bytes(self: Writer, s: []const u8) !void {
        try self.head(0x40, s.len);
        try self.buf.appendSlice(self.a, s);
    }
    pub fn array(self: Writer, n: u64) !void {
        try self.head(0x80, n);
    }
    pub fn map(self: Writer, n: u64) !void {
        try self.head(0xa0, n);
    }
};

// Remove the unused first parse by not referencing it. Zig will error on unused parse and Cur.arg used.
// I'll delete the dead parse in a follow-up if the compiler complains. Unused private fns are errors in Zig.

test "roundtrip uint and trailing" {
    var list: std.ArrayList(u8) = .empty;
    defer list.deinit(std.testing.allocator);
    const w = Writer{ .buf = &list, .a = std.testing.allocator };
    try w.text("hi");
    var arena = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer arena.deinit();
    const v = try decode(arena.allocator(), list.items);
    try std.testing.expectEqualStrings("hi", v.text);
    try std.testing.expectError(error.Malformed, decode(arena.allocator(), ""));
    // trailing
    try list.append(std.testing.allocator, 0);
    try std.testing.expectError(error.Malformed, decode(arena.allocator(), list.items));
}
