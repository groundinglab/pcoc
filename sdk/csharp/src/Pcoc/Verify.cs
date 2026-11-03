namespace Pcoc;

/// §14
public static class Verify
{
    public const string Nd = "not defined";
    public const string Nr = "not read";

    /// §14
    public sealed class Outcome
    {
        public int Exit { get; init; }
        public string Record { get; init; } = "";
    }

    /// §14
    public static Outcome Measure(byte[] input, Profile profile, IIndex? index, System.Func<byte[]?>? drawBytes, bool runSixLeg)
    {
        var hike = HikeObject.Read(input);
        if (hike.DecodeFailure)
        {
            return new Outcome { Exit = 1, Record = "" };
        }
        if (hike.Malformed)
        {
            return new Outcome { Exit = 0, Record = MalformedRecord(profile) };
        }
        if (hike.KindText != Const.KindText)
        {
            return new Outcome { Exit = 0, Record = Record.UnknownKind(hike.KindText ?? "") };
        }
        return new Outcome { Exit = 0, Record = Full(hike, profile, index, drawBytes, runSixLeg) };
    }

    /// §7.3
    public static string MalformedRecord(Profile profile)
    {
        var sb = new System.Text.StringBuilder();
        sb.Append('{');
        ProfilePrefix(sb, profile);
        sb.Append("\"shape\":\"mismatch\",\"shape_item\":[\"malformed cbor\"],\"headers\":[");
        for (var i = 0; i < 19; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append(HeaderJson(Nd, Nd, Nr, Nr));
        }
        sb.Append("],\"seed_value\":\"mismatch\",\"checkpoints\":[");
        for (var g = 0; g < 18; g++)
        {
            if (g > 0) sb.Append(',');
            sb.Append("{\"g\":").Append(g).Append(",\"legs\":\"").Append(Nd).Append("\"}");
        }
        sb.Append("],\"waypoints\":[");
        for (var g = 1; g <= 17; g++)
        {
            if (g > 1) sb.Append(',');
            sb.Append("{\"g\":").Append(g).Append(",\"result\":\"").Append(Nd).Append("\"}");
        }
        sb.Append("],\"paths\":[");
        for (var g = 1; g <= 16; g++)
        {
            if (g > 1) sb.Append(',');
            sb.Append(Path16(g, Nd, Nd, Nd, Nd, Nd, Nd));
        }
        sb.Append(',').Append(Path17(Nd, Nd));
        sb.Append("],\"anchors\":[");
        for (var i = 0; i < 16; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append("\"not anchored\"");
        }
        sb.Append("],\"anchored_total\":0,\"skip_list\":[],");
        BookendTokens(sb, Nd);
        sb.Append(",\"six_leg\":\"").Append(Nd).Append("\",\"six_leg_runs\":[],\"analysis\":{");
        sb.Append("\"whole_hike_legs\":\"").Append(Nd).Append('"');
        sb.Append(",\"whole_hike_span\":\"").Append(Nr).Append('"');
        sb.Append(",\"per_stage\":[");
        for (var g = 0; g < 18; g++)
        {
            if (g > 0) sb.Append(',');
            sb.Append("{\"g\":").Append(g).Append(",\"checkpoint_count\":\"").Append(Nd).Append("\",\"span\":\"").Append(Nr).Append("\"}");
        }
        sb.Append("],\"legs_outside\":\"").Append(Nd).Append("\",\"hike_legs\":\"").Append(Nd).Append("\",\"hike_span\":\"").Append(Nr).Append("\"}}");
        return sb.ToString();
    }

    private static string Full(HikeObject hike, Profile profile, IIndex? index, System.Func<byte[]?>? drawBytes, bool runSixLeg)
    {
        var items = ShapeItems(hike, profile);
        var shape = items.Count == 0 ? "match" : "mismatch";
        var headers = Headers(hike, index);
        var seedValue = SeedValue(hike);
        Terrain? terrain = null;
        if (hike.PublicKey != null && hike.SeedBeacon != null)
        {
            terrain = Terrain.Fill(hike.PublicKey, hike.SeedBeacon, profile);
        }
        var checkpoints = Checkpoints(hike, terrain);
        var waypoints = Waypoints(hike, terrain);
        var paths = Paths(hike, index);
        var analysis = Analysis(hike, headers);
        var book = Bookends(hike, paths, headers, analysis, index);
        var six = Six(hike, terrain, drawBytes, runSixLeg);
        var sb = new System.Text.StringBuilder();
        sb.Append('{');
        ProfilePrefix(sb, profile);
        sb.Append("\"shape\":\"").Append(shape).Append("\",\"shape_item\":[");
        for (var i = 0; i < items.Count; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append('"').Append(items[i]).Append('"');
        }
        sb.Append("],\"headers\":[");
        for (var i = 0; i < 19; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append(headers[i]);
        }
        sb.Append("],\"seed_value\":\"").Append(seedValue).Append("\",\"checkpoints\":[");
        for (var g = 0; g < 18; g++)
        {
            if (g > 0) sb.Append(',');
            sb.Append(checkpoints[g]);
        }
        sb.Append("],\"waypoints\":[");
        for (var g = 0; g < 17; g++)
        {
            if (g > 0) sb.Append(',');
            sb.Append(waypoints[g]);
        }
        sb.Append("],\"paths\":[");
        for (var g = 0; g < 17; g++)
        {
            if (g > 0) sb.Append(',');
            sb.Append(paths.Json[g]);
        }
        sb.Append("],\"anchors\":[");
        for (var i = 0; i < 16; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append('"').Append(paths.Anchors[i]).Append('"');
        }
        sb.Append("],\"anchored_total\":").Append(paths.AnchoredTotal);
        sb.Append(",\"skip_list\":[");
        for (var i = 0; i < paths.Skip.Count; i++)
        {
            if (i > 0) sb.Append(',');
            sb.Append(paths.Skip[i]);
        }
        sb.Append("],");
        sb.Append(book);
        sb.Append(",\"six_leg\":").Append(six.Six);
        sb.Append(",\"six_leg_runs\":").Append(six.Runs);
        sb.Append(",\"analysis\":").Append(analysis.Json);
        sb.Append('}');
        return sb.ToString();
    }

    private static void ProfilePrefix(System.Text.StringBuilder sb, Profile profile)
    {
        sb.Append("\"profile\":\"").Append(profile.Name).Append('"');
        if (profile.Name == "conformance-small")
        {
            sb.Append(",\"sample_bytes\":").Append(profile.SampleBytes);
            sb.Append(",\"slot_bytes\":").Append(profile.SlotBytes);
            sb.Append(",\"terrain_bytes\":").Append(profile.TerrainBytes);
            sb.Append(",\"steps_per_leg\":").Append(profile.StepsPerLeg);
        }
        sb.Append(',');
    }

    private static System.Collections.Generic.List<string> ShapeItems(HikeObject hike, Profile profile)
    {
        var set = new System.Collections.Generic.List<string>();
        void Add(string s)
        {
            if (!set.Contains(s)) set.Add(s);
        }
        if (!hike.TopFieldSet)
        {
            Add("field set");
        }
        for (var g = 0; g < Const.Stages; g++)
        {
            var st = g < hike.StagesLength ? hike.Stages[g] : new HikeObject.Stage();
            if (g < hike.StagesLength && !st.FieldSetOk)
            {
                Add("field set");
            }
            if (g == 0 && st.HasPathKey)
            {
                Add("stage-0 path key");
            }
        }
        var s17 = hike.StagesLength > 17 ? hike.Stages[17] : null;
        if (s17 == null || !s17.HasPathKey || s17.Path == null || s17.Path.Length != 0)
        {
            Add("W_17 path");
        }
        var constant = !hike.FillPresent || hike.FillText != Const.FillText
            || !hike.SlotBytesPresent || hike.SlotBytesRaw != (ulong)profile.SlotBytes
            || !hike.NSlotsPresent || hike.NSlotsRaw != (ulong)profile.NSlots
            || !hike.SampleBytesPresent || hike.SampleBytesRaw != (ulong)profile.SampleBytes
            || !hike.StepsPresent || hike.StepsPerLegRaw != (ulong)profile.StepsPerLeg;
        if (constant)
        {
            Add("constant");
        }
        if (!hike.StagesPresent || hike.StagesLength != Const.Stages)
        {
            Add("stages length");
        }
        var legBad = false;
        var cpWidth = false;
        var lrWidth = false;
        for (var g = 0; g < Const.Stages; g++)
        {
            if (g >= hike.StagesLength || !hike.Stages[g].Present)
            {
                legBad = true;
                continue;
            }
            var st = hike.Stages[g];
            if (!st.CheckpointsUsable || !st.LegResultsUsable || !st.LegCountPresent || !st.LegCountFits)
            {
                legBad = true;
            }
            else
            {
                if (st.LegCountRaw < 1 || st.LegCountRaw != (ulong)st.Checkpoints.Length || st.Checkpoints.Length != st.LegResults.Length)
                {
                    legBad = true;
                }
            }
            if (st.CheckpointsUsable)
            {
                foreach (var c in st.Checkpoints)
                {
                    if (c == null || c.Length != 32) cpWidth = true;
                }
            }
            if (st.LegResultsUsable)
            {
                foreach (var c in st.LegResults)
                {
                    if (c == null || c.Length != 8) lrWidth = true;
                }
            }
        }
        if (legBad) Add("leg_count");
        if (cpWidth) Add("checkpoint width");
        if (lrWidth) Add("leg result width");
        var s0 = hike.StagesLength > 0 ? hike.Stages[0] : null;
        if (hike.SeedBeacon == null || s0 == null || s0.Beacon == null || !Bytes.Equal(s0.Beacon, hike.SeedBeacon))
        {
            Add("seed beacon");
        }
        var heightRange = false;
        bool HeightBad(bool present, bool fits)
        {
            if (present && !fits)
            {
                heightRange = true;
                return true;
            }
            return !present;
        }
        var heights = new long?[19];
        var hashes = new byte[]?[19];
        for (var g = 0; g < 18; g++)
        {
            if (g < hike.StagesLength)
            {
                var st = hike.Stages[g];
                hashes[g] = st.Beacon != null && st.Beacon.Length == 32 ? st.Beacon : null;
                if (HeightBad(st.HeightPresent, st.HeightFits))
                {
                    heights[g] = null;
                }
                else
                {
                    heights[g] = (long)st.HeightRaw;
                }
            }
        }
        hashes[18] = hike.LastInterrupt != null && hike.LastInterrupt.Length == 32 ? hike.LastInterrupt : null;
        if (HeightBad(hike.LastHeightPresent, hike.LastHeightFits))
        {
            heights[18] = null;
        }
        else
        {
            heights[18] = (long)hike.LastHeightRaw;
        }
        if (heightRange) Add("height_range");
        var lastOk = hike.LastInterrupt != null && hike.LastInterrupt.Length == 32
            && hashes[17] != null && !Bytes.Equal(hike.LastInterrupt, hashes[17]!)
            && heights[18] != null && heights[17] != null && heights[18] > heights[17];
        if (!lastOk) Add("last_interrupt");
        var order = false;
        var differ = false;
        for (var k = 1; k < 19; k++)
        {
            if (hashes[k] == null || hashes[k - 1] == null || Bytes.Equal(hashes[k]!, hashes[k - 1]!))
            {
                differ = true;
            }
            if (heights[k] == null || heights[k - 1] == null || heights[k] <= heights[k - 1])
            {
                order = true;
            }
        }
        if (order) Add("header height order");
        if (differ) Add("header hash differ");
        string[] orderNames = {
            "malformed cbor", "field set", "stage-0 path key", "W_17 path", "constant", "stages length",
            "leg_count", "checkpoint width", "leg result width", "seed beacon", "last_interrupt",
            "header height order", "header hash differ", "height_range"
        };
        var ordered = new System.Collections.Generic.List<string>();
        foreach (var name in orderNames)
        {
            if (set.Contains(name)) ordered.Add(name);
        }
        return ordered;
    }

    private static string[] Headers(HikeObject hike, IIndex? index)
    {
        var rows = new string[19];
        for (var k = 0; k < 19; k++)
        {
            byte[]? hash = null;
            bool heightPresent = false;
            bool heightFits = false;
            ulong heightRaw = 0;
            if (k < 18 && k < hike.StagesLength)
            {
                var st = hike.Stages[k];
                hash = st.Beacon;
                heightPresent = st.HeightPresent;
                heightFits = st.HeightFits;
                heightRaw = st.HeightRaw;
            }
            else if (k == 18)
            {
                hash = hike.LastInterrupt;
                heightPresent = hike.LastHeightPresent;
                heightFits = hike.LastHeightFits;
                heightRaw = hike.LastHeightRaw;
            }
            var hashTok = hash != null && hash.Length == 32 ? Bytes.Hex(hash) : Nd;
            var heightTok = heightPresent && heightFits ? Record.Num((long)heightRaw) : Nd;
            var cmp = Nr;
            var t = Nr;
            if (hashTok == Nd || heightTok == Nd || index == null)
            {
                rows[k] = HeaderJson(hashTok == Nd ? Nd : hashTok, heightTok, Nr, Nr);
                continue;
            }
            var doc = index.Get((long)heightRaw);
            if (doc == null || !doc.Document || !doc.HeaderOk || doc.NaturalHash == null)
            {
                cmp = Nr;
                t = Nr;
            }
            else if (!Bytes.Equal(doc.NaturalHash, hash!) || doc.Height != (long)heightRaw)
            {
                cmp = "mismatch";
                t = Nr;
            }
            else
            {
                cmp = "match";
                t = doc.TimeOk ? Record.Num(doc.Time) : Nr;
            }
            rows[k] = HeaderJson(hashTok, heightTok, cmp, t);
        }
        return rows;
    }

    private static string SeedValue(HikeObject hike)
    {
        if (hike.PublicKey == null || hike.SeedBeacon == null || hike.StagesLength < 1 || hike.Stages[0].Waypoint == null || hike.Stages[0].Waypoint!.Length != 32)
        {
            return "mismatch";
        }
        var d = Seed.Digest(hike.PublicKey, hike.SeedBeacon);
        return Bytes.Equal(d, hike.Stages[0].Waypoint!) ? "match" : "mismatch";
    }

    private static string[] Checkpoints(HikeObject hike, Terrain? terrain)
    {
        var rows = new string[18];
        byte[]? computedSeed = null;
        if (hike.PublicKey != null && hike.SeedBeacon != null)
        {
            computedSeed = Seed.Digest(hike.PublicKey, hike.SeedBeacon);
        }
        for (var g = 0; g < 18; g++)
        {
            if (g >= hike.StagesLength || !hike.Stages[g].CheckpointsUsable)
            {
                rows[g] = "{\"g\":" + g + ",\"legs\":\"" + Nd + "\"}";
                continue;
            }
            var st = hike.Stages[g];
            var sb = new System.Text.StringBuilder();
            sb.Append("{\"g\":").Append(g).Append(",\"legs\":[");
            for (var j = 0; j < st.Checkpoints.Length; j++)
            {
                if (j > 0) sb.Append(',');
                var result = "mismatch";
                var limbPresent = st.LegResultsUsable && j < st.LegResults.Length && st.LegResults[j] != null && st.LegResults[j].Length == 8;
                var cp = st.Checkpoints[j];
                if (limbPresent && cp != null && cp.Length == 32 && terrain != null)
                {
                    byte[]? previous = null;
                    byte[]? stageLimb = null;
                    if (j == 0)
                    {
                        if (g == 0)
                        {
                            previous = computedSeed;
                            stageLimb = computedSeed;
                        }
                        else if (st.Waypoint != null && st.Waypoint.Length == 32)
                        {
                            previous = st.Waypoint;
                            stageLimb = st.Waypoint;
                        }
                    }
                    else if (st.Checkpoints[j - 1] != null && st.Checkpoints[j - 1].Length == 32)
                    {
                        previous = st.Checkpoints[j - 1];
                        stageLimb = g == 0 ? computedSeed : (st.Waypoint != null && st.Waypoint.Length == 32 ? st.Waypoint : null);
                    }
                    if (previous != null && stageLimb != null)
                    {
                        var bound = Checkpoint.Bind(terrain, previous, stageLimb, Bytes.U64Le(st.LegResults[j], 0));
                        result = Bytes.Equal(bound, cp) ? "match" : "mismatch";
                    }
                }
                sb.Append("{\"j\":").Append(j).Append(",\"result\":\"").Append(result).Append("\"}");
            }
            sb.Append("]}");
            rows[g] = sb.ToString();
        }
        return rows;
    }

    private static string[] Waypoints(HikeObject hike, Terrain? terrain)
    {
        var rows = new string[17];
        for (var g = 0; g <= 16; g++)
        {
            var result = Nd;
            var left = g < hike.StagesLength ? hike.Stages[g] : null;
            var right = g + 1 < hike.StagesLength ? hike.Stages[g + 1] : null;
            var usable = terrain != null && left != null && right != null
                && left.CheckpointsUsable && left.Waypoint != null && left.Waypoint.Length == 32
                && right.Beacon != null && right.Beacon.Length == 32
                && right.Waypoint != null && right.Waypoint.Length == 32
                && All32(left.Checkpoints);
            if (usable)
            {
                var built = Waypoint.Build(terrain!, right!.Beacon!, left!.Waypoint!, left.Checkpoints);
                result = Bytes.Equal(built, right.Waypoint!) ? "match" : "mismatch";
            }
            rows[g] = "{\"g\":" + (g + 1) + ",\"result\":\"" + result + "\"}";
        }
        return rows;
    }

    private sealed class PathSet
    {
        public string[] Json = new string[17];
        public string[] Anchors = new string[16];
        public int AnchoredTotal;
        public System.Collections.Generic.List<int> Skip = new();
        public bool[] Valid = new bool[16];
        public bool[] Issued = new bool[16];
        public bool[] IssuedMiss = new bool[16];
        public string[] Plant = new string[16];
        public long[] BlockHeight = new long[16];
        public bool[] HasBlock = new bool[16];
        public string[] AnchorTok = new string[16];
    }

    private static PathSet Paths(HikeObject hike, IIndex? index)
    {
        var set = new PathSet();
        var interrupts = new (long Height, byte[] Hash)[18];
        var nInt = 0;
        for (var g = 1; g <= 17 && g < hike.StagesLength; g++)
        {
            var st = hike.Stages[g];
            if (st.HeightPresent && st.HeightFits && st.Beacon != null && st.Beacon.Length == 32)
            {
                interrupts[nInt++] = ((long)st.HeightRaw, st.Beacon);
            }
        }
        if (hike.LastHeightPresent && hike.LastHeightFits && hike.LastInterrupt != null && hike.LastInterrupt.Length == 32)
        {
            interrupts[nInt++] = ((long)hike.LastHeightRaw, hike.LastInterrupt);
        }
        for (var g = 1; g <= 16; g++)
        {
            var st = g < hike.StagesLength ? hike.Stages[g] : null;
            if (st == null || !st.HasPathKey || st.Path == null)
            {
                set.Json[g - 1] = Path16(g, Nd, Nd, Nd, Nd, Nd, Nd);
                set.Anchors[g - 1] = "not anchored";
                set.Plant[g - 1] = Nd;
                continue;
            }
            if (st.Path.Length == 0)
            {
                set.Skip.Add(g);
                set.Anchors[g - 1] = "not anchored";
                set.Plant[g - 1] = "n/a";
                set.Json[g - 1] = Path16(g, "skip", "n/a", Nd, Nd, Nd, Nd);
                continue;
            }
            var claimed = Nd;
            PathBody? body = null;
            var decoded = PathBody.TryDecode(st.Path, out body);
            var waypoint = st.Waypoint;
            var valid = decoded && waypoint != null && waypoint.Length == 32 && body!.ValidateFor(waypoint);
            if (decoded)
            {
                claimed = BlockJson(body!);
            }
            if (!valid)
            {
                set.Anchors[g - 1] = "not anchored";
                set.Plant[g - 1] = "mismatch";
                set.Json[g - 1] = Path16(g, "present", "mismatch", Nd, Nd, Nd, Nd);
                continue;
            }
            set.Valid[g - 1] = true;
            set.HasBlock[g - 1] = true;
            set.BlockHeight[g - 1] = body!.BlockHeight;
            var h = g < hike.StagesLength && hike.Stages[g].HeightPresent && hike.Stages[g].HeightFits ? (long)hike.Stages[g].HeightRaw : long.MinValue;
            var inWindow = h != long.MinValue && (body.BlockHeight == h + 2 || body.BlockHeight == h + 3 || body.BlockHeight == h + 4);
            var stored = false;
            for (var k = 0; k < nInt; k++)
            {
                if (interrupts[k].Height == body.BlockHeight && Bytes.Equal(interrupts[k].Hash, body.BlockHash))
                {
                    stored = true;
                }
            }
            if (!inWindow || !stored)
            {
                set.Anchors[g - 1] = "not anchored";
                set.Plant[g - 1] = Nr;
                set.Json[g - 1] = Path16(g, "present", Nr, Nd, Nd, Nd, claimed);
                continue;
            }
            set.Issued[g - 1] = true;
            var doc = index?.Get(body.BlockHeight);
            if (doc == null || !doc.Document)
            {
                set.IssuedMiss[g - 1] = true;
                set.Anchors[g - 1] = "not anchored";
                set.Plant[g - 1] = Nr;
                set.Json[g - 1] = Path16(g, "present", Nr, Nd, Nd, Nd, claimed);
                continue;
            }
            var matched = false;
            if (doc.Network == "main" || doc.Network == "synthetic")
            {
                foreach (var plant in doc.Plants)
                {
                    if (!plant.Ok) continue;
                    if (Bytes.Equal(plant.Root, body.Root)
                        && Bytes.Equal(plant.Txid, body.Txid)
                        && plant.Vout == body.Vout
                        && Bytes.Equal(plant.BlockHash, body.BlockHash)
                        && plant.BlockHeight == body.BlockHeight)
                    {
                        matched = true;
                    }
                }
            }
            if (!matched)
            {
                set.Anchors[g - 1] = "not anchored";
                set.Plant[g - 1] = "mismatch";
                set.Json[g - 1] = Path16(g, "present", "mismatch", Nd, Nd, Nd, claimed);
                continue;
            }
            var pg = BlockJson(body);
            var heightPg = Record.Num(body.BlockHeight);
            var dg = Nd;
            var headerCmp = HeaderCmp(hike, g, index);
            if (headerCmp == "match" && index != null)
            {
                var idoc = index.Get(HeaderHeight(hike, g));
                if (idoc != null && idoc.HeaderOk)
                {
                    dg = Record.Num(body.BlockHeight - idoc.Height);
                }
            }
            set.Anchors[g - 1] = "anchored";
            set.Plant[g - 1] = "match";
            set.AnchoredTotal++;
            set.Json[g - 1] = Path16(g, "present", "match", pg, heightPg, dg, claimed);
        }
        var s17 = hike.StagesLength > 17 ? hike.Stages[17] : null;
        if (s17 != null && s17.Path != null && s17.Path.Length == 0)
        {
            set.Json[16] = Path17("not fetched by design", Nd);
        }
        else if (s17 != null && s17.Path != null && s17.Path.Length > 0 && PathBody.TryDecode(s17.Path, out var w17))
        {
            set.Json[16] = Path17(Nd, BlockJson(w17));
        }
        else
        {
            set.Json[16] = Path17(Nd, Nd);
        }
        return set;
    }

    private static string HeaderCmp(HikeObject hike, int g, IIndex? index)
    {
        if (g >= hike.StagesLength || index == null)
        {
            return Nr;
        }
        var st = hike.Stages[g];
        if (st.Beacon == null || st.Beacon.Length != 32 || !st.HeightPresent || !st.HeightFits)
        {
            return Nr;
        }
        var doc = index.Get((long)st.HeightRaw);
        if (doc == null || !doc.HeaderOk || doc.NaturalHash == null)
        {
            return Nr;
        }
        if (!Bytes.Equal(doc.NaturalHash, st.Beacon) || doc.Height != (long)st.HeightRaw)
        {
            return "mismatch";
        }
        return "match";
    }

    private static long HeaderHeight(HikeObject hike, int g)
    {
        return (long)hike.Stages[g].HeightRaw;
    }

    private sealed class AnalysisOut
    {
        public string Json = "";
        public string WholeLegs = Nd;
        public string HikeSpan = Nr;
        public string[] Count = new string[18];
        public string[] Span = new string[18];
    }

    private static AnalysisOut Analysis(HikeObject hike, string[] headerJson)
    {
        var a = new AnalysisOut();
        long? sum = 0;
        for (var g = 0; g < 18; g++)
        {
            if (g >= hike.StagesLength || !hike.Stages[g].CheckpointsUsable)
            {
                a.Count[g] = Nd;
                sum = null;
            }
            else
            {
                a.Count[g] = Record.Num(hike.Stages[g].Checkpoints.Length);
                if (sum != null && g >= 1)
                {
                    sum += hike.Stages[g].Checkpoints.Length;
                }
            }
            a.Span[g] = SpanOf(headerJson, g == 0 ? 0 : g, g + 1);
        }
        a.WholeLegs = sum == null ? Nd : Record.Num(sum.Value);
        a.HikeSpan = SpanOf(headerJson, 1, 18);
        var sb = new System.Text.StringBuilder();
        sb.Append("{\"whole_hike_legs\":").Append(JsonTokOrNum(a.WholeLegs));
        sb.Append(",\"whole_hike_span\":").Append(JsonTokOrNum(a.HikeSpan));
        sb.Append(",\"per_stage\":[");
        for (var g = 0; g < 18; g++)
        {
            if (g > 0) sb.Append(',');
            sb.Append("{\"g\":").Append(g).Append(",\"checkpoint_count\":").Append(JsonTokOrNum(a.Count[g]));
            sb.Append(",\"span\":").Append(JsonTokOrNum(a.Span[g])).Append('}');
        }
        sb.Append("],\"legs_outside\":\"").Append("PLACE").Append("\",\"hike_legs\":").Append(JsonTokOrNum(a.WholeLegs));
        sb.Append(",\"hike_span\":").Append(JsonTokOrNum(a.HikeSpan)).Append('}');
        a.Json = sb.ToString();
        return a;
    }

    private static string Bookends(HikeObject hike, PathSet paths, string[] headerJson, AnalysisOut analysis, IIndex? index)
    {
        string fields;
        var validCount = 0;
        for (var i = 0; i < 16; i++) if (paths.Valid[i]) validCount++;
        if (validCount < 2)
        {
            fields = BookendBody(Nd);
        }
        else
        {
            var miss = false;
            for (var i = 0; i < 16; i++) if (paths.IssuedMiss[i]) miss = true;
            if (miss)
            {
                fields = BookendBody(Nr);
            }
            else if (paths.AnchoredTotal < 2)
            {
                fields = BookendBody(Nd);
            }
            else
            {
                var a = 0;
                var b = 0;
                for (var i = 0; i < 16; i++)
                {
                    if (paths.Anchors[i] == "anchored")
                    {
                        if (a == 0) a = i + 1;
                        b = i + 1;
                    }
                }
                long n = 0;
                var nOk = true;
                for (var g = a; g <= b - 1; g++)
                {
                    if (analysis.Count[g] == Nd)
                    {
                        nOk = false;
                    }
                    else
                    {
                        n += long.Parse(analysis.Count[g], System.Globalization.CultureInfo.InvariantCulture);
                    }
                }
                var headerA = HeaderField(headerJson[a], "index_comparison");
                var pbJson = paths.HasBlock[b - 1] ? BlockFromPath(paths.Json[b - 1]) : Nd;
                if (headerA == Nr || pbJson == Nd)
                {
                    fields = BookendBody(Nr);
                }
                else
                {
                    var cmpPb = Nr;
                    var heightPb = Nd;
                    var tPb = Nr;
                    var heightI = Nd;
                    var tI = HeaderField(headerJson[a], "T");
                    if (tI != Nr && tI != Nd && !long.TryParse(tI, System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out _))
                    {
                        tI = Nr;
                    }
                    var bh = paths.BlockHeight[b - 1];
                    var pdoc = index == null ? null : index.Get(bh);
                    if (pdoc == null || !pdoc.Document || !pdoc.HeaderOk || pdoc.NaturalHash == null)
                    {
                        cmpPb = Nr;
                    }
                    else
                    {
                        var want = DisplayHash(pbJson);
                        var got = Bytes.Hex(Bytes.Reverse32(pdoc.NaturalHash));
                        if (want == got && pdoc.Height == bh)
                        {
                            cmpPb = "match";
                            heightPb = Record.Num(pdoc.Height);
                            tPb = pdoc.TimeOk ? Record.Num(pdoc.Time) : Nr;
                        }
                        else
                        {
                            cmpPb = "mismatch";
                        }
                    }
                    if (headerA == "match")
                    {
                        heightI = HeaderField(headerJson[a], "stored_height");
                    }
                    if (cmpPb == Nr)
                    {
                        fields = BookendBody(Nr);
                    }
                    else
                    {
                        var sTok = Nr;
                        if (tPb != Nr && tI != Nr && long.TryParse(tPb, out var tb) && long.TryParse(tI, out var ti))
                        {
                            sTok = Record.Num(tb - ti);
                        }
                        var dTok = Nd;
                        if (cmpPb == "match" && headerA == "match"
                            && long.TryParse(heightPb, out var hp) && long.TryParse(heightI, out var hi))
                        {
                            dTok = Record.Num(hp - hi);
                        }
                        var fs = FixedStart(analysis, headerJson, b, cmpPb, heightPb, tPb);
                        fields = BookendIntegers(a, b, nOk ? Record.Num(n) : Nd, sTok, dTok, pbJson, heightPb, cmpPb, tPb, heightI, tI, analysis, a, b, fs);
                    }
                }
            }
        }
        var outside = FieldOf(fields, "legs_token");
        analysis.Json = analysis.Json.Replace("\"PLACE\"", JsonTokOrNum(outside));
        var marker = ",\"legs_token\":";
        var at = fields.LastIndexOf(marker, System.StringComparison.Ordinal);
        return at < 0 ? fields : fields.Substring(0, at);
    }

    private static string BookendIntegers(int a, int b, string n, string s, string delta, string pb, string heightPb, string cmp, string tPb, string heightI, string tI, AnalysisOut analysis, int aa, int bb, string fixedStart)
    {
        long outside = 0;
        var ok = true;
        for (var g = 0; g < 18; g++)
        {
            if (g >= aa && g <= bb - 1)
            {
                continue;
            }
            if (analysis.Count[g] == Nd)
            {
                ok = false;
            }
            else
            {
                outside += long.Parse(analysis.Count[g], System.Globalization.CultureInfo.InvariantCulture);
            }
        }
        var legs = ok ? Record.Num(outside) : Nd;
        var sb = new System.Text.StringBuilder();
        sb.Append("\"a\":").Append(a);
        sb.Append(",\"b\":").Append(b);
        sb.Append(",\"N\":").Append(JsonTokOrNum(n));
        sb.Append(",\"S\":").Append(JsonTokOrNum(s));
        sb.Append(",\"delta_h\":").Append(JsonTokOrNum(delta));
        sb.Append(",\"fixed_start\":").Append(fixedStart);
        sb.Append(",\"P_b\":").Append(pb.StartsWith('{') ? pb : "\"" + pb + "\"");
        sb.Append(",\"height_P_b\":").Append(JsonTokOrNum(heightPb));
        sb.Append(",\"index_comparison_P_b\":\"").Append(cmp).Append('"');
        sb.Append(",\"T_P_b\":").Append(JsonTokOrNum(tPb));
        sb.Append(",\"height_I_a_minus_1\":").Append(JsonTokOrNum(heightI));
        sb.Append(",\"T_I_a_minus_1\":").Append(JsonTokOrNum(tI));
        sb.Append(",\"legs_token\":").Append(JsonTokOrNum(legs));
        return sb.ToString();
    }

    private static string FixedStart(AnalysisOut analysis, string[] headerJson, int b, string cmpPb, string heightPb, string tPb)
    {
        long n = 0;
        for (var g = 1; g <= b - 1; g++)
        {
            if (analysis.Count[g] == Nd)
            {
                return "{\"N\":\"" + Nd + "\",\"S\":\"" + Nr + "\",\"delta_h\":\"" + Nd + "\"}";
            }
            n += long.Parse(analysis.Count[g], System.Globalization.CultureInfo.InvariantCulture);
        }
        var t1 = HeaderField(headerJson[1], "T");
        var sTok = Nr;
        if (t1 != Nr && tPb != Nr && long.TryParse(t1, out var left) && long.TryParse(tPb, out var right))
        {
            sTok = Record.Num(right - left);
        }
        var h1 = HeaderField(headerJson[1], "index_comparison");
        var dTok = Nd;
        if (h1 == Nr || cmpPb == Nr)
        {
            dTok = Nr;
        }
        else if (h1 == "match" && cmpPb == "match")
        {
            var hi = long.Parse(HeaderField(headerJson[1], "stored_height"), System.Globalization.CultureInfo.InvariantCulture);
            dTok = Record.Num(long.Parse(heightPb, System.Globalization.CultureInfo.InvariantCulture) - hi);
        }
        return "{\"N\":" + n + ",\"S\":" + JsonTokOrNum(sTok) + ",\"delta_h\":" + JsonTokOrNum(dTok) + "}";
    }

    private static string DisplayHash(string blockJson)
    {
        var key = "\"block_hash\":\"";
        var at = blockJson.IndexOf(key, System.StringComparison.Ordinal);
        if (at < 0) return "";
        at += key.Length;
        var end = blockJson.IndexOf('"', at);
        return blockJson.Substring(at, end - at);
    }

    private static string FixedN(AnalysisOut analysis, int b)
    {
        long n = 0;
        for (var g = 1; g <= b - 1; g++)
        {
            if (analysis.Count[g] == Nd) return "\"" + Nd + "\"";
            n += long.Parse(analysis.Count[g], System.Globalization.CultureInfo.InvariantCulture);
        }
        return Record.Num(n);
    }

    private static string BookendBody(string token)
    {
        var sb = new System.Text.StringBuilder();
        sb.Append("\"a\":\"").Append(token).Append('"');
        sb.Append(",\"b\":\"").Append(token).Append('"');
        sb.Append(",\"N\":\"").Append(token).Append('"');
        sb.Append(",\"S\":\"").Append(token).Append('"');
        sb.Append(",\"delta_h\":\"").Append(token).Append('"');
        sb.Append(",\"fixed_start\":{\"N\":\"").Append(token).Append("\",\"S\":\"").Append(token).Append("\",\"delta_h\":\"").Append(token).Append("\"}");
        sb.Append(",\"P_b\":\"").Append(token).Append('"');
        sb.Append(",\"height_P_b\":\"").Append(token).Append('"');
        sb.Append(",\"index_comparison_P_b\":\"").Append(token).Append('"');
        sb.Append(",\"T_P_b\":\"").Append(token).Append('"');
        sb.Append(",\"height_I_a_minus_1\":\"").Append(token).Append('"');
        sb.Append(",\"T_I_a_minus_1\":\"").Append(token).Append('"');
        sb.Append(",\"legs_token\":\"").Append(token).Append('"');
        return sb.ToString();
    }

    private sealed class SixOut
    {
        public string Six = "\"" + Nd + "\"";
        public string Runs = "[]";
    }

    private static SixOut Six(HikeObject hike, Terrain? terrain, System.Func<byte[]?>? drawBytes, bool run)
    {
        var o = new SixOut();
        var total = 0;
        var usable = true;
        for (var g = 0; g < 18; g++)
        {
            if (g >= hike.StagesLength || !hike.Stages[g].CheckpointsUsable)
            {
                usable = false;
                break;
            }
            total += hike.Stages[g].Checkpoints.Length;
        }
        if (!usable || total < 6)
        {
            return o;
        }
        if (!run || terrain == null)
        {
            o.Six = "\"" + Nr + "\"";
            o.Runs = "\"" + Nr + "\"";
            return o;
        }
        var src = drawBytes ?? OsRandom;
        var indices = Draw.Six(src, total);
        if (indices == null)
        {
            o.Six = "\"" + Nr + "\"";
            o.Runs = "\"" + Nr + "\"";
            return o;
        }
        var results = new string[6];
        for (var k = 0; k < 6; k++)
        {
            results[k] = LegMatch(hike, terrain, indices[k]);
        }
        var sb = new System.Text.StringBuilder();
        sb.Append("{\"indices\":[");
        for (var k = 0; k < 6; k++)
        {
            if (k > 0) sb.Append(',');
            sb.Append(indices[k]);
        }
        sb.Append("],\"results\":[");
        for (var k = 0; k < 6; k++)
        {
            if (k > 0) sb.Append(',');
            sb.Append('"').Append(results[k]).Append('"');
        }
        sb.Append("]}");
        o.Six = sb.ToString();
        o.Runs = "[" + sb + "]";
        return o;
    }

    private static string LegMatch(HikeObject hike, Terrain terrain, int q)
    {
        var start = 0;
        for (var g = 0; g < 18; g++)
        {
            var n = hike.Stages[g].Checkpoints.Length;
            if (q < start + n)
            {
                var j = q - start;
                var st = hike.Stages[g];
                if (!st.LegResultsUsable || j >= st.LegResults.Length || st.LegResults[j] == null || st.LegResults[j].Length != 8)
                {
                    return "mismatch";
                }
                byte[]? previous;
                if (j == 0)
                {
                    previous = g == 0 ? Seed.Digest(hike.PublicKey!, hike.SeedBeacon!) : st.Waypoint;
                }
                else
                {
                    previous = st.Checkpoints[j - 1];
                }
                if (previous == null || previous.Length != 32)
                {
                    return "mismatch";
                }
                var got = Leg.Result(terrain, previous);
                var want = Bytes.U64Le(st.LegResults[j], 0);
                return got == want ? "match" : "mismatch";
            }
            start += n;
        }
        return "mismatch";
    }

    private static byte[]? OsRandom()
    {
        var b = new byte[8];
        System.Security.Cryptography.RandomNumberGenerator.Fill(b);
        return b;
    }

    private static string SpanOf(string[] headerJson, int left, int right)
    {
        var tl = TimeOf(headerJson[left]);
        var tr = TimeOf(headerJson[right]);
        if (tl == null || tr == null)
        {
            return Nr;
        }
        return Record.Num(tr.Value - tl.Value);
    }

    private static long? TimeOf(string header)
    {
        var t = HeaderField(header, "T");
        if (t == Nr || t == Nd)
        {
            return null;
        }
        if (long.TryParse(t, System.Globalization.NumberStyles.AllowLeadingSign, System.Globalization.CultureInfo.InvariantCulture, out var n))
        {
            return n;
        }
        return null;
    }

    private static string HeaderField(string header, string key)
    {
        var needle = "\"" + key + "\":";
        var at = header.IndexOf(needle, System.StringComparison.Ordinal);
        if (at < 0) return Nd;
        var i = at + needle.Length;
        if (i < header.Length && header[i] == '"')
        {
            var end = header.IndexOf('"', i + 1);
            return header.Substring(i + 1, end - i - 1);
        }
        var j = i;
        while (j < header.Length && header[j] != ',' && header[j] != '}') j++;
        return header.Substring(i, j - i);
    }

    private static string FieldOf(string body, string key) => HeaderField("{" + body + "}", key);

    private static string JsonTokOrNum(string value)
    {
        if (value.Length > 0)
        {
            var i = value[0] == '-' ? 1 : 0;
            if (i < value.Length)
            {
                var digits = true;
                for (; i < value.Length; i++)
                {
                    if (value[i] < '0' || value[i] > '9')
                    {
                        digits = false;
                        break;
                    }
                }
                if (digits)
                {
                    return value;
                }
            }
        }
        return "\"" + value + "\"";
    }

    private static string BlockJson(PathBody body)
    {
        return "{\"block_hash\":\"" + Bytes.Hex(Bytes.Reverse32(body.BlockHash)) + "\",\"block_height\":" + body.BlockHeight + "}";
    }

    private static string BlockFromPath(string pathJson)
    {
        var at = pathJson.IndexOf("\"claimed_block\":", System.StringComparison.Ordinal);
        if (at < 0) return Nd;
        return pathJson.Substring(at + "\"claimed_block\":".Length).TrimEnd('}');
    }

    private static bool All32(byte[][] rows)
    {
        foreach (var r in rows)
        {
            if (r == null || r.Length != 32) return false;
        }
        return true;
    }

    private static void BookendTokens(System.Text.StringBuilder sb, string token)
    {
        sb.Append(BookendBody(token).Replace(",\"legs_token\":\"" + token + "\"", ""));
    }

    private static string HeaderJson(string hash, string height, string cmp, string t)
    {
        return "{\"stored_hash\":" + JsonTokOrNum(hash)
            + ",\"stored_height\":" + JsonTokOrNum(height)
            + ",\"index_comparison\":\"" + cmp + "\""
            + ",\"T\":" + JsonTokOrNum(t) + "}";
    }

    private static string Path16(int g, string state, string plant, string pg, string height, string dg, string claimed)
    {
        return "{\"g\":" + g
            + ",\"path_state\":" + JsonTokOrNum(state)
            + ",\"plant\":" + JsonTokOrNum(plant)
            + ",\"P_g\":" + (pg.StartsWith('{') ? pg : JsonTokOrNum(pg))
            + ",\"height_P_g\":" + JsonTokOrNum(height)
            + ",\"d_g\":" + JsonTokOrNum(dg)
            + ",\"claimed_block\":" + (claimed.StartsWith('{') ? claimed : JsonTokOrNum(claimed))
            + "}";
    }

    private static string Path17(string w17, string claimed)
    {
        return "{\"g\":17,\"w17_path\":" + JsonTokOrNum(w17)
            + ",\"claimed_block\":" + (claimed.StartsWith('{') ? claimed : JsonTokOrNum(claimed)) + "}";
    }
}
