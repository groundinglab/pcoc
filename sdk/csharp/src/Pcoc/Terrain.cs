namespace Pcoc;

/// §10.4
public sealed class Terrain
{
    public byte[] Data { get; }
    public Profile Profile { get; }
    public ulong[] Lanes { get; }

    private Terrain(byte[] bytes, Profile profile, ulong[] lanes)
    {
        Data = bytes;
        Profile = profile;
        Lanes = lanes;
    }

    /// §10.4
    public static Terrain Fill(byte[] publicKey, byte[] seedBeacon, Profile profile)
    {
        var lanes = Seed.Lanes(Seed.Digest(publicKey, seedBeacon));
        var terrainBytes = new byte[profile.TerrainBytes];
        var words = profile.WordsPerSlot;
        var w = new ulong[words];
        for (var i = 0; i < profile.NSlots; i++)
        {
            BaseWords(lanes, i, w);
            if (i > 0)
            {
                var parents = new int[3];
                parents[0] = i - 1;
                parents[1] = Mix.BitrevParent(i);
                parents[2] = Mix.MulParent(i);
                for (var k = 1; k <= 3; k++)
                {
                    var p = parents[k - 1];
                    var poff = p * profile.SlotBytes;
                    var km = (ulong)k * Const.ParentMul;
                    for (var j = 0; j < words; j++)
                    {
                        var pw = Pcoc.Bytes.U64Le(terrainBytes, poff + 8 * j);
                        var rot = (j + k) & 63;
                        w[j] = Mix.Mix64((w[j] ^ Mix.Rotl(pw, rot)) ^ km);
                    }
                }
                var slotMix = (ulong)i * Const.SlotMul;
                for (var j = 0; j < words; j++)
                {
                    w[j] = Mix.Mix64(w[j] ^ slotMix);
                }
            }
            var off = i * profile.SlotBytes;
            for (var j = 0; j < words; j++)
            {
                Pcoc.Bytes.WriteLe64(terrainBytes, off + 8 * j, w[j]);
            }
        }
        return new Terrain(terrainBytes, profile, lanes);
    }

    /// §10.4
    public static void BaseWords(ulong[] lanes, int i, ulong[] w)
    {
        for (var j = 0; j < w.Length; j++)
        {
            var c = ((ulong)i << 32) ^ (ulong)j ^ Const.WordXor;
            var t = Mix.Mix64(lanes[j & 3] ^ c);
            t = Mix.Mix64(t ^ Mix.Rotl(lanes[(j + 1) & 3], 17));
            t = Mix.Mix64(t ^ Mix.Rotl(lanes[(j + 2) & 3], 31));
            t = Mix.Mix64(t ^ Mix.Rotl(lanes[(j + 3) & 3], 47));
            w[j] = t;
        }
    }

    /// §11.1
    public byte[] Slot(int index)
    {
        var d = new byte[Profile.SlotBytes];
        System.Buffer.BlockCopy(Data, index * Profile.SlotBytes, d, 0, Profile.SlotBytes);
        return d;
    }

    /// §11.3
    public byte[] TerrainSample(ulong state)
    {
        var offset = (int)(state % (ulong)Profile.OffsetModulus);
        var d = new byte[Profile.SampleBytes];
        System.Buffer.BlockCopy(Data, offset, d, 0, Profile.SampleBytes);
        return d;
    }

    /// §10.4
    public byte[] Digest() => Sha256.Hash(Data);
}
