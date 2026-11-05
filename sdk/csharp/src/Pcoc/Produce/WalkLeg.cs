namespace Pcoc.Produce;

/// §11.2
[Spec("§11.2", "leg_result ← LegResult(terrain, previous)")]
public static class WalkLeg
{
    public static bool ReuseSlot;
    public static bool HalfWords;
    public static bool SlotSummaryFold;
    public static bool FlipFirstResult;

    [Spec("§11.1", "each step must be driven by the full 16 KB slot")]
    public static ulong Apply(Terrain terrain, byte[] previousCheckpoint)
    {
        var s = Bytes.U64Le(previousCheckpoint, 0);
        var steps = terrain.Profile.StepsPerLeg;
        byte[]? held = null;
        for (var t = 0; t < steps; t++)
        {
            var slotIndex = (int)(s % (ulong)terrain.Profile.NSlots);
            byte[] slot;
            // ReuseSlot (mutation N4a): once a slot is held, keep using it even when the index changes.
            if (ReuseSlot && held != null)
            {
                slot = held;
            }
            else
            {
                slot = new byte[terrain.Profile.SlotBytes];
                System.Buffer.BlockCopy(terrain.Data, slotIndex * terrain.Profile.SlotBytes, slot, 0, terrain.Profile.SlotBytes);
                held = slot;
            }
            var words = HalfWords ? terrain.Profile.WordsPerSlot / 2 : terrain.Profile.WordsPerSlot;
            ulong folded;
            if (SlotSummaryFold)
            {
                ulong inner = 0;
                for (var w = 0; w < words; w++)
                {
                    inner ^= Bytes.U64Le(slot, 8 * w) ^ (ulong)w;
                }
                folded = s ^ inner;
            }
            else
            {
                ulong acc = 0;
                for (var w = 0; w < words; w++)
                {
                    var word = Bytes.U64Le(slot, 8 * w);
                    acc += Mix.Mix64(word ^ s);
                }
                folded = acc;
            }
            var seal = Sha256.Seal(folded);
            s = Bytes.U64Le(seal, 0);
        }
        if (FlipFirstResult)
        {
            s ^= 1UL;
        }
        return s;
    }
}
