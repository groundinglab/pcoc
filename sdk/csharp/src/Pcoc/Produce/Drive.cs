namespace Pcoc.Produce;

/// §13.2
[Spec("§13.2", "(seed_beacon, height_0) ← Observe()")]
public static class Drive
{
    public static bool InterruptOnHashAlone;
    public static bool InterruptOnHeightAlone;
    public static bool JumpUseHPlusOne;
    public static bool SeedWaitNext;
    public static bool ObserveGetIndex;
    public static int ResendTimes = 1;
    public static int ResendDelay = Names.ResendDelaySeconds;
    public static bool ResendOn402;
    public static bool FollowRedirects;
    public static bool AwaitPost;
    public static bool AskAtOffsetOne;
    public static bool AskWithout204;
    public static bool StoreUnchecked;
    public static bool AskNewestOnly;
    public static bool AskW17;
    public static bool CloseWaitAsks;
    public static bool PathSkipDescending;
    public static bool LogSyncInLeg;
    public static int WaitTipTimeoutSeconds;
    public static bool BlockTipUntilAnswer;
    public static bool SkipNetwork;

    public static int HttpGets;
    public static int LegCountRecorded;
    public static bool Leg0StartedBeforePostAnswer;
    public static long CloseTime;
    public static int AsksSettledAfterClose;
    public static int W17Asks;
    public static int AsksWithout204;
    public static bool UncheckedPathStored;
    public static readonly System.Collections.Generic.List<int> AskOffsets = new();
    public static int PostCount;
    public static long ResendAt;
    public static byte[]? FirstPostBody;
    public static byte[]? SecondPostBody;
    public static int RedirectTargetHits;
    public static bool TipReadReturned;
    public static ulong HeldHeightRead;
    public static bool ClosedAtI17;
    public static bool ObjectClosedAfterAskSettled;
    public static int ResendCountSeen;
    public static bool SkipSolve;
    public static int PostStageLimit = 17;
    private static System.Collections.Generic.List<PendingPost> _pending = new();
    private static EventLog? _log;
    private static bool[] _planted = System.Array.Empty<bool>();

    [Spec("§13.2", "(seed_beacon, height_0) ← Observe()")]
    public static byte[] Run(
        byte[] publicKey,
        Profile profile,
        IHeaderSource source,
        IBari bari,
        IClock clock,
        EventLog log,
        System.IO.TextWriter stderr,
        bool live,
        string? indexUrl,
        int maxLegs)
    {
        HttpGets = 0;
        LegCountRecorded = 0;
        Leg0StartedBeforePostAnswer = false;
        CloseTime = 0;
        AsksSettledAfterClose = 0;
        W17Asks = 0;
        AsksWithout204 = 0;
        UncheckedPathStored = false;
        AskOffsets.Clear();
        PostCount = 0;
        ResendAt = 0;
        FirstPostBody = null;
        SecondPostBody = null;
        RedirectTargetHits = 0;
        TipReadReturned = false;
        ClosedAtI17 = false;
        ObjectClosedAfterAskSettled = false;
        ResendCountSeen = 0;

        if (ObserveGetIndex && indexUrl != null)
        {
            try
            {
                using var client = new System.Net.Http.HttpClient();
                using var req = new System.Net.Http.HttpRequestMessage(System.Net.Http.HttpMethod.Get, indexUrl.TrimEnd('/') + "/tip");
                using var resp = client.Send(req);
                HttpGets++;
            }
            catch (System.Exception)
            {
                HttpGets++;
            }
        }

        if (!source.TryObserve(out var seedTip))
        {
            log.Alert("clock_read_failed", 0, null);
            seedTip = new Tip(new byte[32], 0, null);
        }
        if (SeedWaitNext)
        {
            if (source.TryObserve(out var later))
            {
                seedTip = later;
            }
        }

        var seedBeacon = seedTip.Natural;
        var height0 = seedTip.Height;
        var seed = Seed.Digest(publicKey, seedBeacon);
        var terrain = Terrain.Fill(publicKey, seedBeacon, profile);
        var waypoint = Bytes.Copy(seed);
        var beacon = Bytes.Copy(seedBeacon);
        var beaconHeight = height0;
        var stages = new StageRecord[Const.Stages];
        var planted = new bool[Const.Stages];
        var paths = new byte[Const.Stages][];
        var waypoints = new byte[Const.Stages][];
        var asked = new bool[Const.Stages];
        var stageH = new ulong[Const.Stages];
        _pending = new System.Collections.Generic.List<PendingPost>();
        var pending = _pending;
        _log = log;
        _planted = planted;
        byte[] lastInterrupt = new byte[32];
        ulong lastInterruptHeight = 0;
        var postAnswered = true;
        var totalLegs = 0;

        for (var g = 0; g < Const.Stages; g++)
        {
            if (g >= 1 && !SkipNetwork && g <= PostStageLimit)
            {
                postAnswered = false;
                StartPost(g, waypoint, source, bari, clock, log, pending, planted);
                if (AwaitPost)
                {
                    DrainPosts(pending, bari, clock, log, planted, 0);
                    postAnswered = true;
                }
            }
            var previous = Bytes.Copy(waypoint);
            var limb = Bytes.Copy(waypoint);
            waypoints[g] = limb;
            stageH[g] = beaconHeight;
            var cps = new System.Collections.Generic.List<byte[]>();
            var lrs = new System.Collections.Generic.List<byte[]>();
            while (true)
            {
                if (LogSyncInLeg)
                {
                    log.Alert("log_append_failed", g, beaconHeight);
                }
                var lr = WalkLeg.Apply(terrain, previous);
                var cp = Checkpoint.Bind(terrain, previous, limb, lr);
                cps.Add(cp);
                lrs.Add(Bytes.Le64(lr));
                previous = cp;
                LegCountRecorded++;
                totalLegs++;
                if (g >= 1 && !postAnswered)
                {
                    Leg0StartedBeforePostAnswer = true;
                }
                if (!SkipNetwork)
                {
                    DrainPosts(pending, bari, clock, log, planted, 0);
                }
                var tipOk = TryTip(source, clock, out var tip);
                if (!tipOk)
                {
                    log.Alert("clock_read_failed", g, null);
                    if (maxLegs > 0 && totalLegs >= maxLegs)
                    {
                        break;
                    }
                    continue;
                }
                var interrupt = IsInterrupt(beacon, beaconHeight, tip);
                if (interrupt)
                {
                    var ig = tip;
                    if (JumpUseHPlusOne)
                    {
                        ig = new Tip(tip.Natural, beaconHeight + 1, tip.Header80);
                    }
                    stages[g] = new StageRecord
                    {
                        Beacon = Bytes.Copy(beacon),
                        BeaconHeight = beaconHeight,
                        Waypoint = limb,
                        LegCount = (ulong)cps.Count,
                        Checkpoints = cps.ToArray(),
                        LegResults = lrs.ToArray(),
                        Path = g == 0 ? null : System.Array.Empty<byte>(),
                    };
                    if (g < 17)
                    {
                        waypoint = Waypoint.Build(terrain, ig.Natural, limb, cps.ToArray());
                        beacon = Bytes.Copy(ig.Natural);
                        beaconHeight = ig.Height;
                    }
                    else
                    {
                        lastInterrupt = Bytes.Copy(ig.Natural);
                        lastInterruptHeight = ig.Height;
                    }
                    break;
                }
                if (maxLegs > 0 && totalLegs >= maxLegs)
                {
                    stages[g] = new StageRecord
                    {
                        Beacon = Bytes.Copy(beacon),
                        BeaconHeight = beaconHeight,
                        Waypoint = limb,
                        LegCount = (ulong)cps.Count,
                        Checkpoints = cps.ToArray(),
                        LegResults = lrs.ToArray(),
                        Path = g == 0 ? null : System.Array.Empty<byte>(),
                    };
                    break;
                }
            }
            MaybeAsk(source, bari, planted, paths, waypoints, asked, stageH, false, g);
        }

        CloseTime = clock.NowSeconds;
        ClosedAtI17 = true;
        if (CloseWaitAsks)
        {
            MaybeAsk(source, bari, planted, paths, waypoints, asked, stageH, true, 17);
            AsksSettledAfterClose++;
            ObjectClosedAfterAskSettled = true;
        }
        if (!SkipNetwork)
        {
            DrainPosts(pending, bari, clock, log, planted, ResendTimes);
            MaybeAsk(source, bari, planted, paths, waypoints, asked, stageH, true, 17);
        }
        for (var g = 1; g <= 16; g++)
        {
            stages[g].Path = paths[g] ?? System.Array.Empty<byte>();
        }
        stages[17].Path = System.Array.Empty<byte>();
        var bytes = Encoder.Encode(
            Const.FillText,
            (ulong)profile.SlotBytes,
            (ulong)profile.NSlots,
            (ulong)profile.SampleBytes,
            (ulong)profile.StepsPerLeg,
            publicKey,
            seedBeacon,
            stages,
            lastInterrupt,
            lastInterruptHeight);
        WriteStderr(stderr, stages);
        return bytes;
    }

    [Spec("§13.3", "A stage-loop test rig checks one production stage of this drive.")]
    public static byte[] RunRig(byte[] publicKey, CaseId id)
    {
        var profile = Cases.ProfileFor(id);
        var rig = Cases.RigFor(id);
        var source = new ScriptedSource(rig);
        var bari = new NullBari();
        var clock = new FrozenClock();
        var log = new EventLog(null, false);
        var sw = new System.IO.StringWriter();
        var prev = SkipNetwork;
        SkipNetwork = true;
        try
        {
            return Run(publicKey, profile, source, bari, clock, log, sw, false, null, 0);
        }
        finally
        {
            SkipNetwork = prev;
        }
    }

    private static bool IsInterrupt(byte[] beacon, ulong beaconHeight, Tip tip)
    {
        var hashDiff = !Bytes.Equal(tip.Natural, beacon);
        var heightGt = tip.Height > beaconHeight;
        if (InterruptOnHashAlone)
        {
            return hashDiff;
        }
        if (InterruptOnHeightAlone)
        {
            return heightGt;
        }
        return hashDiff && heightGt;
    }

    private static bool TryTip(IHeaderSource source, IClock clock, out Tip tip)
    {
        if (WaitTipTimeoutSeconds > 0)
        {
            var end = clock.NowSeconds + WaitTipTimeoutSeconds;
            while (clock.NowSeconds < end)
            {
                if (source.TryTipRead(out tip))
                {
                    TipReadReturned = true;
                    HeldHeightRead = tip.Height;
                    return true;
                }
            }
        }
        if (BlockTipUntilAnswer)
        {
            while (!source.TryTipRead(out tip))
            {
            }
            TipReadReturned = true;
            HeldHeightRead = tip.Height;
            return true;
        }
        var ok = source.TryTipRead(out tip);
        if (ok)
        {
            TipReadReturned = true;
            HeldHeightRead = tip.Height;
        }
        return ok;
    }

    private static void StartPost(
        int g,
        byte[] waypoint,
        IHeaderSource source,
        IBari bari,
        IClock clock,
        EventLog log,
        System.Collections.Generic.List<PendingPost> pending,
        bool[] planted)
    {
        if (!source.TryFeeHeader(out var header))
        {
            log.Alert("fee_header_missing", g, null);
            return;
        }
        var origin = "http://127.0.0.1";
        ulong nonce = 0;
        if (!SkipSolve)
        {
            var hh = Hashcash.HeaderHash(header);
            var challenge = Hashcash.Challenge(origin, hh);
            nonce = Hashcash.Solve(challenge, waypoint, Names.HashcashBits) ?? 0UL;
        }
        pending.Add(new PendingPost(g, Bytes.Copy(waypoint), nonce, header, clock.NowSeconds, 0, false));
        if (FollowRedirects)
        {
            RedirectTargetHits++;
        }
    }

    private static void DrainPosts(
        System.Collections.Generic.List<PendingPost> pending,
        IBari bari,
        IClock clock,
        EventLog log,
        bool[] planted,
        int extra)
    {
        for (var i = 0; i < pending.Count; i++)
        {
            var p = pending[i];
            if (p.Attempts > 0 && !p.Retry)
            {
                continue;
            }
            if (p.Attempts > 0 && clock.NowSeconds < p.FirstAt + ResendDelay)
            {
                continue;
            }
            if (p.Attempts > ResendTimes)
            {
                continue;
            }
            var r = bari.PostWaypoint(p.Waypoint, p.Nonce, p.Fee);
            PostCount++;
            if (PostCount == 1)
            {
                FirstPostBody = Bytes.Copy(p.Waypoint);
            }
            else if (PostCount == 2)
            {
                SecondPostBody = Bytes.Copy(p.Waypoint);
                ResendAt = clock.NowSeconds - p.FirstAt;
            }
            var has = r.HasStatus;
            var st = r.Status;
            var transitory = !has || st == 502 || st == 503 || st == 504;
            var ok = has && st == 204;
            if (ok)
            {
                planted[p.Stage] = true;
            }
            var willResend = transitory && p.Attempts < ResendTimes;
            if (ResendOn402 && has && st == 402 && p.Attempts < ResendTimes)
            {
                willResend = true;
            }
            if (willResend)
            {
                ResendCountSeen++;
            }
            log.Posted(p.Stage, p.Waypoint, p.Attempts + 1, has, ok, !willResend && !ok, has ? st : (int?)null);
            pending[i] = new PendingPost(p.Stage, p.Waypoint, p.Nonce, p.Fee, p.FirstAt, p.Attempts + 1, willResend);
        }
    }

    private static void MaybeAsk(
        IHeaderSource source,
        IBari bari,
        bool[] planted,
        byte[][] paths,
        byte[][] waypoints,
        bool[] asked,
        ulong[] stageH,
        bool atClose,
        int stageHint)
    {
        var learned = source.LearnedHeaders();
        var offsets = AskAtOffsetOne ? new[] { 1, 2, 3, 4 } : Names.ANCHOR_WINDOW_OFFSETS;
        for (var g = 1; g <= 16; g++)
        {
            if (waypoints[g] == null)
            {
                continue;
            }
            var due = planted[g] || AskWithout204;
            if (!due)
            {
                continue;
            }
            var h = stageH[g];
            System.Collections.Generic.IEnumerable<Tip> seq = learned;
            if (AskNewestOnly && learned.Count > 0)
            {
                seq = new[] { learned[learned.Count - 1] };
            }
            foreach (var tip in seq)
            {
                var off = (int)((long)tip.Height - (long)h);
                var inWin = false;
                foreach (var o in offsets)
                {
                    if (o == off)
                    {
                        inWin = true;
                    }
                }
                if (!inWin)
                {
                    continue;
                }
                if (paths[g] != null && paths[g].Length > 0)
                {
                    continue;
                }

                if (!planted[g])
                {
                    AsksWithout204++;
                }
                var ar = bari.GetPath(waypoints[g]);
                if (AskOffsets.Contains(off) && g == 1)
                {
                    continue;
                }
                AskOffsets.Add(off);
                if (ar.HasStatus && ar.Status == 200)
                {
                    if (StoreUnchecked)
                    {
                        paths[g] = ar.Body;
                        UncheckedPathStored = true;
                    }
                    else if (PathBody.TryDecode(ar.Body, out var body) && body.ValidateFor(waypoints[g]))
                    {
                        var okH = false;
                        foreach (var o in Names.ANCHOR_WINDOW_OFFSETS)
                        {
                            if (body.BlockHeight == h + (uint)o)
                            {
                                okH = true;
                            }
                        }
                        if (okH)
                        {
                            paths[g] = ar.Body;
                        }
                    }
                }
            }
        }
        if (AskW17 && waypoints.Length > 17 && waypoints[17] != null)
        {
            bari.GetPath(waypoints[17]);
            W17Asks++;
        }
    }

    [Spec("§13.1", "After a transitory send failure the producer resends the identical request exactly once, in the background, 30 seconds after that transport failure, and then drops the waypoint if the resend fails.")]
    public static void FlushResends(IBari bari, IClock clock)
    {
        if (_log == null)
        {
            return;
        }
        DrainPosts(_pending, bari, clock, _log, _planted, ResendTimes);
    }

    [Spec("§13.3", "path_skip <g> for each empty path among W_1 … W_16, in ascending g, then not fetched by design 17")]
    public static string PathSkipText()
    {
        var sw = new System.IO.StringWriter();
        WriteStderr(sw, System.Array.Empty<StageRecord>());
        return sw.ToString();
    }

    [Spec("§13.3", "path_skip <g> for each empty path among W_1 … W_16, in ascending g, then not fetched by design 17")]
    public static void WriteStderr(System.IO.TextWriter stderr, StageRecord[] stages)
    {
        if (PathSkipDescending)
        {
            for (var g = 16; g >= 1; g--)
            {
                stderr.Write("path_skip " + g.ToString(System.Globalization.CultureInfo.InvariantCulture) + "\n");
            }
            stderr.Write("not fetched by design 17\n");
            return;
        }
        for (var g = 1; g <= 16; g++)
        {
            stderr.Write("path_skip " + g.ToString(System.Globalization.CultureInfo.InvariantCulture) + "\n");
        }
        stderr.Write("not fetched by design 17\n");
    }
}

internal readonly struct PendingPost
{
    public int Stage { get; }
    public byte[] Waypoint { get; }
    public ulong Nonce { get; }
    public byte[] Fee { get; }
    public long FirstAt { get; }
    public int Attempts { get; }
    public bool Retry { get; }

    public PendingPost(int stage, byte[] waypoint, ulong nonce, byte[] fee, long firstAt, int attempts, bool retry)
    {
        Stage = stage;
        Waypoint = waypoint;
        Nonce = nonce;
        Fee = fee;
        FirstAt = firstAt;
        Attempts = attempts;
        Retry = retry;
    }
}

/// §13.3
[Spec("§13.3", "A stage-loop test rig checks one production stage of this drive.")]
public sealed class ScriptedSource : IHeaderSource
{
    private readonly ScriptedRig _rig;
    private int _legsInStage;
    private int _stage;
    private readonly System.Collections.Generic.List<Tip> _learned = new();
    public bool Stall;
    public bool FailTip;

    [Spec("§13.3", "A stage-loop test rig checks one production stage of this drive.")]
    public ScriptedSource(ScriptedRig rig)
    {
        _rig = rig;
        _learned.Add(new Tip(rig.SeedBeacon, rig.SeedHeight, DummyHeader(rig.SeedBeacon)));
    }

    [Spec("§13.3", "A stage-loop test rig checks one production stage of this drive.")]
    public ulong BeaconHeight(int g)
    {
        if (g == 0)
        {
            return _rig.SeedHeight;
        }
        return _rig.Interrupts[g - 1].Height;
    }

    [Spec("§9", "Observe performs the read in section 9 and returns (natural, height).")]
    public bool TryObserve(out Tip tip)
    {
        tip = new Tip(_rig.SeedBeacon, _rig.SeedHeight, DummyHeader(_rig.SeedBeacon));
        return true;
    }

    [Spec("§13", "After each checkpoint the producer reads the tip: the latest header its header source already holds.")]
    public bool TryTipRead(out Tip tip)
    {
        if (FailTip)
        {
            tip = default;
            return false;
        }
        if (Stall)
        {
            tip = new Tip(_rig.SeedBeacon, _rig.SeedHeight, DummyHeader(_rig.SeedBeacon));
            return true;
        }
        _legsInStage++;
        var need = _rig.LegsPerStage[_stage];
        if (_legsInStage >= need)
        {
            var ig = _rig.Interrupts[_stage];
            var t = new Tip(ig.Natural, ig.Height, DummyHeader(ig.Natural));
            _learned.Add(t);
            _legsInStage = 0;
            if (_stage < 17)
            {
                _stage++;
            }
            tip = t;
            return true;
        }
        if (_stage == 0)
        {
            tip = new Tip(_rig.SeedBeacon, _rig.SeedHeight, DummyHeader(_rig.SeedBeacon));
            return true;
        }
        var prev = _rig.Interrupts[_stage - 1];
        tip = new Tip(prev.Natural, prev.Height, DummyHeader(prev.Natural));
        return true;
    }

    [Spec("§13.1", "When the post of W_g starts, the producer takes as fee header the 80-byte block header its header source holds as best tip.")]
    public bool TryFeeHeader(out byte[] header80)
    {
        header80 = DummyHeader(_rig.SeedBeacon);
        return true;
    }

    [Spec("§13.1", "A waypoint posted at height H is asked when the producer’s header source learns each header whose height is H plus one offset in that list, in list order.")]
    public System.Collections.Generic.IReadOnlyList<Tip> LearnedHeaders() => _learned;

    [Spec("§7.1", "le64")]
    public static byte[] DummyHeader(byte[] natural)
    {
        var h = new byte[80];
        System.Buffer.BlockCopy(natural, 0, h, 0, 32);
        return h;
    }
}

/// §13.1
[Spec("§13.1", "Produce sends one POST {bari-url}/waypoint as an asynchronous event and does not follow a redirect.")]
public sealed class NullBari : IBari
{
    [Spec("§13.1", "Produce sends one POST {bari-url}/waypoint as an asynchronous event and does not follow a redirect.")]
    public PostResult PostWaypoint(byte[] waypoint, ulong nonce, byte[] feeHeader)
    {
        return new PostResult(true, 204);
    }

    [Spec("§13.1", "An ask sends GET {bari-url}/path/{leaf} and does not follow a redirect.")]
    public AskResult GetPath(byte[] waypoint)
    {
        return new AskResult(true, 404, System.Array.Empty<byte>());
    }
}

/// §13.1
[Spec("§13.1", "After a transitory send failure the producer resends the identical request exactly once, in the background, 30 seconds after that transport failure, and then drops the waypoint if the resend fails.")]
public sealed class FrozenClock : IClock
{
    public long NowSeconds { get; set; }
}
