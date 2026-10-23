using System;
using System.Collections.Generic;
using System.Security.Cryptography;
using System.Text;

namespace Pcoc;

/// <summary>The byte source a section 14.6 draw consumes, 8 bytes at a time.</summary>
public interface IDrawByteSource
{
    /// <summary>Reads the next 8 bytes; false when the source is exhausted.</summary>
    bool TryRead8(Span<byte> dst);
}

/// <summary>
/// The rig byte source of section 16.1 (Draw bytes): b_0 = SHA-256(ASCII
/// label), b_(n+1) = SHA-256(b_n), concatenated, consumed 8 bytes at a time
/// from the start. The verifier consumes the bytes in order and does not
/// hash them again.
/// </summary>
public sealed class ChainDrawBytes : IDrawByteSource
{
    private byte[] _block;
    private int _pos;

    public ChainDrawBytes(string asciiLabel)
    {
        _block = SHA256.HashData(Encoding.ASCII.GetBytes(asciiLabel));
        _pos = 0;
    }

    public ChainDrawBytes(byte[] firstBlock)
    {
        if (firstBlock.Length != 32) throw new ArgumentException("b_0 must be 32 bytes");
        _block = (byte[])firstBlock.Clone();
        _pos = 0;
    }

    public bool TryRead8(Span<byte> dst)
    {
        if (_pos + 8 > 32)
        {
            _block = SHA256.HashData(_block);
            _pos = 0;
        }
        _block.AsSpan(_pos, 8).CopyTo(dst);
        _pos += 8;
        return true;
    }
}

/// <summary>A rig source over caller-supplied bytes (section 16.1 rig input).</summary>
public sealed class FixedDrawBytes : IDrawByteSource
{
    private readonly byte[] _bytes;
    private int _pos;
    public FixedDrawBytes(byte[] bytes) { _bytes = bytes; }

    public bool TryRead8(Span<byte> dst)
    {
        if (_pos + 8 > _bytes.Length) return false;
        _bytes.AsSpan(_pos, 8).CopyTo(dst);
        _pos += 8;
        return true;
    }
}

/// <summary>The production byte source: the operating-system CSPRNG.</summary>
public sealed class CsprngDrawBytes : IDrawByteSource
{
    public bool TryRead8(Span<byte> dst)
    {
        RandomNumberGenerator.Fill(dst);
        return true;
    }
}

/// <summary>The section 14.6 unbiased draw and the section 15.5 thorough draw.</summary>
public static class Draw
{
    /// <summary>
    /// One unbiased draw from 0 … legTotal−1: limit = 2^64 − (2^64 mod
    /// legTotal); accept the first u64_le of 8 bytes below the limit.
    /// </summary>
    public static bool TryDrawOne(IDrawByteSource source, ulong legTotal, out ulong q)
    {
        q = 0;
        if (legTotal == 0) return false;
        Span<byte> buf = stackalloc byte[8];
        UInt128 m = (UInt128)ulong.MaxValue + 1;
        UInt128 limit = m - m % legTotal;
        while (true)
        {
            if (!source.TryRead8(buf)) return false;
            ulong u = Prims.U64Le(buf);
            if ((UInt128)u < limit)
            {
                q = u % legTotal;
                return true;
            }
            // u ≥ limit is consumed; those bytes are not read again
        }
    }

    /// <summary>Rule A4.4: six distinct indices in draw order.</summary>
    public static bool SixLeg(IDrawByteSource source, ulong legTotal, out ulong[] six)
    {
        six = Array.Empty<ulong>();
        if (legTotal < 6) return false;
        var chosen = new List<ulong>(6);
        var seen = new HashSet<ulong>();
        while (chosen.Count < 6)
        {
            if (!TryDrawOne(source, legTotal, out ulong q)) return false;
            if (seen.Add(q)) chosen.Add(q);
            // an index already chosen is discarded
        }
        six = chosen.ToArray();
        return true;
    }

    /// <summary>
    /// Section 15.5: D = 6 · thoroughRounds indices into phase, in draw order.
    /// A candidate already in the current group of 6 is discarded; a candidate
    /// already in phase is discarded while some index is still unused; repeats
    /// are allowed only after every index has been used.
    /// </summary>
    public static bool Thorough(IDrawByteSource source, ulong legTotal, int thoroughRounds, out ulong[] phase)
    {
        phase = Array.Empty<ulong>();
        if (legTotal < 6 || thoroughRounds < 1) return false;
        long d = 6L * thoroughRounds;
        var list = new List<ulong>((int)Math.Min(d, 1 << 20));
        var inPhase = new HashSet<ulong>();
        while (list.Count < d)
        {
            if (!TryDrawOne(source, legTotal, out ulong q)) return false;
            // the current group of 6: phase[6·floor(len/6) …]
            int groupStart = (list.Count / 6) * 6;
            bool inGroup = false;
            for (int k = groupStart; k < list.Count; k++)
                if (list[k] == q) { inGroup = true; break; }
            if (inGroup) continue;
            if (inPhase.Contains(q) && inPhase.Count < (long)legTotal) continue;
            list.Add(q);
            inPhase.Add(q);
        }
        phase = list.ToArray();
        return true;
    }
}
