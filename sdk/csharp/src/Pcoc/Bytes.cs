namespace Pcoc;

public static class Bytes
{
/// §7.1
    public static byte[] Le64(ulong x)
    {
        var b = new byte[8];
        WriteLe64(b, 0, x);
        return b;
    }

    /// §7.1
    public static void WriteLe64(byte[] dest, int offset, ulong x)
    {
        dest[offset] = (byte)x;
        dest[offset + 1] = (byte)(x >> 8);
        dest[offset + 2] = (byte)(x >> 16);
        dest[offset + 3] = (byte)(x >> 24);
        dest[offset + 4] = (byte)(x >> 32);
        dest[offset + 5] = (byte)(x >> 40);
        dest[offset + 6] = (byte)(x >> 48);
        dest[offset + 7] = (byte)(x >> 56);
    }

    /// §7.1
    public static ulong U64Le(byte[] b, int offset)
    {
        return (ulong)b[offset]
            | ((ulong)b[offset + 1] << 8)
            | ((ulong)b[offset + 2] << 16)
            | ((ulong)b[offset + 3] << 24)
            | ((ulong)b[offset + 4] << 32)
            | ((ulong)b[offset + 5] << 40)
            | ((ulong)b[offset + 6] << 48)
            | ((ulong)b[offset + 7] << 56);
    }

    /// §7.1
    public static ulong U64Le(System.ReadOnlySpan<byte> b)
    {
        return (ulong)b[0]
            | ((ulong)b[1] << 8)
            | ((ulong)b[2] << 16)
            | ((ulong)b[3] << 24)
            | ((ulong)b[4] << 32)
            | ((ulong)b[5] << 40)
            | ((ulong)b[6] << 48)
            | ((ulong)b[7] << 56);
    }

    /// §7.2
    public static string Hex(byte[] b) => Hex(b, 0, b.Length);

    /// §7.2
    public static string Hex(byte[] b, int offset, int length)
    {
        var c = new char[length * 2];
        for (var i = 0; i < length; i++)
        {
            var v = b[offset + i];
            c[i * 2] = HexDigit(v >> 4);
            c[i * 2 + 1] = HexDigit(v & 0xF);
        }
        return new string(c);
    }

    /// §7.2
    public static bool TryParseHex(string text, out byte[] bytes)
    {
        bytes = System.Array.Empty<byte>();
        if ((text.Length & 1) != 0)
        {
            return false;
        }
        var n = text.Length / 2;
        var outb = new byte[n];
        for (var i = 0; i < n; i++)
        {
            var hi = HexValue(text[i * 2]);
            var lo = HexValue(text[i * 2 + 1]);
            if (hi < 0 || lo < 0)
            {
                return false;
            }
            outb[i] = (byte)((hi << 4) | lo);
        }
        bytes = outb;
        return true;
    }

    /// §9
    public static byte[] Reverse32(byte[] natural)
    {
        var d = new byte[32];
        for (var i = 0; i < 32; i++)
        {
            d[i] = natural[31 - i];
        }
        return d;
    }

    /// §3
    public static bool Equal(byte[] a, byte[] b)
    {
        if (a.Length != b.Length)
        {
            return false;
        }
        var diff = 0;
        for (var i = 0; i < a.Length; i++)
        {
            diff |= a[i] ^ b[i];
        }
        return diff == 0;
    }

    /// §3
    public static byte[] Copy(byte[] b)
    {
        var d = new byte[b.Length];
        System.Buffer.BlockCopy(b, 0, d, 0, b.Length);
        return d;
    }

    private static char HexDigit(int v) => (char)(v < 10 ? '0' + v : 'a' + (v - 10));

    private static int HexValue(char c)
    {
        if (c >= '0' && c <= '9') return c - '0';
        if (c >= 'a' && c <= 'f') return c - 'a' + 10;
        if (c >= 'A' && c <= 'F') return c - 'A' + 10;
        return -1;
    }
}
