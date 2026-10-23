//! SPEC §7.3 and §8 hike object encode.

const std = @import("std");
const cbor = @import("cbor.zig");
const profile = @import("profile.zig");

pub const Stage = struct {
    beacon: [32]u8,
    beacon_height: u64,
    waypoint: [32]u8,
    /// Null omits the key (stage 0). Empty is a skip or W_17.
    path: ?[]const u8 = null,
    checkpoints: [][32]u8,
    leg_results: [][8]u8,
};

pub const Object = struct {
    kind: []const u8 = profile.KIND,
    fill: []const u8 = profile.FILL,
    slot_bytes: u64,
    n_slots: u64,
    sample_bytes: u64,
    steps_per_leg: u64,
    public_key: [32]u8,
    seed_beacon: [32]u8,
    stages: [18]Stage,
    last_interrupt: [32]u8,
    last_interrupt_height: u64,
};

pub fn encode(a: std.mem.Allocator, obj: *const Object) ![]u8 {
    var list: std.ArrayList(u8) = .empty;
    errdefer list.deinit(a);
    const w = cbor.Writer{ .buf = &list, .a = a };
    try w.map(11);
    try w.text("fill");
    try w.text(obj.fill);
    try w.text("kind");
    try w.text(obj.kind);
    try w.text("stages");
    try w.array(18);
    for (obj.stages, 0..) |st, g| try encodeStage(w, st, g == 0);
    try w.text("n_slots");
    try w.uint(obj.n_slots);
    try w.text("public_key");
    try w.bytes(&obj.public_key);
    try w.text("slot_bytes");
    try w.uint(obj.slot_bytes);
    try w.text("seed_beacon");
    try w.bytes(&obj.seed_beacon);
    try w.text("sample_bytes");
    try w.uint(obj.sample_bytes);
    try w.text("steps_per_leg");
    try w.uint(obj.steps_per_leg);
    try w.text("last_interrupt");
    try w.bytes(&obj.last_interrupt);
    try w.text("last_interrupt_height");
    try w.uint(obj.last_interrupt_height);
    return list.toOwnedSlice(a);
}

fn encodeStage(w: cbor.Writer, st: Stage, stage0: bool) !void {
    if (stage0) {
        try w.map(6);
    } else {
        try w.map(7);
        try w.text("path");
        try w.bytes(st.path orelse "");
    }
    try w.text("beacon");
    try w.bytes(&st.beacon);
    try w.text("waypoint");
    try w.bytes(&st.waypoint);
    try w.text("leg_count");
    try w.uint(@intCast(st.checkpoints.len));
    try w.text("checkpoints");
    try w.array(@intCast(st.checkpoints.len));
    for (st.checkpoints) |c| try w.bytes(&c);
    try w.text("leg_results");
    try w.array(@intCast(st.leg_results.len));
    for (st.leg_results) |r| try w.bytes(&r);
    try w.text("beacon_height");
    try w.uint(st.beacon_height);
}

/// SPEC §13.3 stderr at the close. Empty paths on W_1 … W_16, then W_17.
pub fn writePathSkip(file: std.fs.File) !void {
    var g: u8 = 1;
    while (g <= 16) : (g += 1) {
        var buf: [32]u8 = undefined;
        const line = std.fmt.bufPrint(&buf, "path_skip {d}\n", .{g}) catch unreachable;
        try file.writeAll(line);
    }
    try file.writeAll("not fetched by design 17\n");
}
