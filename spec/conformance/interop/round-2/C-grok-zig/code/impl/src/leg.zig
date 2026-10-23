//! SPEC §11 leg, §11.4 checkpoint, §12.2 waypoint fold.

const std = @import("std");
const profile = @import("profile.zig");
const sha = @import("sha.zig");
const mix = @import("mix.zig");
const Engine = profile.Engine;

pub const StepOut = struct {
    slot: u64,
    acc: u64,
    next: u64,
};

fn slotIndex(S: u64, n_slots: u64) u64 {
    return S % n_slots;
}

/// SPEC §11.1 one step. Loads the whole slot.
pub fn stepDetail(terrain: []const u64, words: u64, n_slots: u64, S: u64, engine: Engine) StepOut {
    const i = slotIndex(S, n_slots);
    const start: usize = @intCast(i * words);
    const w: usize = @intCast(words);
    const acc = mix.foldSlot(terrain[start .. start + w], S, engine);
    return .{ .slot = i, .acc = acc, .next = seal(acc, engine) };
}

pub fn step(terrain: []const u64, words: u64, n_slots: u64, S: u64, engine: Engine) u64 {
    return stepDetail(terrain, words, n_slots, S, engine).next;
}

fn sealE(folded: u64, comptime engine: Engine) u64 {
    var msg: [20]u8 = undefined;
    @memcpy(msg[0..12], profile.SEAL_DOMAIN);
    std.mem.writeInt(u64, msg[12..20], folded, .little);
    var dig: [32]u8 = undefined;
    sha.hash20(&msg, &dig, engine);
    return std.mem.readInt(u64, dig[0..8], .little);
}

/// SPEC §11.1 seal. One SHA-256 of the 20-byte block.
pub fn seal(folded: u64, engine: Engine) u64 {
    return switch (engine) {
        .fast => sealE(folded, .fast),
        .scalar => sealE(folded, .scalar),
    };
}

fn stepE(terrain: []const u64, words: u64, n_slots: u64, S: u64, comptime engine: Engine) u64 {
    const i = slotIndex(S, n_slots);
    const start: usize = @intCast(i * words);
    const w: usize = @intCast(words);
    const slot = terrain[start .. start + w];
    const acc = switch (engine) {
        .fast => mix.foldFast(slot, S),
        .scalar => mix.foldScalar(slot, S),
    };
    return sealE(acc, engine);
}

fn legResultE(terrain: []const u64, p: profile.Profile, previous: *const [32]u8, comptime engine: Engine) u64 {
    var S = std.mem.readInt(u64, previous[0..8], .little);
    const words = p.wordsPerSlot();
    const slots = p.nSlots();
    const n: usize = @intCast(p.steps_per_leg);
    for (0..n) |_| S = stepE(terrain, words, slots, S, engine);
    return S;
}

/// SPEC §11.2.
pub fn legResult(terrain: []const u64, p: profile.Profile, previous: *const [32]u8, engine: Engine) u64 {
    return switch (engine) {
        .fast => legResultE(terrain, p, previous, .fast),
        .scalar => legResultE(terrain, p, previous, .scalar),
    };
}

/// SPEC §11.3. `terrain_bytes` is the byte view of the terrain.
pub fn sample(terrain_bytes: []const u8, previous: *const [32]u8, p: profile.Profile) []const u8 {
    const off = std.mem.readInt(u64, previous[0..8], .little) % p.offsetModulus();
    const o: usize = @intCast(off);
    const n: usize = @intCast(p.sample_bytes);
    return terrain_bytes[o .. o + n];
}

/// SPEC §11.4.
pub fn checkpoint(
    previous: *const [32]u8,
    limb: *const [32]u8,
    terrain_sample: []const u8,
    leg: u64,
    engine: Engine,
) [32]u8 {
    var h = sha.Hasher.init(engine);
    h.update(previous);
    h.update(limb);
    h.update(terrain_sample);
    var le: [8]u8 = undefined;
    std.mem.writeInt(u64, &le, leg, .little);
    h.update(&le);
    var out: [32]u8 = undefined;
    h.final(&out);
    return out;
}

/// SPEC §12.2 checkpoint fold. Empty list yields 0^32.
pub fn checkpointFold(checkpoints: []const [32]u8, engine: Engine) [32]u8 {
    var f = [_]u8{0} ** 32;
    for (checkpoints) |c| {
        var h = sha.Hasher.init(engine);
        h.update(&f);
        h.update(&c);
        h.final(&f);
    }
    return f;
}

/// SPEC §12.2 waypoint. `slot_sample` is the raw slot bytes.
pub fn waypoint(
    interrupt: *const [32]u8,
    slot_sample: []const u8,
    prev_waypoint: *const [32]u8,
    fold: *const [32]u8,
    engine: Engine,
) [32]u8 {
    var h = sha.Hasher.init(engine);
    h.update(interrupt);
    h.update(slot_sample);
    h.update(prev_waypoint);
    h.update(fold);
    var out: [32]u8 = undefined;
    h.final(&out);
    return out;
}

pub fn slotSample(terrain: []const u64, words: u64, interrupt: *const [32]u8, n_slots: u64) []const u8 {
    const i = std.mem.readInt(u64, interrupt[0..8], .little) % n_slots;
    const bytes = std.mem.sliceAsBytes(terrain);
    const start: usize = @intCast(i * words * 8);
    const n: usize = @intCast(words * 8);
    return bytes[start .. start + n];
}

/// SPEC §13.1 integer anchor comparison (also §16.1). Inclusion is anchored
/// when it is H plus one of 2, 3, 4.
pub fn anchorOffset(h: u64, inclusion: u64) bool {
    if (inclusion < h) return false;
    const d = inclusion - h;
    return d == 2 or d == 3 or d == 4;
}
