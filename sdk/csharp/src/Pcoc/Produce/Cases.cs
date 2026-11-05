namespace Pcoc.Produce;

/// §13.3
[Spec("§13.3", "A stage-loop test rig checks one production stage of this drive.")]
public enum CaseId
{
    S1,
    S2,
    Prod
}

/// §13.3
[Spec("§13.3", "A stage-loop test rig checks one production stage of this drive.")]
public static class Cases
{
    public static bool EchoO1;
    public static bool EchoO2;
    public static int S1Legs = 1;
    public static int S2Stage5Legs = 3;

    [Spec("§10", "SHA-256 of an ASCII pcoc-pack-2.2/ label.")]
    public static byte[] LabelDigest(string label)
    {
        return Sha256.Hash(System.Text.Encoding.ASCII.GetBytes("pcoc-pack-2.2/" + label));
    }

    [Spec("§16.1", "The production profile is SAMPLE_BYTES 1024, SLOT_BYTES 16384, TERRAIN_BYTES 268435456, STEPS_PER_LEG 2000000.")]
    public static Profile ProfileFor(CaseId id)
    {
        if (id == CaseId.S1)
        {
            Profile.TrySmall(32, 64, 65536, 100, out var p, out _);
            return p;
        }
        if (id == CaseId.S2)
        {
            Profile.TrySmall(64, 256, 1048576, 1000, out var p, out _);
            return p;
        }
        return Profile.Production;
    }

    [Spec("§13.3", "A stage-loop test rig checks one production stage of this drive.")]
    public static ScriptedRig RigFor(CaseId id)
    {
        var legs = new int[Const.Stages];
        for (var g = 0; g < Const.Stages; g++)
        {
            if (id == CaseId.S2)
            {
                legs[g] = g == 5 ? S2Stage5Legs : 3;
            }
            else
            {
                legs[g] = id == CaseId.S1 ? S1Legs : 1;
            }
        }
        var interrupts = new Tip[18];
        for (var k = 0; k < 18; k++)
        {
            interrupts[k] = new Tip(LabelDigest("beacon/" + k.ToString(System.Globalization.CultureInfo.InvariantCulture)), 1001UL + (ulong)k, null);
        }
        return new ScriptedRig(LabelDigest("beacon/seed"), 1000, interrupts, legs);
    }

    [Spec("§13.2", "Inputs are the 32-byte public key and one plane profile")]
    public static byte[] PackPublicKey()
    {
        return System.Convert.FromHexString("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
    }
}

/// §13.3
[Spec("§13.3", "A stage-loop test rig checks one production stage of this drive.")]
public sealed class ScriptedRig
{
    public byte[] SeedBeacon { get; }
    public ulong SeedHeight { get; }
    public Tip[] Interrupts { get; }
    public int[] LegsPerStage { get; }

    [Spec("§13.3", "A stage-loop test rig checks one production stage of this drive.")]
    public ScriptedRig(byte[] seedBeacon, ulong seedHeight, Tip[] interrupts, int[] legsPerStage)
    {
        SeedBeacon = seedBeacon;
        SeedHeight = seedHeight;
        Interrupts = interrupts;
        LegsPerStage = legsPerStage;
    }
}
