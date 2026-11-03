namespace Pcoc;

public static class Step
{
/// §11.1
    public readonly struct Result
    {
        public ulong Next { get; }
        public ulong Folded { get; }
        public int SlotIndex { get; }

        public Result(ulong next, ulong folded, int slotIndex)
        {
            Next = next;
            Folded = folded;
            SlotIndex = slotIndex;
        }
    }

    /// §11.1
    public static Result Apply(Terrain terrain, ulong s)
    {
        var slotIndex = (int)(s % (ulong)terrain.Profile.NSlots);
        var folded = FoldSlot(terrain.Data, slotIndex * terrain.Profile.SlotBytes, terrain.Profile.WordsPerSlot, s);
        var seal = Sha256.Seal(folded);
        var next = Bytes.U64Le(seal, 0);
        return new Result(next, folded, slotIndex);
    }

    /// §11.1
    public static ulong FoldSlot(byte[] terrain, int offset, int words, ulong s)
    {
        ulong acc = 0;
        for (var w = 0; w < words; w++)
        {
            var word = Bytes.U64Le(terrain, offset + 8 * w);
            acc += Mix.Mix64(word ^ s);
        }
        return acc;
    }
}
