//! SPEC §13.1 Hashcash and Rule BR1 paid-waypoint check.
//! Live-plane Bitcoin difficulty retarget is not computed here. The caller
//! supplies the required nBits for the "after" case (test plane: 0x207fffff).

const std = @import("std");
const sha = @import("sha.zig");
const profile = @import("profile.zig");

pub fn leadingZeroBits(dig: *const [32]u8) u32 {
    var n: u32 = 0;
    for (dig) |b| {
        if (b == 0) {
            n += 8;
            continue;
        }
        n += @as(u32, @clz(b));
        break;
    }
    return n;
}

pub fn challenge(bari_origin: []const u8, header_hash: *const [32]u8) [32]u8 {
    var h = sha.Hasher.init(.fast);
    h.update("bari-hc-v2");
    h.update(bari_origin);
    h.update(header_hash);
    var out: [32]u8 = undefined;
    h.final(&out);
    return out;
}

pub fn digest(chal: *const [32]u8, waypoint: *const [32]u8, nonce: u64) [32]u8 {
    var msg: [72]u8 = undefined;
    @memcpy(msg[0..32], chal);
    @memcpy(msg[32..64], waypoint);
    std.mem.writeInt(u64, msg[64..72], nonce, .little);
    var out: [32]u8 = undefined;
    sha.hashFast(&msg, &out);
    return out;
}

pub fn dsha(header: []const u8) [32]u8 {
    var a: [32]u8 = undefined;
    sha.hashFast(header, &a);
    var b: [32]u8 = undefined;
    sha.hashFast(&a, &b);
    return b;
}

/// First nonce from 0 whose digest has at least `bits` leading zero bits.
pub fn solve(chal: *const [32]u8, waypoint: *const [32]u8, bits: u32) struct { nonce: u64, dig: [32]u8, zeros: u32 } {
    var nonce: u64 = 0;
    while (true) : (nonce += 1) {
        const dig = digest(chal, waypoint, nonce);
        const z = leadingZeroBits(&dig);
        if (z >= bits) return .{ .nonce = nonce, .dig = dig, .zeros = z };
    }
}

fn compactTarget(nbits: u32, out: *[32]u8) void {
    @memset(out, 0);
    if ((nbits & 0x00800000) != 0) return;
    const size: u32 = nbits >> 24;
    const word: u32 = nbits & 0x007fffff;
    if (size <= 3) {
        var w = word;
        w >>= @intCast(8 * (3 - size));
        std.mem.writeInt(u32, out[0..4], w, .little);
        return;
    }
    const off: usize = size - 3;
    if (off >= 32) return;
    if (off < 32) out[off] = @truncate(word);
    if (off + 1 < 32) out[off + 1] = @truncate(word >> 8);
    if (off + 2 < 32) out[off + 2] = @truncate(word >> 16);
}

fn leLessEq(hash_le: *const [32]u8, target_le: *const [32]u8) bool {
    var i: usize = 32;
    while (i > 0) {
        i -= 1;
        if (hash_le[i] < target_le[i]) return true;
        if (hash_le[i] > target_le[i]) return false;
    }
    return true;
}

pub const Answer = enum { accept, deny };

/// SPEC Rule BR1. `fee` and `tip` are 80-byte headers. `required_nbits` is the
/// plane's target for the after case. Natural-order comparisons only.
pub fn br1(
    body: []const u8,
    nonce: u64,
    fee: *const [80]u8,
    origin: []const u8,
    tip: *const [80]u8,
    required_nbits: u32,
) Answer {
    if (body.len != 32) return .deny;
    const fee_hash = dsha(fee);
    const tip_hash = dsha(tip);
    var prev: [32]u8 = undefined;
    @memcpy(&prev, tip[4..36]);
    const current = std.mem.eql(u8, &fee_hash, &tip_hash);
    const before = std.mem.eql(u8, &fee_hash, &prev);
    var fee_prev: [32]u8 = undefined;
    @memcpy(&fee_prev, fee[4..36]);
    const nbits = std.mem.readInt(u32, fee[72..76], .little);
    var target: [32]u8 = undefined;
    compactTarget(nbits, &target);
    const pow = leLessEq(&fee_hash, &target);
    const after = std.mem.eql(u8, &fee_prev, &tip_hash) and nbits == required_nbits and pow;
    if (!(current or before or after)) return .deny;
    const chal = challenge(origin, &fee_hash);
    var wp: [32]u8 = undefined;
    @memcpy(&wp, body[0..32]);
    const dig = digest(&chal, &wp, nonce);
    if (leadingZeroBits(&dig) < profile.HASHCASH_BITS) return .deny;
    return .accept;
}

pub const synth_nbits: u32 = 0x207fffff;
