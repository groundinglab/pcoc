namespace Pcoc;

public static class Waypoint
{
/// §12.2
    public static byte[] FoldCheckpoints(byte[][] checkpoints)
    {
        var f = new byte[32];
        foreach (var c in checkpoints)
        {
            var pre = new byte[64];
            System.Buffer.BlockCopy(f, 0, pre, 0, 32);
            System.Buffer.BlockCopy(c, 0, pre, 32, 32);
            f = Sha256.Hash(pre);
        }
        return f;
    }

    /// §12.2
    public static byte[] Build(Terrain terrain, byte[] interrupt, byte[] stageWaypoint, byte[][] checkpoints)
    {
        var fold = FoldCheckpoints(checkpoints);
        var slotIndex = (int)(Bytes.U64Le(interrupt, 0) % (ulong)terrain.Profile.NSlots);
        var n = 32 + terrain.Profile.SlotBytes + 32 + 32;
        var pre = new byte[n];
        System.Buffer.BlockCopy(interrupt, 0, pre, 0, 32);
        System.Buffer.BlockCopy(terrain.Data, slotIndex * terrain.Profile.SlotBytes, pre, 32, terrain.Profile.SlotBytes);
        System.Buffer.BlockCopy(stageWaypoint, 0, pre, 32 + terrain.Profile.SlotBytes, 32);
        System.Buffer.BlockCopy(fold, 0, pre, 32 + terrain.Profile.SlotBytes + 32, 32);
        return Sha256.Hash(pre);
    }
}
