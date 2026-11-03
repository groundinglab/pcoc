namespace Pcoc;

public sealed class HikeObject
{
/// §8
    public sealed class Stage
    {
        public bool Present { get; set; }
        public bool HasPathKey { get; set; }
        public byte[]? Path { get; set; }
        public byte[]? Beacon { get; set; }
        public byte[]? Waypoint { get; set; }
        public bool LegCountPresent { get; set; }
        public ulong LegCountRaw { get; set; }
        public bool LegCountFits { get; set; }
        public byte[][] Checkpoints { get; set; } = System.Array.Empty<byte[]>();
        public bool CheckpointsUsable { get; set; }
        public byte[][] LegResults { get; set; } = System.Array.Empty<byte[]>();
        public bool LegResultsUsable { get; set; }
        public bool HeightPresent { get; set; }
        public ulong HeightRaw { get; set; }
        public bool HeightFits { get; set; }
        public bool FieldSetOk { get; set; } = true;
    }

    public bool Malformed { get; private set; }
    public bool DecodeFailure { get; private set; }
    public string? KindText { get; private set; }
    public bool KindPresent { get; private set; }
    public string? FillText { get; private set; }
    public bool FillPresent { get; private set; }
    public ulong SlotBytesRaw { get; private set; }
    public bool SlotBytesPresent { get; private set; }
    public ulong NSlotsRaw { get; private set; }
    public bool NSlotsPresent { get; private set; }
    public ulong SampleBytesRaw { get; private set; }
    public bool SampleBytesPresent { get; private set; }
    public ulong StepsPerLegRaw { get; private set; }
    public bool StepsPresent { get; private set; }
    public byte[]? PublicKey { get; private set; }
    public byte[]? SeedBeacon { get; private set; }
    public Stage[] Stages { get; private set; } = EmptyStages();
    public bool StagesPresent { get; private set; }
    public int StagesLength { get; private set; }
    public bool TopFieldSet { get; private set; } = true;
    public byte[]? LastInterrupt { get; private set; }
    public bool LastPresent { get; private set; }
    public ulong LastHeightRaw { get; private set; }
    public bool LastHeightPresent { get; private set; }
    public bool LastHeightFits { get; private set; }

    /// §7.3
    public static HikeObject Read(byte[] input)
    {
        var hike = new HikeObject();
        if (!Cbor.TryDecodeOne(input, out var value))
        {
            hike.Malformed = true;
            return hike;
        }
        if (value is not Cbor.MapVal map)
        {
            hike.DecodeFailure = true;
            return hike;
        }
        if (!map.TryGet("kind", out var kindVal) || kindVal is not Cbor.Text kindText)
        {
            hike.DecodeFailure = true;
            return hike;
        }
        hike.KindPresent = true;
        hike.KindText = kindText.S;
        if (kindText.S != Const.KindText)
        {
            return hike;
        }
        hike.TopFieldSet = CheckTop(map);
        if (map.TryGet("fill", out var fill) && fill is Cbor.Text ft)
        {
            hike.FillPresent = true;
            hike.FillText = ft.S;
        }
        TakeU(map, "slot_bytes", out var sb, out var sbp);
        hike.SlotBytesRaw = sb;
        hike.SlotBytesPresent = sbp;
        TakeU(map, "n_slots", out var ns, out var nsp);
        hike.NSlotsRaw = ns;
        hike.NSlotsPresent = nsp;
        TakeU(map, "sample_bytes", out var sab, out var sabp);
        hike.SampleBytesRaw = sab;
        hike.SampleBytesPresent = sabp;
        TakeU(map, "steps_per_leg", out var st, out var stp);
        hike.StepsPerLegRaw = st;
        hike.StepsPresent = stp;
        if (map.TryGet("public_key", out var pk) && pk is Cbor.BytesVal pkb && pkb.B.Length == 32)
        {
            hike.PublicKey = pkb.B;
        }
        if (map.TryGet("seed_beacon", out var sbv) && sbv is Cbor.BytesVal sbb && sbb.B.Length == 32)
        {
            hike.SeedBeacon = sbb.B;
        }
        if (map.TryGet("last_interrupt", out var li))
        {
            hike.LastPresent = true;
            if (li is Cbor.BytesVal lib)
            {
                hike.LastInterrupt = lib.B;
            }
        }
        if (map.TryGet("last_interrupt_height", out var lh) && lh is Cbor.Unsigned lhu)
        {
            hike.LastHeightPresent = true;
            hike.LastHeightRaw = lhu.N;
            hike.LastHeightFits = lhu.N <= ((1UL << 53) - 1);
        }
        if (map.TryGet("stages", out var stages) && stages is Cbor.ArrayVal arr)
        {
            hike.StagesPresent = true;
            hike.StagesLength = arr.Items.Length;
            for (var g = 0; g < Const.Stages && g < arr.Items.Length; g++)
            {
                hike.Stages[g] = ReadStage(arr.Items[g], g);
            }
        }
        return hike;
    }

    private static bool CheckTop(Cbor.MapVal map)
    {
        string[] keys = {
            "fill", "kind", "stages", "n_slots", "public_key", "slot_bytes",
            "seed_beacon", "sample_bytes", "steps_per_leg", "last_interrupt", "last_interrupt_height"
        };
        var seen = new System.Collections.Generic.HashSet<string>();
        foreach (var (k, _) in map.Pairs)
        {
            seen.Add(k);
        }
        foreach (var k in keys)
        {
            if (!seen.Contains(k) && k != "last_interrupt" && k != "last_interrupt_height")
            {
                return false;
            }
        }
        foreach (var k in seen)
        {
            var ok = false;
            foreach (var want in keys)
            {
                if (want == k)
                {
                    ok = true;
                    break;
                }
            }
            if (!ok)
            {
                return false;
            }
        }
        if (!Typed(map, "fill", 3) || !Typed(map, "kind", 3) || !Typed(map, "stages", 4))
        {
            return false;
        }
        if (!Typed(map, "n_slots", 0) || !Typed(map, "public_key", 2) || !Typed(map, "slot_bytes", 0))
        {
            return false;
        }
        if (!Typed(map, "seed_beacon", 2) || !Typed(map, "sample_bytes", 0) || !Typed(map, "steps_per_leg", 0))
        {
            return false;
        }
        if (map.TryGet("last_interrupt", out var li) && li is not Cbor.BytesVal)
        {
            return false;
        }
        if (map.TryGet("last_interrupt_height", out var lh) && lh is not Cbor.Unsigned)
        {
            return false;
        }
        if (map.TryGet("public_key", out var pk) && pk is Cbor.BytesVal pkb && pkb.B.Length != 32)
        {
            return false;
        }
        if (map.TryGet("seed_beacon", out var sb) && sb is Cbor.BytesVal sbb && sbb.B.Length != 32)
        {
            return false;
        }
        return true;
    }

    private static bool Typed(Cbor.MapVal map, string key, int major)
    {
        if (!map.TryGet(key, out var v))
        {
            return false;
        }
        return major switch
        {
            0 => v is Cbor.Unsigned,
            2 => v is Cbor.BytesVal,
            3 => v is Cbor.Text,
            4 => v is Cbor.ArrayVal,
            _ => false,
        };
    }

    private static void TakeU(Cbor.MapVal map, string key, out ulong n, out bool present)
    {
        n = 0;
        present = false;
        if (map.TryGet(key, out var v) && v is Cbor.Unsigned u)
        {
            present = true;
            n = u.N;
        }
    }

    private static Stage ReadStage(Cbor.Value value, int g)
    {
        var stage = new Stage();
        if (value is not Cbor.MapVal map)
        {
            stage.FieldSetOk = false;
            return stage;
        }
        stage.Present = true;
        string[] s0 = { "beacon", "waypoint", "leg_count", "checkpoints", "leg_results", "beacon_height" };
        string[] sg = { "path", "beacon", "waypoint", "leg_count", "checkpoints", "leg_results", "beacon_height" };
        var want = g == 0 ? s0 : sg;
        var seen = new System.Collections.Generic.HashSet<string>();
        foreach (var (k, _) in map.Pairs)
        {
            seen.Add(k);
        }
        stage.HasPathKey = seen.Contains("path");
        var ok = true;
        foreach (var k in want)
        {
            if (!seen.Contains(k))
            {
                ok = false;
            }
        }
        foreach (var k in seen)
        {
            if (g == 0 && k == "path")
            {
                continue;
            }
            var known = false;
            foreach (var w in want)
            {
                if (w == k)
                {
                    known = true;
                }
            }
            if (!known)
            {
                ok = false;
            }
        }
        if (map.TryGet("beacon", out var beacon))
        {
            if (beacon is Cbor.BytesVal bb)
            {
                stage.Beacon = bb.B;
            }
            else
            {
                ok = false;
            }
        }
        if (map.TryGet("waypoint", out var wp))
        {
            if (wp is Cbor.BytesVal wb)
            {
                stage.Waypoint = wb.B;
            }
            else
            {
                ok = false;
            }
        }
        if (map.TryGet("leg_count", out var lc))
        {
            if (lc is Cbor.Unsigned lcu)
            {
                stage.LegCountPresent = true;
                stage.LegCountRaw = lcu.N;
                stage.LegCountFits = lcu.N <= int.MaxValue;
            }
            else
            {
                ok = false;
            }
        }
        if (map.TryGet("beacon_height", out var bh))
        {
            if (bh is Cbor.Unsigned bhu)
            {
                stage.HeightPresent = true;
                stage.HeightRaw = bhu.N;
                stage.HeightFits = bhu.N <= ((1UL << 53) - 1);
            }
            else
            {
                ok = false;
            }
        }
        if (map.TryGet("checkpoints", out var cps))
        {
            if (cps is Cbor.ArrayVal ca)
            {
                stage.CheckpointsUsable = true;
                stage.Checkpoints = new byte[ca.Items.Length][];
                for (var j = 0; j < ca.Items.Length; j++)
                {
                    if (ca.Items[j] is Cbor.BytesVal cb)
                    {
                        stage.Checkpoints[j] = cb.B;
                    }
                    else
                    {
                        stage.CheckpointsUsable = false;
                        ok = false;
                    }
                }
            }
            else
            {
                ok = false;
            }
        }
        if (map.TryGet("leg_results", out var lrs))
        {
            if (lrs is Cbor.ArrayVal la)
            {
                stage.LegResultsUsable = true;
                stage.LegResults = new byte[la.Items.Length][];
                for (var j = 0; j < la.Items.Length; j++)
                {
                    if (la.Items[j] is Cbor.BytesVal lb)
                    {
                        stage.LegResults[j] = lb.B;
                    }
                    else
                    {
                        stage.LegResultsUsable = false;
                        ok = false;
                    }
                }
            }
            else
            {
                ok = false;
            }
        }
        if (map.TryGet("path", out var path))
        {
            if (path is Cbor.BytesVal pb)
            {
                stage.Path = pb.B;
            }
            else
            {
                ok = false;
            }
        }
        stage.FieldSetOk = ok;
        return stage;
    }

    private static Stage[] EmptyStages()
    {
        var s = new Stage[Const.Stages];
        for (var i = 0; i < s.Length; i++)
        {
            s[i] = new Stage();
        }
        return s;
    }
}
