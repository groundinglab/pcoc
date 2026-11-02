namespace Pcoc;

/// §16.1
public sealed class Profile
{
    public string Name { get; }
    public int SampleBytes { get; }
    public int SlotBytes { get; }
    public int TerrainBytes { get; }
    public int StepsPerLeg { get; }
    public int WordsPerSlot { get; }
    public int NSlots { get; }
    public int OffsetModulus { get; }

    private Profile(string name, int sampleBytes, int slotBytes, int terrainBytes, int stepsPerLeg)
    {
        Name = name;
        SampleBytes = sampleBytes;
        SlotBytes = slotBytes;
        TerrainBytes = terrainBytes;
        StepsPerLeg = stepsPerLeg;
        WordsPerSlot = slotBytes / 8;
        NSlots = terrainBytes / slotBytes;
        OffsetModulus = terrainBytes - sampleBytes + 1;
    }

    public static Profile Production { get; } = new(
        "production", Const.SampleBytes, Const.SlotBytes, Const.TerrainBytes, Const.StepsPerLeg);

    /// §16.1
    public static bool TrySmall(int sampleBytes, int slotBytes, int terrainBytes, int stepsPerLeg, out Profile profile, out string error)
    {
        profile = Production;
        error = "";
        if (slotBytes < 8 || slotBytes > Const.SlotBytes || (slotBytes & 7) != 0)
        {
            error = "slot-bytes";
            return false;
        }
        if (stepsPerLeg < 1 || stepsPerLeg > Const.StepsPerLeg)
        {
            error = "steps-per-leg";
            return false;
        }
        if (sampleBytes < 1 || sampleBytes > Const.SampleBytes)
        {
            error = "sample-bytes";
            return false;
        }
        if (terrainBytes > Const.TerrainBytes || terrainBytes < sampleBytes || slotBytes == 0 || terrainBytes % slotBytes != 0)
        {
            error = "terrain-bytes";
            return false;
        }
        if (terrainBytes - sampleBytes + 1 < 1)
        {
            error = "terrain-bytes";
            return false;
        }
        profile = new Profile("conformance-small", sampleBytes, slotBytes, terrainBytes, stepsPerLeg);
        return true;
    }
}
