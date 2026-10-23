//! Deterministic CASES.md candidate files. Negatives are imported only here.

const std = @import("std");
const hike = @import("hike");
const negatives = @import("negatives.zig");

const profile = hike.profile;
const terrain = hike.terrain;
const leg = hike.leg;
const hex = hike.hex;
const fixture = hike.fixture;
const producer = hike.producer;
const verify = hike.verify;
const draw = hike.draw;
const hashcash = hike.hashcash;
const rate = hike.rate;
const Engine = profile.Engine;

fn less(_: void, a: []u8, b: []u8) bool {
    return std.mem.lessThan(u8, a, b);
}

fn add(a: std.mem.Allocator, rows: *std.ArrayList([]u8), key: []const u8, val: []const u8) !void {
    var line: std.ArrayList(u8) = .empty;
    try line.appendSlice(a, key);
    try line.append(a, '\t');
    try line.appendSlice(a, val);
    try rows.append(a, try line.toOwnedSlice(a));
}

fn addNum(a: std.mem.Allocator, rows: *std.ArrayList([]u8), key: []const u8, v: u64) !void {
    var tmp: [32]u8 = undefined;
    const s = std.fmt.bufPrint(&tmp, "{d}", .{v}) catch unreachable;
    try add(a, rows, key, s);
}

fn addKey(a: std.mem.Allocator, rows: *std.ArrayList([]u8), comptime fmt: []const u8, args: anytype, val: []const u8) !void {
    var kbuf: [160]u8 = undefined;
    const key = std.fmt.bufPrint(&kbuf, fmt, args) catch unreachable;
    try add(a, rows, key, val);
}

fn commit(dir: std.fs.Dir, name: []const u8, rows: *std.ArrayList([]u8)) !void {
    std.mem.sort([]u8, rows.items, {}, less);
    var f = try dir.createFile(name, .{ .truncate = true });
    defer f.close();
    for (rows.items) |line| {
        try f.writeAll(line);
        try f.writeAll("\n");
    }
}

fn hexSlice(a: std.mem.Allocator, bytes: []const u8) ![]u8 {
    const out = try a.alloc(u8, bytes.len * 2);
    hex.write(out, bytes);
    return out;
}

fn hikeRows(
    a: std.mem.Allocator,
    gpa: std.mem.Allocator,
    rows: *std.ArrayList([]u8),
    prefix: []const u8,
    prof: profile.Profile,
    legs_n: u32,
    engine: Engine,
) !void {
    var legs: [18]u32 = undefined;
    @memset(&legs, legs_n);
    var obj = try producer.build(gpa, fixture.pkA(), prof, legs, engine);
    defer producer.deinit(gpa, &obj);
    const seed = terrain.derive(fixture.pkA(), fixture.seedBeacon(), engine);
    const words = try terrain.fill(gpa, seed.p, prof, engine);
    defer gpa.free(words);
    const dbytes = terrain.digest(words, engine);
    const dig = hex.of(32, &dbytes);
    var kbuf: [80]u8 = undefined;
    const kdig = std.fmt.bufPrint(&kbuf, "{s}.terrain_sha256", .{prefix}) catch unreachable;
    try add(a, rows, kdig, &dig);
    for (obj.stages, 0..) |st, g| {
        const wh = hex.of(32, &st.waypoint);
        const kw = std.fmt.bufPrint(&kbuf, "{s}.stage.{d}.waypoint", .{ prefix, g }) catch unreachable;
        try add(a, rows, kw, &wh);
        for (st.leg_results, st.checkpoints, 0..) |lr, cp, j| {
            const lh = hex.of(8, &lr);
            const kl = std.fmt.bufPrint(&kbuf, "{s}.stage.{d}.leg.{d}.legs_le_hex", .{ prefix, g, j }) catch unreachable;
            try add(a, rows, kl, &lh);
            const ch = hex.of(32, &cp);
            const kc = std.fmt.bufPrint(&kbuf, "{s}.stage.{d}.leg.{d}.checkpoint", .{ prefix, g, j }) catch unreachable;
            try add(a, rows, kc, &ch);
        }
    }
}

pub fn writeAll(gpa: std.mem.Allocator, dir: std.fs.Dir, engine: Engine) !void {
    var arena_state = std.heap.ArenaAllocator.init(gpa);
    defer arena_state.deinit();
    const a = arena_state.allocator();

    var seed_rows: std.ArrayList([]u8) = .empty;
    const pks = [_]struct { name: []const u8, pk: [32]u8 }{
        .{ .name = "pk_A", .pk = fixture.pkA() },
        .{ .name = "pk_B", .pk = fixture.pkB() },
    };
    for (pks) |pk| {
        const seed = terrain.derive(pk.pk, fixture.seedBeacon(), engine);
        const pre = hex.of(64, &seed.preimage);
        const d = hex.of(32, &seed.d);
        try addKey(a, &seed_rows, "seed.{s}.preimage", .{pk.name}, &pre);
        try addKey(a, &seed_rows, "seed.{s}.D", .{pk.name}, &d);
        for (0..4) |k| {
            const lane = hex.u64be(seed.p[k]);
            var tmp: [8]u8 = undefined;
            const ks = std.fmt.bufPrint(&tmp, "P{d}_u64", .{k}) catch unreachable;
            var kbuf: [64]u8 = undefined;
            const key = std.fmt.bufPrint(&kbuf, "seed.{s}.{s}", .{ pk.name, ks }) catch unreachable;
            try add(a, &seed_rows, key, &lane);
        }
    }

    var nl4_rows: std.ArrayList([]u8) = .empty;
    var fold_rows: std.ArrayList([]u8) = .empty;
    var terr_rows: std.ArrayList([]u8) = .empty;
    {
        const seed = terrain.derive(fixture.pkA(), fixture.seedBeacon(), engine);
        const words = try terrain.fill(gpa, seed.p, profile.s1, engine);
        defer gpa.free(words);
        const slots = [_]u64{ 0, 1, 1023 };
        for (slots) |i| {
            for (0..8) |j| {
                const base = hex.u64be(terrain.nl4Base(i, j, seed.p));
                const stored = hex.u64be(words[@intCast(i * 8 + j)]);
                try addKey(a, &nl4_rows, "nl4.S1.slot.{d}.word.{d}.base_u64", .{ i, j }, &base);
                try addKey(a, &nl4_rows, "nl4.S1.slot.{d}.word.{d}.stored_u64", .{ i, j }, &stored);
            }
        }
        const dbytes = terrain.digest(words, engine);
        const dig = hex.of(32, &dbytes);
        try add(a, &terr_rows, "terrain.S1.terrain_sha256", &dig);
        var S = std.mem.readInt(u64, seed.d[0..8], .little);
        for (0..4) |n| {
            const step = leg.stepDetail(words, 8, 1024, S, engine);
            const su = hex.u64be(S);
            const au = hex.u64be(step.acc);
            const nu = hex.u64be(step.next);
            var slot_buf: [32]u8 = undefined;
            const slot_s = std.fmt.bufPrint(&slot_buf, "{d}", .{step.slot}) catch unreachable;
            try addKey(a, &fold_rows, "fold.S1.step.{d}.S_u64", .{n}, &su);
            try addKey(a, &fold_rows, "fold.S1.step.{d}.slot_index", .{n}, slot_s);
            try addKey(a, &fold_rows, "fold.S1.step.{d}.acc_u64", .{n}, &au);
            try addKey(a, &fold_rows, "fold.S1.step.{d}.S_next_u64", .{n}, &nu);
            S = step.next;
        }
        const full = leg.legResult(words, profile.s1, &seed.d, engine);
        const fu = hex.u64be(full);
        try add(a, &fold_rows, "fold.S1.leg_result_u64", &fu);
    }

    var kat_rows: std.ArrayList([]u8) = .empty;
    try hikeRows(a, gpa, &kat_rows, "kat-small.S1", profile.s1, 1, engine);
    var wes_rows: std.ArrayList([]u8) = .empty;
    {
        const seed = terrain.derive(fixture.pkA(), fixture.seedBeacon(), engine);
        var legs: [18]u32 = undefined;
        @memset(&legs, 1);
        var obj = try producer.build(gpa, fixture.pkA(), profile.s1, legs, engine);
        defer producer.deinit(gpa, &obj);
        const d = hex.of(32, &seed.d);
        const w0 = hex.of(32, &obj.stages[0].waypoint);
        const eq = std.mem.eql(u8, &seed.d, &obj.stages[0].waypoint);
        try add(a, &wes_rows, "wes.S1.D", &d);
        try add(a, &wes_rows, "wes.S1.stage.0.waypoint", &w0);
        try add(a, &wes_rows, "wes.S1.equal", if (eq) "true" else "false");
    }

    var t3_rows: std.ArrayList([]u8) = .empty;
    try hikeRows(a, gpa, &t3_rows, "test-3leg.S2", profile.s2, 3, engine);
    {
        const seed = terrain.derive(fixture.pkA(), fixture.seedBeacon(), engine);
        const words = try terrain.fill(gpa, seed.p, profile.s2, engine);
        defer gpa.free(words);
        const dbytes = terrain.digest(words, engine);
        const dig = hex.of(32, &dbytes);
        try add(a, &terr_rows, "terrain.S2.terrain_sha256", &dig);
    }

    var prod_rows: std.ArrayList([]u8) = .empty;
    {
        const seed = terrain.derive(fixture.pkA(), fixture.seedBeacon(), engine);
        const words = try terrain.fill(gpa, seed.p, profile.production, engine);
        defer gpa.free(words);
        const dbytes = terrain.digest(words, engine);
        const dig = hex.of(32, &dbytes);
        try add(a, &terr_rows, "terrain.PROD.terrain_sha256", &dig);
        try add(a, &prod_rows, "prod.PROD.terrain_sha256", &dig);
        const result = leg.legResult(words, profile.production, &seed.d, engine);
        var le: [8]u8 = undefined;
        std.mem.writeInt(u64, &le, result, .little);
        const lh = hex.of(8, &le);
        const bytes = std.mem.sliceAsBytes(words);
        const samp = leg.sample(bytes, &seed.d, profile.production);
        const cp = leg.checkpoint(&seed.d, &seed.d, samp, result, engine);
        const ch = hex.of(32, &cp);
        try add(a, &prod_rows, "prod.PROD.stage.0.leg.0.legs_le_hex", &lh);
        try add(a, &prod_rows, "prod.PROD.stage.0.leg.0.checkpoint", &ch);
    }

    var rate_rows: std.ArrayList([]u8) = .empty;
    const Ls = [_]u64{ 0, 1, 5, 6, 1000 };
    const spans = [_]u64{ 1, 300, 3600 };
    const floors = [_]struct { p: u64, q: u64 }{
        .{ .p = 1, .q = 5 },
        .{ .p = 1, .q = 4 },
        .{ .p = 7, .q = 20 },
    };
    for (Ls) |L| for (spans) |s| for (floors) |f| {
        const tok = rate.legRate(L, s, f.p, f.q);
        try addKey(a, &rate_rows, "rate.legs.{d}.span.{d}.floor.{d}_{d}", .{ L, s, f.p, f.q }, tok);
    };

    var anchor_rows: std.ArrayList([]u8) = .empty;
    var h: u64 = 100;
    while (h <= 106) : (h += 1) {
        try addKey(a, &anchor_rows, "anchor.H.100.height.{d}", .{h}, rate.anchor(100, h));
    }

    var draw_rows: std.ArrayList([]u8) = .empty;
    const label = "pcoc-pack-2.2/draw";
    try add(a, &draw_rows, "draw.b0_label", label);
    try add(a, &draw_rows, "draw.thorough_rounds", "3");
    const totals = [_]u64{ 6, 18, 54 };
    for (totals) |L| {
        const six = draw.sixLeg(label, L);
        const six_s = try draw.formatIndices(a, &six);
        try addKey(a, &draw_rows, "draw.legs.{d}.six_leg.indices", .{L}, six_s);
        const th = try draw.thorough(label, L, 3, a);
        const th_s = try draw.formatIndices(a, th);
        try addKey(a, &draw_rows, "draw.legs.{d}.thorough.indices", .{L}, th_s);
    }

    var unk_rows: std.ArrayList([]u8) = .empty;
    const labels = [_][]const u8{ "v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT" };
    for (labels, 0..) |lab, n| {
        const kh = try hexSlice(a, lab);
        try addKey(a, &unk_rows, "unknown-kind.{d}.kind_text_hex", .{n}, kh);
        const rec = try verify.unknownKindRecord(a, lab);
        const rh = try hexSlice(a, rec);
        try addKey(a, &unk_rows, "unknown-kind.{d}.record_hex", .{n}, rh);
    }

    var hc_rows: std.ArrayList([]u8) = .empty;
    {
        var fee: [80]u8 = undefined;
        const h0 = hashNamed("pcoc-pack-2.2/hashcash/fee_header/0");
        const h1 = hashNamed("pcoc-pack-2.2/hashcash/fee_header/1");
        const h2 = hashNamed("pcoc-pack-2.2/hashcash/fee_header/2");
        @memcpy(fee[0..32], &h0);
        @memcpy(fee[32..64], &h1);
        @memcpy(fee[64..80], h2[0..16]);
        const hh = hashcash.dsha(&fee);
        const wp = hashNamed("pcoc-pack-2.2/hashcash/waypoint");
        const origin = "https://example.com";
        const chal = hashcash.challenge(origin, &hh);
        const solved = hashcash.solve(&chal, &wp, 21);
        const fee_h = hex.of(80, &fee);
        const hh_h = hex.of(32, &hh);
        const wp_h = hex.of(32, &wp);
        const ch_h = hex.of(32, &chal);
        const dg_h = hex.of(32, &solved.dig);
        try add(a, &hc_rows, "hashcash.bari_origin", origin);
        try add(a, &hc_rows, "hashcash.fee_header", &fee_h);
        try add(a, &hc_rows, "hashcash.header_hash", &hh_h);
        try add(a, &hc_rows, "hashcash.waypoint", &wp_h);
        try add(a, &hc_rows, "hashcash.difficulty_bits", "21");
        try add(a, &hc_rows, "hashcash.challenge", &ch_h);
        try addNum(a, &hc_rows, "hashcash.nonce", solved.nonce);
        try add(a, &hc_rows, "hashcash.digest", &dg_h);
        try addNum(a, &hc_rows, "hashcash.leading_zero_bits", solved.zeros);
    }

    var neg_rows_raw: std.ArrayList(negatives.Row) = .empty;
    try negatives.append(a, &neg_rows_raw, engine);
    var neg_rows: std.ArrayList([]u8) = .empty;
    for (neg_rows_raw.items) |r| try add(a, &neg_rows, r.key, r.val);

    try commit(dir, "seed.txt", &seed_rows);
    try commit(dir, "nl4.txt", &nl4_rows);
    try commit(dir, "terrain.txt", &terr_rows);
    try commit(dir, "fold.txt", &fold_rows);
    try commit(dir, "kat-small.txt", &kat_rows);
    try commit(dir, "test-3leg.txt", &t3_rows);
    try commit(dir, "prod.txt", &prod_rows);
    try commit(dir, "wes.txt", &wes_rows);
    try commit(dir, "rate.txt", &rate_rows);
    try commit(dir, "anchor.txt", &anchor_rows);
    try commit(dir, "draw.txt", &draw_rows);
    try commit(dir, "unknown-kind.txt", &unk_rows);
    try commit(dir, "hashcash.txt", &hc_rows);
    try commit(dir, "negatives.txt", &neg_rows);
}

fn hashNamed(s: []const u8) [32]u8 {
    var out: [32]u8 = undefined;
    hike.sha.hashFast(s, &out);
    return out;
}
