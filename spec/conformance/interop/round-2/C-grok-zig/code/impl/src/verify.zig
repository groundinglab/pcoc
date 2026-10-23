//! SPEC §14 verifier, including Rule A5 and the §18 record byte order.
//! Profile sizes are the verifier's. The object's size fields are shape only.

const std = @import("std");
const profile = @import("profile.zig");
const cbor = @import("cbor.zig");
const terrain = @import("terrain.zig");
const leg = @import("leg.zig");
const hex = @import("hex.zig");
const index = @import("index.zig");
const pathbody = @import("pathbody.zig");
const draw = @import("draw.zig");
const Engine = profile.Engine;

pub const Options = struct {
    profile_name: enum { production, conformance_small },
    prof: profile.Profile,
    index_mode: index.Mode,
    draw_label: ?[]const u8 = null,
    engine: Engine = .fast,
};

const Flags = struct {
    field_set: bool = false,
    stage0_path: bool = false,
    w17_path: bool = false,
    constant: bool = false,
    stages_length: bool = false,
    leg_count: bool = false,
    checkpoint_width: bool = false,
    leg_result_width: bool = false,
    seed_beacon: bool = false,
    last_interrupt: bool = false,
    header_height_order: bool = false,
    header_hash_differ: bool = false,
    height_range: bool = false,

    fn any(self: Flags) bool {
        return self.field_set or self.stage0_path or self.w17_path or self.constant or
            self.stages_length or self.leg_count or self.checkpoint_width or self.leg_result_width or
            self.seed_beacon or self.last_interrupt or self.header_height_order or
            self.header_hash_differ or self.height_range;
    }
};

const Stage = struct {
    beacon: ?[32]u8 = null,
    height: ?u64 = null,
    waypoint: ?[32]u8 = null,
    path: ?[]const u8 = null,
    leg_count: ?u64 = null,
    cp_len: ?usize = null,
    lr_len: ?usize = null,
    checkpoints: ?[][32]u8 = null,
    leg_results: ?[][8]u8 = null,
};

const Obj = struct {
    flags: Flags = .{},
    fill: ?[]const u8 = null,
    slot_bytes: ?u64 = null,
    n_slots: ?u64 = null,
    sample_bytes: ?u64 = null,
    steps_per_leg: ?u64 = null,
    public_key: ?[32]u8 = null,
    seed_beacon: ?[32]u8 = null,
    stages: [18]Stage = undefined,
    present: [18]bool = [_]bool{false} ** 18,
    last_hash: ?[32]u8 = null,
    last_height: ?u64 = null,
};

const Meas = union(enum) {
    tok: []const u8,
    u: u64,
    i: i64,
};

fn nd() Meas {
    return .{ .tok = "not defined" };
}
fn nr() Meas {
    return .{ .tok = "not read" };
}

const Block = struct { hash: [32]u8, height: u64 };

const W = struct {
    b: *std.ArrayList(u8),
    a: std.mem.Allocator,

    fn raw(self: W, s: []const u8) !void {
        try self.b.appendSlice(self.a, s);
    }
    fn ch(self: W, c: u8) !void {
        try self.b.append(self.a, c);
    }
    fn str(self: W, s: []const u8) !void {
        try self.ch('"');
        const hexd = "0123456789ABCDEF";
        for (s) |c| switch (c) {
            '"' => try self.raw("\\\""),
            '\\' => try self.raw("\\\\"),
            '\n' => try self.raw("\\n"),
            '\r' => try self.raw("\\r"),
            '\t' => try self.raw("\\t"),
            else => if (c < 0x20) {
                var tmp = [_]u8{ '\\', 'u', '0', '0', hexd[c >> 4], hexd[c & 0xf] };
                try self.raw(&tmp);
            } else try self.ch(c),
        };
        try self.ch('"');
    }
    fn u(self: W, v: u64) !void {
        var tmp: [32]u8 = undefined;
        const s = std.fmt.bufPrint(&tmp, "{d}", .{v}) catch unreachable;
        try self.raw(s);
    }
    fn i(self: W, v: i64) !void {
        var tmp: [32]u8 = undefined;
        const s = std.fmt.bufPrint(&tmp, "{d}", .{v}) catch unreachable;
        try self.raw(s);
    }
    fn meas(self: W, m: Meas) !void {
        switch (m) {
            .tok => |t| try self.str(t),
            .u => |n| try self.u(n),
            .i => |n| try self.i(n),
        }
    }
    fn hex32(self: W, p: *const [32]u8) !void {
        try self.ch('"');
        const h = hex.of(32, p);
        try self.raw(&h);
        try self.ch('"');
    }
    fn display(self: W, nat: *const [32]u8) !void {
        var d: [32]u8 = undefined;
        for (0..32) |k| d[k] = nat[31 - k];
        try self.hex32(&d);
    }
    fn block(self: W, blk: ?Block) !void {
        if (blk) |x| {
            try self.raw("{\"block_hash\":");
            try self.display(&x.hash);
            try self.raw(",\"block_height\":");
            try self.u(x.height);
            try self.ch('}');
        } else try self.str("not defined");
    }
};

pub fn unknownKindRecord(a: std.mem.Allocator, kind_text: []const u8) ![]u8 {
    var list: std.ArrayList(u8) = .empty;
    errdefer list.deinit(a);
    const w = W{ .b = &list, .a = a };
    try w.raw("{\"kind\":\"unknown kind\",\"kind_text\":");
    try w.str(kind_text);
    try w.ch('}');
    return list.toOwnedSlice(a);
}

fn writeProfile(w: W, opts: Options) !void {
    try w.raw("{\"profile\":");
    if (opts.profile_name == .conformance_small) {
        try w.raw("\"conformance-small\",\"sample_bytes\":");
        try w.u(opts.prof.sample_bytes);
        try w.raw(",\"slot_bytes\":");
        try w.u(opts.prof.slot_bytes);
        try w.raw(",\"terrain_bytes\":");
        try w.u(opts.prof.terrain_bytes);
        try w.raw(",\"steps_per_leg\":");
        try w.u(opts.prof.steps_per_leg);
    } else try w.raw("\"production\"");
}

fn malformedRecord(a: std.mem.Allocator, opts: Options) ![]u8 {
    var list: std.ArrayList(u8) = .empty;
    errdefer list.deinit(a);
    const w = W{ .b = &list, .a = a };
    try writeProfile(w, opts);
    try w.raw(",\"shape\":\"mismatch\",\"shape_item\":[\"malformed cbor\"],\"headers\":[");
    for (0..19) |i| {
        if (i != 0) try w.ch(',');
        try w.raw("{\"stored_hash\":\"not defined\",\"stored_height\":\"not defined\",\"index_comparison\":\"not read\",\"T\":\"not read\"}");
    }
    try w.raw("],\"seed_value\":\"mismatch\",\"checkpoints\":[");
    for (0..18) |g| {
        if (g != 0) try w.ch(',');
        try w.raw("{\"g\":");
        try w.u(g);
        try w.raw(",\"legs\":\"not defined\"}");
    }
    try w.raw("],\"waypoints\":[");
    for (0..17) |k| {
        if (k != 0) try w.ch(',');
        try w.raw("{\"g\":");
        try w.u(k + 1);
        try w.raw(",\"result\":\"not defined\"}");
    }
    try w.raw("],\"paths\":[");
    for (1..17) |g| {
        if (g != 1) try w.ch(',');
        try w.raw("{\"g\":");
        try w.u(g);
        try w.raw(",\"path_state\":\"not defined\",\"plant\":\"not defined\",\"P_g\":\"not defined\",\"height_P_g\":\"not defined\",\"d_g\":\"not defined\",\"claimed_block\":\"not defined\"}");
    }
    try w.raw(",{\"g\":17,\"w17_path\":\"not defined\",\"claimed_block\":\"not defined\"}]");
    try w.raw(",\"anchors\":[");
    for (0..16) |i| {
        if (i != 0) try w.ch(',');
        try w.raw("\"not anchored\"");
    }
    try w.raw("],\"anchored_total\":0,\"skip_list\":[],\"a\":\"not defined\",\"b\":\"not defined\",\"N\":\"not defined\",\"S\":\"not defined\",\"delta_h\":\"not defined\"");
    try w.raw(",\"fixed_start\":{\"N\":\"not defined\",\"S\":\"not defined\",\"delta_h\":\"not defined\"}");
    try w.raw(",\"P_b\":\"not defined\",\"height_P_b\":\"not defined\",\"index_comparison_P_b\":\"not defined\",\"T_P_b\":\"not defined\"");
    try w.raw(",\"height_I_a_minus_1\":\"not defined\",\"T_I_a_minus_1\":\"not defined\"");
    try w.raw(",\"six_leg\":\"not defined\",\"six_leg_runs\":[]");
    try w.raw(",\"analysis\":{\"whole_hike_legs\":\"not defined\",\"whole_hike_span\":\"not read\",\"per_stage\":[");
    for (0..18) |g| {
        if (g != 0) try w.ch(',');
        try w.raw("{\"g\":");
        try w.u(g);
        try w.raw(",\"checkpoint_count\":\"not defined\",\"span\":\"not read\"}");
    }
    try w.raw("],\"legs_outside\":\"not defined\",\"hike_legs\":\"not defined\",\"hike_span\":\"not read\"}}");
    return list.toOwnedSlice(a);
}

fn scan(v: cbor.Value, over: *bool) void {
    switch (v) {
        .uint => |n| {
            if (n > profile.HEIGHT_MAX) over.* = true;
        },
        .array => |arr| for (arr) |e| scan(e, over),
        .map => |es| for (es) |e| scan(e.val, over),
        else => {},
    }
}

fn known(key: []const u8, keys: []const []const u8) bool {
    for (keys) |k| if (std.mem.eql(u8, k, key)) return true;
    return false;
}

fn takeUint(map: []const cbor.Entry, key: []const u8, flags: *Flags) ?u64 {
    const v = cbor.get(map, key) orelse {
        flags.field_set = true;
        return null;
    };
    switch (v) {
        .uint => |n| {
            if (n > profile.HEIGHT_MAX) {
                flags.height_range = true;
                return null;
            }
            return n;
        },
        else => {
            flags.field_set = true;
            return null;
        },
    }
}

fn take32(map: []const cbor.Entry, key: []const u8, flags: *Flags) ?[32]u8 {
    const v = cbor.get(map, key) orelse {
        flags.field_set = true;
        return null;
    };
    switch (v) {
        .bytes => |b| {
            if (b.len != 32) {
                flags.field_set = true;
                return null;
            }
            var o: [32]u8 = undefined;
            @memcpy(&o, b[0..32]);
            return o;
        },
        else => {
            flags.field_set = true;
            return null;
        },
    }
}

fn parseStage(map: []const cbor.Entry, g: usize, aa: std.mem.Allocator, flags: *Flags) !Stage {
    var st = Stage{};
    const base = [_][]const u8{ "beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results" };
    if (g == 0) {
        if (cbor.get(map, "path") != null) {
            flags.stage0_path = true;
            flags.field_set = true;
        }
        for (map) |e| {
            if (!known(e.key, &base)) flags.field_set = true;
        }
    } else {
        var keys: [7][]const u8 = undefined;
        keys[0] = "path";
        for (base, 0..) |k, i| keys[i + 1] = k;
        for (map) |e| {
            if (!known(e.key, &keys)) flags.field_set = true;
        }
        if (cbor.get(map, "path")) |v| switch (v) {
            .bytes => |b| st.path = b,
            else => flags.field_set = true,
        } else flags.field_set = true;
    }
    st.beacon = take32(map, "beacon", flags);
    st.height = takeUint(map, "beacon_height", flags);
    st.waypoint = take32(map, "waypoint", flags);
    st.leg_count = takeUint(map, "leg_count", flags);
    if (cbor.get(map, "checkpoints")) |v| switch (v) {
        .array => |arr| {
            st.cp_len = arr.len;
            const cps = try aa.alloc([32]u8, arr.len);
            var bad = false;
            for (arr, 0..) |el, i| switch (el) {
                .bytes => |b| if (b.len == 32) {
                    @memcpy(&cps[i], b[0..32]);
                } else {
                    bad = true;
                    flags.checkpoint_width = true;
                },
                else => {
                    bad = true;
                    flags.checkpoint_width = true;
                    flags.field_set = true;
                },
            };
            if (!bad) st.checkpoints = cps;
        },
        else => {
            flags.field_set = true;
            flags.checkpoint_width = true;
        },
    } else flags.field_set = true;
    if (cbor.get(map, "leg_results")) |v| switch (v) {
        .array => |arr| {
            st.lr_len = arr.len;
            const lrs = try aa.alloc([8]u8, arr.len);
            var bad = false;
            for (arr, 0..) |el, i| switch (el) {
                .bytes => |b| if (b.len == 8) {
                    @memcpy(&lrs[i], b[0..8]);
                } else {
                    bad = true;
                    flags.leg_result_width = true;
                },
                else => {
                    bad = true;
                    flags.leg_result_width = true;
                    flags.field_set = true;
                },
            };
            if (!bad) st.leg_results = lrs;
        },
        else => {
            flags.field_set = true;
            flags.leg_result_width = true;
        },
    } else flags.field_set = true;
    return st;
}

fn load(aa: std.mem.Allocator, map: []const cbor.Entry, prof: profile.Profile) !Obj {
    var obj = Obj{};
    for (&obj.stages) |*s| s.* = .{};
    const top = [_][]const u8{
        "fill", "kind", "stages", "n_slots", "public_key", "slot_bytes", "seed_beacon",
        "sample_bytes", "steps_per_leg", "last_interrupt", "last_interrupt_height",
    };
    for (map) |e| {
        if (!known(e.key, &top)) obj.flags.field_set = true;
        scan(e.val, &obj.flags.height_range);
    }
    if (cbor.get(map, "fill")) |v| switch (v) {
        .text => |t| obj.fill = t,
        else => obj.flags.field_set = true,
    } else obj.flags.field_set = true;
    obj.slot_bytes = takeUint(map, "slot_bytes", &obj.flags);
    obj.n_slots = takeUint(map, "n_slots", &obj.flags);
    obj.sample_bytes = takeUint(map, "sample_bytes", &obj.flags);
    obj.steps_per_leg = takeUint(map, "steps_per_leg", &obj.flags);
    obj.public_key = take32(map, "public_key", &obj.flags);
    obj.seed_beacon = take32(map, "seed_beacon", &obj.flags);
    obj.last_hash = take32(map, "last_interrupt", &obj.flags);
    obj.last_height = takeUint(map, "last_interrupt_height", &obj.flags);
    if (cbor.get(map, "stages")) |v| switch (v) {
        .array => |arr| {
            if (arr.len != 18) obj.flags.stages_length = true;
            const n = @min(arr.len, 18);
            for (arr[0..n], 0..) |el, g| switch (el) {
                .map => |sm| {
                    obj.present[g] = true;
                    obj.stages[g] = try parseStage(sm, g, aa, &obj.flags);
                },
                else => obj.flags.field_set = true,
            };
        },
        else => {
            obj.flags.field_set = true;
            obj.flags.stages_length = true;
        },
    } else {
        obj.flags.field_set = true;
        obj.flags.stages_length = true;
    }

    const fill_ok = if (obj.fill) |f| std.mem.eql(u8, f, profile.FILL) else false;
    const slot_ok = if (obj.slot_bytes) |n| n == prof.slot_bytes else false;
    const ns_ok = if (obj.n_slots) |n| n == prof.nSlots() else false;
    const sample_ok = if (obj.sample_bytes) |n| n == prof.sample_bytes else false;
    const steps_ok = if (obj.steps_per_leg) |n| n == prof.steps_per_leg else false;
    if (!(fill_ok and slot_ok and ns_ok and sample_ok and steps_ok)) obj.flags.constant = true;

    var heads: [19]struct { hash: ?[32]u8, height: ?u64 } = undefined;
    for (0..18) |g| heads[g] = .{ .hash = obj.stages[g].beacon, .height = obj.stages[g].height };
    heads[18] = .{ .hash = obj.last_hash, .height = obj.last_height };
    for (0..18) |i| {
        const x = heads[i];
        const y = heads[i + 1];
        if (x.hash == null or y.hash == null or std.mem.eql(u8, &x.hash.?, &y.hash.?)) obj.flags.header_hash_differ = true;
        if (x.height == null or y.height == null or !(y.height.? > x.height.?)) obj.flags.header_height_order = true;
    }
    const s17h = obj.stages[17].beacon;
    const s17t = obj.stages[17].height;
    const li_ok = obj.last_hash != null and s17h != null and !std.mem.eql(u8, &obj.last_hash.?, &s17h.?) and
        obj.last_height != null and s17t != null and obj.last_height.? > s17t.?;
    if (!li_ok) obj.flags.last_interrupt = true;
    const b0 = obj.stages[0].beacon;
    if (b0 == null or obj.seed_beacon == null or !std.mem.eql(u8, &b0.?, &obj.seed_beacon.?)) obj.flags.seed_beacon = true;

    for (0..18) |g| {
        if (!obj.present[g]) {
            obj.flags.leg_count = true;
            obj.flags.checkpoint_width = true;
            obj.flags.leg_result_width = true;
            if (g == 17) obj.flags.w17_path = true;
            continue;
        }
        const st = obj.stages[g];
        if (st.leg_count == null or st.cp_len == null or st.lr_len == null or st.leg_count.? < 1 or
            st.leg_count.? != st.cp_len.? or st.leg_count.? != st.lr_len.?)
            obj.flags.leg_count = true;
        if (st.cp_len == null) obj.flags.checkpoint_width = true;
        if (st.lr_len == null) obj.flags.leg_result_width = true;
        if (g == 17 and (st.path == null or st.path.?.len != 0)) obj.flags.w17_path = true;
    }
    return obj;
}

fn eql32(a: ?[32]u8, b: [32]u8) bool {
    return if (a) |x| std.mem.eql(u8, &x, &b) else false;
}

fn isInterrupt(obj: *const Obj, height: u64, hash: [32]u8) bool {
    for (1..18) |g| {
        if (obj.stages[g].height == height and eql32(obj.stages[g].beacon, hash)) return true;
    }
    return obj.last_height == height and eql32(obj.last_hash, hash);
}

const Hdr = struct { cmp: []const u8, t: ?u64, conf: ?u64 };

fn readHeader(ix: *index.Index, hash: ?[32]u8, height: ?u64) !Hdr {
    const hsh = hash orelse return .{ .cmp = "not read", .t = null, .conf = null };
    const ht = height orelse return .{ .cmp = "not read", .t = null, .conf = null };
    const doc = try ix.get(ht);
    if (!doc.returned or !doc.header_usable or doc.hash == null or doc.height == null)
        return .{ .cmp = "not read", .t = null, .conf = null };
    if (!std.mem.eql(u8, &doc.hash.?, &hsh) or doc.height.? != ht)
        return .{ .cmp = "mismatch", .t = null, .conf = null };
    return .{ .cmp = "match", .t = doc.time, .conf = doc.height };
}

fn sumLens(obj: *const Obj, lo: usize, hi: usize) ?u64 {
    var s: u64 = 0;
    var g = lo;
    while (g < hi) : (g += 1) {
        const cps = obj.stages[g].checkpoints orelse return null;
        s += @intCast(cps.len);
    }
    return s;
}

fn mtp(ix: *index.Index, h: u64, own: ?u64) !?u64 {
    const n: u64 = @min(@as(u64, 11), h + 1);
    var times: [11]u64 = undefined;
    var i: u64 = 0;
    while (i < n) : (i += 1) {
        const height = h - n + 1 + i;
        if (height == h) {
            times[@intCast(i)] = own orelse return null;
        } else {
            const doc = try ix.get(height);
            if (!doc.returned or !doc.header_usable or doc.height == null or doc.height.? != height or doc.time == null)
                return null;
            times[@intCast(i)] = doc.time.?;
        }
    }
    std.mem.sort(u64, times[0..@intCast(n)], {}, struct {
        fn less(_: void, x: u64, y: u64) bool {
            return x < y;
        }
    }.less);
    return times[@intCast(n / 2)];
}

const PathOut = struct {
    state: []const u8 = "not defined",
    plant: []const u8 = "not defined",
    claimed: ?Block = null,
    pg: ?Block = null,
    height_p: ?u64 = null,
    d: ?i64 = null,
    valid: bool = false,
    anchored: bool = false,
    issued_miss: bool = false,
    body_hash: ?[32]u8 = null,
    body_height: ?u64 = null,
};

fn plantMatch(doc: index.Doc, body: pathbody.Body) bool {
    const net = doc.network orelse return false;
    if (!std.mem.eql(u8, net, "main") and !std.mem.eql(u8, net, "synthetic")) return false;
    for (doc.plants) |p| {
        if (std.mem.eql(u8, &p.root, &body.root) and std.mem.eql(u8, &p.txid, &body.txid) and
            p.vout == body.vout and std.mem.eql(u8, &p.block_hash, &body.block_hash) and
            p.block_height == body.block_height) return true;
    }
    return false;
}

const Book = struct {
    a: Meas = nd(),
    b: Meas = nd(),
    N: Meas = nd(),
    S: Meas = nd(),
    delta_h: Meas = nd(),
    pb: ?Block = null,
    pb_tok: ?[]const u8 = "not defined",
    height_pb: Meas = nd(),
    cmp_pb: Meas = nd(),
    t_pb: Meas = nd(),
    height_i: Meas = nd(),
    t_i: Meas = nd(),
    fs_n: Meas = nd(),
    fs_s: Meas = nd(),
    fs_d: Meas = nd(),
    outside: Meas = nd(),
};

fn allBook(tok: []const u8) Book {
    const m = Meas{ .tok = tok };
    return .{
        .a = m,
        .b = m,
        .N = m,
        .S = m,
        .delta_h = m,
        .pb = null,
        .pb_tok = tok,
        .height_pb = m,
        .cmp_pb = m,
        .t_pb = m,
        .height_i = m,
        .t_i = m,
        .fs_n = m,
        .fs_s = m,
        .fs_d = m,
        .outside = m,
    };
}

fn subU(x: ?u64, y: ?u64) ?i64 {
    if (x == null or y == null) return null;
    return @as(i64, @intCast(x.?)) - @as(i64, @intCast(y.?));
}

fn fullRecord(a: std.mem.Allocator, aa: std.mem.Allocator, map: []const cbor.Entry, opts: Options) ![]u8 {
    const obj = try load(aa, map, opts.prof);
    var derived: ?terrain.Seed = null;
    if (obj.public_key != null and obj.seed_beacon != null)
        derived = terrain.derive(obj.public_key.?, obj.seed_beacon.?, opts.engine);
    const seed_bytes: ?[32]u8 = if (derived) |d| d.d else null;
    var words: ?[]align(64) u64 = null;
    if (derived) |d| words = try terrain.fill(a, d.p, opts.prof, opts.engine);
    defer if (words) |w| a.free(w);
    const tbytes: ?[]const u8 = if (words) |w| std.mem.sliceAsBytes(w) else null;

    const seed_value: []const u8 = if (seed_bytes != null and obj.stages[0].waypoint != null and
        std.mem.eql(u8, &seed_bytes.?, &obj.stages[0].waypoint.?)) "match" else "mismatch";

    var ix = index.Index.init(a, opts.index_mode);
    defer ix.deinit();
    var hdrs: [19]Hdr = undefined;
    for (0..18) |g| hdrs[g] = try readHeader(&ix, obj.stages[g].beacon, obj.stages[g].height);
    hdrs[18] = try readHeader(&ix, obj.last_hash, obj.last_height);

    var leg_tok: [18][]const []const u8 = undefined;
    var leg_def: [18]bool = undefined;
    for (0..18) |g| {
        const st = obj.stages[g];
        const cps = st.checkpoints orelse {
            leg_def[g] = false;
            leg_tok[g] = &.{};
            continue;
        };
        leg_def[g] = true;
        const toks = try aa.alloc([]const u8, cps.len);
        const limb = if (g == 0) seed_bytes else st.waypoint;
        for (cps, 0..) |cp, j| {
            const prev: ?[32]u8 = if (j == 0) limb else if (j - 1 < cps.len) cps[j - 1] else null;
            const lr = if (st.leg_results) |lrs| (if (j < lrs.len) lrs[j] else null) else null;
            if (prev == null or limb == null or lr == null or tbytes == null) {
                toks[j] = "mismatch";
                continue;
            }
            const pv = prev.?;
            const lm = limb.?;
            const samp = leg.sample(tbytes.?, &pv, opts.prof);
            const ru = std.mem.readInt(u64, &lr.?, .little);
            const got = leg.checkpoint(&pv, &lm, samp, ru, opts.engine);
            toks[j] = if (std.mem.eql(u8, &got, &cp)) "match" else "mismatch";
        }
        leg_tok[g] = toks;
    }

    var wp: [17][]const u8 = undefined;
    for (0..17) |g| {
        const st = obj.stages[g];
        const nx = obj.stages[g + 1];
        const cps = st.checkpoints;
        if (cps == null or st.waypoint == null or nx.beacon == null or nx.waypoint == null) {
            wp[g] = "not defined";
            continue;
        }
        if (words == null) {
            wp[g] = "mismatch";
            continue;
        }
        const fold = leg.checkpointFold(cps.?, opts.engine);
        const slot = leg.slotSample(words.?, opts.prof.wordsPerSlot(), &nx.beacon.?, opts.prof.nSlots());
        const dig = leg.waypoint(&nx.beacon.?, slot, &st.waypoint.?, &fold, opts.engine);
        wp[g] = if (std.mem.eql(u8, &dig, &nx.waypoint.?)) "match" else "mismatch";
    }

    var paths: [16]PathOut = [_]PathOut{.{}} ** 16;
    for (1..17) |g| {
        var po = PathOut{};
        const st = obj.stages[g];
        if (!obj.present[g] or st.path == null) {
            po.state = "not defined";
            po.plant = "not defined";
            paths[g - 1] = po;
            continue;
        }
        const pb = st.path.?;
        if (pb.len == 0) {
            po.state = "skip";
            po.plant = "n/a";
            paths[g - 1] = po;
            continue;
        }
        po.state = "present";
        const body_opt = try pathbody.decode(pb, aa);
        const wpnt = st.waypoint;
        if (body_opt == null or wpnt == null or !pathbody.validateFor(body_opt.?, &wpnt.?)) {
            po.plant = "mismatch";
            paths[g - 1] = po;
            continue;
        }
        const body = body_opt.?;
        po.valid = true;
        po.body_hash = body.block_hash;
        po.body_height = body.block_height;
        po.claimed = .{ .hash = body.block_hash, .height = body.block_height };
        const in_window = if (st.height) |h| leg.anchorOffset(h, body.block_height) else false;
        if (!in_window or !isInterrupt(&obj, body.block_height, body.block_hash)) {
            po.plant = "not read";
            paths[g - 1] = po;
            continue;
        }
        const doc = try ix.get(body.block_height);
        if (!doc.returned) {
            po.plant = "not read";
            po.issued_miss = true;
            paths[g - 1] = po;
            continue;
        }
        if (plantMatch(doc, body)) {
            po.plant = "match";
            po.anchored = true;
            po.pg = po.claimed;
            po.height_p = body.block_height;
            if (std.mem.eql(u8, hdrs[g].cmp, "match")) po.d = subU(po.height_p, hdrs[g].conf);
        } else po.plant = "mismatch";
        paths[g - 1] = po;
    }

    var w17_tok: []const u8 = "not defined";
    var w17_claim: ?Block = null;
    if (obj.present[17] and obj.stages[17].path != null and obj.stages[17].path.?.len == 0) {
        w17_tok = "not fetched by design";
    } else if (obj.present[17] and obj.stages[17].path != null and obj.stages[17].path.?.len != 0) {
        if (pathbody.claimedFields(obj.stages[17].path.?)) |c| w17_claim = .{ .hash = c.hash, .height = c.height };
    }

    var n_anch: u64 = 0;
    var n_valid: usize = 0;
    var miss = false;
    var a_i: ?usize = null;
    var b_i: ?usize = null;
    for (paths, 0..) |p, i| {
        if (p.anchored) {
            n_anch += 1;
            if (a_i == null) a_i = i + 1;
            b_i = i + 1;
        }
        if (p.valid) n_valid += 1;
        if (p.valid and p.issued_miss) miss = true;
    }

    var book: Book = undefined;
    if (n_valid < 2) {
        book = allBook("not defined");
    } else if (miss) {
        book = allBook("not read");
    } else if (n_anch < 2) {
        book = allBook("not defined");
    } else {
        const ai = a_i.?;
        const bi = b_i.?;
        const pend = paths[bi - 1];
        if (!std.mem.eql(u8, hdrs[ai].cmp, "not read") and pend.body_hash != null and pend.body_height != null) {
            const docp = try ix.get(pend.body_height.?);
            const successful = docp.returned and docp.header_usable and docp.hash != null and docp.height != null;
            if (!docp.returned or !successful) {
                book = allBook("not read");
            } else {
                const match_pb = std.mem.eql(u8, &docp.hash.?, &pend.body_hash.?) and docp.height.? == pend.body_height.?;
                book = .{};
                book.a = .{ .u = ai };
                book.b = .{ .u = bi };
                book.pb = pend.claimed;
                book.pb_tok = null;
                if (sumLens(&obj, ai, bi)) |n| book.N = .{ .u = n };
                book.cmp_pb = .{ .tok = if (match_pb) "match" else "mismatch" };
                if (match_pb) {
                    book.height_pb = .{ .u = docp.height.? };
                    if (docp.time) |t| book.t_pb = .{ .u = t } else book.t_pb = nr();
                } else {
                    book.height_pb = nd();
                    book.t_pb = nr();
                }
                if (std.mem.eql(u8, hdrs[ai].cmp, "match")) {
                    if (hdrs[ai].conf) |h| book.height_i = .{ .u = h };
                    if (hdrs[ai].t) |t| book.t_i = .{ .u = t } else book.t_i = nr();
                } else {
                    book.height_i = nd();
                    book.t_i = if (std.mem.eql(u8, hdrs[ai].cmp, "not read")) nr() else nr();
                }
                const tpb: ?u64 = switch (book.t_pb) {
                    .u => |n| n,
                    else => null,
                };
                const ti: ?u64 = switch (book.t_i) {
                    .u => |n| n,
                    else => null,
                };
                if (subU(tpb, ti)) |d| book.S = .{ .i = d } else book.S = nr();
                const hpb: ?u64 = switch (book.height_pb) {
                    .u => |n| n,
                    else => null,
                };
                const hi: ?u64 = switch (book.height_i) {
                    .u => |n| n,
                    else => null,
                };
                if (subU(hpb, hi)) |d| book.delta_h = .{ .i = d } else book.delta_h = nd();
                if (sumLens(&obj, 1, bi)) |n| book.fs_n = .{ .u = n };
                const t1: ?u64 = hdrs[1].t;
                if (subU(tpb, t1)) |d| book.fs_s = .{ .i = d } else book.fs_s = nr();
                if (std.mem.eql(u8, hdrs[1].cmp, "not read")) {
                    book.fs_d = nr();
                } else if (match_pb and std.mem.eql(u8, hdrs[1].cmp, "match")) {
                    if (subU(docp.height, hdrs[1].conf)) |d| book.fs_d = .{ .i = d } else book.fs_d = nd();
                } else book.fs_d = nd();
                if (sumLens(&obj, 0, 18)) |_| {
                    var outc: u64 = 0;
                    for (0..18) |g| {
                        if (g >= ai and g < bi) continue;
                        outc += @intCast(obj.stages[g].checkpoints.?.len);
                    }
                    book.outside = .{ .u = outc };
                }
            }
        } else book = allBook("not read");
    }

    const whole = sumLens(&obj, 1, 18);
    const hike = sumLens(&obj, 0, 18);
    var spans: [18]Meas = undefined;
    var counts: [18]Meas = undefined;
    for (0..18) |g| {
        if (obj.stages[g].checkpoints) |c| counts[g] = .{ .u = c.len } else counts[g] = nd();
        if (subU(hdrs[g + 1].t, hdrs[g].t)) |d| spans[g] = .{ .i = d } else spans[g] = nr();
    }
    const whole_span: Meas = if (subU(hdrs[18].t, hdrs[1].t)) |d| .{ .i = d } else nr();
    var hike_span: Meas = nr();
    if (obj.stages[0].height != null and obj.last_height != null) {
        const m0 = try mtp(&ix, obj.stages[0].height.?, hdrs[0].t);
        const m1 = try mtp(&ix, obj.last_height.?, hdrs[18].t);
        if (subU(m1, m0)) |d| hike_span = .{ .i = d };
    }

    var six_def = false;
    var six_read = false;
    var indices: [6]u64 = undefined;
    var results: [6][]const u8 = undefined;
    if (hike) |total| {
        if (total >= 6) {
            six_def = true;
            if (opts.draw_label) |lab| {
                indices = draw.sixLeg(lab, total);
                six_read = true;
            } else if (draw.sixLegOs(total)) |xs| {
                indices = xs;
                six_read = true;
            } else |_| {}
            if (six_read) {
                var counts18: [18]u64 = undefined;
                for (0..18) |g| counts18[g] = obj.stages[g].checkpoints.?.len;
                for (indices, 0..) |q, n| {
                    var start: u64 = 0;
                    var found = false;
                    for (counts18, 0..) |cn, g| {
                        if (q < start + cn) {
                            const j: usize = @intCast(q - start);
                            const prev: ?[32]u8 = if (j == 0)
                                (if (g == 0) seed_bytes else obj.stages[g].waypoint)
                            else
                                obj.stages[g].checkpoints.?[j - 1];
                            const stored: ?u64 = if (obj.stages[g].leg_results) |lrs|
                                (if (j < lrs.len) std.mem.readInt(u64, &lrs[j], .little) else null)
                            else
                                null;
                            if (prev == null or words == null or stored == null) {
                                results[n] = "mismatch";
                            } else {
                                const pv = prev.?;
                                const got = leg.legResult(words.?, opts.prof, &pv, opts.engine);
                                results[n] = if (got == stored.?) "match" else "mismatch";
                            }
                            found = true;
                            break;
                        }
                        start += cn;
                    }
                    if (!found) results[n] = "mismatch";
                }
            }
        }
    }

    var list: std.ArrayList(u8) = .empty;
    errdefer list.deinit(a);
    const w = W{ .b = &list, .a = a };
    try writeProfile(w, opts);
    try w.raw(",\"shape\":");
    try w.str(if (obj.flags.any()) "mismatch" else "match");
    try w.raw(",\"shape_item\":[");
    var first_item = true;
    const items = [_]struct { on: bool, s: []const u8 }{
        .{ .on = obj.flags.field_set, .s = "field set" },
        .{ .on = obj.flags.stage0_path, .s = "stage-0 path key" },
        .{ .on = obj.flags.w17_path, .s = "W_17 path" },
        .{ .on = obj.flags.constant, .s = "constant" },
        .{ .on = obj.flags.stages_length, .s = "stages length" },
        .{ .on = obj.flags.leg_count, .s = "leg_count" },
        .{ .on = obj.flags.checkpoint_width, .s = "checkpoint width" },
        .{ .on = obj.flags.leg_result_width, .s = "leg result width" },
        .{ .on = obj.flags.seed_beacon, .s = "seed beacon" },
        .{ .on = obj.flags.last_interrupt, .s = "last_interrupt" },
        .{ .on = obj.flags.header_height_order, .s = "header height order" },
        .{ .on = obj.flags.header_hash_differ, .s = "header hash differ" },
        .{ .on = obj.flags.height_range, .s = "height_range" },
    };
    for (items) |it| if (it.on) {
        if (!first_item) try w.ch(',');
        first_item = false;
        try w.str(it.s);
    };
    try w.raw("],\"headers\":[");
    for (0..19) |i| {
        if (i != 0) try w.ch(',');
        const hash = if (i < 18) obj.stages[i].beacon else obj.last_hash;
        const ht = if (i < 18) obj.stages[i].height else obj.last_height;
        try w.raw("{\"stored_hash\":");
        if (hash) |h| try w.hex32(&h) else try w.str("not defined");
        try w.raw(",\"stored_height\":");
        if (ht) |h| try w.u(h) else try w.str("not defined");
        try w.raw(",\"index_comparison\":");
        try w.str(hdrs[i].cmp);
        try w.raw(",\"T\":");
        if (hdrs[i].t) |t| try w.u(t) else try w.str("not read");
        try w.ch('}');
    }
    try w.raw("],\"seed_value\":");
    try w.str(seed_value);
    try w.raw(",\"checkpoints\":[");
    for (0..18) |g| {
        if (g != 0) try w.ch(',');
        try w.raw("{\"g\":");
        try w.u(g);
        try w.raw(",\"legs\":");
        if (!leg_def[g]) try w.str("not defined") else {
            try w.ch('[');
            for (leg_tok[g], 0..) |tok, j| {
                if (j != 0) try w.ch(',');
                try w.raw("{\"j\":");
                try w.u(j);
                try w.raw(",\"result\":");
                try w.str(tok);
                try w.ch('}');
            }
            try w.ch(']');
        }
        try w.ch('}');
    }
    try w.raw("],\"waypoints\":[");
    for (wp, 0..) |tok, k| {
        if (k != 0) try w.ch(',');
        try w.raw("{\"g\":");
        try w.u(k + 1);
        try w.raw(",\"result\":");
        try w.str(tok);
        try w.ch('}');
    }
    try w.raw("],\"paths\":[");
    for (paths, 0..) |p, i| {
        if (i != 0) try w.ch(',');
        try w.raw("{\"g\":");
        try w.u(i + 1);
        try w.raw(",\"path_state\":");
        try w.str(p.state);
        try w.raw(",\"plant\":");
        try w.str(p.plant);
        try w.raw(",\"P_g\":");
        try w.block(p.pg);
        try w.raw(",\"height_P_g\":");
        if (p.height_p) |h| try w.u(h) else try w.str("not defined");
        try w.raw(",\"d_g\":");
        if (p.d) |d| try w.i(d) else try w.str("not defined");
        try w.raw(",\"claimed_block\":");
        try w.block(p.claimed);
        try w.ch('}');
    }
    try w.raw(",{\"g\":17,\"w17_path\":");
    try w.str(w17_tok);
    try w.raw(",\"claimed_block\":");
    try w.block(w17_claim);
    try w.raw("}],\"anchors\":[");
    for (paths, 0..) |p, i| {
        if (i != 0) try w.ch(',');
        try w.str(if (p.anchored) "anchored" else "not anchored");
    }
    try w.raw("],\"anchored_total\":");
    try w.u(n_anch);
    try w.raw(",\"skip_list\":[");
    var sk = false;
    for (paths, 0..) |p, i| if (std.mem.eql(u8, p.state, "skip")) {
        if (sk) try w.ch(',');
        sk = true;
        try w.u(i + 1);
    };
    try w.raw("],\"a\":");
    try w.meas(book.a);
    try w.raw(",\"b\":");
    try w.meas(book.b);
    try w.raw(",\"N\":");
    try w.meas(book.N);
    try w.raw(",\"S\":");
    try w.meas(book.S);
    try w.raw(",\"delta_h\":");
    try w.meas(book.delta_h);
    try w.raw(",\"fixed_start\":{\"N\":");
    try w.meas(book.fs_n);
    try w.raw(",\"S\":");
    try w.meas(book.fs_s);
    try w.raw(",\"delta_h\":");
    try w.meas(book.fs_d);
    try w.raw("},\"P_b\":");
    if (book.pb_tok) |t| try w.str(t) else try w.block(book.pb);
    try w.raw(",\"height_P_b\":");
    try w.meas(book.height_pb);
    try w.raw(",\"index_comparison_P_b\":");
    try w.meas(book.cmp_pb);
    try w.raw(",\"T_P_b\":");
    try w.meas(book.t_pb);
    try w.raw(",\"height_I_a_minus_1\":");
    try w.meas(book.height_i);
    try w.raw(",\"T_I_a_minus_1\":");
    try w.meas(book.t_i);
    try w.raw(",\"six_leg\":");
    if (!six_def) {
        try w.str("not defined");
        try w.raw(",\"six_leg_runs\":[]");
    } else if (!six_read) {
        try w.str("not read");
        try w.raw(",\"six_leg_runs\":\"not read\"");
    } else {
        try emitSix(w, &indices, &results);
        try w.raw(",\"six_leg_runs\":[");
        try emitSix(w, &indices, &results);
        try w.ch(']');
    }
    try w.raw(",\"analysis\":{\"whole_hike_legs\":");
    if (whole) |n| try w.u(n) else try w.str("not defined");
    try w.raw(",\"whole_hike_span\":");
    try w.meas(whole_span);
    try w.raw(",\"per_stage\":[");
    for (0..18) |g| {
        if (g != 0) try w.ch(',');
        try w.raw("{\"g\":");
        try w.u(g);
        try w.raw(",\"checkpoint_count\":");
        try w.meas(counts[g]);
        try w.raw(",\"span\":");
        try w.meas(spans[g]);
        try w.ch('}');
    }
    try w.raw("],\"legs_outside\":");
    try w.meas(book.outside);
    try w.raw(",\"hike_legs\":");
    if (hike) |n| try w.u(n) else try w.str("not defined");
    try w.raw(",\"hike_span\":");
    try w.meas(hike_span);
    try w.raw("}}");
    return list.toOwnedSlice(a);
}

fn emitSix(w: W, indices: *const [6]u64, results: *const [6][]const u8) !void {
    try w.raw("{\"indices\":[");
    for (indices, 0..) |q, i| {
        if (i != 0) try w.ch(',');
        try w.u(q);
    }
    try w.raw("],\"results\":[");
    for (results, 0..) |r, i| {
        if (i != 0) try w.ch(',');
        try w.str(r);
    }
    try w.raw("]}");
}

pub fn record(a: std.mem.Allocator, bytes: []const u8, opts: Options) ![]u8 {
    if (!profile.profileOk(opts.prof)) return error.InvalidProfile;
    var arena_state = std.heap.ArenaAllocator.init(a);
    defer arena_state.deinit();
    const aa = arena_state.allocator();
    const parsed = cbor.decode(aa, bytes) catch |e| switch (e) {
        error.Malformed => return malformedRecord(a, opts),
        else => |err| return err,
    };
    const map = switch (parsed) {
        .map => |m| m,
        else => return error.DecodeFailure,
    };
    const kindv = cbor.get(map, "kind") orelse return error.DecodeFailure;
    const kind = switch (kindv) {
        .text => |t| t,
        else => return error.DecodeFailure,
    };
    if (!std.mem.eql(u8, kind, profile.KIND)) return unknownKindRecord(a, kind);
    return fullRecord(a, aa, map, opts);
}

test "unknown kind and decode order" {
    const a = std.testing.allocator;
    const opts = Options{
        .profile_name = .production,
        .prof = profile.production,
        .index_mode = .{ .none = {} },
        .engine = .fast,
    };
    const mal = try record(a, "", opts);
    defer a.free(mal);
    try std.testing.expect(std.mem.indexOf(u8, mal, "\"shape_item\":[\"malformed cbor\"]") != null);
    try std.testing.expectError(error.DecodeFailure, record(a, &[_]u8{0xa0}, opts));
    try std.testing.expectError(error.DecodeFailure, record(a, &[_]u8{0x00}, opts));

    var list: std.ArrayList(u8) = .empty;
    defer list.deinit(a);
    const cw = cbor.Writer{ .buf = &list, .a = a };
    try cw.map(1);
    try cw.text("kind");
    try cw.text("nope");
    const rec = try record(a, list.items, opts);
    defer a.free(rec);
    try std.testing.expectEqualStrings("{\"kind\":\"unknown kind\",\"kind_text\":\"nope\"}", rec);
}

test "o1 conformance-small record" {
    const producer = @import("producer.zig");
    const fixture = @import("fixture.zig");
    const a = std.testing.allocator;
    const legs = [_]u32{1} ** 18;
    const bytes = try producer.produce(a, fixture.pkA(), profile.s1, legs, .fast);
    defer a.free(bytes);
    const rec = try record(a, bytes, .{
        .profile_name = .conformance_small,
        .prof = profile.s1,
        .index_mode = .fixture,
        .draw_label = "pcoc-pack-2.2/draw",
        .engine = .fast,
    });
    defer a.free(rec);
    if (std.mem.indexOf(u8, rec, "mismatch") != null or std.mem.indexOf(u8, rec, "not read") != null) {
        std.debug.print("\n{s}\n", .{rec});
        return error.UnexpectedToken;
    }
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"shape\":\"match\"") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"shape_item\":[]") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"hike_span\":10800") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"whole_hike_span\":10200") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"whole_hike_legs\":17") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"hike_legs\":18") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"anchored_total\":0") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"skip_list\":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16]") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"results\":[\"match\",\"match\",\"match\",\"match\",\"match\",\"match\"]") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"sample_bytes\":32") != null);
    try std.testing.expect(std.mem.indexOf(u8, rec, "\"T\":1700600000") != null);
}
