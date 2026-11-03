namespace Pcoc;

public static class Draw
{
/// §14.6
    public static int? Unbiased(System.Func<byte[]?> take8, int legTotal)
    {
        if (legTotal <= 0)
        {
            return null;
        }
        var mod = (ulong)legTotal;
        var limit = (ulong.MaxValue % mod) == 0
            ? ulong.MaxValue
            : ulong.MaxValue - (ulong.MaxValue % mod);
        while (true)
        {
            var raw = take8();
            if (raw == null || raw.Length < 8)
            {
                return null;
            }
            var u = Bytes.U64Le(raw, 0);
            if (limit == ulong.MaxValue || u < limit)
            {
                return (int)(u % mod);
            }
        }
    }

    /// §14.6
    public static int[]? Six(System.Func<byte[]?> take8, int legTotal)
    {
        if (legTotal < Const.SixLegCount)
        {
            return System.Array.Empty<int>();
        }
        var chosen = new int[Const.SixLegCount];
        var n = 0;
        while (n < Const.SixLegCount)
        {
            var q = Unbiased(take8, legTotal);
            if (q == null)
            {
                return null;
            }
            var seen = false;
            for (var i = 0; i < n; i++)
            {
                if (chosen[i] == q.Value)
                {
                    seen = true;
                    break;
                }
            }
            if (!seen)
            {
                chosen[n++] = q.Value;
            }
        }
        return chosen;
    }

    /// §15.5
    public static int[]? Thorough(System.Func<byte[]?> take8, int legTotal, int thoroughRounds)
    {
        if (legTotal < Const.SixLegCount)
        {
            return System.Array.Empty<int>();
        }
        var d = Const.SixLegCount * thoroughRounds;
        var phase = new System.Collections.Generic.List<int>(d);
        while (phase.Count < d)
        {
            var q = Unbiased(take8, legTotal);
            if (q == null)
            {
                return null;
            }
            var groupStart = phase.Count - (phase.Count % Const.SixLegCount);
            var inGroup = false;
            for (var i = groupStart; i < phase.Count; i++)
            {
                if (phase[i] == q.Value)
                {
                    inGroup = true;
                    break;
                }
            }
            if (inGroup)
            {
                continue;
            }
            var inPhase = false;
            for (var i = 0; i < phase.Count; i++)
            {
                if (phase[i] == q.Value)
                {
                    inPhase = true;
                    break;
                }
            }
            if (inPhase && phase.Count < legTotal)
            {
                continue;
            }
            phase.Add(q.Value);
        }
        return phase.ToArray();
    }

    /// §16.1
    public static byte[] Chain(string label)
    {
        return Sha256.Hash(System.Text.Encoding.ASCII.GetBytes(label));
    }

    /// §16.1
    public static System.Func<byte[]?> LabelSource(string label)
    {
        var current = Sha256.Hash(System.Text.Encoding.ASCII.GetBytes(label));
        var offset = 0;
        return () =>
        {
            if (offset + 8 > current.Length && offset != 0)
            {
                current = Sha256.Hash(current);
                offset = 0;
            }
            if (offset == 0 && current.Length < 8)
            {
                return null;
            }
            if (offset + 8 > 32)
            {
                current = Sha256.Hash(current);
                offset = 0;
            }
            var raw = new byte[8];
            System.Buffer.BlockCopy(current, offset, raw, 0, 8);
            offset += 8;
            if (offset == 32)
            {
                current = Sha256.Hash(current);
                offset = 0;
            }
            return raw;
        };
    }
}
