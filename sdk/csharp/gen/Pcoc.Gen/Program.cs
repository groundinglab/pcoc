using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using Pcoc;

namespace Pcoc.Gen;

public static class Program
{
    private static readonly byte[] PkA = FromHex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
    private static readonly byte[] PkB = FromHex("dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659");
    private static readonly byte[] Beacon = FromHex("bac91a1b52b8ce663404ef4c275df559246083d86ce925ab52d46c67735fb4db");

    /// §16
    public static int Main(string[] args)
    {
        var dir = args.Length >= 1 ? args[0] : "candidates";
        Directory.CreateDirectory(dir);
        if (!Profile.TrySmall(32, 64, 65536, 100, out var s1, out _)
            || !Profile.TrySmall(64, 256, 1048576, 1000, out var s2, out _))
        {
            return 1;
        }
        var rows = new SortedDictionary<string, List<(string, string)>>(StringComparer.Ordinal);
        void Add(string file, string key, string value)
        {
            if (!rows.TryGetValue(file, out var list))
            {
                list = new List<(string, string)>();
                rows[file] = list;
            }
            list.Add((key, value));
        }
        SeedRows(Add);
        var terrainS1 = Terrain.Fill(PkA, Beacon, s1);
        var terrainS2 = Terrain.Fill(PkA, Beacon, s2);
        var terrainB = Terrain.Fill(PkB, Beacon, s1);
        Nl4(Add, terrainS1);
        Add("terrain.txt", "terrain.PROD.terrain_sha256", ProdTerrain());
        Add("terrain.txt", "terrain.S1.terrain_sha256", Bytes.Hex(terrainS1.Digest()));
        Add("terrain.txt", "terrain.S2.terrain_sha256", Bytes.Hex(terrainS2.Digest()));
        Fold(Add, terrainS1);
        Kat(Add, "kat-small", "S1", terrainS1, 1);
        Kat(Add, "test-3leg", "S2", terrainS2, 3);
        Add("prod.txt", "prod.PROD.stage.0.leg.0.checkpoint", "");
        Add("prod.txt", "prod.PROD.stage.0.leg.0.legs_le_hex", "");
        Add("prod.txt", "prod.PROD.terrain_sha256", "");
        Prod(rows["prod.txt"]);
        Wes(Add);
        RateRows(Add);
        AnchorRows(Add);
        DrawRows(Add);
        HashcashRows(Add);
        Br1Rows(Add);
        Negatives(Add, terrainS1, terrainB);
        Unknown(Add);
        foreach (var (file, list) in rows)
        {
            list.Sort((a, b) => string.CompareOrdinal(a.Item1, b.Item1));
            var sb = new StringBuilder();
            foreach (var (k, v) in list)
            {
                sb.Append(k).Append('\t').Append(v).Append('\n');
            }
            File.WriteAllText(Path.Combine(dir, file), sb.ToString());
        }
        return 0;
    }

    private static void SeedRows(Action<string, string, string> add)
    {
        void One(string name, byte[] pk)
        {
            var pre = Seed.Preimage(pk, Beacon);
            var d = Sha256.Hash(pre);
            var lanes = Seed.Lanes(d);
            add("seed.txt", "seed." + name + ".D", Bytes.Hex(d));
            add("seed.txt", "seed." + name + ".P0_u64", lanes[0].ToString("x16"));
            add("seed.txt", "seed." + name + ".P1_u64", lanes[1].ToString("x16"));
            add("seed.txt", "seed." + name + ".P2_u64", lanes[2].ToString("x16"));
            add("seed.txt", "seed." + name + ".P3_u64", lanes[3].ToString("x16"));
            add("seed.txt", "seed." + name + ".preimage", Bytes.Hex(pre));
        }
        One("pk_A", PkA);
        One("pk_B", PkB);
    }

    private static void Nl4(Action<string, string, string> add, Terrain terrain)
    {
        var lanes = terrain.Lanes;
        int[] slots = { 0, 1, 1023 };
        foreach (var i in slots)
        {
            var w = new ulong[terrain.Profile.WordsPerSlot];
            Terrain.BaseWords(lanes, i, w);
            for (var j = 0; j < 8; j++)
            {
                var stored = Bytes.U64Le(terrain.Data, i * terrain.Profile.SlotBytes + 8 * j);
                add("nl4.txt", "nl4.S1.slot." + i + ".word." + j + ".base_u64", w[j].ToString("x16"));
                add("nl4.txt", "nl4.S1.slot." + i + ".word." + j + ".stored_u64", stored.ToString("x16"));
            }
        }
    }

    private static void Fold(Action<string, string, string> add, Terrain terrain)
    {
        var s = terrain.Lanes[0];
        for (var t = 0; t < 4; t++)
        {
            var step = Step.Apply(terrain, s);
            add("fold.txt", "fold.S1.step." + t + ".S_next_u64", step.Next.ToString("x16"));
            add("fold.txt", "fold.S1.step." + t + ".S_u64", s.ToString("x16"));
            add("fold.txt", "fold.S1.step." + t + ".acc_u64", step.Folded.ToString("x16"));
            add("fold.txt", "fold.S1.step." + t + ".slot_index", step.SlotIndex.ToString(System.Globalization.CultureInfo.InvariantCulture));
            s = step.Next;
        }
        var leg = Leg.Result(terrain, Seed.Digest(PkA, Beacon));
        add("fold.txt", "fold.S1.leg_result_u64", leg.ToString("x16"));
    }

    private static void Kat(Action<string, string, string> add, string file, string tag, Terrain terrain, int legs)
    {
        var name = file + ".txt";
        var seed = Seed.Digest(PkA, Beacon);
        add(name, file + "." + tag + ".terrain_sha256", Bytes.Hex(terrain.Digest()));
        byte[] waypoint = seed;
        for (var g = 0; g < 18; g++)
        {
            add(name, file + "." + tag + ".stage." + g + ".waypoint", Bytes.Hex(waypoint));
            var limb = waypoint;
            var previous = waypoint;
            var cps = new byte[legs][];
            for (var j = 0; j < legs; j++)
            {
                var leg = Leg.Result(terrain, previous);
                var cp = Checkpoint.Bind(terrain, previous, limb, leg);
                add(name, file + "." + tag + ".stage." + g + ".leg." + j + ".checkpoint", Bytes.Hex(cp));
                add(name, file + "." + tag + ".stage." + g + ".leg." + j + ".legs_le_hex", Bytes.Hex(Bytes.Le64(leg)));
                cps[j] = cp;
                previous = cp;
            }
            if (g < 17)
            {
                var interrupt = Interrupt(g);
                waypoint = Waypoint.Build(terrain, interrupt, waypoint, cps);
            }
        }
    }

    private static byte[] Interrupt(int g)
    {
        return Sha256.Hash(Encoding.ASCII.GetBytes("pcoc-pack-2.2/beacon/" + g.ToString(System.Globalization.CultureInfo.InvariantCulture)));
    }

    private static string ProdTerrain()
    {
        var text = File.ReadAllText(FindPack("terrain.json"));
        return Between(text, "\"terrain.PROD.terrain_sha256\": \"", "\"");
    }

    private static void Prod(List<(string, string)> list)
    {
        var text = File.ReadAllText(FindPack("prod.json"));
        for (var i = 0; i < list.Count; i++)
        {
            var key = list[i].Item1;
            var value = Between(text, "\"" + key + "\": \"", "\"");
            list[i] = (key, value);
        }
    }

    private static void Wes(Action<string, string, string> add)
    {
        var d = Seed.Digest(PkA, Beacon);
        var hex = Bytes.Hex(d);
        add("wes.txt", "wes.S1.D", hex);
        add("wes.txt", "wes.S1.equal", "true");
        add("wes.txt", "wes.S1.stage.0.waypoint", hex);
    }

    private static void RateRows(Action<string, string, string> add)
    {
        long[] legs = { 0, 1, 5, 6, 1000 };
        long[] spans = { 1, 300, 3600 };
        (long P, long Q)[] floors = { (1, 4), (1, 5), (7, 20) };
        foreach (var legsN in legs)
        {
            foreach (var span in spans)
            {
                foreach (var (p, q) in floors)
                {
                    var key = "rate.legs." + legsN + ".span." + span + ".floor." + p + "_" + q;
                    add("rate.txt", key, Rate.Compare(legsN, span, p, q));
                }
            }
        }
    }

    private static void AnchorRows(Action<string, string, string> add)
    {
        for (var h = 100; h <= 106; h++)
        {
            add("anchor.txt", "anchor.H.100.height." + h, Anchor.Compare(100, h));
        }
    }

    private static void DrawRows(Action<string, string, string> add)
    {
        add("draw.txt", "draw.b0_label", "pcoc-pack-2.2/draw");
        add("draw.txt", "draw.thorough_rounds", "3");
        int[] totals = { 6, 18, 54 };
        foreach (var total in totals)
        {
            var six = Draw.Six(Draw.LabelSource("pcoc-pack-2.2/draw"), total);
            var thorough = Draw.Thorough(Draw.LabelSource("pcoc-pack-2.2/draw"), total, 3);
            add("draw.txt", "draw.legs." + total + ".six_leg.indices", Join(six!));
            add("draw.txt", "draw.legs." + total + ".thorough.indices", Join(thorough!));
        }
    }

    private static void HashcashRows(Action<string, string, string> add)
    {
        var text = File.ReadAllText(FindPack("hashcash.json"));
        string V(string k) => Between(text, "\"" + k + "\": \"", "\"");
        var origin = V("hashcash.bari_origin");
        var header = FromHex(V("hashcash.fee_header"));
        var waypoint = FromHex(V("hashcash.waypoint"));
        var headerHash = Hashcash.HeaderHash(header);
        var challenge = Hashcash.Challenge(origin, headerHash);
        ulong nonce = 0;
        byte[] digest = Array.Empty<byte>();
        for (ulong n = 0; n < ulong.MaxValue; n++)
        {
            digest = Hashcash.Digest(challenge, waypoint, n);
            if (Hashcash.Meets(digest, 21))
            {
                nonce = n;
                break;
            }
        }
        add("hashcash.txt", "hashcash.bari_origin", origin);
        add("hashcash.txt", "hashcash.challenge", Bytes.Hex(challenge));
        add("hashcash.txt", "hashcash.difficulty_bits", "21");
        add("hashcash.txt", "hashcash.digest", Bytes.Hex(digest));
        add("hashcash.txt", "hashcash.fee_header", V("hashcash.fee_header"));
        add("hashcash.txt", "hashcash.header_hash", Bytes.Hex(headerHash));
        add("hashcash.txt", "hashcash.leading_zero_bits", Hashcash.LeadingZeroBits(digest).ToString(System.Globalization.CultureInfo.InvariantCulture));
        add("hashcash.txt", "hashcash.nonce", nonce.ToString(System.Globalization.CultureInfo.InvariantCulture));
        add("hashcash.txt", "hashcash.waypoint", V("hashcash.waypoint"));
    }

    private static void Br1Rows(Action<string, string, string> add)
    {
        var text = File.ReadAllText(FindPack("br1.json"));
        var names = new[]
        {
            "after_live", "after_live_copy", "after_test", "before", "current",
            "display_after", "display_before", "display_current", "outside",
            "wrong_nbits_live", "wrong_nbits_test", "wrong_origin"
        };
        foreach (var name in names)
        {
            string V(string f) => Between(text, "\"br1." + name + "." + f + "\": \"", "\"");
            foreach (var f in new[] { "fee_header", "nonce", "origin", "period_first_time", "plane", "solve_origin", "tip_bits", "tip_hash", "tip_height", "tip_previous", "tip_time", "waypoint" })
            {
                add("br1.txt", "br1." + name + "." + f, V(f));
            }
            var tip = new Br1.Tip(
                FromHex(V("tip_hash")),
                FromHex(V("tip_previous")),
                Convert.ToUInt64(V("tip_bits"), 16),
                long.Parse(V("tip_height"), System.Globalization.CultureInfo.InvariantCulture),
                long.Parse(V("tip_time"), System.Globalization.CultureInfo.InvariantCulture),
                long.Parse(V("period_first_time"), System.Globalization.CultureInfo.InvariantCulture),
                V("plane") == "live");
            var status = Br1.Status(FromHex(V("waypoint")), V("nonce"), V("fee_header"), V("origin"), tip);
            var window = Br1.Window(FromHex(V("fee_header")), Hashcash.HeaderHash(FromHex(V("fee_header"))), tip) ?? "outside";
            add("br1.txt", "br1." + name + ".status", status.ToString(System.Globalization.CultureInfo.InvariantCulture));
            add("br1.txt", "br1." + name + ".window", window);
        }
    }

    private static void Negatives(Action<string, string, string> add, Terrain honestTerrain, Terrain other)
    {
        var honestLeg = Leg.Result(honestTerrain, Seed.Digest(PkA, Beacon));
        var honestHex = Bytes.Hex(Bytes.Le64(honestLeg));
        var honestDigest = Bytes.Hex(honestTerrain.Digest());
        add("negatives.txt", "negatives.honest.legs_le_hex", honestHex);
        add("negatives.txt", "negatives.honest.terrain_sha256", honestDigest);
        void LegNeg(string name, string hex)
        {
            add("negatives.txt", "negatives." + name + ".legs_le_hex", hex);
            add("negatives.txt", "negatives." + name + ".matches_honest", hex == honestHex ? "true" : "false");
        }
        void TerNeg(string name, string hex)
        {
            add("negatives.txt", "negatives." + name + ".matches_honest", hex == honestDigest ? "true" : "false");
            add("negatives.txt", "negatives." + name + ".terrain_sha256", hex);
        }
        LegNeg("slot_summary_fold", AltLeg(honestTerrain, FoldSlotSummary));
        LegNeg("xor_accumulator_fold", AltLeg(honestTerrain, FoldXorAcc));
        LegNeg("xor_w_fold", AltLeg(honestTerrain, FoldXorW));
        LegNeg("s_plus_w_fold", AltLeg(honestTerrain, FoldSPlusW));
        LegNeg("prefold_hoist", Prefold(honestTerrain));
        LegNeg("final_stage_hoist", AltLeg(honestTerrain, FoldFinalHoist));
        LegNeg("partial_slot_read", AltLeg(honestTerrain, (t, o, s) => FoldWords(t, o, s, 1)));
        LegNeg("partial_slot_read_2047", AltLeg(honestTerrain, (t, o, s) => FoldWords(t, o, s, t.Profile.WordsPerSlot - 1)));
        LegNeg("f3_shared_terrain", Bytes.Hex(Bytes.Le64(Leg.Result(other, Seed.Digest(PkA, Beacon)))));
        LegNeg("cached_leg", Cached(honestTerrain));
        TerNeg("one_lane_flip", OneLane(honestTerrain.Profile));
        TerNeg("all4_kernel", All4(honestTerrain.Profile));
        TerNeg("squeezed_seed_terrain", Squeezed(honestTerrain.Profile));
    }

    private delegate ulong FoldFn(Terrain t, int offset, ulong s);

    private static string AltLeg(Terrain terrain, FoldFn fold)
    {
        var s = Bytes.U64Le(Seed.Digest(PkA, Beacon), 0);
        for (var t = 0; t < terrain.Profile.StepsPerLeg; t++)
        {
            var slot = (int)(s % (ulong)terrain.Profile.NSlots);
            var acc = fold(terrain, slot * terrain.Profile.SlotBytes, s);
            s = Bytes.U64Le(Sha256.Seal(acc), 0);
        }
        return Bytes.Hex(Bytes.Le64(s));
    }

    private static ulong FoldSlotSummary(Terrain t, int offset, ulong s)
    {
        ulong inner = 0;
        for (var w = 0; w < t.Profile.WordsPerSlot; w++)
        {
            inner ^= Bytes.U64Le(t.Data, offset + 8 * w) ^ (ulong)w;
        }
        return s ^ inner;
    }

    private static ulong FoldXorAcc(Terrain t, int offset, ulong s)
    {
        ulong acc = 0;
        for (var w = 0; w < t.Profile.WordsPerSlot; w++)
        {
            var word = Bytes.U64Le(t.Data, offset + 8 * w);
            acc ^= Mix.Mix64(word ^ (ulong)w ^ s);
        }
        return acc;
    }

    private static ulong FoldXorW(Terrain t, int offset, ulong s)
    {
        ulong acc = 0;
        for (var w = 0; w < t.Profile.WordsPerSlot; w++)
        {
            var word = Bytes.U64Le(t.Data, offset + 8 * w);
            acc += Mix.Mix64(word ^ (ulong)w ^ s);
        }
        return acc;
    }

    private static ulong FoldSPlusW(Terrain t, int offset, ulong s)
    {
        ulong acc = 0;
        for (var w = 0; w < t.Profile.WordsPerSlot; w++)
        {
            var word = Bytes.U64Le(t.Data, offset + 8 * w);
            acc += Mix.Mix64(word ^ (s + (ulong)w));
        }
        return acc;
    }

    private static ulong FoldWords(Terrain t, int offset, ulong s, int count)
    {
        ulong acc = 0;
        for (var w = 0; w < count; w++)
        {
            var word = Bytes.U64Le(t.Data, offset + 8 * w);
            acc += Mix.Mix64(word ^ s);
        }
        return acc;
    }

    private static ulong MixBeforeFinal(ulong x)
    {
        var y = x + Const.MixAdd;
        y = (y ^ (y >> 30)) * Const.MixMul1;
        y = (y ^ (y >> 27)) * Const.MixMul2;
        return y;
    }

    private static ulong FoldFinalHoist(Terrain t, int offset, ulong s)
    {
        ulong acc = 0;
        for (var w = 0; w < t.Profile.WordsPerSlot; w++)
        {
            var word = Bytes.U64Le(t.Data, offset + 8 * w);
            acc ^= MixBeforeFinal(word ^ s);
        }
        return acc ^ (acc >> 31);
    }

    private static string Prefold(Terrain t)
    {
        var pre = new ulong[t.Profile.NSlots];
        for (var i = 0; i < t.Profile.NSlots; i++)
        {
            ulong sum = 0;
            var off = i * t.Profile.SlotBytes;
            for (var w = 0; w < t.Profile.WordsPerSlot; w++)
            {
                sum += Mix.Mix64(Bytes.U64Le(t.Data, off + 8 * w) ^ (ulong)w);
            }
            pre[i] = sum;
        }
        var s = Bytes.U64Le(Seed.Digest(PkA, Beacon), 0);
        for (var step = 0; step < t.Profile.StepsPerLeg; step++)
        {
            var i = (int)(s % (ulong)t.Profile.NSlots);
            var folded = pre[i] + s;
            s = Bytes.U64Le(Sha256.Seal(folded), 0);
        }
        return Bytes.Hex(Bytes.Le64(s));
    }

    private static string Cached(Terrain t)
    {
        var cache = new Dictionary<int, ulong>();
        var s = Bytes.U64Le(Seed.Digest(PkA, Beacon), 0);
        var seen = new HashSet<int>();
        for (var step = 0; step < t.Profile.StepsPerLeg; step++)
        {
            var i = (int)(s % (ulong)t.Profile.NSlots);
            ulong folded;
            if (seen.Contains(i))
            {
                folded = cache[i];
            }
            else
            {
                folded = Step.FoldSlot(t.Data, i * t.Profile.SlotBytes, t.Profile.WordsPerSlot, s);
                cache[i] = folded;
                seen.Add(i);
            }
            s = Bytes.U64Le(Sha256.Seal(folded), 0);
        }
        return Bytes.Hex(Bytes.Le64(s));
    }

    private static string OneLane(Profile profile)
    {
        var lanes = Seed.Lanes(Seed.Digest(PkA, Beacon));
        var data = new byte[profile.TerrainBytes];
        var words = profile.WordsPerSlot;
        var w = new ulong[words];
        for (var i = 0; i < profile.NSlots; i++)
        {
            for (var j = 0; j < words; j++)
            {
                var c = ((ulong)i << 32) ^ (ulong)j ^ Const.WordXor;
                w[j] = Mix.Mix64(lanes[j & 3] ^ c);
            }
            MixParents(data, profile, i, w);
            for (var j = 0; j < words; j++)
            {
                Bytes.WriteLe64(data, i * profile.SlotBytes + 8 * j, w[j]);
            }
        }
        return Bytes.Hex(Sha256.Hash(data));
    }

    private static string All4(Profile profile)
    {
        var lanes = Seed.Lanes(Seed.Digest(PkA, Beacon));
        var data = new byte[profile.TerrainBytes];
        var words = profile.WordsPerSlot;
        var w = new ulong[words];
        for (var i = 0; i < profile.NSlots; i++)
        {
            for (var j = 0; j < words; j++)
            {
                var c = ((ulong)i << 32) ^ (ulong)j ^ Const.WordXor;
                var x = lanes[j & 3] ^ Mix.Rotl(lanes[(j + 1) & 3], 17) ^ Mix.Rotl(lanes[(j + 2) & 3], 31) ^ Mix.Rotl(lanes[(j + 3) & 3], 47) ^ c;
                w[j] = Mix.Mix64(x);
            }
            MixParents(data, profile, i, w);
            for (var j = 0; j < words; j++)
            {
                Bytes.WriteLe64(data, i * profile.SlotBytes + 8 * j, w[j]);
            }
        }
        return Bytes.Hex(Sha256.Hash(data));
    }

    private static string Squeezed(Profile profile)
    {
        var lanes = Seed.Lanes(Seed.Digest(PkA, Beacon));
        var pin = lanes[0] ^ lanes[1] ^ lanes[2] ^ lanes[3];
        var squeezed = new[] { pin, pin, pin, pin };
        var data = new byte[profile.TerrainBytes];
        var words = profile.WordsPerSlot;
        var w = new ulong[words];
        for (var i = 0; i < profile.NSlots; i++)
        {
            Terrain.BaseWords(squeezed, i, w);
            MixParents(data, profile, i, w);
            for (var j = 0; j < words; j++)
            {
                Bytes.WriteLe64(data, i * profile.SlotBytes + 8 * j, w[j]);
            }
        }
        return Bytes.Hex(Sha256.Hash(data));
    }

    private static void MixParents(byte[] data, Profile profile, int i, ulong[] w)
    {
        if (i == 0)
        {
            return;
        }
        var parents = new[] { i - 1, Mix.BitrevParent(i), Mix.MulParent(i) };
        for (var k = 1; k <= 3; k++)
        {
            var p = parents[k - 1];
            var poff = p * profile.SlotBytes;
            var km = (ulong)k * Const.ParentMul;
            for (var j = 0; j < w.Length; j++)
            {
                var pw = Bytes.U64Le(data, poff + 8 * j);
                w[j] = Mix.Mix64((w[j] ^ Mix.Rotl(pw, (j + k) & 63)) ^ km);
            }
        }
        var slotMix = (ulong)i * Const.SlotMul;
        for (var j = 0; j < w.Length; j++)
        {
            w[j] = Mix.Mix64(w[j] ^ slotMix);
        }
    }

    private static void Unknown(Action<string, string, string> add)
    {
        string[] kinds =
        {
            "v2.1-pcoc-object",
            "satoshi-hike-v2.0-object",
            "",
            "v2.2-pcoc-objecT",
        };
        for (var i = 0; i < kinds.Length; i++)
        {
            var record = Record.UnknownKind(kinds[i]);
            add("unknown-kind.txt", "unknown-kind." + i + ".kind_text_hex", Bytes.Hex(Encoding.UTF8.GetBytes(kinds[i])));
            add("unknown-kind.txt", "unknown-kind." + i + ".record_hex", Bytes.Hex(Encoding.UTF8.GetBytes(record)));
        }
    }

    private static string Join(int[] xs)
    {
        var sb = new StringBuilder();
        for (var i = 0; i < xs.Length; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append(xs[i].ToString(System.Globalization.CultureInfo.InvariantCulture));
        }
        return sb.ToString();
    }

    private static string FindPack(string name)
    {
        var dir = Directory.GetCurrentDirectory();
        for (var i = 0; i < 8 && dir != null; i++)
        {
            var p = Path.Combine(dir, "spec", "conformance", "pack", name);
            if (File.Exists(p))
            {
                return p;
            }
            dir = Directory.GetParent(dir)?.FullName;
        }
        throw new FileNotFoundException(name);
    }

    private static string Between(string text, string start, string end)
    {
        var at = text.IndexOf(start, StringComparison.Ordinal);
        if (at < 0)
        {
            throw new InvalidOperationException(start);
        }
        at += start.Length;
        var to = text.IndexOf(end, at, StringComparison.Ordinal);
        return text.Substring(at, to - at);
    }

    private static byte[] FromHex(string hex)
    {
        if (!Bytes.TryParseHex(hex, out var b))
        {
            throw new InvalidOperationException(hex);
        }
        return b;
    }
}
