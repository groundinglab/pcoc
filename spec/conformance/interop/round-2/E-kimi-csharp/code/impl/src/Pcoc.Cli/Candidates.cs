using System;
using System.Collections.Generic;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace Pcoc.Cli;

/// <summary>
/// The deterministic candidate generator for the round-2 interop fixtures.
/// Writes the fourteen case files with exactly the keys the CASES file
/// names: lines "key<TAB>value<LF>", ASCII, sorted by key in byte order,
/// the file ending with the LF of its last line.
/// </summary>
public static class Candidates
{
    public static readonly string[] FileNames =
    {
        "seed.txt", "nl4.txt", "terrain.txt", "fold.txt", "kat-small.txt", "test-3leg.txt",
        "prod.txt", "wes.txt", "rate.txt", "anchor.txt", "draw.txt", "unknown-kind.txt",
        "hashcash.txt", "negatives.txt",
    };

    private static string U64(ulong v) => v.ToString("x16");
    private static string Hex(byte[] b) => Fixtures.HexLower(b);

    /// <summary>Generates every case file into outDir. Deterministic.</summary>
    public static void Generate(string outDir, TextWriter log)
    {
        Directory.CreateDirectory(outDir);
        var s1 = Fixtures.S1;
        var s2 = Fixtures.S2;
        var prod = Pcoc.Profile.Production;

        // Shared computations: three terrain fills, two hikes, one production leg.
        log.WriteLine("candidates: S1/S2 terrain fills");
        byte[] seedA1 = Pcoc.Terrain.Seed(Fixtures.PkA, Fixtures.SeedBeacon);
        var terrainS1 = Pcoc.Terrain.Fill(s1, seedA1);
        var terrainS2 = Pcoc.Terrain.Fill(s2, seedA1);
        var lanesA = new ulong[4];
        Pcoc.Terrain.Lanes(seedA1, lanesA);

        log.WriteLine("candidates: PROD terrain fill (this is the slow one)");
        var terrainProd = Pcoc.Terrain.Fill(prod, seedA1);

        WriteSeed(outDir);
        WriteNl4(outDir, terrainS1, lanesA);
        WriteTerrain(outDir, terrainS1, terrainS2, terrainProd);
        WriteFold(outDir, terrainS1, seedA1);

        log.WriteLine("candidates: S1 hike (18 × 1)");
        var o1 = Producer.Produce(s1, Fixtures.PkA, Ones(18), Fixtures.SeedBeacon,
            Fixtures.AllInterrupts(), Fixtures.SeedHeight, TextWriter.Null);
        WriteKatSmall(outDir, "kat-small", "S1", terrainS1, o1, 1);

        log.WriteLine("candidates: S2 hike (18 × 3)");
        var o2 = Producer.Produce(s2, Fixtures.PkA, Threes(18), Fixtures.SeedBeacon,
            Fixtures.AllInterrupts(), Fixtures.SeedHeight, TextWriter.Null);
        WriteKatSmall(outDir, "test-3leg", "S2", terrainS2, o2, 3);

        log.WriteLine("candidates: PROD first leg");
        WriteProd(outDir, terrainProd, seedA1);

        WriteWes(outDir, terrainS1, seedA1);
        WriteRate(outDir);
        WriteAnchor(outDir);
        WriteDraw(outDir);
        WriteUnknownKind(outDir, o1);
        WriteHashcash(outDir);
        log.WriteLine("candidates: negatives");
        WriteNegatives(outDir);
        log.WriteLine("candidates: done");
    }

    private static int[] Ones(int n) { var a = new int[n]; Array.Fill(a, 1); return a; }
    private static int[] Threes(int n) { var a = new int[n]; Array.Fill(a, 3); return a; }

    private static void Emit(string outDir, string file, SortedDictionary<string, string> lines)
    {
        var sb = new StringBuilder();
        foreach (var (k, v) in lines)
            sb.Append(k).Append('\t').Append(v).Append('\n');
        File.WriteAllText(Path.Combine(outDir, file), sb.ToString(), new UTF8Encoding(false));
    }

    private static void WriteSeed(string outDir)
    {
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal);
        foreach (var (name, pk) in new[] { ("pk_A", Fixtures.PkA), ("pk_B", Fixtures.PkB) })
        {
            var preimage = new byte[64];
            pk.CopyTo(preimage, 0);
            Fixtures.SeedBeacon.CopyTo(preimage, 32);
            byte[] seed = SHA256.HashData(preimage);
            var lanes = new ulong[4];
            Pcoc.Terrain.Lanes(seed, lanes);
            d[$"seed.{name}.preimage"] = Hex(preimage);
            d[$"seed.{name}.D"] = Hex(seed);
            for (int k = 0; k < 4; k++)
                d[$"seed.{name}.P{k}_u64"] = U64(lanes[k]);
        }
        Emit(outDir, "seed.txt", d);
    }

    private static void WriteNl4(string outDir, Pcoc.Terrain terrainS1, ulong[] lanes)
    {
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal);
        foreach (long i in new long[] { 0, 1, 1023 })
        {
            var slot = terrainS1.SlotWords(i);
            for (int j = 0; j < 8; j++)
            {
                d[$"nl4.S1.slot.{i}.word.{j}.base_u64"] = U64(Pcoc.Terrain.Nl4Base(i, j, lanes));
                d[$"nl4.S1.slot.{i}.word.{j}.stored_u64"] = U64(slot[j]);
            }
        }
        Emit(outDir, "nl4.txt", d);
    }

    private static void WriteTerrain(string outDir, Pcoc.Terrain terrainS1, Pcoc.Terrain terrainS2, Pcoc.Terrain terrainProd)
    {
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal)
        {
            ["terrain.S1.terrain_sha256"] = Hex(SHA256.HashData(terrainS1.Bytes)),
            ["terrain.S2.terrain_sha256"] = Hex(SHA256.HashData(terrainS2.Bytes)),
            ["terrain.PROD.terrain_sha256"] = Hex(SHA256.HashData(terrainProd.Bytes)),
        };
        Emit(outDir, "terrain.txt", d);
    }

    private static void WriteFold(string outDir, Pcoc.Terrain terrainS1, byte[] seedA)
    {
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal);
        var sink = new StepSink();
        ulong legResult = Pcoc.Leg.RunLeg(terrainS1, seedA, terrainS1.Profile.StepsPerLeg, sink);
        for (int n = 0; n < 4; n++)
        {
            var (s, i, acc, sNext) = sink.Steps[n];
            d[$"fold.S1.step.{n}.S_u64"] = U64(s);
            d[$"fold.S1.step.{n}.slot_index"] = i.ToString();
            d[$"fold.S1.step.{n}.acc_u64"] = U64(acc);
            d[$"fold.S1.step.{n}.S_next_u64"] = U64(sNext);
        }
        d["fold.S1.leg_result_u64"] = U64(legResult);
        Emit(outDir, "fold.txt", d);
    }

    private sealed class StepSink : Pcoc.IStepSink
    {
        public readonly List<(ulong S, long I, ulong Acc, ulong SNext)> Steps = new();
        public void Record(long t, ulong s, long slotIndex, ulong acc, ulong sNext)
        {
            if (Steps.Count < 4) Steps.Add((s, slotIndex, acc, sNext));
        }
    }

    private static void WriteKatSmall(string outDir, string prefix, string psuffix,
        Pcoc.Terrain terrain, Pcoc.HikeObject obj, int legsPerStage)
    {
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal)
        {
            [$"{prefix}.{psuffix}.terrain_sha256"] = Hex(SHA256.HashData(terrain.Bytes)),
        };
        for (int g = 0; g < Pcoc.Profile.Stages; g++)
        {
            var stage = obj.Stages[g];
            d[$"{prefix}.{psuffix}.stage.{g}.waypoint"] = Hex(stage.Waypoint);
            for (int j = 0; j < legsPerStage; j++)
            {
                d[$"{prefix}.{psuffix}.stage.{g}.leg.{j}.legs_le_hex"] = Hex(stage.LegResults[j]);
                d[$"{prefix}.{psuffix}.stage.{g}.leg.{j}.checkpoint"] = Hex(stage.Checkpoints[j]);
            }
        }
        Emit(outDir, $"{prefix}.txt", d);
    }

    private static void WriteProd(string outDir, Pcoc.Terrain terrainProd, byte[] seedA)
    {
        ulong leg = Pcoc.Leg.RunLeg(terrainProd, seedA, terrainProd.Profile.StepsPerLeg);
        var le = new byte[8];
        Pcoc.Prims.Le64(leg, le);
        byte[] checkpoint = Pcoc.Leg.Checkpoint(terrainProd, seedA, seedA, leg);

        var d = new SortedDictionary<string, string>(StringComparer.Ordinal)
        {
            ["prod.PROD.stage.0.leg.0.checkpoint"] = Hex(checkpoint),
            ["prod.PROD.stage.0.leg.0.legs_le_hex"] = Hex(le),
            ["prod.PROD.terrain_sha256"] = Hex(SHA256.HashData(terrainProd.Bytes)),
        };
        Emit(outDir, "prod.txt", d);
    }

    private static void WriteWes(string outDir, Pcoc.Terrain terrainS1, byte[] seedA)
    {
        byte[] stage0Waypoint = seedA; // stage 0's stored waypoint is the seed, by design
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal)
        {
            ["wes.S1.D"] = Hex(seedA),
            ["wes.S1.stage.0.waypoint"] = Hex(stage0Waypoint),
            ["wes.S1.equal"] = stage0Waypoint.AsSpan().SequenceEqual(seedA) ? "true" : "false",
        };
        Emit(outDir, "wes.txt", d);
    }

    private static void WriteRate(string outDir)
    {
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal);
        long[] legs = { 0, 1, 5, 6, 1000 };
        long[] spans = { 1, 300, 3600 };
        (long p, long q)[] floors = { (1, 5), (1, 4), (7, 20) };
        foreach (long l in legs)
            foreach (long s in spans)
                foreach (var (p, q) in floors)
                {
                    // Rule B2: q · hike_legs ≥ p · hike_span
                    d[$"rate.legs.{l}.span.{s}.floor.{p}_{q}"] = q * l >= p * s ? "meets" : "refuse";
                }
        Emit(outDir, "rate.txt", d);
    }

    private static void WriteAnchor(string outDir)
    {
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal);
        for (long h = 100; h <= 106; h++)
        {
            // ANCHOR_WINDOW_OFFSETS = 2, 3, 4 after the stored stage height H
            bool anchored = h - 100 >= 2 && h - 100 <= 4;
            d[$"anchor.H.100.height.{h}"] = anchored ? "anchored" : "not anchored";
        }
        Emit(outDir, "anchor.txt", d);
    }

    public const string DrawLabel = "pcoc-pack-2.2/draw";

    private static void WriteDraw(string outDir)
    {
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal)
        {
            ["draw.b0_label"] = DrawLabel,
            ["draw.thorough_rounds"] = "3",
        };
        foreach (ulong legs in new ulong[] { 6, 18, 54 })
        {
            // each of the draws reads the byte source from b_0
            if (!Pcoc.Draw.SixLeg(new Pcoc.ChainDrawBytes(DrawLabel), legs, out ulong[] six))
                throw new InvalidOperationException("six-leg draw exhausted");
            if (!Pcoc.Draw.Thorough(new Pcoc.ChainDrawBytes(DrawLabel), legs, 3, out ulong[] thorough))
                throw new InvalidOperationException("thorough draw exhausted");
            d[$"draw.legs.{legs}.six_leg.indices"] = string.Join(',', Array.ConvertAll(six, q => q.ToString()));
            d[$"draw.legs.{legs}.thorough.indices"] = string.Join(',', Array.ConvertAll(thorough, q => q.ToString()));
        }
        Emit(outDir, "draw.txt", d);
    }

    private static void WriteUnknownKind(string outDir, Pcoc.HikeObject o1)
    {
        string[] labels = { "v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT" };
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal);
        for (int n = 0; n < labels.Length; n++)
        {
            // O1 with only its kind value replaced, re-encoded (section 7.3 sorts it as Rule A5)
            var modified = new Pcoc.HikeObject
            {
                Kind = labels[n],
                Fill = o1.Fill,
                SlotBytes = o1.SlotBytes,
                NSlots = o1.NSlots,
                SampleBytes = o1.SampleBytes,
                StepsPerLeg = o1.StepsPerLeg,
                PublicKey = o1.PublicKey,
                SeedBeacon = o1.SeedBeacon,
                Stages = o1.Stages,
                LastInterrupt = o1.LastInterrupt,
                LastInterruptHeight = o1.LastInterruptHeight,
            };
            byte[] bytes = modified.Encode();
            var outcome = Pcoc.Verifier.Verify(bytes, new Pcoc.VerifyOptions
            {
                Profile = Fixtures.S1,
                Index = Fixtures.SyntheticIndex(),
                DrawBytes = new Pcoc.ChainDrawBytes(DrawLabel),
            });
            if (outcome.Status != Pcoc.VerifyStatus.UnknownKind)
                throw new InvalidOperationException($"label {n}: expected Rule A5, got {outcome.Status}");
            d[$"unknown-kind.{n}.kind_text_hex"] = Hex(Encoding.UTF8.GetBytes(labels[n]));
            d[$"unknown-kind.{n}.record_hex"] = Hex(Encoding.ASCII.GetBytes(outcome.Record));
        }
        Emit(outDir, "unknown-kind.txt", d);
    }

    private static void WriteHashcash(string outDir)
    {
        string origin = Pcoc.Hashcash.BariOrigin("https://example.com");
        var feeHeader = new byte[80];
        Fixtures.Sha256Ascii("pcoc-pack-2.2/hashcash/fee_header/0").CopyTo(feeHeader, 0);
        Fixtures.Sha256Ascii("pcoc-pack-2.2/hashcash/fee_header/1").CopyTo(feeHeader, 32);
        Array.Copy(Fixtures.Sha256Ascii("pcoc-pack-2.2/hashcash/fee_header/2"), 0, feeHeader, 64, 16);
        byte[] headerHash = SHA256.HashData(SHA256.HashData(feeHeader));
        byte[] waypoint = Fixtures.Sha256Ascii("pcoc-pack-2.2/hashcash/waypoint");
        byte[] challenge = Pcoc.Hashcash.Challenge(origin, headerHash);
        ulong nonce = Pcoc.Hashcash.Solve(challenge, waypoint);
        byte[] digest = Pcoc.Hashcash.Digest(challenge, waypoint, nonce);
        int lzb = Pcoc.Hashcash.LeadingZeroBits(digest);

        var d = new SortedDictionary<string, string>(StringComparer.Ordinal)
        {
            ["hashcash.bari_origin"] = origin,
            ["hashcash.challenge"] = Hex(challenge),
            ["hashcash.difficulty_bits"] = Pcoc.Hashcash.DifficultyBits.ToString(),
            ["hashcash.digest"] = Hex(digest),
            ["hashcash.fee_header"] = Hex(feeHeader),
            ["hashcash.header_hash"] = Hex(headerHash),
            ["hashcash.leading_zero_bits"] = lzb.ToString(),
            ["hashcash.nonce"] = nonce.ToString(),
            ["hashcash.waypoint"] = Hex(waypoint),
        };
        Emit(outDir, "hashcash.txt", d);
    }

    private static void WriteNegatives(string outDir)
    {
        negatives.Output output = negatives.RunAll(Fixtures.S1, Fixtures.PkA, Fixtures.PkB, Fixtures.SeedBeacon);
        if (!output.HonestLegRevisitsSlot)
            throw new InvalidOperationException("cached_leg: the honest first leg does not revisit a slot at S1 pk_A");
        var d = new SortedDictionary<string, string>(StringComparer.Ordinal)
        {
            ["negatives.honest.legs_le_hex"] = Hex(output.HonestLegLe),
            ["negatives.honest.terrain_sha256"] = Hex(output.HonestTerrainSha256),
        };
        foreach (negatives.Entry e in output.LegNegatives)
        {
            if (e.MatchesHonest)
                throw new InvalidOperationException($"negative {e.Name} equals the honest leg");
            d[$"negatives.{e.Name}.legs_le_hex"] = Hex(e.Value);
            d[$"negatives.{e.Name}.matches_honest"] = e.MatchesHonest ? "true" : "false";
        }
        foreach (negatives.Entry e in output.TerrainNegatives)
        {
            if (e.MatchesHonest)
                throw new InvalidOperationException($"negative {e.Name} equals the honest terrain");
            d[$"negatives.{e.Name}.terrain_sha256"] = Hex(e.Value);
            d[$"negatives.{e.Name}.matches_honest"] = e.MatchesHonest ? "true" : "false";
        }
        if (d.Count != 28)
            throw new InvalidOperationException($"negatives.txt must have 28 keys, has {d.Count}");
        Emit(outDir, "negatives.txt", d);
    }
}
