using System;
using System.Collections.Generic;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace Pcoc.Cli;

/// <summary>The fixed interop fixture inputs (round-2 CASES file).</summary>
public static class Fixtures
{
    public static readonly byte[] PkA = Hex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
    public static readonly byte[] PkB = Hex("dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659");

    /// <summary>SHA-256(ASCII "pcoc-pack-2.2/beacon/seed").</summary>
    public static readonly byte[] SeedBeacon = Sha256Ascii("pcoc-pack-2.2/beacon/seed");

    /// <summary>I_k = SHA-256(ASCII "pcoc-pack-2.2/beacon/" ‖ decimal k), k = 0 … 17.</summary>
    public static byte[] Interrupt(int k) => Sha256Ascii("pcoc-pack-2.2/beacon/" + k);

    public const long SeedHeight = 1000;
    public const long IndexLow = 989;   // seed height minus 11
    public const long IndexHigh = 1018; // last_interrupt_height

    public static readonly Pcoc.Profile S1 = Pcoc.Profile.ConformanceSmall(64, 32, 65536, 100);
    public static readonly Pcoc.Profile S2 = Pcoc.Profile.ConformanceSmall(256, 64, 1048576, 1000);

    public static byte[][] AllInterrupts()
    {
        var b = new byte[Pcoc.Profile.Stages][];
        for (int k = 0; k < Pcoc.Profile.Stages; k++) b[k] = Interrupt(k);
        return b;
    }

    /// <summary>The synthetic test-plane index the CASES file defines.</summary>
    public static Pcoc.SyntheticIndexSource SyntheticIndex()
    {
        var at = new Dictionary<long, byte[]> { [SeedHeight] = SeedBeacon };
        for (int k = 0; k < Pcoc.Profile.Stages; k++) at[SeedHeight + 1 + k] = Interrupt(k);
        return new Pcoc.SyntheticIndexSource(IndexLow, IndexHigh, at);
    }

    public static byte[] Sha256Ascii(string s) => SHA256.HashData(Encoding.ASCII.GetBytes(s));

    public static byte[] Hex(string hex)
    {
        var b = new byte[hex.Length / 2];
        for (int i = 0; i < b.Length; i++)
            b[i] = (byte)((Pcoc.JsonDoc.HexVal(hex[2 * i]) << 4) | Pcoc.JsonDoc.HexVal(hex[2 * i + 1]));
        return b;
    }

    public static string HexLower(byte[] b)
    {
        var sb = new StringBuilder(b.Length * 2);
        foreach (byte x in b) sb.Append(x.ToString("x2"));
        return sb.ToString();
    }
}

/// <summary>
/// The offline test-plane producer: a scripted clock with the fixed beacons
/// and heights, no Bari (every post misses, every path is the empty skip
/// bytes of section 13.3), the section 13.2 drive, and the deterministic
/// section 7.3 encoding. With the heights fixed, every core's objects are
/// byte-identical.
/// </summary>
public static class Producer
{
    /// <summary>
    /// Runs one hike: legs_per_stage[g] legs of STEPS_PER_LEG steps on stage
    /// g, then the scripted interrupt I_g at height seedHeight+1+g.
    /// </summary>
    public static Pcoc.HikeObject Produce(Pcoc.Profile profile, byte[] publicKey32,
        IReadOnlyList<int> legsPerStage, byte[] seedBeacon32, byte[][] interrupts18,
        long seedHeight, TextWriter stderr)
    {
        if (legsPerStage.Count != Pcoc.Profile.Stages)
            throw new ArgumentException("legs_per_stage must have 18 entries");
        byte[] seed = Pcoc.Terrain.Seed(publicKey32, seedBeacon32);
        var terrain = Pcoc.Terrain.Fill(profile, seed);

        var obj = new Pcoc.HikeObject
        {
            Kind = Pcoc.Profile.Kind,
            Fill = Pcoc.Profile.Fill,
            SlotBytes = (ulong)profile.SlotBytes,
            NSlots = (ulong)profile.NSlots,
            SampleBytes = (ulong)profile.SampleBytes,
            StepsPerLeg = (ulong)profile.StepsPerLeg,
            PublicKey = publicKey32,
            SeedBeacon = seedBeacon32,
        };

        byte[] waypoint = seed; // stage 0: the seed, by design equal to stages[0].waypoint
        for (int g = 0; g < Pcoc.Profile.Stages; g++)
        {
            var stage = new Pcoc.HikeStage
            {
                Beacon = g == 0 ? seedBeacon32 : interrupts18[g - 1],
                BeaconHeight = (ulong)(seedHeight + g),
                Waypoint = waypoint,
                Path = g == 0 ? null : Array.Empty<byte>(),
            };
            byte[] previous = waypoint;
            for (int j = 0; j < legsPerStage[g]; j++)
            {
                ulong legResult = Pcoc.Leg.RunLeg(terrain, previous, profile.StepsPerLeg);
                byte[] checkpoint = Pcoc.Leg.Checkpoint(terrain, previous, waypoint, legResult);
                stage.Checkpoints.Add(checkpoint);
                var le = new byte[8];
                Pcoc.Prims.Le64(legResult, le);
                stage.LegResults.Add(le);
                previous = checkpoint;
            }
            stage.LegCount = (ulong)stage.Checkpoints.Count;
            obj.Stages.Add(stage);
            if (g < Pcoc.Profile.Stages - 1)
                waypoint = Pcoc.Leg.Waypoint(terrain, interrupts18[g], waypoint, stage.Checkpoints);
        }
        obj.LastInterrupt = interrupts18[Pcoc.Profile.Stages - 1];
        obj.LastInterruptHeight = (ulong)(seedHeight + Pcoc.Profile.Stages);

        // The close (Rule A3.4): every ask missed on the offline plane.
        for (int g = 1; g <= Pcoc.Profile.Stages - 2; g++)
            stderr.WriteLine($"path_skip {g}");
        stderr.WriteLine("not fetched by design 17");
        return obj;
    }
}
