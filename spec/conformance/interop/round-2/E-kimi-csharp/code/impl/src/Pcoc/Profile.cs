using System;
namespace Pcoc;

/// <summary>
/// SPEC section 6 constants and the section 16.1 <c>conformance-small</c> profile.
/// A profile may differ from section 6 only in SLOT_BYTES, SAMPLE_BYTES,
/// TERRAIN_BYTES and STEPS_PER_LEG; WORDS_PER_SLOT, N_SLOTS and OFFSET_MODULUS
/// stay the section 6 formulas applied to the profile values (P6.23–P6.25).
/// </summary>
public sealed class Profile
{
    // SPEC 6, production values.
    public const int ProdSlotBytes = 16384;
    public const int ProdSampleBytes = 1024;
    public const long ProdTerrainBytes = 268435456L;
    public const long ProdStepsPerLeg = 2000000L;

    // SPEC 6, profile-independent constants.
    public const int Stages = 18;
    public const int HashBytes = 32;
    public const int LegResultBytes = 8;
    public const int PublicKeyBytes = 32;
    public const int BeaconBytes = 32;
    public const ulong MixAdd = 0x9E3779B97F4A7C15UL;
    public const ulong MixMul1 = 0xBF58476D1CE4E5B9UL;
    public const ulong MixMul2 = 0x94D049BB133111EBUL;
    public const ulong WordXor = 0xC0FFEEUL;
    public const ulong ParentMul = 0xD1B54A32D192ED03UL;
    public const ulong SlotMul = 0xA24BAED4963EE407UL;

    /// <summary>SPEC 6: the twelve ASCII bytes <c>pcoc-walk-v1</c>.</summary>
    public static readonly byte[] SealDomain = "pcoc-walk-v1"u8.ToArray();

    /// <summary>SPEC 6: <c>v2.2-pcoc-object</c>.</summary>
    public const string Kind = "v2.2-pcoc-object";

    /// <summary>SPEC 6: <c>dag2</c>.</summary>
    public const string Fill = "dag2";

    public int SlotBytes { get; }
    public int SampleBytes { get; }
    public long TerrainBytes { get; }
    public long StepsPerLeg { get; }

    public int WordsPerSlot { get; }   // SLOT_BYTES / 8, exact (P6.23)
    public long NSlots { get; }        // TERRAIN_BYTES / SLOT_BYTES, exact (P6.24)
    public long OffsetModulus { get; } // TERRAIN_BYTES - SAMPLE_BYTES + 1 (P6.25)

    public string Name { get; }        // "production" or "conformance-small"

    private Profile(string name, int slotBytes, int sampleBytes, long terrainBytes, long stepsPerLeg)
    {
        Name = name;
        SlotBytes = slotBytes;
        SampleBytes = sampleBytes;
        TerrainBytes = terrainBytes;
        StepsPerLeg = stepsPerLeg;
        WordsPerSlot = slotBytes / 8;
        NSlots = terrainBytes / slotBytes;
        OffsetModulus = terrainBytes - sampleBytes + 1;
    }

    public static readonly Profile Production =
        new("production", ProdSlotBytes, ProdSampleBytes, ProdTerrainBytes, ProdStepsPerLeg);

    /// <summary>
    /// Builds a conformance-small profile, enforcing the section 16.1 constraints.
    /// </summary>
    public static Profile ConformanceSmall(int slotBytes, int sampleBytes, long terrainBytes, long stepsPerLeg)
    {
        if (slotBytes < 8 || slotBytes % 8 != 0 || slotBytes > ProdSlotBytes)
            throw new ArgumentException("SLOT_BYTES must be a multiple of 8 in 8 … 16384");
        if (sampleBytes < 1 || sampleBytes > ProdSampleBytes)
            throw new ArgumentException("SAMPLE_BYTES must be in 1 … 1024");
        if (terrainBytes > ProdTerrainBytes || terrainBytes % (long)slotBytes != 0 || terrainBytes < sampleBytes)
            throw new ArgumentException("TERRAIN_BYTES must be at most the section 6 value, an exact multiple of SLOT_BYTES, and at least SAMPLE_BYTES");
        if (stepsPerLeg < 1 || stepsPerLeg > ProdStepsPerLeg)
            throw new ArgumentException("STEPS_PER_LEG must be in 1 … 2000000");
        return new Profile("conformance-small", slotBytes, sampleBytes, terrainBytes, stepsPerLeg);
    }
}
