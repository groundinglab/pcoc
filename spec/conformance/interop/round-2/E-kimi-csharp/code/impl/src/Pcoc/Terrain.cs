using System;
using System.Numerics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Intrinsics;
using System.Runtime.Intrinsics.X86;

namespace Pcoc;

/// <summary>Which vector engine the host uses. Resolved once at startup.</summary>
public enum Engine { Scalar = 0, Avx2 = 1, Avx512 = 2 }

public static class EngineSelection
{
    /// <summary>
    /// The engine in force. Defaults to the best the host supports; the
    /// PCOC_ENGINE environment variable (scalar|avx2|avx512) can force a
    /// lower one for measurement. Every engine computes identical bytes.
    /// </summary>
    public static readonly Engine Current = Resolve();

    private static Engine Resolve()
    {
        string? forced = Environment.GetEnvironmentVariable("PCOC_ENGINE");
        if (!string.IsNullOrEmpty(forced))
        {
            switch (forced.Trim().ToLowerInvariant())
            {
                case "scalar": return Engine.Scalar;
                case "avx2": return Avx2.IsSupported ? Engine.Avx2 : Engine.Scalar;
                case "avx512":
                    return (Vector512.IsHardwareAccelerated && Avx512DQ.IsSupported) ? Engine.Avx512
                        : Avx2.IsSupported ? Engine.Avx2 : Engine.Scalar;
            }
        }
        if (Vector512.IsHardwareAccelerated && Avx512DQ.IsSupported && Avx512BW.IsSupported)
            return Engine.Avx512;
        if (Avx2.IsSupported) return Engine.Avx2;
        return Engine.Scalar;
    }
}

/// <summary>
/// SPEC section 10: the seed and the dag2 terrain. The terrain is filled once
/// per PCOC from the seed, before the first leg, and is not shared between
/// PCOCs. Words are stored as u64 lanes; the byte view is their little-endian
/// layout, so it is the terrain byte array of the SPEC exactly.
/// </summary>
public sealed class Terrain
{
    private readonly ulong[] _words;
    private readonly Profile _profile;

    private Terrain(Profile profile, ulong[] words)
    {
        _profile = profile;
        _words = words;
    }

    public Profile Profile => _profile;

    /// <summary>Whole terrain as bytes (the array the SPEC hashes and samples).</summary>
    public ReadOnlySpan<byte> Bytes => MemoryMarshal.AsBytes(_words.AsSpan());

    /// <summary>Slot i as words: terrain[i × SLOT_BYTES : (i+1) × SLOT_BYTES].</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public ReadOnlySpan<ulong> SlotWords(long i) =>
        _words.AsSpan((int)(i * _profile.WordsPerSlot), _profile.WordsPerSlot);

    /// <summary>SPEC 11.3: SAMPLE_BYTES at u64_le(previous[0:8]) mod OFFSET_MODULUS.</summary>
    public ReadOnlySpan<byte> Sample(ReadOnlySpan<byte> previousCheckpoint32)
    {
        long off = (long)(Prims.U64Le(previousCheckpoint32) % (ulong)_profile.OffsetModulus);
        return Bytes.Slice((int)off, _profile.SampleBytes);
    }

    /// <summary>SPEC 12.2: the full slot indexed by u64_le(B[0:8]) mod N_SLOTS, as bytes.</summary>
    public ReadOnlySpan<byte> SlotSample(ReadOnlySpan<byte> interrupt32)
    {
        long i = (long)(Prims.U64Le(interrupt32) % (ulong)_profile.NSlots);
        int byteOff = (int)(i * _profile.SlotBytes);
        return Bytes.Slice(byteOff, _profile.SlotBytes);
    }

    /// <summary>SPEC 10: D = SHA-256(public_key ‖ seed_beacon), 32 bytes, no label.</summary>
    public static byte[] Seed(ReadOnlySpan<byte> publicKey32, ReadOnlySpan<byte> seedBeacon32)
    {
        Span<byte> preimage = stackalloc byte[64];
        publicKey32.CopyTo(preimage);
        seedBeacon32.CopyTo(preimage.Slice(32));
        return System.Security.Cryptography.SHA256.HashData(preimage);
    }

    /// <summary>SPEC 10: lanes P[k] = u64_le(D[8k : 8k+8]), k = 0 … 3.</summary>
    public static void Lanes(ReadOnlySpan<byte> seed32, Span<ulong> lanes4)
    {
        for (int k = 0; k < 4; k++)
            lanes4[k] = Prims.U64Le(seed32.Slice(8 * k, 8));
    }

    /// <summary>Fills the terrain once from the seed (section 10.4), on the engine in force.</summary>
    public static Terrain Fill(Profile profile, ReadOnlySpan<byte> seed32)
    {
        return Fill(profile, seed32, EngineSelection.Current);
    }

    /// <summary>Fill on an explicit engine, for measurement. Every engine computes identical bytes.</summary>
    public static Terrain Fill(Profile profile, ReadOnlySpan<byte> seed32, Engine engine)
    {
        long total = profile.NSlots * profile.WordsPerSlot;
        if (total > int.MaxValue) throw new InvalidOperationException("terrain too large");
        var words = new ulong[(int)total];
        Span<ulong> lanes = stackalloc ulong[4];
        Lanes(seed32, lanes);
        switch (engine)
        {
            case Engine.Avx512:
                if (Vector512.IsHardwareAccelerated && Avx512DQ.IsSupported) { FillAvx512(profile, words, lanes); break; }
                goto case Engine.Avx2;
            case Engine.Avx2:
                if (Avx2.IsSupported) { FillAvx2(profile, words, lanes); break; }
                goto default;
            default:
                FillScalar(profile, words, lanes);
                break;
        }
        return new Terrain(profile, words);
    }

    // ---------------- scalar ----------------

    internal static void FillScalar(Profile profile, ulong[] terrain, ReadOnlySpan<ulong> lanes)
    {
        int wps = profile.WordsPerSlot;
        long nSlots = profile.NSlots;
        for (int j = 0; j < wps; j++)
            terrain[j] = Nl4Base(0, j, lanes);
        for (long i = 1; i < nSlots; i++)
        {
            long p1 = i - 1, p2 = Prims.BitrevParent(i), p3 = Prims.MulParent(i);
            ulong slotMix = (ulong)i * Profile.SlotMul;
            int dst = (int)(i * wps);
            int s1 = (int)(p1 * wps), s2 = (int)(p2 * wps), s3 = (int)(p3 * wps);
            ulong km1 = unchecked(1UL * Profile.ParentMul);
            ulong km2 = unchecked(2UL * Profile.ParentMul);
            ulong km3 = unchecked(3UL * Profile.ParentMul);
            for (int j = 0; j < wps; j++)
            {
                ulong t = Nl4Base(i, j, lanes);
                t = Prims.Mix64((t ^ Prims.Rotl(terrain[s1 + j], (j + 1) & 63)) ^ km1);
                t = Prims.Mix64((t ^ Prims.Rotl(terrain[s2 + j], (j + 2) & 63)) ^ km2);
                t = Prims.Mix64((t ^ Prims.Rotl(terrain[s3 + j], (j + 3) & 63)) ^ km3);
                t = Prims.Mix64(t ^ slotMix);
                terrain[dst + j] = t;
            }
        }
    }

    /// <summary>SPEC 10.4 NL4 base word at slot i, word j (before parents and slot_mix).</summary>
    public static ulong Nl4Base(long i, long j, ReadOnlySpan<ulong> lanes)
    {
        ulong c = ((ulong)i << 32) ^ (ulong)j ^ Profile.WordXor;
        ulong t = Prims.Mix64(lanes[(int)(j & 3)] ^ c);
        t = Prims.Mix64(t ^ Prims.Rotl(lanes[(int)((j + 1) & 3)], 17));
        t = Prims.Mix64(t ^ Prims.Rotl(lanes[(int)((j + 2) & 3)], 31));
        t = Prims.Mix64(t ^ Prims.Rotl(lanes[(int)((j + 3) & 3)], 47));
        return t;
    }

    // ---------------- AVX2 ----------------

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static Vector256<ulong> MulLo64Avx2(Vector256<ulong> a, Vector256<ulong> b)
    {
        // 64×64 → low 64 product from 32×32 partials (vpmuludq reads the low 32 of each lane).
        Vector256<ulong> lo = Avx2.Multiply(a.AsUInt32(), b.AsUInt32());
        Vector256<ulong> aHi = Avx2.ShiftRightLogical(a, 32);
        Vector256<ulong> bHi = Avx2.ShiftRightLogical(b, 32);
        Vector256<ulong> m1 = Avx2.Multiply(aHi.AsUInt32(), b.AsUInt32());
        Vector256<ulong> m2 = Avx2.Multiply(a.AsUInt32(), bHi.AsUInt32());
        return Avx2.Add(lo, Avx2.ShiftLeftLogical(Avx2.Add(m1, m2), 32));
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static Vector256<ulong> Mix64Avx2(Vector256<ulong> x, Vector256<ulong> add, Vector256<ulong> m1c, Vector256<ulong> m2c)
    {
        Vector256<ulong> y = Avx2.Add(x, add);
        y = Avx2.Xor(y, Avx2.ShiftRightLogical(y, 30));
        y = MulLo64Avx2(y, m1c);
        y = Avx2.Xor(y, Avx2.ShiftRightLogical(y, 27));
        y = MulLo64Avx2(y, m2c);
        return Avx2.Xor(y, Avx2.ShiftRightLogical(y, 31));
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static Vector256<ulong> RotlVarAvx2(Vector256<ulong> x, Vector256<ulong> r)
    {
        // count ≥ 64 yields 0 on vpsllvq/vpsrlvq, so r = 0 works via (64 − r) = 64.
        Vector256<ulong> left = Avx2.ShiftLeftLogicalVariable(x, r);
        Vector256<ulong> right = Avx2.ShiftRightLogicalVariable(x, Avx2.Subtract(Vector256.Create(64UL), r));
        return Avx2.Or(left, right);
    }

    internal static void FillAvx2(Profile profile, ulong[] terrain, ReadOnlySpan<ulong> lanes)
    {
        int wps = profile.WordsPerSlot;
        long nSlots = profile.NSlots;
        var add = Vector256.Create(Profile.MixAdd);
        var m1 = Vector256.Create(Profile.MixMul1);
        var m2 = Vector256.Create(Profile.MixMul2);
        ulong p0 = lanes[0], p1l = lanes[1], p2l = lanes[2], p3 = lanes[3];
        var lane0 = Vector256.Create(p0, p1l, p2l, p3);
        var lane1r = Vector256.Create(Prims.Rotl(p1l, 17), Prims.Rotl(p2l, 17), Prims.Rotl(p3, 17), Prims.Rotl(p0, 17));
        var lane2r = Vector256.Create(Prims.Rotl(p2l, 31), Prims.Rotl(p3, 31), Prims.Rotl(p0, 31), Prims.Rotl(p1l, 31));
        var lane3r = Vector256.Create(Prims.Rotl(p3, 47), Prims.Rotl(p0, 47), Prims.Rotl(p1l, 47), Prims.Rotl(p2l, 47));
        var jv0 = Vector256.Create(0UL, 1UL, 2UL, 3UL);
        var eight = Vector256.Create(4UL);
        var mask63 = Vector256.Create(63UL);
        var km1 = Vector256.Create(unchecked(1UL * Profile.ParentMul));
        var km2 = Vector256.Create(unchecked(2UL * Profile.ParentMul));
        var km3 = Vector256.Create(unchecked(3UL * Profile.ParentMul));
        var one = Vector256.Create(1UL);
        var two = Vector256.Create(2UL);
        var three = Vector256.Create(3UL);

        int vec = wps & ~3;
        // slot 0: NL4 base words only, no parents and no slot mix (section 10.4).
        FillBaseAvx2(profile, terrain, 0, wps, vec, lane0, lane1r, lane2r, lane3r, jv0, eight, add, m1, m2, lanes);
        for (long i = 1; i < nSlots; i++)
        {
            int dst = (int)(i * wps);
            int s1 = (int)((i - 1) * wps);
            int s2 = (int)(Prims.BitrevParent(i) * wps);
            int s3 = (int)(Prims.MulParent(i) * wps);
            var cBase = Vector256.Create(((ulong)i << 32) ^ Profile.WordXor);
            var slotMix = Vector256.Create((ulong)i * Profile.SlotMul);
            var jv = jv0;
            ref ulong dstRef = ref MemoryMarshal.GetReference(terrain.AsSpan());
            for (int j = 0; j < vec; j += 4)
            {
                var c = Avx2.Xor(cBase, jv);
                var t = Mix64Avx2(Avx2.Xor(lane0, c), add, m1, m2);
                t = Mix64Avx2(Avx2.Xor(t, lane1r), add, m1, m2);
                t = Mix64Avx2(Avx2.Xor(t, lane2r), add, m1, m2);
                t = Mix64Avx2(Avx2.Xor(t, lane3r), add, m1, m2);
                var rot1 = Avx2.And(Avx2.Add(jv, one), mask63);
                var rot2 = Avx2.And(Avx2.Add(jv, two), mask63);
                var rot3 = Avx2.And(Avx2.Add(jv, three), mask63);
                var pw1 = Vector256.LoadUnsafe(ref Unsafe.Add(ref dstRef, s1 + j));
                var pw2 = Vector256.LoadUnsafe(ref Unsafe.Add(ref dstRef, s2 + j));
                var pw3 = Vector256.LoadUnsafe(ref Unsafe.Add(ref dstRef, s3 + j));
                t = Mix64Avx2(Avx2.Xor(Avx2.Xor(t, RotlVarAvx2(pw1, rot1)), km1), add, m1, m2);
                t = Mix64Avx2(Avx2.Xor(Avx2.Xor(t, RotlVarAvx2(pw2, rot2)), km2), add, m1, m2);
                t = Mix64Avx2(Avx2.Xor(Avx2.Xor(t, RotlVarAvx2(pw3, rot3)), km3), add, m1, m2);
                t = Mix64Avx2(Avx2.Xor(t, slotMix), add, m1, m2);
                t.StoreUnsafe(ref Unsafe.Add(ref dstRef, dst + j));
                jv = Avx2.Add(jv, eight);
            }
            for (int j = vec; j < wps; j++)
            {
                ulong t = Nl4Base(i, j, lanes);
                t = Prims.Mix64((t ^ Prims.Rotl(terrain[s1 + j], (j + 1) & 63)) ^ km1.GetElement(0));
                t = Prims.Mix64((t ^ Prims.Rotl(terrain[s2 + j], (j + 2) & 63)) ^ km2.GetElement(0));
                t = Prims.Mix64((t ^ Prims.Rotl(terrain[s3 + j], (j + 3) & 63)) ^ km3.GetElement(0));
                t = Prims.Mix64(t ^ slotMix.GetElement(0));
                terrain[dst + j] = t;
            }
        }
    }

    private static void FillBaseAvx2(Profile profile, ulong[] terrain, long i, int wps, int vec,
        Vector256<ulong> lane0, Vector256<ulong> lane1r, Vector256<ulong> lane2r, Vector256<ulong> lane3r,
        Vector256<ulong> jv0, Vector256<ulong> step, Vector256<ulong> add, Vector256<ulong> m1, Vector256<ulong> m2,
        ReadOnlySpan<ulong> lanes)
    {
        var cBase = Vector256.Create(((ulong)i << 32) ^ Profile.WordXor);
        var jv = jv0;
        ref ulong dstRef = ref MemoryMarshal.GetReference(terrain.AsSpan());
        for (int j = 0; j < vec; j += 4)
        {
            var c = Avx2.Xor(cBase, jv);
            var t = Mix64Avx2(Avx2.Xor(lane0, c), add, m1, m2);
            t = Mix64Avx2(Avx2.Xor(t, lane1r), add, m1, m2);
            t = Mix64Avx2(Avx2.Xor(t, lane2r), add, m1, m2);
            t = Mix64Avx2(Avx2.Xor(t, lane3r), add, m1, m2);
            t.StoreUnsafe(ref Unsafe.Add(ref dstRef, (int)(i * wps) + j));
            jv = Avx2.Add(jv, step);
        }
        for (int j = vec; j < wps; j++)
            terrain[(int)(i * wps) + j] = Nl4Base(i, j, lanes);
    }

    // ---------------- AVX-512 ----------------

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static Vector512<ulong> RotlVarAvx512(Vector512<ulong> x, Vector512<ulong> r)
    {
        // count ≥ 64 yields 0 on vpsllvq/vpsrlvq, so r = 0 works via (64 − r) = 64.
        Vector512<ulong> left = Avx512F.ShiftLeftLogicalVariable(x, r);
        Vector512<ulong> right = Avx512F.ShiftRightLogicalVariable(x, Avx512F.Subtract(Vector512.Create(64UL), r));
        return Avx512F.Or(left, right);
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static Vector512<ulong> Mix64Avx512(Vector512<ulong> x, Vector512<ulong> add, Vector512<ulong> m1c, Vector512<ulong> m2c)
    {
        Vector512<ulong> y = Avx512F.Add(x, add);
        y = Avx512F.Xor(y, Avx512F.ShiftRightLogical(y, 30));
        y = Avx512DQ.MultiplyLow(y, m1c);
        y = Avx512F.Xor(y, Avx512F.ShiftRightLogical(y, 27));
        y = Avx512DQ.MultiplyLow(y, m2c);
        return Avx512F.Xor(y, Avx512F.ShiftRightLogical(y, 31));
    }

    internal static void FillAvx512(Profile profile, ulong[] terrain, ReadOnlySpan<ulong> lanes)
    {
        int wps = profile.WordsPerSlot;
        long nSlots = profile.NSlots;
        var add = Vector512.Create(Profile.MixAdd);
        var m1 = Vector512.Create(Profile.MixMul1);
        var m2 = Vector512.Create(Profile.MixMul2);
        ulong p0 = lanes[0], p1l = lanes[1], p2l = lanes[2], p3 = lanes[3];
        var lane0 = Vector512.Create(p0, p1l, p2l, p3, p0, p1l, p2l, p3);
        var lane1r = Vector512.Create(Prims.Rotl(p1l, 17), Prims.Rotl(p2l, 17), Prims.Rotl(p3, 17), Prims.Rotl(p0, 17),
                                      Prims.Rotl(p1l, 17), Prims.Rotl(p2l, 17), Prims.Rotl(p3, 17), Prims.Rotl(p0, 17));
        var lane2r = Vector512.Create(Prims.Rotl(p2l, 31), Prims.Rotl(p3, 31), Prims.Rotl(p0, 31), Prims.Rotl(p1l, 31),
                                      Prims.Rotl(p2l, 31), Prims.Rotl(p3, 31), Prims.Rotl(p0, 31), Prims.Rotl(p1l, 31));
        var lane3r = Vector512.Create(Prims.Rotl(p3, 47), Prims.Rotl(p0, 47), Prims.Rotl(p1l, 47), Prims.Rotl(p2l, 47),
                                      Prims.Rotl(p3, 47), Prims.Rotl(p0, 47), Prims.Rotl(p1l, 47), Prims.Rotl(p2l, 47));
        var jv0 = Vector512.Create(0UL, 1UL, 2UL, 3UL, 4UL, 5UL, 6UL, 7UL);
        var eight = Vector512.Create(8UL);
        var mask63 = Vector512.Create(63UL);
        var km1 = Vector512.Create(unchecked(1UL * Profile.ParentMul));
        var km2 = Vector512.Create(unchecked(2UL * Profile.ParentMul));
        var km3 = Vector512.Create(unchecked(3UL * Profile.ParentMul));
        var one = Vector512.Create(1UL);
        var two = Vector512.Create(2UL);
        var three = Vector512.Create(3UL);

        int vec = wps & ~7;
        FillBaseAvx512(profile, terrain, 0, wps, vec, lane0, lane1r, lane2r, lane3r, jv0, eight, add, m1, m2, lanes);
        ref ulong dstRef = ref MemoryMarshal.GetReference(terrain.AsSpan());
        for (long i = 1; i < nSlots; i++)
        {
            int dst = (int)(i * wps);
            int s1 = (int)((i - 1) * wps);
            int s2 = (int)(Prims.BitrevParent(i) * wps);
            int s3 = (int)(Prims.MulParent(i) * wps);
            var cBase = Vector512.Create(((ulong)i << 32) ^ Profile.WordXor);
            var slotMix = Vector512.Create((ulong)i * Profile.SlotMul);
            var jv = jv0;
            for (int j = 0; j < vec; j += 8)
            {
                var c = Avx512F.Xor(cBase, jv);
                var t = Mix64Avx512(Avx512F.Xor(lane0, c), add, m1, m2);
                t = Mix64Avx512(Avx512F.Xor(t, lane1r), add, m1, m2);
                t = Mix64Avx512(Avx512F.Xor(t, lane2r), add, m1, m2);
                t = Mix64Avx512(Avx512F.Xor(t, lane3r), add, m1, m2);
                var rot1 = Avx512F.And(Avx512F.Add(jv, one), mask63);
                var rot2 = Avx512F.And(Avx512F.Add(jv, two), mask63);
                var rot3 = Avx512F.And(Avx512F.Add(jv, three), mask63);
                var pw1 = Vector512.LoadUnsafe(ref Unsafe.Add(ref dstRef, s1 + j));
                var pw2 = Vector512.LoadUnsafe(ref Unsafe.Add(ref dstRef, s2 + j));
                var pw3 = Vector512.LoadUnsafe(ref Unsafe.Add(ref dstRef, s3 + j));
                t = Mix64Avx512(Avx512F.Xor(Avx512F.Xor(t, RotlVarAvx512(pw1, rot1)), km1), add, m1, m2);
                t = Mix64Avx512(Avx512F.Xor(Avx512F.Xor(t, RotlVarAvx512(pw2, rot2)), km2), add, m1, m2);
                t = Mix64Avx512(Avx512F.Xor(Avx512F.Xor(t, RotlVarAvx512(pw3, rot3)), km3), add, m1, m2);
                t = Mix64Avx512(Avx512F.Xor(t, slotMix), add, m1, m2);
                t.StoreUnsafe(ref Unsafe.Add(ref dstRef, dst + j));
                jv = Avx512F.Add(jv, eight);
            }
            for (int j = vec; j < wps; j++)
            {
                ulong t = Nl4Base(i, j, lanes);
                t = Prims.Mix64((t ^ Prims.Rotl(terrain[s1 + j], (j + 1) & 63)) ^ km1.GetElement(0));
                t = Prims.Mix64((t ^ Prims.Rotl(terrain[s2 + j], (j + 2) & 63)) ^ km2.GetElement(0));
                t = Prims.Mix64((t ^ Prims.Rotl(terrain[s3 + j], (j + 3) & 63)) ^ km3.GetElement(0));
                t = Prims.Mix64(t ^ slotMix.GetElement(0));
                terrain[dst + j] = t;
            }
        }
    }

    private static void FillBaseAvx512(Profile profile, ulong[] terrain, long i, int wps, int vec,
        Vector512<ulong> lane0, Vector512<ulong> lane1r, Vector512<ulong> lane2r, Vector512<ulong> lane3r,
        Vector512<ulong> jv0, Vector512<ulong> step, Vector512<ulong> add, Vector512<ulong> m1, Vector512<ulong> m2,
        ReadOnlySpan<ulong> lanes)
    {
        var cBase = Vector512.Create(((ulong)i << 32) ^ Profile.WordXor);
        var jv = jv0;
        ref ulong dstRef = ref MemoryMarshal.GetReference(terrain.AsSpan());
        for (int j = 0; j < vec; j += 8)
        {
            var c = Avx512F.Xor(cBase, jv);
            var t = Mix64Avx512(Avx512F.Xor(lane0, c), add, m1, m2);
            t = Mix64Avx512(Avx512F.Xor(t, lane1r), add, m1, m2);
            t = Mix64Avx512(Avx512F.Xor(t, lane2r), add, m1, m2);
            t = Mix64Avx512(Avx512F.Xor(t, lane3r), add, m1, m2);
            t.StoreUnsafe(ref Unsafe.Add(ref dstRef, (int)(i * wps) + j));
            jv = Avx512F.Add(jv, step);
        }
        for (int j = vec; j < wps; j++)
            terrain[(int)(i * wps) + j] = Nl4Base(i, j, lanes);
    }
}
