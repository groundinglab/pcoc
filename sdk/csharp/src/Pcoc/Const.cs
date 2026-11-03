namespace Pcoc;

/// §6
public static class Const
{
    public const int SlotBytes = 16384;
    public const int WordsPerSlot = 2048;
    public const int TerrainBytes = 268435456;
    public const int NSlots = 16384;
    public const int SampleBytes = 1024;
    public const int OffsetModulus = 268434433;
    public const int StepsPerLeg = 2000000;
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
    public const int HashcashBits = 21;
    public const int AnchorOffset0 = 2;
    public const int AnchorOffset1 = 3;
    public const int AnchorOffset2 = 4;
    public const int SixLegCount = 6;
    public const ulong PowLimitCompact = 0x1d00ffffUL;
    public const ulong SynthNBits = 0x207fffffUL;
    public const long RetargetSpacing = 2016;
    public const long TargetTimespan = 1209600;

    public static readonly byte[] SealDomain = System.Text.Encoding.ASCII.GetBytes("pcoc-walk-v1");
    public static readonly byte[] Kind = System.Text.Encoding.ASCII.GetBytes("v2.2-pcoc-object");
    public static readonly byte[] Fill = System.Text.Encoding.ASCII.GetBytes("dag2");
    public static readonly byte[] HashcashPrefix = System.Text.Encoding.ASCII.GetBytes("bari-hc-v2");

    public const string KindText = "v2.2-pcoc-object";
    public const string FillText = "dag2";
}
