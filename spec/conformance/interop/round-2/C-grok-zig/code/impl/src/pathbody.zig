//! SPEC §14.3 canonical path body and validate_for.

const std = @import("std");
const sha = @import("sha.zig");

pub const Body = struct {
    leaf: [32]u8,
    leaf_index: u32,
    tree_size: u32,
    siblings: []const [32]u8,
    root: [32]u8,
    txid: [32]u8,
    vout: u32,
    block_hash: [32]u8,
    block_height: u32,
};

pub fn siblingCount(tree_size: u32) u32 {
    var width: u32 = tree_size;
    var n: u32 = 0;
    while (width > 1) {
        n += 1;
        width = (width / 2) + (width % 2);
    }
    return n;
}

pub fn decode(bytes: []const u8, a: std.mem.Allocator) !?Body {
    if (bytes.len < 146 or bytes[0] != 0x01) return null;
    const sc = bytes[41];
    if (sc > 32) return null;
    const need: usize = 146 + 32 * @as(usize, sc);
    if (bytes.len != need) return null;
    const leaf_index = std.mem.readInt(u32, bytes[33..37], .big);
    const tree_size = std.mem.readInt(u32, bytes[37..41], .big);
    if (tree_size < 1 or leaf_index >= tree_size) return null;
    if (siblingCount(tree_size) != sc) return null;
    const sibs = try a.alloc([32]u8, sc);
    var off: usize = 42;
    for (sibs) |*s| {
        @memcpy(s, bytes[off..][0..32]);
        off += 32;
    }
    var body = Body{
        .leaf = undefined,
        .leaf_index = leaf_index,
        .tree_size = tree_size,
        .siblings = sibs,
        .root = undefined,
        .txid = undefined,
        .vout = std.mem.readInt(u32, bytes[off + 64 ..][0..4], .big),
        .block_hash = undefined,
        .block_height = std.mem.readInt(u32, bytes[off + 100 ..][0..4], .big),
    };
    @memcpy(&body.leaf, bytes[1..33]);
    @memcpy(&body.root, bytes[off..][0..32]);
    off += 32;
    @memcpy(&body.txid, bytes[off..][0..32]);
    off += 32;
    off += 4; // vout already read
    @memcpy(&body.block_hash, bytes[off..][0..32]);
    return body;
}

fn parent(left: *const [32]u8, right: *const [32]u8) [32]u8 {
    var h = sha.Hasher.init(.fast);
    h.update(&[_]u8{0x1b});
    h.update(left);
    h.update(right);
    var out: [32]u8 = undefined;
    h.final(&out);
    return out;
}

pub fn validateFor(body: Body, waypoint: *const [32]u8) bool {
    if (!std.mem.eql(u8, &body.leaf, waypoint)) return false;
    var h = sha.Hasher.init(.fast);
    h.update(&[_]u8{0x1a});
    h.update(&body.leaf);
    var node: [32]u8 = undefined;
    h.final(&node);
    var index: u32 = body.leaf_index;
    var width: u32 = body.tree_size;
    for (body.siblings) |sib| {
        if ((width % 2 == 1) and index == width - 1) {
            if (!std.mem.eql(u8, &sib, &node)) return false;
            node = parent(&node, &node);
        } else if (index % 2 == 0) {
            node = parent(&node, &sib);
        } else {
            node = parent(&sib, &node);
        }
        index = index / 2;
        width = (width / 2) + (width % 2);
    }
    return std.mem.eql(u8, &node, &body.root);
}

/// Read block_hash and block_height if the layout reaches them. No validate_for.
pub fn claimedFields(bytes: []const u8) ?struct { hash: [32]u8, height: u32 } {
    if (bytes.len < 42 or bytes[0] != 0x01) return null;
    const sc = bytes[41];
    if (sc > 32) return null;
    const off = 42 + 32 * @as(usize, sc);
    // root 32 + txid 32 + vout 4 + hash 32 + height 4
    if (bytes.len < off + 32 + 32 + 4 + 32 + 4) return null;
    const hash_at = off + 32 + 32 + 4;
    var hash: [32]u8 = undefined;
    @memcpy(&hash, bytes[hash_at..][0..32]);
    const height = std.mem.readInt(u32, bytes[hash_at + 32 ..][0..4], .big);
    return .{ .hash = hash, .height = height };
}
