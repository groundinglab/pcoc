using System;
using System.Collections.Generic;
using System.Security.Cryptography;
using System.Text;

namespace Pcoc;

public sealed class VerifyOptions
{
    public required Profile Profile { get; init; }
    public required IIndexSource Index { get; init; }

    /// <summary>Rig-only draw bytes (section 16.1). Null reads the OS CSPRNG.</summary>
    public IDrawByteSource? DrawBytes { get; init; }
}

public enum VerifyStatus
{
    /// <summary>A section 14 record was produced (exit 0), whatever it measured.</summary>
    Record,
    /// <summary>Rule A5: a well-formed map whose kind text is not KIND (exit 0).</summary>
    UnknownKind,
    /// <summary>Section 7.3 decode failure (exit 1): not a well-formed map with a kind text.</summary>
    DecodeFailure,
}

public sealed class VerifyOutcome
{
    public required VerifyStatus Status { get; init; }

    /// <summary>The record JSON text without the CLI's trailing newline.</summary>
    public required string Record { get; init; }
}

/// <summary>
/// The section 14 verifier: shape (A4.1), headers (A4.2), chain (A4.3),
/// paths (A4.5), bookends (A4.6–A4.11), the six-leg check (A4.4), analysis
/// (A4.10), and the Rule A5 unknown-kind record. It records measurements and
/// does not judge them.
/// </summary>
public static class Verifier
{
    private const string ND = "not defined";
    private const string NR = "not read";

    // shape_item strings, in the table order of Rule A4.1
    private const string ShMalformed = "malformed cbor";
    private const string ShFieldSet = "field set";
    private const string ShStage0Path = "stage-0 path key";
    private const string ShW17Path = "W_17 path";
    private const string ShConstant = "constant";
    private const string ShStagesLength = "stages length";
    private const string ShLegCount = "leg_count";
    private const string ShCheckpointWidth = "checkpoint width";
    private const string ShLegResultWidth = "leg result width";
    private const string ShSeedBeacon = "seed beacon";
    private const string ShLastInterrupt = "last_interrupt";
    private const string ShHeightOrder = "header height order";
    private const string ShHashDiffer = "header hash differ";
    private const string ShHeightRange = "height_range";

    public static VerifyOutcome Verify(byte[] objectBytes, VerifyOptions opts)
    {
        // Section 7.3 decode order (erratum 1): malformed value → record;
        // well-formed but not a map, or a map with no kind text → decode
        // failure; kind ≠ KIND → Rule A5; otherwise section 14.
        if (CborReader.Decode(objectBytes, out CborValue? root) == CborReader.Status.Malformed)
            return new VerifyOutcome { Status = VerifyStatus.Record, Record = BuildRecord(null, opts) };
        if (root is not CborValue.Map map)
            return new VerifyOutcome { Status = VerifyStatus.DecodeFailure, Record = "" };
        if (map.Find("kind") is not CborValue.Text kindText)
            return new VerifyOutcome { Status = VerifyStatus.DecodeFailure, Record = "" };
        if (kindText.Value != Profile.Kind)
        {
            var sb = new StringBuilder();
            sb.Append("{\"kind\":\"unknown kind\",\"kind_text\":");
            Rfc8785.WriteString(sb, kindText.Value);
            sb.Append('}');
            return new VerifyOutcome { Status = VerifyStatus.UnknownKind, Record = sb.ToString() };
        }
        return new VerifyOutcome { Status = VerifyStatus.Record, Record = BuildRecord(map, opts) };
    }

    // ---------------- tolerant object views ----------------

    private sealed class StageView
    {
        public bool IsMap;
        public HashSet<string> Keys = new(StringComparer.Ordinal);
        public byte[]? Beacon;
        public ulong? BeaconHeight;
        public ulong? LegCount;
        public byte[]? Waypoint;
        public bool HasPathKey;
        public byte[]? Path;
        public List<CborValue>? Checkpoints;
        public List<CborValue>? LegResults;

        public bool HeightUsable => BeaconHeight is <= IndexRead.MaxHeight;
    }

    private sealed class ObjView
    {
        public string? Fill;
        public ulong? SlotBytes, NSlots, SampleBytes, StepsPerLeg, LastInterruptHeight;
        public byte[]? PublicKey, SeedBeacon, LastInterrupt;
        public bool StagesIsArray;
        public List<CborValue>? StageItems;
        public StageView?[] Stages = new StageView?[Profile.Stages];
        public bool TopFieldSetOk;

        public byte[]? HeaderHash(int k) =>
            k < Profile.Stages ? Stages[k]?.Beacon : LastInterrupt;

        public ulong? HeaderHeight(int k) =>
            k < Profile.Stages ? Stages[k]?.BeaconHeight : LastInterruptHeight;
    }

    private static byte[]? AsBstr(CborValue? v) => (v as CborValue.Bstr)?.Value;
    private static ulong? AsUint(CborValue? v) => (v as CborValue.Uint)?.Value;

    private static StageView? ReadStage(CborValue? v)
    {
        if (v is not CborValue.Map m) return new StageView { IsMap = false };
        var keys = new HashSet<string>(StringComparer.Ordinal);
        foreach (var (k, _) in m.Entries) keys.Add(k);
        return new StageView
        {
            IsMap = true,
            Keys = keys,
            Beacon = AsBstr(m.Find("beacon")),
            BeaconHeight = AsUint(m.Find("beacon_height")),
            LegCount = AsUint(m.Find("leg_count")),
            Waypoint = AsBstr(m.Find("waypoint")),
            HasPathKey = m.Find("path") is not null,
            Path = AsBstr(m.Find("path")),
            Checkpoints = (m.Find("checkpoints") as CborValue.Array)?.Items,
            LegResults = (m.Find("leg_results") as CborValue.Array)?.Items,
        };
    }

    private static ObjView ReadObject(CborValue.Map map)
    {
        var o = new ObjView
        {
            Fill = (map.Find("fill") as CborValue.Text)?.Value,
            SlotBytes = AsUint(map.Find("slot_bytes")),
            NSlots = AsUint(map.Find("n_slots")),
            SampleBytes = AsUint(map.Find("sample_bytes")),
            StepsPerLeg = AsUint(map.Find("steps_per_leg")),
            PublicKey = AsBstr(map.Find("public_key")),
            SeedBeacon = AsBstr(map.Find("seed_beacon")),
            LastInterrupt = AsBstr(map.Find("last_interrupt")),
            LastInterruptHeight = AsUint(map.Find("last_interrupt_height")),
        };
        o.StageItems = (map.Find("stages") as CborValue.Array)?.Items;
        o.StagesIsArray = o.StageItems is not null;
        for (int g = 0; g < Profile.Stages; g++)
            o.Stages[g] = (o.StageItems is not null && g < o.StageItems.Count) ? ReadStage(o.StageItems[g]) : null;

        // top-level field set: exactly the 11 keys of Rule A3 with the listed types
        var want = new Dictionary<string, Type>
        {
            ["kind"] = typeof(CborValue.Text),
            ["fill"] = typeof(CborValue.Text),
            ["slot_bytes"] = typeof(CborValue.Uint),
            ["n_slots"] = typeof(CborValue.Uint),
            ["sample_bytes"] = typeof(CborValue.Uint),
            ["steps_per_leg"] = typeof(CborValue.Uint),
            ["public_key"] = typeof(CborValue.Bstr),
            ["seed_beacon"] = typeof(CborValue.Bstr),
            ["stages"] = typeof(CborValue.Array),
            ["last_interrupt"] = typeof(CborValue.Bstr),
            ["last_interrupt_height"] = typeof(CborValue.Uint),
        };
        bool ok = map.Entries.Count == want.Count;
        if (ok)
            foreach (var (k, v) in map.Entries)
                if (!want.TryGetValue(k, out var t) || v.GetType() != t) { ok = false; break; }
        o.TopFieldSetOk = ok;
        return o;
    }

    private static readonly string[] StageKeys =
        { "beacon", "beacon_height", "leg_count", "waypoint", "checkpoints", "leg_results" };

    private static bool StageFieldSetOk(StageView s, bool stage0)
    {
        if (!s.IsMap) return false; // not "a map that is present"; reported per-field instead
        // exactly the keys section 8 lists for that stage, each with the listed type
        int want = StageKeys.Length + (stage0 ? 0 : 1);
        if (s.Keys.Count != want) return false;
        foreach (string k in StageKeys) if (!s.Keys.Contains(k)) return false;
        if (!stage0 && !s.Keys.Contains("path")) return false;
        return s.Beacon is not null && s.BeaconHeight is not null && s.LegCount is not null
            && s.Waypoint is not null && s.Checkpoints is not null && s.LegResults is not null
            && (stage0 ? !s.HasPathKey : s.Path is not null);
    }

    // ---------------- shape (Rule A4.1) ----------------

    private sealed class Shape
    {
        public bool MalformedCbor;
        public readonly bool[] Violated = new bool[15]; // 1-based by table order
        public bool Mismatch => MalformedCbor || AnyViolated();
        private bool AnyViolated()
        {
            for (int i = 1; i <= 14; i++) if (Violated[i]) return true;
            return false;
        }
        public void WriteItems(JsonWriter w)
        {
            ReadOnlySpan<string> names = new[]
            {
                "", ShMalformed, ShFieldSet, ShStage0Path, ShW17Path, ShConstant, ShStagesLength,
                ShLegCount, ShCheckpointWidth, ShLegResultWidth, ShSeedBeacon, ShLastInterrupt,
                ShHeightOrder, ShHashDiffer, ShHeightRange,
            };
            w.BeginArray();
            if (MalformedCbor) w.Str(ShMalformed);
            else for (int i = 1; i <= 14; i++) if (Violated[i]) w.Str(names[i]);
            w.EndArray();
        }
    }

    private static Shape EvalShape(ObjView? o, Profile profile)
    {
        var sh = new Shape();
        if (o is null) { sh.MalformedCbor = true; return sh; }

        // 2 field set (top level and every present stage map)
        bool fieldSet = !o.TopFieldSetOk;
        for (int g = 0; g < Profile.Stages; g++)
        {
            var s = o.Stages[g];
            if (s is { IsMap: true } && !StageFieldSetOk(s, g == 0)) fieldSet = true;
        }
        sh.Violated[2] = fieldSet;

        // 3 stage-0 path key (violated only when present)
        sh.Violated[3] = o.Stages[0] is { IsMap: true, HasPathKey: true };

        // 4 W_17 path: stages[17].path must be the empty byte string
        var s17 = o.Stages[17];
        sh.Violated[4] = s17 is not { IsMap: true } || s17.Path is not { Length: 0 };

        // 5 constant
        sh.Violated[5] =
            o.Fill != Profile.Fill ||
            o.SlotBytes != (ulong)profile.SlotBytes ||
            o.NSlots != (ulong)profile.NSlots ||
            o.SampleBytes != (ulong)profile.SampleBytes ||
            o.StepsPerLeg != (ulong)profile.StepsPerLeg;

        // 6 stages length
        sh.Violated[6] = !o.StagesIsArray || o.StageItems!.Count != Profile.Stages;

        // 14 height_range: an integer above 2^53 − 1 in the object's own bytes
        bool heightRange = false;
        if (o.LastInterruptHeight > IndexRead.MaxHeight) heightRange = true;
        if (o.SlotBytes > IndexRead.MaxHeight || o.NSlots > IndexRead.MaxHeight ||
            o.SampleBytes > IndexRead.MaxHeight || o.StepsPerLeg > IndexRead.MaxHeight) heightRange = true;
        foreach (var s in o.Stages)
        {
            if (s is null) continue;
            if (s.BeaconHeight > IndexRead.MaxHeight || s.LegCount > IndexRead.MaxHeight) heightRange = true;
        }
        sh.Violated[14] = heightRange;

        // 7 leg_count, 8 checkpoint width, 9 leg result width
        bool legCount = false, cpWidth = false, lrWidth = false;
        foreach (var s in o.Stages)
        {
            if (s is null || !s.IsMap) { legCount = true; cpWidth = true; lrWidth = true; continue; }
            if (s.LegCount is null || s.LegCount < 1 || s.Checkpoints is null || s.LegResults is null ||
                (ulong)s.Checkpoints.Count != s.LegCount || s.LegResults.Count != s.Checkpoints.Count)
                legCount = true;
            if (s.Checkpoints is null) cpWidth = true;
            else foreach (var c in s.Checkpoints) if (c is not CborValue.Bstr { Value.Length: 32 }) cpWidth = true;
            if (s.LegResults is null) lrWidth = true;
            else foreach (var l in s.LegResults) if (l is not CborValue.Bstr { Value.Length: 8 }) lrWidth = true;
        }
        sh.Violated[7] = legCount;
        sh.Violated[8] = cpWidth;
        sh.Violated[9] = lrWidth;

        // 10 seed beacon: stages[0].beacon = seed_beacon
        var s0 = o.Stages[0];
        sh.Violated[10] = s0?.Beacon is null || o.SeedBeacon is null ||
            !s0.Beacon.AsSpan().SequenceEqual(o.SeedBeacon);

        // 11 last_interrupt
        var s17b = o.Stages[17];
        bool liHeightAbsent = o.LastInterruptHeight is null || o.LastInterruptHeight > IndexRead.MaxHeight;
        bool s17HeightAbsent = s17b?.BeaconHeight is null || s17b.BeaconHeight > IndexRead.MaxHeight;
        sh.Violated[11] =
            o.LastInterrupt is not { Length: 32 } ||
            s17b?.Beacon is null ||
            o.LastInterrupt.AsSpan().SequenceEqual(s17b.Beacon) ||
            liHeightAbsent || s17HeightAbsent || o.LastInterruptHeight <= s17b.BeaconHeight;

        // 12 header height order, 13 header hash differ (oversized heights are treated as absent)
        bool order = false, differ = false;
        for (int k = 1; k <= Profile.Stages; k++)
        {
            ulong? h0 = o.HeaderHeight(k - 1), h1 = o.HeaderHeight(k);
            if (h0 is null || h1 is null || h0 > IndexRead.MaxHeight || h1 > IndexRead.MaxHeight || h1 <= h0)
                order = true;
            byte[]? b0 = o.HeaderHash(k - 1), b1 = o.HeaderHash(k);
            if (b0 is null || b1 is null || b0.AsSpan().SequenceEqual(b1))
                differ = true;
        }
        sh.Violated[12] = order;
        sh.Violated[13] = differ;
        return sh;
    }

    // ---------------- the full record ----------------

    private static string BuildRecord(CborValue.Map? map, VerifyOptions opts)
    {
        Profile profile = opts.Profile;
        ObjView? o = map is null ? null : ReadObject(map);
        Shape shape = EvalShape(o, profile);
        var index = new IndexReads(opts.Index);

        // The one terrain fill of this verification (Rule I6, section 14.3).
        // Needed for the Rule A4.3 samples, the waypoint recomputes and the
        // six-leg recomputes; built only when the seed preimage is usable.
        Terrain? terrain = null;
        byte[]? seed = null;
        if (o?.PublicKey is { Length: 32 } pk && o.SeedBeacon is { Length: 32 } sb)
        {
            seed = Terrain.Seed(pk, sb);
            terrain = Terrain.Fill(profile, seed);
        }

        var w = new JsonWriter();
        w.BeginObject();
        w.Key("profile").Str(profile.Name);
        if (profile.Name == "conformance-small")
        {
            w.Key("sample_bytes").Num(profile.SampleBytes);
            w.Key("slot_bytes").Num(profile.SlotBytes);
            w.Key("terrain_bytes").Num(profile.TerrainBytes);
            w.Key("steps_per_leg").Num(profile.StepsPerLeg);
        }
        w.Key("shape").Str(shape.Mismatch ? "mismatch" : "match");
        w.Key("shape_item");
        shape.WriteItems(w);

        // ---- Rule A4.2 headers (19) ----
        var headers = new (string? HashHex, long? Height, string Comparison, long? T)[Profile.Stages + 1];
        w.Key("headers").BeginArray();
        for (int k = 0; k <= Profile.Stages; k++)
        {
            string? hashHex = null;
            long? storedHeight = null;
            string comparison = NR;
            long? t = null;
            byte[]? hb = o?.HeaderHash(k);
            ulong? hh = o?.HeaderHeight(k);
            if (hb is { Length: 32 }) hashHex = HexLower(hb);
            if (hh is not null && hh <= IndexRead.MaxHeight) storedHeight = (long)hh.Value;
            if (hashHex is not null && storedHeight is not null)
            {
                HeaderRead read = index.Header(storedHeight.Value);
                if (read.Success)
                {
                    bool same = read.NaturalHash.AsSpan().SequenceEqual(hb) && read.Height == storedHeight.Value;
                    comparison = same ? "match" : "mismatch";
                    if (same && read.TimeOk) t = read.Time;
                }
            }
            headers[k] = (hashHex, storedHeight, comparison, t);
            w.BeginObject();
            w.Key("stored_hash");
            if (hashHex is null) w.Str(ND); else w.Str(hashHex);
            w.Key("stored_height");
            if (storedHeight is null) w.Str(ND); else w.Num(storedHeight.Value);
            w.Key("index_comparison").Str(comparison);
            w.Key("T");
            if (t is null) w.Str(NR); else w.Num(t.Value);
            w.EndObject();
        }
        w.EndArray();

        // ---- seed_value (Rule A4.3) ----
        string seedValue = ND;
        if (seed is not null && o?.Stages[0]?.Waypoint is { } w0)
            seedValue = w0.AsSpan().SequenceEqual(seed) ? "match" : "mismatch";
        w.Key("seed_value").Str(seedValue);

        // ---- Rule A4.3 checkpoint digests (18 stages) ----
        w.Key("checkpoints").BeginArray();
        for (int g = 0; g < Profile.Stages; g++)
        {
            w.BeginObject();
            w.Key("g").Num(g);
            w.Key("legs");
            StageView? s = o?.Stages[g];
            if (s is null || !s.IsMap || s.Checkpoints is null)
            {
                w.Str(ND);
            }
            else
            {
                w.BeginArray();
                for (int j = 0; j < s.Checkpoints.Count; j++)
                {
                    string result = "mismatch";
                    byte[]? cpBytes = AsBstr(s.Checkpoints[j]);
                    byte[]? lrBytes = (s.LegResults is not null && j < s.LegResults.Count) ? AsBstr(s.LegResults[j]) : null;
                    byte[]? previous = LegStart(o, seed, g, s, j, out bool startOk);
                    byte[]? limb = g == 0 ? seed : (s.Waypoint is { Length: 32 } ? s.Waypoint : null);
                    if (cpBytes is not null && lrBytes is not null && startOk && limb is not null && terrain is not null)
                    {
                        byte[] digest = RecomputeCheckpoint(terrain, previous!, limb, lrBytes);
                        if (digest.AsSpan().SequenceEqual(cpBytes)) result = "match";
                    }
                    w.BeginObject();
                    w.Key("j").Num(j);
                    w.Key("result").Str(result);
                    w.EndObject();
                }
                w.EndArray();
            }
            w.EndObject();
        }
        w.EndArray();

        // ---- Rule A4.3 waypoint recomputes (W_1 … W_17) ----
        w.Key("waypoints").BeginArray();
        for (int g = 0; g < Profile.Stages - 1; g++)
        {
            string result = ND;
            StageView? sg = o?.Stages[g];
            StageView? sg1 = o?.Stages[g + 1];
            if (terrain is not null && sg is { IsMap: true } && sg1 is { IsMap: true } &&
                sg.Waypoint is { Length: 32 } sw && sg.Checkpoints is { } cps && AllBstr(cps) &&
                sg1.Beacon is { Length: 32 } interrupt && sg1.Waypoint is { Length: 32 } storedW)
            {
                var cpList = new List<byte[]>(cps.Count);
                foreach (var c in cps) cpList.Add(((CborValue.Bstr)c).Value);
                byte[] recomputed = Leg.Waypoint(terrain, interrupt, sw, cpList);
                result = recomputed.AsSpan().SequenceEqual(storedW) ? "match" : "mismatch";
            }
            w.BeginObject();
            w.Key("g").Num(g + 1);
            w.Key("result").Str(result);
            w.EndObject();
        }
        w.EndArray();

        // ---- Rule A4.5 paths (17) ----
        var anchorsTok = new string[Profile.Stages - 2]; // W_1 … W_16
        int anchoredTotal = 0;
        var skipList = new List<int>();
        var validPath = new bool[Profile.Stages];   // g in 1 … 16
        var anchoredAt = new bool[Profile.Stages];  // g in 1 … 16
        var bodyAt = new PathBody?[Profile.Stages];
        var plantReadIssuedNoDoc = new bool[Profile.Stages];
        var plantMatch = new bool[Profile.Stages];
        w.Key("paths").BeginArray();
        for (int g = 1; g <= Profile.Stages - 1; g++)
        {
            StageView? s = o?.Stages[g];
            if (g == Profile.Stages - 1)
            {
                // W_17
                string w17;
                string? cbHash = null; long? cbHeight = null;
                if (s?.Path is { Length: 0 }) { w17 = "not fetched by design"; }
                else if (s?.Path is { Length: > 0 } p17)
                {
                    w17 = ND;
                    if (PathBody.TryReadClaimedBlock(p17, out byte[]? cbh, out uint cbht))
                    {
                        cbHash = HexLowerReversed(cbh!);
                        cbHeight = cbht;
                    }
                }
                else { w17 = ND; }
                w.BeginObject();
                w.Key("g").Num(g);
                w.Key("w17_path").Str(w17);
                w.Key("claimed_block");
                WriteBlock(w, cbHash, cbHeight);
                w.EndObject();
                continue;
            }

            string pathState, plant = NR;
            string? pgHash = null; long? pgHeight = null; long? pgHeightConfirmed = null; long? dg = null;
            string? cbHash2 = null; long? cbHeight2 = null;
            if (s is null || !s.IsMap || !s.HasPathKey || s.Path is null)
            {
                pathState = ND; plant = NR;
            }
            else if (s.Path.Length == 0)
            {
                pathState = "skip"; plant = "n/a";
                skipList.Add(g);
            }
            else
            {
                pathState = "present";
                PathBody? body = PathBody.Decode(s.Path);
                bool validated = body is not null && s.Waypoint is { Length: 32 } wp && body.ValidateFor(wp);
                if (body is null || !validated)
                {
                    plant = "mismatch"; // path facts disagree with a canonical body
                }
                else
                {
                    validPath[g] = true;
                    bodyAt[g] = body;
                    cbHash2 = HexLowerReversed(body!.BlockHash);
                    cbHeight2 = body.BlockHeight;
                    bool inWindow = s.HeightUsable &&
                        (long)body.BlockHeight - (long)s.BeaconHeight!.Value >= 2 &&
                        (long)body.BlockHeight - (long)s.BeaconHeight!.Value <= 4;
                    bool isInterrupt = inWindow && IsStoredInterrupt(o, body.BlockHeight, body.BlockHash);
                    if (!inWindow || !isInterrupt)
                    {
                        plant = NR; // the read was not issued
                    }
                    else
                    {
                        byte[]? doc = index.PlantDoc(body.BlockHeight);
                        var outcome = IndexRead.Plant(doc, body, out _);
                        plant = outcome switch
                        {
                            IndexRead.PlantOutcome.Match => "match",
                            IndexRead.PlantOutcome.Mismatch => "mismatch",
                            _ => NR,
                        };
                        if (doc is null) plantReadIssuedNoDoc[g] = true;
                        if (outcome == IndexRead.PlantOutcome.Match)
                        {
                            plantMatch[g] = true;
                            pgHash = cbHash2;
                            pgHeight = cbHeight2;
                            pgHeightConfirmed = cbHeight2;
                            if (headers[g].Comparison == "match")
                                dg = cbHeight2 - headers[g].Height!.Value;
                        }
                    }
                }
            }
            anchorsTok[g - 1] = plantMatch[g] ? "anchored" : "not anchored";
            if (plantMatch[g]) { anchoredTotal++; anchoredAt[g] = true; }

            w.BeginObject();
            w.Key("g").Num(g);
            w.Key("path_state").Str(pathState);
            w.Key("plant").Str(plant);
            w.Key("P_g");
            WriteBlock(w, pgHash, pgHeight);
            w.Key("height_P_g");
            if (pgHeightConfirmed is null) w.Str(ND); else w.Num(pgHeightConfirmed.Value);
            w.Key("d_g");
            if (dg is null) w.Str(ND); else w.Num(dg.Value);
            w.Key("claimed_block");
            WriteBlock(w, cbHash2, cbHeight2);
            w.EndObject();
        }
        w.EndArray();

        w.Key("anchors").BeginArray();
        foreach (var anchorTok in anchorsTok) w.Str(anchorTok);
        w.EndArray();
        w.Key("anchored_total").Num(anchoredTotal);
        w.Key("skip_list").BeginArray();
        foreach (int g in skipList) w.Num(g);
        w.EndArray();

        // ---- Rules A4.6–A4.11 bookends ----
        int validCount = 0;
        for (int g = 1; g <= Profile.Stages - 2; g++) if (validPath[g]) validCount++;
        bool anyIssuedNoDoc = false;
        for (int g = 1; g <= Profile.Stages - 2; g++) if (plantReadIssuedNoDoc[g]) anyIssuedNoDoc = true;
        int anchoredCount = 0, aIdx = 0, bIdx = 0;
        for (int g = 1; g <= Profile.Stages - 2; g++)
            if (anchoredAt[g]) { if (anchoredCount == 0) aIdx = g; bIdx = g; anchoredCount++; }

        string bookendState; // "undefined" | "unread" | "integers"
        if (validCount < 2) bookendState = "undefined";          // step 1
        else if (anyIssuedNoDoc) bookendState = "unread";        // step 2
        else if (anchoredCount < 2) bookendState = "undefined";  // step 3
        else bookendState = "integers";                          // step 4

        long? aV = null, bV = null, nV = null;
        long? sV = null;
        long? dhV = null;
        string? pbHash = null; long? pbHeight = null;
        long? heightPb = null; string pbComparison = ND; long? tPb = null;
        long? heightIam1 = null; long? tIam1 = null;
        long? fsN = null; long? fsS = null; long? fsDh = null; string fsDhTok = ND;

        if (bookendState == "integers")
        {
            aV = aIdx; bV = bIdx;
            // step 4 endpoint reads: headers[a] is I_{a−1}; the P_b read reuses
            // the section 14.3 plant document of that height (Rule A4.8).
            PathBody bodyB = bodyAt[bIdx]!;
            HeaderRead pbRead0 = IndexRead.Header(index.PlantDoc(bodyB.BlockHeight));
            if (headers[aIdx].Comparison == NR || !pbRead0.Success)
            {
                bookendState = "unread"; // every bookend field is not read
                aV = bV = null;
            }
            else
            {
                pbHash = HexLowerReversed(bodyB.BlockHash);
                pbHeight = bodyB.BlockHeight;
            }
        }

        if (bookendState == "integers")
        {
            nV = CountCheckpoints(o, aIdx, bIdx - 1); // stages a … b−1
            HeaderRead pbRead = IndexRead.Header(index.PlantDoc(bodyAt[bIdx]!.BlockHeight));
            // index_comparison_P_b: display bytes on both sides, no reversal
            var bodyDisplay = (byte[])bodyAt[bIdx]!.BlockHash.Clone();
            Array.Reverse(bodyDisplay);
            bool pbSame = pbRead.NaturalHash.AsSpan().SequenceEqual(bodyDisplay) &&
                          pbRead.Height == pbHeight!.Value;
            pbComparison = pbSame ? "match" : "mismatch";
            if (pbSame)
            {
                heightPb = pbRead.Height;
                if (pbRead.TimeOk) tPb = pbRead.Time;
            }

            if (headers[aIdx].Comparison == "match")
            {
                heightIam1 = headers[aIdx].Height!.Value;
                if (headers[aIdx].T is not null) tIam1 = headers[aIdx].T!.Value;
            }

            // S (Rule A4.8): both T values, or not read
            if (tPb is not null && tIam1 is not null) sV = tPb.Value - tIam1.Value;
            // delta_h: both heights index-confirmed, or not defined
            if (heightPb is not null && heightIam1 is not null) dhV = heightPb.Value - heightIam1.Value;

            // fixed_start
            fsN = CountCheckpoints(o, 1, bIdx - 1);
            if (tPb is not null && headers[1].T is not null) fsS = tPb.Value - headers[1].T!.Value;
            if (headers[1].Comparison == "match" && pbComparison == "match")
                fsDh = heightPb!.Value - headers[1].Height!.Value;
            else if (headers[1].Comparison == NR || pbComparison == NR)
                fsDhTok = NR;
            else
                fsDhTok = ND;
        }

        string tok = bookendState == "unread" ? NR : ND;
        w.Key("a"); if (aV is null) w.Str(tok); else w.Num(aV.Value);
        w.Key("b"); if (bV is null) w.Str(tok); else w.Num(bV.Value);
        w.Key("N"); if (nV is null) w.Str(tok); else w.Num(nV.Value);
        w.Key("S"); if (sV is not null) w.Num(sV.Value); else w.Str(bookendState == "integers" ? NR : tok);
        w.Key("delta_h"); if (dhV is not null) w.Num(dhV.Value); else w.Str(bookendState == "integers" ? ND : tok);
        w.Key("P_b"); if (pbHash is null) { w.Str(tok); } else WriteBlock(w, pbHash, pbHeight);
        w.Key("height_P_b"); if (heightPb is null) w.Str(bookendState == "integers" ? ND : tok); else w.Num(heightPb.Value);
        w.Key("index_comparison_P_b"); w.Str(bookendState == "integers" ? pbComparison : tok);
        w.Key("T_P_b"); if (tPb is not null) w.Num(tPb.Value); else w.Str(bookendState == "integers" ? NR : tok);
        w.Key("height_I_a_minus_1"); if (heightIam1 is null) w.Str(bookendState == "integers" ? ND : tok); else w.Num(heightIam1.Value);
        w.Key("T_I_a_minus_1"); if (tIam1 is not null) w.Num(tIam1.Value); else w.Str(bookendState == "integers" ? NR : tok);
        w.Key("fixed_start").BeginObject();
        w.Key("N"); if (fsN is null) w.Str(tok); else w.Num(fsN.Value);
        w.Key("S"); if (fsS is not null) w.Num(fsS.Value); else w.Str(bookendState == "integers" ? NR : tok);
        w.Key("delta_h"); if (fsDh is not null) w.Num(fsDh.Value); else w.Str(bookendState == "integers" ? fsDhTok : tok);
        w.EndObject();

        // ---- Rule A4.4 six-leg ----
        long legTotal = 0;
        bool legTotalSettled = o is not null;
        if (o is not null)
        {
            for (int g = 0; g < Profile.Stages; g++)
            {
                var s = o.Stages[g];
                if (s is null || !s.IsMap || s.Checkpoints is null) { legTotalSettled = false; break; }
                legTotal += s.Checkpoints.Count;
            }
        }
        else legTotalSettled = false;

        w.Key("six_leg");
        var sixIndices = new List<long>();
        var sixResults = new List<string>();
        string? sixTok = null;
        if (!legTotalSettled || legTotal < 6)
        {
            sixTok = ND;
        }
        else if (terrain is null)
        {
            // The seed preimage is unusable, so no leg can be recomputed; the
            // draw is still made and every result is mismatch.
            IDrawByteSource src0 = opts.DrawBytes ?? new CsprngDrawBytes();
            if (!Draw.SixLeg(src0, (ulong)legTotal, out ulong[] qs0)) { sixTok = NR; }
            else
            {
                foreach (ulong q in qs0) { sixIndices.Add((long)q); sixResults.Add("mismatch"); }
            }
        }
        else
        {
            IDrawByteSource src = opts.DrawBytes ?? new CsprngDrawBytes();
            if (!Draw.SixLeg(src, (ulong)legTotal, out ulong[] qs))
            {
                sixTok = NR;
            }
            else
            {
                // the draw finishes before any leg result is recomputed (A4.4)
                var starts = new long[Profile.Stages + 1];
                for (int g = 0; g < Profile.Stages; g++) starts[g + 1] = starts[g] + o!.Stages[g]!.Checkpoints!.Count;
                foreach (ulong q in qs)
                {
                    int g = 0;
                    while (!(starts[g] <= (long)q && (long)q < starts[g + 1])) g++;
                    int j = (int)((long)q - starts[g]);
                    StageView s = o!.Stages[g]!;
                    string res = "mismatch";
                    byte[]? previous = LegStart(o, seed, g, s, j, out bool startOk);
                    byte[]? lr = (s.LegResults is not null && j < s.LegResults.Count) ? AsBstr(s.LegResults[j]) : null;
                    if (startOk && lr is { Length: 8 })
                    {
                        ulong recomputed = Leg.RunLeg(terrain!, previous!, profile.StepsPerLeg);
                        if (recomputed == Prims.U64Le(lr)) res = "match";
                    }
                    sixIndices.Add((long)q);
                    sixResults.Add(res);
                }
            }
        }
        WriteSixLeg(w, sixTok, sixIndices, sixResults);
        w.Key("six_leg_runs");
        if (sixTok == NR)
        {
            w.Str(NR);
        }
        else if (sixTok == ND)
        {
            w.BeginArray().EndArray();
        }
        else
        {
            w.BeginArray();
            WriteSixLeg(w, null, sixIndices, sixResults);
            w.EndArray();
        }

        // ---- Rule A4.10 analysis ----
        w.Key("analysis").BeginObject();
        long? wholeLegs = CountCheckpointsOrNull(o, 1, Profile.Stages - 1);
        w.Key("whole_hike_legs"); if (wholeLegs is null) w.Str(ND); else w.Num(wholeLegs.Value);
        w.Key("whole_hike_span");
        if (headers[Profile.Stages].T is not null && headers[1].T is not null)
            w.Num(headers[Profile.Stages].T!.Value - headers[1].T!.Value);
        else w.Str(NR);
        w.Key("per_stage").BeginArray();
        for (int g = 0; g < Profile.Stages; g++)
        {
            var s = o?.Stages[g];
            w.BeginObject();
            w.Key("g").Num(g);
            w.Key("checkpoint_count");
            if (s is null || !s.IsMap || s.Checkpoints is null) w.Str(ND); else w.Num(s.Checkpoints.Count);
            w.Key("span");
            if (headers[g + 1].T is not null && headers[g].T is not null)
                w.Num(headers[g + 1].T!.Value - headers[g].T!.Value);
            else w.Str(NR);
            w.EndObject();
        }
        w.EndArray();
        w.Key("legs_outside");
        if (bookendState != "integers") w.Str(tok);
        else
        {
            long inside = CountCheckpoints(o, aIdx, bIdx - 1);
            long total = CountCheckpoints(o, 0, Profile.Stages - 1);
            w.Num(total - inside);
        }
        long? hikeLegs = CountCheckpointsOrNull(o, 0, Profile.Stages - 1);
        w.Key("hike_legs"); if (hikeLegs is null) w.Str(ND); else w.Num(hikeLegs.Value);
        w.Key("hike_span");
        long? mtpFirst = Mtp(index, o, headers, 0);
        long? mtpLast = Mtp(index, o, headers, Profile.Stages);
        if (mtpFirst is not null && mtpLast is not null) w.Num(mtpLast.Value - mtpFirst.Value);
        else w.Str(NR);
        w.EndObject();

        w.EndObject();
        return w.ToString();
    }

    private static void WriteSixLeg(JsonWriter w, string? token, List<long> indices, List<string> results)
    {
        if (token is not null) { w.Str(token); return; }
        w.BeginObject();
        w.Key("indices").BeginArray();
        foreach (long q in indices) w.Num(q);
        w.EndArray();
        w.Key("results").BeginArray();
        foreach (string r in results) w.Str(r);
        w.EndArray();
        w.EndObject();
    }

    private static void WriteBlock(JsonWriter w, string? displayHash, long? height)
    {
        if (displayHash is null || height is null) { w.Str(ND); return; }
        w.BeginObject();
        w.Key("block_hash").Str(displayHash);
        w.Key("block_height").Num(height.Value);
        w.EndObject();
    }

    /// <summary>
    /// Median-time-past (Rule A4.10) for the beacon recorded at headers[k]
    /// (k = 0: first beacon; k = 18: last beacon). Null when any window time
    /// is missing.
    /// </summary>
    private static long? Mtp(IndexReads index, ObjView? o,
        (string? HashHex, long? Height, string Comparison, long? T)[] headers, int k)
    {
        long? h = o?.HeaderHeight(k) is ulong uh && uh <= IndexRead.MaxHeight ? (long)uh : null;
        if (h is null) return null;
        long n = Math.Min(11, h.Value + 1);
        var times = new List<long>((int)n);
        for (long height = h.Value - n + 1; height <= h.Value; height++)
        {
            if (height == h.Value)
            {
                // the beacon's own header is the Rule A4.2 read: its time only on match
                if (headers[k].Comparison != "match" || headers[k].T is null) return null;
                times.Add(headers[k].T!.Value);
            }
            else
            {
                if (!IndexRead.MtpTime(index.MtpOtherDoc(height), height, out long tOther)) return null;
                times.Add(tOther);
            }
        }
        times.Sort();
        return times[(int)(n / 2)];
    }

    private static bool IsStoredInterrupt(ObjView? o, uint blockHeight, byte[] blockHashNatural)
    {
        if (o is null) return false;
        for (int k = 1; k <= Profile.Stages - 1; k++)
        {
            var s = o.Stages[k];
            if (s is { HeightUsable: true, Beacon: { Length: 32 } } &&
                s.BeaconHeight!.Value == blockHeight &&
                s.Beacon.AsSpan().SequenceEqual(blockHashNatural))
                return true;
        }
        return o.LastInterrupt is { Length: 32 } &&
               o.LastInterruptHeight is not null && o.LastInterruptHeight <= IndexRead.MaxHeight &&
               o.LastInterruptHeight.Value == blockHeight &&
               o.LastInterrupt.AsSpan().SequenceEqual(blockHashNatural);
    }

    /// <summary>The previous checkpoint of leg j of stage g, when usable.</summary>
    private static byte[]? LegStart(ObjView? o, byte[]? seed, int g, StageView s, int j, out bool ok)
    {
        ok = true;
        if (j == 0)
        {
            if (g == 0)
            {
                if (seed is null) { ok = false; return null; }
                return seed;
            }
            if (s.Waypoint is { Length: 32 }) return s.Waypoint;
            ok = false;
            return null;
        }
        if (s.Checkpoints is not null && j - 1 < s.Checkpoints.Count &&
            AsBstr(s.Checkpoints[j - 1]) is { Length: >= 8 } prev)
            return prev;
        ok = false;
        return null;
    }

    private static byte[] RecomputeCheckpoint(Terrain terrain, byte[] previous, byte[] stageLimb, byte[] legResultLimb)
    {
        int sampleBytes = terrain.Profile.SampleBytes;
        // the digest binds the stored leg-result limb as section 11.4 states
        var preimage = new byte[32 + 32 + sampleBytes + 8];
        // previous limb: the full stored previous checkpoint bytes (32 on a conforming object)
        if (previous.Length != 32 || legResultLimb.Length != 8)
            return new byte[32]; // unusable widths never equal a stored checkpoint
        previous.CopyTo(preimage, 0);
        stageLimb.CopyTo(preimage, 32);
        terrain.Sample(previous).CopyTo(preimage.AsSpan(64));
        legResultLimb.CopyTo(preimage, 64 + sampleBytes);
        return SHA256.HashData(preimage);
    }

    private static bool AllBstr(List<CborValue> items)
    {
        foreach (var c in items) if (c is not CborValue.Bstr) return false;
        return true;
    }

    private static long CountCheckpoints(ObjView? o, int gFrom, int gTo)
    {
        long n = 0;
        for (int g = gFrom; g <= gTo; g++)
        {
            var s = o?.Stages[g];
            if (s is not null && s.IsMap && s.Checkpoints is not null) n += s.Checkpoints.Count;
        }
        return n;
    }

    /// <summary>Null when any stage in the range has no usable checkpoint array.</summary>
    private static long? CountCheckpointsOrNull(ObjView? o, int gFrom, int gTo)
    {
        long n = 0;
        for (int g = gFrom; g <= gTo; g++)
        {
            var s = o?.Stages[g];
            if (s is null || !s.IsMap || s.Checkpoints is null) return null;
            n += s.Checkpoints.Count;
        }
        return n;
    }

    internal static string HexLower(byte[] b)
    {
        var sb = new StringBuilder(b.Length * 2);
        foreach (byte x in b) sb.Append(x.ToString("x2"));
        return sb.ToString();
    }

    /// <summary>Bitcoin display order: the section 9 reversal of natural bytes.</summary>
    internal static string HexLowerReversed(byte[] natural)
    {
        var sb = new StringBuilder(natural.Length * 2);
        for (int i = natural.Length - 1; i >= 0; i--) sb.Append(natural[i].ToString("x2"));
        return sb.ToString();
    }
}
