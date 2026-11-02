namespace Pcoc;

public static class Mix
{
/// §10.1
    public static ulong Mix64(ulong x)
    {
        var y = x + Const.MixAdd;
        y = (y ^ (y >> 30)) * Const.MixMul1;
        y = (y ^ (y >> 27)) * Const.MixMul2;
        return y ^ (y >> 31);
    }

    /// §10.3
    public static ulong Rotl(ulong w, int r)
    {
        r &= 63;
        if (r == 0)
        {
            return w;
        }
        return (w << r) | (w >> (64 - r));
    }

    /// §10.3
    public static int FloorLog2(int n)
    {
        var b = 0;
        while ((1 << (b + 1)) <= n && b < 30)
        {
            b++;
        }
        return b;
    }

    /// §10.3
    public static uint Reverse32(uint v)
    {
        var r = 0u;
        for (var k = 0; k < 32; k++)
        {
            if (((v >> (31 - k)) & 1u) != 0)
            {
                r |= 1u << k;
            }
        }
        return r;
    }

    /// §10.3
    public static int BitrevParent(int i)
    {
        var bits = System.Math.Max(FloorLog2(i), 1);
        var rev = Reverse32((uint)i) >> (32 - bits);
        return (int)(rev % (uint)i);
    }

    /// §10.3
    public static int MulParent(int i)
    {
        var h = (ulong)i * Const.MixAdd;
        return (int)((h >> 32) % (ulong)i);
    }
}
