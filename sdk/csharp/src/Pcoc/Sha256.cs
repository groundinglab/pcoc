using System.Security.Cryptography;

namespace Pcoc;

public static class Sha256
{
/// §7.4
    public static byte[] Hash(byte[] data) => Hash(data, 0, data.Length);

    /// §7.4
    public static byte[] Hash(byte[] data, int offset, int count)
    {
        var slice = new byte[count];
        System.Buffer.BlockCopy(data, offset, slice, 0, count);
        return SHA256.HashData(slice);
    }

    /// §7.4
    public static byte[] Hash(System.ReadOnlySpan<byte> data) => SHA256.HashData(data);

    /// §11.1
    public static byte[] Concat(params byte[][] parts)
    {
        var n = 0;
        foreach (var p in parts)
        {
            n += p.Length;
        }
        var d = new byte[n];
        var o = 0;
        foreach (var p in parts)
        {
            System.Buffer.BlockCopy(p, 0, d, o, p.Length);
            o += p.Length;
        }
        return d;
    }

    /// §11.1
    public static byte[] Seal(ulong folded)
    {
        var pre = new byte[20];
        System.Buffer.BlockCopy(Const.SealDomain, 0, pre, 0, 12);
        Bytes.WriteLe64(pre, 12, folded);
        return Hash(pre);
    }

    /// §6
    public static string BackendName()
    {
        return "openssl";
    }
}
