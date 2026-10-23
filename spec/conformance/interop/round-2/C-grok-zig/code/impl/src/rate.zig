//! SPEC Rule B2 integer comparison and §16.1 anchor comparison.

const leg = @import("leg.zig");

/// `meets` when hike_span > 0 and q·hike_legs ≥ p·hike_span. Otherwise `refuse`.
/// Products are mathematical integers (no mod 2^64). SPEC §15 Rule B2 does not
/// name a modulus; see RESULTS.md.
pub fn legRate(hike_legs: u64, hike_span: u64, p: u64, q: u64) []const u8 {
    if (hike_span == 0) return "refuse";
    const lhs: u128 = @as(u128, q) * hike_legs;
    const rhs: u128 = @as(u128, p) * hike_span;
    return if (lhs >= rhs) "meets" else "refuse";
}

pub fn anchor(h: u64, inclusion: u64) []const u8 {
    return if (leg.anchorOffset(h, inclusion)) "anchored" else "not anchored";
}

const std = @import("std");

test "rate grid spot" {
    // 6 legs, span 1, floor 1/5: 5*6 >= 1*1
    try std.testing.expectEqualStrings("meets", legRate(6, 1, 1, 5));
    // 1 leg, span 3600, floor 1/5: 5*1 >= 1*3600? no
    try std.testing.expectEqualStrings("refuse", legRate(1, 3600, 1, 5));
    try std.testing.expectEqualStrings("anchored", anchor(100, 102));
    try std.testing.expectEqualStrings("not anchored", anchor(100, 101));
    try std.testing.expectEqualStrings("not anchored", anchor(100, 105));
}
