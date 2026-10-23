using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Security.Cryptography;

namespace Pcoc.Cli;

/// <summary>
/// The permanent negatives of SPEC section 16. They live only in the
/// generator, in this module, never on an SDK code path. Each negative is a
/// construction the SPEC forbids; the generator computes it, compares it
/// with the honest value of the same vector, and stops with an error when
/// they are equal.
/// </summary>
#pragma warning disable CS8981 // the required module name is exactly "negatives"
public static class negatives
{
    public sealed class Entry
    {
        public required string Name { get; init; }
        /// <summary>le64 of the leg result for a leg negative; the whole-terrain SHA-256 for a terrain negative.</summary>
        public required byte[] Value { get; init; }
        public required bool MatchesHonest { get; init; }
        public required bool IsTerrain { get; init; }
    }

    public sealed class Output
    {
        public required byte[] HonestLegLe { get; init; }
        public required byte[] HonestTerrainSha256 { get; init; }
        public required List<Entry> LegNegatives { get; init; }
        public required List<Entry> TerrainNegatives { get; init; }
        /// <summary>True when the honest first leg revisits at least one slot (required by cached_leg).</summary>
        public required bool HonestLegRevisitsSlot { get; init; }
    }

    public static readonly string[] LegNames =
    {
        "slot_summary_fold", "xor_accumulator_fold", "xor_w_fold", "s_plus_w_fold",
        "prefold_hoist", "final_stage_hoist", "partial_slot_read", "partial_slot_read_2047",
        "f3_shared_terrain", "cached_leg",
    };

    public static readonly string[] TerrainNames =
    {
        "one_lane_flip", "all4_kernel", "squeezed_seed_terrain",
    };

    /// <summary>
    /// Computes the honest first leg of stage 0 and every negative at the
    /// given profile (S1 for the round-2 vectors), pk_A with seed_beacon;
    /// f3_shared_terrain runs pk_A's leg on pk_B's terrain.
    /// </summary>
    public static Output RunAll(Pcoc.Profile profile, byte[] pkA, byte[] pkB, byte[] seedBeacon)
    {
        byte[] seedA = Pcoc.Terrain.Seed(pkA, seedBeacon);
        byte[] seedB = Pcoc.Terrain.Seed(pkB, seedBeacon);
        var terrainA = Pcoc.Terrain.Fill(profile, seedA);
        var terrainB = Pcoc.Terrain.Fill(profile, seedB);
        long steps = profile.StepsPerLeg;

        // honest leg, with the revisit check cached_leg requires
        var visited = new HashSet<long>();
        bool revisits = false;
        long nSlots = profile.NSlots;
        ulong sHonest = Pcoc.Prims.U64Le(seedA);
        for (long t = 0; t < steps; t++)
        {
            long i = (long)(sHonest % (ulong)nSlots);
            if (!visited.Add(i)) revisits = true;
            sHonest = Pcoc.Leg.Step(terrainA, sHonest);
        }
        ulong honestLeg = Pcoc.Leg.RunLeg(terrainA, seedA, steps);
        byte[] honestLe = Le64(honestLeg);
        byte[] honestTerrainSha = SHA256.HashData(terrainA.Bytes);

        var legs = new List<Entry>();
        void AddLeg(string name, ulong result)
        {
            byte[] le = Le64(result);
            legs.Add(new Entry
            {
                Name = name,
                Value = le,
                MatchesHonest = le.AsSpan().SequenceEqual(honestLe),
                IsTerrain = false,
            });
        }

        AddLeg("slot_summary_fold", RunCustomLeg(terrainA, seedA, steps, FoldSlotSummary));
        AddLeg("xor_accumulator_fold", RunCustomLeg(terrainA, seedA, steps, FoldXorAccumulator));
        AddLeg("xor_w_fold", RunCustomLeg(terrainA, seedA, steps, FoldXorW));
        AddLeg("s_plus_w_fold", RunCustomLeg(terrainA, seedA, steps, FoldSPlusW));
        AddLeg("prefold_hoist", RunCustomLeg(terrainA, seedA, steps, FoldPrefoldHoist));
        AddLeg("final_stage_hoist", RunCustomLeg(terrainA, seedA, steps, FoldFinalStageHoist));
        AddLeg("partial_slot_read", RunCustomLeg(terrainA, seedA, steps, FoldPartial1));
        AddLeg("partial_slot_read_2047", RunCustomLeg(terrainA, seedA, steps, FoldPartial2047));
        // pk_A's own start on pk_B's terrain (F4 shared terrain)
        AddLeg("f3_shared_terrain", RunCustomLeg(terrainB, seedA, steps, Pcoc.Leg.FoldScalar));
        AddLeg("cached_leg", RunCachedLeg(terrainA, seedA, steps));

        var lanesA = new ulong[4];
        Pcoc.Terrain.Lanes(seedA, lanesA);
        var terrains = new List<Entry>();
        void AddTerrain(string name, Func<long, long, ulong> baseWord)
        {
            ulong[] words = FillVariant(profile, lanesA, baseWord);
            byte[] sha = SHA256.HashData(MemoryMarshal.AsBytes(words.AsSpan()));
            terrains.Add(new Entry
            {
                Name = name,
                Value = sha,
                MatchesHonest = sha.AsSpan().SequenceEqual(honestTerrainSha),
                IsTerrain = true,
            });
        }

        // the one-lane map of section 10.4 in place of NL4
        AddTerrain("one_lane_flip", (i, j) =>
        {
            ulong c = ((ulong)i << 32) ^ (ulong)j ^ Pcoc.Profile.WordXor;
            return Pcoc.Prims.Mix64(lanesA[(int)(j & 3)] ^ c);
        });
        // the linear map of section 10.4 in place of NL4
        AddTerrain("all4_kernel", (i, j) =>
        {
            ulong c = ((ulong)i << 32) ^ (ulong)j ^ Pcoc.Profile.WordXor;
            ulong x = lanesA[(int)(j & 3)]
                ^ Pcoc.Prims.Rotl(lanesA[(int)((j + 1) & 3)], 17)
                ^ Pcoc.Prims.Rotl(lanesA[(int)((j + 2) & 3)], 31)
                ^ Pcoc.Prims.Rotl(lanesA[(int)((j + 3) & 3)], 47)
                ^ c;
            return Pcoc.Prims.Mix64(x);
        });
        // the section 10.2 squeeze: every lane replaced by P0 ^ P1 ^ P2 ^ P3
        ulong squeeze = lanesA[0] ^ lanesA[1] ^ lanesA[2] ^ lanesA[3];
        var squeezed = new[] { squeeze, squeeze, squeeze, squeeze };
        {
            ulong[] words = FillVariant(profile, squeezed, (i, j) => Pcoc.Terrain.Nl4Base(i, j, squeezed));
            byte[] sha = SHA256.HashData(MemoryMarshal.AsBytes(words.AsSpan()));
            terrains.Add(new Entry
            {
                Name = "squeezed_seed_terrain",
                Value = sha,
                MatchesHonest = sha.AsSpan().SequenceEqual(honestTerrainSha),
                IsTerrain = true,
            });
        }

        return new Output
        {
            HonestLegLe = honestLe,
            HonestTerrainSha256 = honestTerrainSha,
            LegNegatives = legs,
            TerrainNegatives = terrains,
            HonestLegRevisitsSlot = revisits,
        };
    }

    // ---------------- forbidden step variants ----------------

    private delegate ulong FoldVariant(ReadOnlySpan<ulong> slot, ulong s);

    private static ulong RunCustomLeg(Pcoc.Terrain terrain, byte[] start32, long steps, FoldVariant fold)
    {
        long nSlots = terrain.Profile.NSlots;
        ulong s = Pcoc.Prims.U64Le(start32);
        for (long t = 0; t < steps; t++)
        {
            long i = (long)(s % (ulong)nSlots);
            s = Pcoc.Leg.Seal(fold(terrain.SlotWords(i), s));
        }
        return s;
    }

    /// <summary>F1: S_folded = S XOR (XOR over w of (word(w) XOR w)) — independent of S inside.</summary>
    private static ulong FoldSlotSummary(ReadOnlySpan<ulong> slot, ulong s)
    {
        ulong x = 0;
        for (int w = 0; w < slot.Length; w++) x ^= slot[w] ^ (ulong)w;
        return s ^ x;
    }

    /// <summary>acc = acc XOR mix64(word(w) XOR w XOR S).</summary>
    private static ulong FoldXorAccumulator(ReadOnlySpan<ulong> slot, ulong s)
    {
        ulong x = 0;
        for (int w = 0; w < slot.Length; w++) x ^= Pcoc.Prims.Mix64(slot[w] ^ (ulong)w ^ s);
        return x;
    }

    /// <summary>acc = Σ mix64(word(w) XOR w XOR S) — word(w) XOR w does not depend on S.</summary>
    private static ulong FoldXorW(ReadOnlySpan<ulong> slot, ulong s)
    {
        ulong acc = 0;
        for (int w = 0; w < slot.Length; w++) acc += Pcoc.Prims.Mix64(slot[w] ^ (ulong)w ^ s);
        return acc;
    }

    /// <summary>acc = Σ mix64(word(w) XOR ((S + w) mod 2^64)).</summary>
    private static ulong FoldSPlusW(ReadOnlySpan<ulong> slot, ulong s)
    {
        ulong acc = 0;
        for (int w = 0; w < slot.Length; w++) acc += Pcoc.Prims.Mix64(slot[w] ^ (s + (ulong)w));
        return acc;
    }

    /// <summary>S_folded = pre(i) + S with pre(i) = Σ mix64(word(w) XOR w).</summary>
    private static ulong FoldPrefoldHoist(ReadOnlySpan<ulong> slot, ulong s)
    {
        ulong pre = 0;
        for (int w = 0; w < slot.Length; w++) pre += Pcoc.Prims.Mix64(slot[w] ^ (ulong)w);
        return pre + s;
    }

    /// <summary>S_folded = x XOR (x >> 31), x = XOR of mix64-without-final-line(word(w) XOR S).</summary>
    private static ulong FoldFinalStageHoist(ReadOnlySpan<ulong> slot, ulong s)
    {
        ulong x = 0;
        for (int w = 0; w < slot.Length; w++)
        {
            ulong y = (slot[w] ^ s) + Pcoc.Profile.MixAdd;
            y = (y ^ (y >> 30)) * Pcoc.Profile.MixMul1;
            y = (y ^ (y >> 27)) * Pcoc.Profile.MixMul2;
            x ^= y; // the value just before return y XOR (y >> 31)
        }
        return x ^ (x >> 31);
    }

    /// <summary>The section 11.1 fold over word 0 only.</summary>
    private static ulong FoldPartial1(ReadOnlySpan<ulong> slot, ulong s) => Pcoc.Prims.Mix64(slot[0] ^ s);

    /// <summary>The section 11.1 fold over words 0 … WORDS_PER_SLOT − 2.</summary>
    private static ulong FoldPartial2047(ReadOnlySpan<ulong> slot, ulong s)
    {
        ulong acc = 0;
        for (int w = 0; w < slot.Length - 1; w++) acc += Pcoc.Prims.Mix64(slot[w] ^ s);
        return acc;
    }

    /// <summary>Reuses the S_folded of an earlier read of the same slot in the same leg.</summary>
    private static ulong RunCachedLeg(Pcoc.Terrain terrain, byte[] start32, long steps)
    {
        long nSlots = terrain.Profile.NSlots;
        var memo = new Dictionary<long, ulong>();
        ulong s = Pcoc.Prims.U64Le(start32);
        for (long t = 0; t < steps; t++)
        {
            long i = (long)(s % (ulong)nSlots);
            if (!memo.TryGetValue(i, out ulong folded))
            {
                folded = Pcoc.Leg.FoldScalar(terrain.SlotWords(i), s);
                memo[i] = folded;
            }
            s = Pcoc.Leg.Seal(folded);
        }
        return s;
    }

    // ---------------- forbidden terrain variants ----------------

    /// <summary>The section 10.4 dag2 fill with a replaced base word; parents and slot_mix unchanged.</summary>
    private static ulong[] FillVariant(Pcoc.Profile profile, ulong[] lanes, Func<long, long, ulong> baseWord)
    {
        int wps = profile.WordsPerSlot;
        long total = profile.NSlots * wps;
        var words = new ulong[total];
        for (int j = 0; j < wps; j++) words[j] = baseWord(0, j); // slot 0: base only
        for (long i = 1; i < profile.NSlots; i++)
        {
            long p1 = i - 1, p2 = Pcoc.Prims.BitrevParent(i), p3 = Pcoc.Prims.MulParent(i);
            ulong slotMix = (ulong)i * Pcoc.Profile.SlotMul;
            int dst = (int)(i * wps);
            int s1 = (int)(p1 * wps), s2 = (int)(p2 * wps), s3 = (int)(p3 * wps);
            for (int j = 0; j < wps; j++)
            {
                ulong t = baseWord(i, j);
                t = Pcoc.Prims.Mix64((t ^ Pcoc.Prims.Rotl(words[s1 + j], (j + 1) & 63)) ^ unchecked(1UL * Pcoc.Profile.ParentMul));
                t = Pcoc.Prims.Mix64((t ^ Pcoc.Prims.Rotl(words[s2 + j], (j + 2) & 63)) ^ unchecked(2UL * Pcoc.Profile.ParentMul));
                t = Pcoc.Prims.Mix64((t ^ Pcoc.Prims.Rotl(words[s3 + j], (j + 3) & 63)) ^ unchecked(3UL * Pcoc.Profile.ParentMul));
                t = Pcoc.Prims.Mix64(t ^ slotMix);
                words[dst + j] = t;
            }
        }
        return words;
    }

    private static byte[] Le64(ulong x)
    {
        var b = new byte[8];
        Pcoc.Prims.Le64(x, b);
        return b;
    }
}
