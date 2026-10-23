using System;
using System.Runtime.CompilerServices;

namespace Pcoc;

/// <summary>
/// Scalar primitives of SPEC sections 10.1–10.3: mix64, rotl, floor_log2,
/// reverse32 and the two parent indices. All integer arithmetic is u64 with
/// mod 2^64 wrapping (section 3).
/// </summary>
public static class Prims
{
    /// <summary>SPEC 10.1, including the initial + MIX_ADD.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static ulong Mix64(ulong x)
    {
        ulong y = x + Profile.MixAdd;
        y = (y ^ (y >> 30)) * Profile.MixMul1;
        y = (y ^ (y >> 27)) * Profile.MixMul2;
        return y ^ (y >> 31);
    }

    /// <summary>SPEC 10.3: rotate left by r in 0 … 63 on a u64.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static ulong Rotl(ulong w, int r) => (w << r) | (w >> (64 - r));

    /// <summary>SPEC 10.3: greatest b with 2^b ≤ n, for n ≥ 1.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static int FloorLog2(long n) => 63 - System.Numerics.BitOperations.LeadingZeroCount((ulong)n);

    /// <summary>SPEC 10.3: bit k of the result is bit 31 − k of the 32-bit input.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static uint Reverse32(uint v)
    {
        // Swap halves down to single bits (32-bit bit reversal).
        uint r = v;
        r = ((r & 0x55555555u) << 1) | ((r >> 1) & 0x55555555u);
        r = ((r & 0x33333333u) << 2) | ((r >> 2) & 0x33333333u);
        r = ((r & 0x0F0F0F0Fu) << 4) | ((r >> 4) & 0x0F0F0F0Fu);
        r = (r << 24) | ((r & 0xFF00u) << 8) | ((r >> 8) & 0xFF00u) | (r >> 24);
        return r;
    }

    /// <summary>SPEC 10.3: (reverse32(i) >> (32 − bits)) mod i, bits = max(floor_log2(i), 1).</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static long BitrevParent(long i)
    {
        int bits = FloorLog2(i);
        if (bits < 1) bits = 1;
        uint rev = Reverse32((uint)i);
        long top = rev >> (32 - bits);
        return top % i;
    }

    /// <summary>SPEC 10.3: floor((i × MIX_ADD mod 2^64) / 2^32) mod i.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static long MulParent(long i)
    {
        ulong h = (ulong)i * Profile.MixAdd;
        long top = (long)(h >> 32);
        return top % i;
    }

    /// <summary>SPEC 7.1: 8-byte little-endian encoding.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static void Le64(ulong x, Span<byte> dst) =>
        System.Buffers.Binary.BinaryPrimitives.WriteUInt64LittleEndian(dst, x);

    /// <summary>SPEC 7.1: the unsigned integer of 8 little-endian bytes.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static ulong U64Le(ReadOnlySpan<byte> b) =>
        System.Buffers.Binary.BinaryPrimitives.ReadUInt64LittleEndian(b);
}
