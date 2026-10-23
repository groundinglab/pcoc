using System;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Runtime.Intrinsics;
using System.Runtime.Intrinsics.X86;
using System.Security.Cryptography;

namespace Pcoc;

/// <summary>Optional per-step trace sink, used only by the fixture generator.</summary>
public interface IStepSink
{
    void Record(long t, ulong s, long slotIndex, ulong acc, ulong sNext);
}

/// <summary>
/// SPEC section 11: the step (fold + seal) and the leg. Every step loads all
/// SLOT_BYTES of the selected slot and folds every word with the state by the
/// section 11.1 loop. Vector engines fold 4 (AVX2) or 8 (AVX-512) words per
/// lane-group and reduce with wrapping addition, which is associative mod
/// 2^64, so the result is bit-identical to the scalar loop.
/// </summary>
public static class Leg
{
    private delegate ulong FoldDelegate(ReadOnlySpan<ulong> slot, ulong s);

    private static readonly FoldDelegate FoldImpl = EngineSelection.Current switch
    {
        Engine.Avx512 => FoldAvx512,
        Engine.Avx2 => FoldAvx2,
        _ => FoldScalar,
    };

    /// <summary>SPEC 11.1 fold: acc = Σ mix64(word(w) XOR S) mod 2^64.</summary>
    public static ulong Fold(ReadOnlySpan<ulong> slot, ulong s) => FoldImpl(slot, s);

    /// <summary>SPEC 11.1 seal: S' = u64_le(SHA-256(SEAL_DOMAIN ‖ le64(S_folded))[0:8]).</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static ulong Seal(ulong sFolded)
    {
        Span<byte> msg = stackalloc byte[20];
        Profile.SealDomain.CopyTo(msg);
        Prims.Le64(sFolded, msg.Slice(12));
        Span<byte> digest = stackalloc byte[32];
        Sha256Fast.Hash(msg, digest);
        return Prims.U64Le(digest);
    }

    /// <summary>SPEC 11.1: one step from state S to S'.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public static ulong Step(Terrain terrain, ulong s)
    {
        long i = (long)((ulong)s % (ulong)terrain.Profile.NSlots);
        return Seal(FoldImpl(terrain.SlotWords(i), s));
    }

    /// <summary>
    /// SPEC 11.2: STEPS_PER_LEG steps from S_0 = u64_le(previous[0:8]);
    /// returns the u64 leg result.
    /// </summary>
    public static ulong RunLeg(Terrain terrain, ReadOnlySpan<byte> previousCheckpoint32, long steps, IStepSink? sink = null)
    {
        return RunLeg(terrain, previousCheckpoint32, steps, FoldImpl, sink);
    }

    /// <summary>SPEC 11.2 with an explicit fold engine, for measurement. Identical bytes.</summary>
    public static ulong RunLeg(Terrain terrain, ReadOnlySpan<byte> previousCheckpoint32, long steps, Engine engine)
    {
        FoldDelegate fold = engine switch
        {
            Engine.Avx512 => FoldAvx512,
            Engine.Avx2 => FoldAvx2,
            _ => FoldScalar,
        };
        return RunLeg(terrain, previousCheckpoint32, steps, fold, null);
    }

    private static ulong RunLeg(Terrain terrain, ReadOnlySpan<byte> previousCheckpoint32, long steps,
        FoldDelegate fold, IStepSink? sink)
    {
        long nSlots = terrain.Profile.NSlots;
        ulong s = Prims.U64Le(previousCheckpoint32);
        if (sink is null)
        {
            for (long t = 0; t < steps; t++)
            {
                long i = (long)(s % (ulong)nSlots);
                s = Seal(fold(terrain.SlotWords(i), s));
            }
        }
        else
        {
            for (long t = 0; t < steps; t++)
            {
                long i = (long)(s % (ulong)nSlots);
                ulong acc = fold(terrain.SlotWords(i), s);
                ulong sNext = Seal(acc);
                sink.Record(t, s, i, acc, sNext);
                s = sNext;
            }
        }
        return s;
    }

    /// <summary>SPEC 11.4: H(previous ‖ stage limb ‖ terrain sample ‖ le64(leg_result)).</summary>
    public static byte[] Checkpoint(Terrain terrain, ReadOnlySpan<byte> previousCheckpoint32,
        ReadOnlySpan<byte> stageLimb32, ulong legResult)
    {
        int sampleBytes = terrain.Profile.SampleBytes;
        Span<byte> preimage = new byte[32 + 32 + sampleBytes + 8];
        previousCheckpoint32.CopyTo(preimage);
        stageLimb32.CopyTo(preimage.Slice(32));
        terrain.Sample(previousCheckpoint32).CopyTo(preimage.Slice(64));
        Prims.Le64(legResult, preimage.Slice(64 + sampleBytes));
        return SHA256.HashData(preimage);
    }

    /// <summary>SPEC 12.2: fold of a stage's checkpoints, F_0 = 0^32.</summary>
    public static byte[] CheckpointFold(IReadOnlyList<byte[]> checkpoints)
    {
        byte[] f = new byte[32];
        Span<byte> msg = stackalloc byte[64];
        for (int j = 0; j < checkpoints.Count; j++)
        {
            f.CopyTo(msg);
            checkpoints[j].CopyTo(msg.Slice(32));
            f = SHA256.HashData(msg);
        }
        return f;
    }

    /// <summary>
    /// SPEC 12.2: W = SHA-256(B ‖ slot_sample(B) ‖ stageWaypoint ‖ fold),
    /// where slot_sample is the full slot indexed by u64_le(B[0:8]) mod N_SLOTS.
    /// </summary>
    public static byte[] Waypoint(Terrain terrain, ReadOnlySpan<byte> interrupt32,
        ReadOnlySpan<byte> stageWaypoint32, IReadOnlyList<byte[]> stageCheckpoints)
    {
        byte[] fold = CheckpointFold(stageCheckpoints);
        using var ih = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        ih.AppendData(interrupt32);
        ih.AppendData(terrain.SlotSample(interrupt32));
        ih.AppendData(stageWaypoint32);
        ih.AppendData(fold);
        return ih.GetHashAndReset();
    }

    // ---------------- fold engines ----------------

    internal static ulong FoldScalar(ReadOnlySpan<ulong> slot, ulong s)
    {
        ulong acc = 0;
        for (int w = 0; w < slot.Length; w++)
            acc += Prims.Mix64(slot[w] ^ s);
        return acc;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static Vector256<ulong> MulLo64Avx2(Vector256<ulong> a, Vector256<ulong> b)
    {
        Vector256<ulong> lo = Avx2.Multiply(a.AsUInt32(), b.AsUInt32());
        Vector256<ulong> aHi = Avx2.ShiftRightLogical(a, 32);
        Vector256<ulong> bHi = Avx2.ShiftRightLogical(b, 32);
        Vector256<ulong> m1 = Avx2.Multiply(aHi.AsUInt32(), b.AsUInt32());
        Vector256<ulong> m2 = Avx2.Multiply(a.AsUInt32(), bHi.AsUInt32());
        return Avx2.Add(lo, Avx2.ShiftLeftLogical(Avx2.Add(m1, m2), 32));
    }

    internal static ulong FoldAvx2(ReadOnlySpan<ulong> slot, ulong s)
    {
        var sV = Vector256.Create(s);
        var add = Vector256.Create(Profile.MixAdd);
        var m1 = Vector256.Create(Profile.MixMul1);
        var m2 = Vector256.Create(Profile.MixMul2);
        var accV = Vector256<ulong>.Zero;
        ref ulong slotRef = ref System.Runtime.InteropServices.MemoryMarshal.GetReference(slot);
        int vec = slot.Length & ~3;
        for (int j = 0; j < vec; j += 4)
        {
            var w = Vector256.LoadUnsafe(ref Unsafe.Add(ref slotRef, j));
            var x = Avx2.Xor(w, sV);
            var y = Avx2.Add(x, add);
            y = Avx2.Xor(y, Avx2.ShiftRightLogical(y, 30));
            y = MulLo64Avx2(y, m1);
            y = Avx2.Xor(y, Avx2.ShiftRightLogical(y, 27));
            y = MulLo64Avx2(y, m2);
            y = Avx2.Xor(y, Avx2.ShiftRightLogical(y, 31));
            accV = Avx2.Add(accV, y);
        }
        ulong acc = accV[0] + accV[1] + accV[2] + accV[3];
        for (int j = vec; j < slot.Length; j++)
            acc += Prims.Mix64(slot[j] ^ s);
        return acc;
    }

    internal static ulong FoldAvx512(ReadOnlySpan<ulong> slot, ulong s)
    {
        var sV = Vector512.Create(s);
        var add = Vector512.Create(Profile.MixAdd);
        var m1 = Vector512.Create(Profile.MixMul1);
        var m2 = Vector512.Create(Profile.MixMul2);
        var accV = Vector512<ulong>.Zero;
        ref ulong slotRef = ref System.Runtime.InteropServices.MemoryMarshal.GetReference(slot);
        int vec = slot.Length & ~7;
        for (int j = 0; j < vec; j += 8)
        {
            var w = Vector512.LoadUnsafe(ref Unsafe.Add(ref slotRef, j));
            var y = Avx512F.Add(Avx512F.Xor(w, sV), add);
            y = Avx512F.Xor(y, Avx512F.ShiftRightLogical(y, 30));
            y = Avx512DQ.MultiplyLow(y, m1);
            y = Avx512F.Xor(y, Avx512F.ShiftRightLogical(y, 27));
            y = Avx512DQ.MultiplyLow(y, m2);
            y = Avx512F.Xor(y, Avx512F.ShiftRightLogical(y, 31));
            accV = Avx512F.Add(accV, y);
        }
        // Wrapping addition is associative mod 2^64: lane order does not matter.
        var lo256 = accV.GetLower();
        var hi256 = accV.GetUpper();
        var sum256 = Avx2.Add(lo256, hi256);
        var lo128 = sum256.GetLower();
        var hi128 = sum256.GetUpper();
        var sum128 = Sse2.Add(lo128, hi128);
        ulong acc = sum128[0] + sum128[1];
        for (int j = vec; j < slot.Length; j++)
            acc += Prims.Mix64(slot[j] ^ s);
        return acc;
    }
}
