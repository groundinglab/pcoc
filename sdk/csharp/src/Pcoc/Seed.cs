namespace Pcoc;

public static class Seed
{
/// §10
    public static byte[] Preimage(byte[] publicKey, byte[] seedBeacon)
    {
        var d = new byte[64];
        System.Buffer.BlockCopy(publicKey, 0, d, 0, 32);
        System.Buffer.BlockCopy(seedBeacon, 0, d, 32, 32);
        return d;
    }

    /// §10
    public static byte[] Digest(byte[] publicKey, byte[] seedBeacon)
    {
        return Sha256.Hash(Preimage(publicKey, seedBeacon));
    }

    /// §10
    public static ulong[] Lanes(byte[] digest)
    {
        var p = new ulong[4];
        for (var k = 0; k < 4; k++)
        {
            p[k] = Bytes.U64Le(digest, 8 * k);
        }
        return p;
    }
}
