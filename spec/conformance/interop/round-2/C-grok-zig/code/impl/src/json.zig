//! JSON parser for portable-index documents. Integers only. Duplicate names fail.

const std = @import("std");

pub const Value = union(enum) {
    null,
    boolean: bool,
    number: i64,
    string: []const u8,
    array: []Value,
    object: []Field,
};
pub const Field = struct { name: []const u8, value: Value };

pub const Error = error{ BadJson, OutOfMemory };

const P = struct {
    s: []const u8,
    i: usize,
    a: std.mem.Allocator,

    fn peek(self: P) ?u8 {
        if (self.i >= self.s.len) return null;
        return self.s[self.i];
    }
    fn bump(self: *P) !u8 {
        if (self.i >= self.s.len) return error.BadJson;
        const c = self.s[self.i];
        self.i += 1;
        return c;
    }
    fn skip(self: *P) void {
        while (self.peek()) |c| switch (c) {
            ' ', '\t', '\n', '\r' => self.i += 1,
            else => return,
        };
    }
};

pub fn parse(a: std.mem.Allocator, text: []const u8) Error!Value {
    if (text.len >= 3 and text[0] == 0xef and text[1] == 0xbb and text[2] == 0xbf) return error.BadJson;
    var p = P{ .s = text, .i = 0, .a = a };
    p.skip();
    const v = try value(&p);
    p.skip();
    if (p.i != text.len) return error.BadJson;
    return v;
}

fn value(p: *P) Error!Value {
    p.skip();
    const c = p.peek() orelse return error.BadJson;
    switch (c) {
        '{' => return .{ .object = try object(p) },
        '[' => return .{ .array = try array(p) },
        '"' => return .{ .string = try string(p) },
        't' => {
            try lit(p, "true");
            return .{ .boolean = true };
        },
        'f' => {
            try lit(p, "false");
            return .{ .boolean = false };
        },
        'n' => {
            try lit(p, "null");
            return .null;
        },
        '-', '0'...'9' => return .{ .number = try number(p) },
        else => return error.BadJson,
    }
}

fn lit(p: *P, s: []const u8) !void {
    if (p.i + s.len > p.s.len or !std.mem.eql(u8, p.s[p.i .. p.i + s.len], s)) return error.BadJson;
    p.i += s.len;
}

fn object(p: *P) Error![]Field {
    _ = try p.bump();
    var list: std.ArrayList(Field) = .empty;
    errdefer list.deinit(p.a);
    p.skip();
    if (p.peek() == '}') {
        p.i += 1;
        return list.toOwnedSlice(p.a);
    }
    while (true) {
        p.skip();
        const name = try string(p);
        p.skip();
        if (try p.bump() != ':') return error.BadJson;
        const v = try value(p);
        for (list.items) |f| if (std.mem.eql(u8, f.name, name)) return error.BadJson;
        try list.append(p.a, .{ .name = name, .value = v });
        p.skip();
        const c = try p.bump();
        if (c == '}') break;
        if (c != ',') return error.BadJson;
    }
    return list.toOwnedSlice(p.a);
}

fn array(p: *P) Error![]Value {
    _ = try p.bump();
    var list: std.ArrayList(Value) = .empty;
    errdefer list.deinit(p.a);
    p.skip();
    if (p.peek() == ']') {
        p.i += 1;
        return list.toOwnedSlice(p.a);
    }
    while (true) {
        try list.append(p.a, try value(p));
        p.skip();
        const c = try p.bump();
        if (c == ']') break;
        if (c != ',') return error.BadJson;
    }
    return list.toOwnedSlice(p.a);
}

fn string(p: *P) Error![]const u8 {
    if (try p.bump() != '"') return error.BadJson;
    const start = p.i;
    var needs_copy = false;
    while (true) {
        const c = try p.bump();
        if (c == '"') break;
        if (c == '\\') {
            needs_copy = true;
            _ = try p.bump();
        } else if (c < 0x20) return error.BadJson;
    }
    const raw = p.s[start .. p.i - 1];
    if (!needs_copy) {
        if (!std.unicode.utf8ValidateSlice(raw)) return error.BadJson;
        return raw;
    }
    var out: std.ArrayList(u8) = .empty;
    errdefer out.deinit(p.a);
    var i: usize = 0;
    while (i < raw.len) : (i += 1) {
        if (raw[i] != '\\') {
            try out.append(p.a, raw[i]);
            continue;
        }
        i += 1;
        if (i >= raw.len) return error.BadJson;
        switch (raw[i]) {
            '"', '\\', '/' => try out.append(p.a, raw[i]),
            'b' => try out.append(p.a, 0x08),
            'f' => try out.append(p.a, 0x0c),
            'n' => try out.append(p.a, '\n'),
            'r' => try out.append(p.a, '\r'),
            't' => try out.append(p.a, '\t'),
            'u' => return error.BadJson,
            else => return error.BadJson,
        }
    }
    if (!std.unicode.utf8ValidateSlice(out.items)) return error.BadJson;
    return out.toOwnedSlice(p.a);
}

fn number(p: *P) Error!i64 {
    const start = p.i;
    if (p.peek() == '-') p.i += 1;
    const digs = p.i;
    if (p.peek() == '0') {
        p.i += 1;
        if (p.peek()) |c| if (c >= '0' and c <= '9') return error.BadJson;
    } else {
        if (p.peek() == null or p.peek().? < '0' or p.peek().? > '9') return error.BadJson;
        while (p.peek()) |c| {
            if (c < '0' or c > '9') break;
            p.i += 1;
        }
    }
    if (p.i == digs) return error.BadJson;
    if (p.peek()) |c| if (c == '.' or c == 'e' or c == 'E') return error.BadJson;
    return std.fmt.parseInt(i64, p.s[start..p.i], 10) catch error.BadJson;
}

pub fn field(obj: []const Field, name: []const u8) ?Value {
    for (obj) |f| if (std.mem.eql(u8, f.name, name)) return f.value;
    return null;
}
