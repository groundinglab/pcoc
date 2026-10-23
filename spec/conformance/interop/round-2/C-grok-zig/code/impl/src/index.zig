//! Portable index. The CASES fixture is generated and parsed as JSON.
//! `--index-url` does one HTTP/1.0 GET and does not follow redirects.

const std = @import("std");
const json = @import("json.zig");
const hex = @import("hex.zig");
const fixture = @import("fixture.zig");

pub const Plant = struct {
    root: [32]u8,
    txid: [32]u8,
    vout: u32,
    block_hash: [32]u8,
    block_height: u32,
};

pub const Doc = struct {
    returned: bool = false,
    network: ?[]const u8 = null,
    hash: ?[32]u8 = null, // natural
    height: ?u64 = null,
    time: ?u64 = null,
    header_usable: bool = false,
    plants: []Plant = &.{},
};

pub const Mode = union(enum) {
    fixture,
    none,
    url: []const u8,
};

pub const Index = struct {
    a: std.mem.Allocator,
    mode: Mode,
    arena: std.heap.ArenaAllocator,
    cache: std.AutoHashMap(u64, Doc),

    pub fn init(a: std.mem.Allocator, mode: Mode) Index {
        return .{
            .a = a,
            .mode = mode,
            .arena = std.heap.ArenaAllocator.init(a),
            .cache = std.AutoHashMap(u64, Doc).init(a),
        };
    }
    pub fn deinit(self: *Index) void {
        self.cache.deinit();
        self.arena.deinit();
    }

    pub fn get(self: *Index, height: u64) !Doc {
        if (self.cache.get(height)) |d| return d;
        const doc = try self.fetch(height);
        try self.cache.put(height, doc);
        return doc;
    }

    fn fetch(self: *Index, height: u64) !Doc {
        const body: ?[]const u8 = switch (self.mode) {
            .none => null,
            .fixture => try fixtureBody(self.arena.allocator(), height),
            .url => |u| self.http(u, height) catch null,
        };
        const text = body orelse return .{};
        return interpret(self.arena.allocator(), text) catch .{ .returned = false };
    }

    fn http(self: *Index, base: []const u8, height: u64) !?[]const u8 {
        const url = std.mem.trim(u8, base, " ");
        const scheme_end = std.mem.indexOf(u8, url, "://") orelse return null;
        const scheme = url[0..scheme_end];
        const rest = url[scheme_end + 3 ..];
        if (rest.len == 0 or std.mem.indexOfScalar(u8, rest, '/') != null) return null;
        var host = rest;
        var port: u16 = if (std.ascii.eqlIgnoreCase(scheme, "https")) 443 else if (std.ascii.eqlIgnoreCase(scheme, "http")) 80 else return null;
        if (std.mem.lastIndexOfScalar(u8, host, ':')) |c| {
            port = std.fmt.parseInt(u16, host[c + 1 ..], 10) catch return null;
            host = host[0..c];
        }
        if (std.ascii.eqlIgnoreCase(scheme, "https")) return null;
        const stream = std.net.tcpConnectToHost(self.a, host, port) catch return null;
        defer stream.close();
        var req: std.ArrayList(u8) = .empty;
        defer req.deinit(self.a);
        var hb: [32]u8 = undefined;
        const hs = std.fmt.bufPrint(&hb, "{d}", .{height}) catch return null;
        const w = req.writer(self.a);
        try w.print("GET /index/{s} HTTP/1.0\r\nHost: {s}\r\nConnection: close\r\n\r\n", .{ hs, host });
        stream.writeAll(req.items) catch return null;
        var buf: std.ArrayList(u8) = .empty;
        errdefer buf.deinit(self.a);
        var tmp: [4096]u8 = undefined;
        while (true) {
            const n = stream.read(&tmp) catch return null;
            if (n == 0) break;
            try buf.appendSlice(self.a, tmp[0..n]);
            if (buf.items.len > 1_000_000) return null;
        }
        const raw = buf.items;
        const sep = std.mem.indexOf(u8, raw, "\r\n\r\n") orelse return null;
        const head = raw[0..sep];
        const line_end = std.mem.indexOf(u8, head, "\r\n") orelse head.len;
        const status = head[0..line_end];
        // "HTTP/1.x 200"
        var it = std.mem.tokenizeScalar(u8, status, ' ');
        _ = it.next() orelse return null;
        const code = it.next() orelse return null;
        if (!std.mem.eql(u8, code, "200")) return null;
        const owned = try self.arena.allocator().dupe(u8, raw[sep + 4 ..]);
        buf.deinit(self.a);
        return owned;
    }
};

fn fixtureBody(a: std.mem.Allocator, h: u64) !?[]const u8 {
    if (h < fixture.index_lo or h > fixture.index_hi) return null;
    const nat = fixture.naturalAt(h);
    var disp: [32]u8 = undefined;
    for (0..32) |i| disp[i] = nat[31 - i];
    const hx = hex.of(32, &disp);
    var list: std.ArrayList(u8) = .empty;
    errdefer list.deinit(a);
    const wr = list.writer(a);
    try wr.print("{{\"network\":\"synthetic\",\"header\":{{\"hash\":\"{s}\",\"height\":{d},\"time\":{d}}}}}", .{ hx, h, fixture.timeAt(h) });
    return try list.toOwnedSlice(a);
}

fn interpret(a: std.mem.Allocator, text: []const u8) !Doc {
    const v = json.parse(a, text) catch return .{};
    const obj = switch (v) {
        .object => |o| o,
        else => return .{},
    };
    var doc = Doc{ .returned = true };
    if (json.field(obj, "network")) |n| switch (n) {
        .string => |s| doc.network = s,
        else => {},
    };
    if (json.field(obj, "header")) |h| switch (h) {
        .object => |ho| {
            const hash_ok = if (json.field(ho, "hash")) |hv| switch (hv) {
                .string => |s| s.len == 64,
                else => false,
            } else false;
            const height_ok = if (json.field(ho, "height")) |hv| switch (hv) {
                .number => |num| num >= 0 and num <= profileHeightMax(),
                else => false,
            } else false;
            if (hash_ok and height_ok) {
                const hs = json.field(ho, "hash").?.string;
                var display: [32]u8 = undefined;
                hex.parse(&display, hs) catch return doc;
                var natural: [32]u8 = undefined;
                for (0..32) |i| natural[i] = display[31 - i];
                doc.hash = natural;
                doc.height = @intCast(json.field(ho, "height").?.number);
                doc.header_usable = true;
                if (json.field(ho, "time")) |tv| switch (tv) {
                    .number => |num| if (num >= 0 and num <= 0xffffffff) {
                        doc.time = @intCast(num);
                    },
                    else => {},
                };
            }
        },
        else => {},
    };
    doc.plants = try plantsOf(a, obj);
    return doc;
}

fn profileHeightMax() i64 {
    return (1 << 53) - 1;
}

fn plantsOf(a: std.mem.Allocator, obj: []const json.Field) ![]Plant {
    var list: std.ArrayList(Plant) = .empty;
    errdefer list.deinit(a);
    if (json.field(obj, "plants")) |p| switch (p) {
        .array => |arr| for (arr) |el| switch (el) {
            .object => |o| if (try onePlant(o)) |pl| try list.append(a, pl),
            else => {},
        },
        else => {},
    };
    if (json.field(obj, "plant")) |p| switch (p) {
        .object => |o| if (try onePlant(o)) |pl| try list.append(a, pl),
        else => {},
    };
    return list.toOwnedSlice(a);
}

fn onePlant(o: []const json.Field) !?Plant {
    const root_s = str64(o, "root") orelse return null;
    const txid_s = str64(o, "txid") orelse return null;
    const bh_s = str64(o, "block_hash") orelse return null;
    const vout = u32of(o, "vout") orelse return null;
    const height = u32of(o, "block_height") orelse return null;
    var root_b: [32]u8 = undefined;
    var tx_d: [32]u8 = undefined;
    var bh_d: [32]u8 = undefined;
    hex.parse(&root_b, root_s) catch return null;
    hex.parse(&tx_d, txid_s) catch return null;
    hex.parse(&bh_d, bh_s) catch return null;
    var tx: [32]u8 = undefined;
    var bh: [32]u8 = undefined;
    for (0..32) |i| {
        tx[i] = tx_d[31 - i];
        bh[i] = bh_d[31 - i];
    }
    return .{
        .root = root_b,
        .txid = tx,
        .vout = vout,
        .block_hash = bh,
        .block_height = height,
    };
}

fn str64(o: []const json.Field, name: []const u8) ?[]const u8 {
    const v = json.field(o, name) orelse return null;
    return switch (v) {
        .string => |s| if (s.len == 64) s else null,
        else => null,
    };
}
fn u32of(o: []const json.Field, name: []const u8) ?u32 {
    const v = json.field(o, name) orelse return null;
    return switch (v) {
        .number => |n| if (n >= 0 and n <= 0xffffffff) @intCast(n) else null,
        else => null,
    };
}
