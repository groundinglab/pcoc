namespace Pcoc;

public static class Checkpoint
{
/// §11.4
    public static byte[] Bind(Terrain terrain, byte[] previous, byte[] stageLimb, ulong legResult)
    {
        var sampleOffset = (int)(Bytes.U64Le(previous, 0) % (ulong)terrain.Profile.OffsetModulus);
        var n = 32 + 32 + terrain.Profile.SampleBytes + 8;
        var pre = new byte[n];
        System.Buffer.BlockCopy(previous, 0, pre, 0, 32);
        System.Buffer.BlockCopy(stageLimb, 0, pre, 32, 32);
        System.Buffer.BlockCopy(terrain.Data, sampleOffset, pre, 64, terrain.Profile.SampleBytes);
        Bytes.WriteLe64(pre, 64 + terrain.Profile.SampleBytes, legResult);
        return Sha256.Hash(pre);
    }
}
