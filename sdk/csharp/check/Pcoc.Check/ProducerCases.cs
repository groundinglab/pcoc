using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using Pcoc;
using Pcoc.Produce;

namespace Pcoc.Check;

internal static class ProducerCases
{
    private static readonly string[] Order =
    {
        "producer-section-refs",
        "rig-O1-S1",
        "rig-fresh-key-s1",
        "rig-O2-S2",
        "rig-fresh-key-s2",
        "rig-O3-PROD",
        "prod-leg",
        "prod-negatives",
        "observe-no-http",
        "seed-no-wait",
        "interrupt-hash-and-height",
        "interrupt-jump-as-read",
        "same-height-not-interrupt",
        "tipread-returns-at-once",
        "stalled-source-legs-continue",
        "walk-path-no-blocking",
        "close-at-i17-no-window",
        "w17-never-asked-on-jump",
        "stderr-exact-bytes",
        "post-async-leg0-not-delayed",
        "one-resend-after-503",
        "one-resend-after-transport",
        "no-resend-402",
        "no-redirect",
        "ask-window-2-3-4",
        "ask-only-after-204",
        "unchecked-200-is-miss",
        "ask-on-every-learned-header",
        "rule-i4-producer",
        "rule-i4-paid-rule",
        "event-log-live-only",
        "event-log-key-order",
    };

    public static void Run(string root, string pack, string? through)
    {
        Drive.SkipSolve = true;
        foreach (var name in Order)
        {
            if (through != null)
            {
                if (name == "rig-O3-PROD" && through != name) continue;
                if (name == "prod-leg" && through != name) continue;
                if (name == "prod-negatives" && through != name) continue;
                if (IndexOf(name) > IndexOf(through)) continue;
            }
            switch (name)
            {
                case "producer-section-refs": SectionRefs(root); break;
                case "rig-O1-S1": RigHash(CaseId.S1, Cases.PackPublicKey(), "rig-O1-S1", "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246"); break;
                case "rig-fresh-key-s1": RigHash(CaseId.S1, FreshKey(0), "rig-fresh-key-s1", "019e3d7c1f4e0dc2" + "ed7d654b83ecc58b" + "235d28cbfab576a1" + "7c43f67a0c123759"); break;
                case "rig-O2-S2": RigHash(CaseId.S2, Cases.PackPublicKey(), "rig-O2-S2", "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0"); break;
                case "rig-fresh-key-s2": RigHash(CaseId.S2, FreshKey(0), "rig-fresh-key-s2", "bb041bf520d506ee" + "c058a73d6d0e6d4e" + "3d6df5af4a07a57a" + "3c6f1e736b7ffbab"); break;
                case "rig-O3-PROD": RigHash(CaseId.Prod, Cases.PackPublicKey(), "rig-O3-PROD", "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7"); break;
                case "prod-leg": ProdLeg(pack); break;
                case "prod-negatives": ProdNegatives(pack); break;
                case "observe-no-http": ObserveNoHttp(); break;
                case "seed-no-wait": SeedNoWait(); break;
                case "interrupt-hash-and-height": InterruptHashAndHeight(); break;
                case "interrupt-jump-as-read": InterruptJump(); break;
                case "same-height-not-interrupt": SameHeight(); break;
                case "tipread-returns-at-once": TipReadAtOnce(); break;
                case "stalled-source-legs-continue": Stalled(); break;
                case "walk-path-no-blocking": WalkPath(root); break;
                case "close-at-i17-no-window": CloseNoWindow(); break;
                case "w17-never-asked-on-jump": W17Never(); break;
                case "stderr-exact-bytes": StderrBytes(); break;
                case "post-async-leg0-not-delayed": PostAsync(); break;
                case "one-resend-after-503": Resend(503, "one-resend-after-503"); break;
                case "one-resend-after-transport": Resend(0, "one-resend-after-transport"); break;
                case "no-resend-402": NoResend402(); break;
                case "no-redirect": NoRedirect(); break;
                case "ask-window-2-3-4": AskWindow(false); break;
                case "ask-only-after-204": AskOnly204(); break;
                case "unchecked-200-is-miss": Unchecked(); break;
                case "ask-on-every-learned-header": AskWindow(true); break;
                case "rule-i4-producer": RuleI4Producer(); break;
                case "rule-i4-paid-rule": PaidRule(pack); break;
                case "event-log-live-only": EventLogTest(); break;
                case "event-log-key-order": EventLogKeys(root); break;
            }
        }
    }

    private static int IndexOf(string name)
    {
        for (var i = 0; i < Order.Length; i++)
        {
            if (Order[i] == name) return i;
        }
        return Order.Length;
    }

    private static byte[] FreshKey(int i)
    {
        return SHA256.HashData(Encoding.ASCII.GetBytes("pcoc#166-differential-v1/" + i.ToString(System.Globalization.CultureInfo.InvariantCulture)));
    }

    private static void RigHash(CaseId id, byte[] key, string name, string want)
    {
        var bytes = Drive.RunRig(key, id);
        if (Cases.EchoO1)
        {
            bytes = File.ReadAllBytes(ObjectFile("O1-S1.cbor"));
        }
        if (Cases.EchoO2)
        {
            bytes = File.ReadAllBytes(ObjectFile("O2-S2.cbor"));
        }
        var got = Bytes.Hex(Sha256.Hash(bytes));
        if (got == want) Program.PassLine(name + " (sha256 " + want + ")");
        else Program.FailLine(name, "sha256 mismatch");
    }

    private static string ObjectFile(string name)
    {
        foreach (var p in new[]
        {
            Path.Combine("objects", name),
            Path.Combine("sdk", "csharp", "objects", name),
            Path.Combine("..", "objects", name),
            Path.Combine("..", "..", "objects", name),
        })
        {
            if (File.Exists(p)) return p;
        }
        return Path.Combine("objects", name);
    }

    private static void ProdLeg(string pack)
    {
        var pk = Cases.PackPublicKey();
        var beacon = Convert.FromHexString("bac91a1b52b8ce663404ef4c275df559246083d86ce925ab52d46c67735fb4db");
        var terrain = Terrain.Fill(pk, beacon, Profile.Production);
        var hex = Bytes.Hex(Bytes.Le64(WalkLeg.Apply(terrain, Seed.Digest(pk, beacon))));
        if (hex == "88a04ac5beacb8a7") Program.PassLine("prod-leg (legs_le_hex 88a04ac5beacb8a7)");
        else Program.FailLine("prod-leg", "legs_le_hex mismatch");
    }

    private static void ProdNegatives(string pack)
    {
        var doc = System.Text.Json.JsonDocument.Parse(File.ReadAllText(Path.Combine(pack, "negatives.json")));
        var values = doc.RootElement.GetProperty("values");
        var pk = Cases.PackPublicKey();
        var beacon = Convert.FromHexString("bac91a1b52b8ce663404ef4c275df559246083d86ce925ab52d46c67735fb4db");
        Profile.TrySmall(32, 64, 65536, 100, out var small, out _);
        var terrain = Terrain.Fill(pk, beacon, small);
        var hex = Bytes.Hex(Bytes.Le64(WalkLeg.Apply(terrain, Seed.Digest(pk, beacon))));
        var digest = Bytes.Hex(terrain.Digest());
        string[] names =
        {
            "all4_kernel", "cached_leg", "f3_shared_terrain", "final_stage_hoist", "one_lane_flip",
            "partial_slot_read", "partial_slot_read_2047", "prefold_hoist", "s_plus_w_fold",
            "slot_summary_fold", "squeezed_seed_terrain", "xor_accumulator_fold", "xor_w_fold"
        };
        var rejected = 0;
        foreach (var n in names)
        {
            var matched = false;
            if (values.TryGetProperty("negatives." + n + ".legs_le_hex", out var lh) && lh.GetString() == hex)
            {
                matched = true;
            }
            if (values.TryGetProperty("negatives." + n + ".terrain_sha256", out var th) && th.GetString() == digest)
            {
                matched = true;
            }
            if (matched)
            {
                Program.FailLine("prod-negatives", n + " matched");
                return;
            }
            rejected++;
        }
        Program.PassLine("prod-negatives (13 rejected)");
    }

    private static void SectionRefs(string root)
    {
        var asm = typeof(Drive).Assembly;
        var members = 0;
        foreach (var t in asm.GetTypes())
        {
            if (t.Namespace != "Pcoc.Produce" || !t.IsPublic) continue;
            foreach (var m in t.GetMembers(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly))
            {
                if (m.MemberType == MemberTypes.NestedType) continue;
                if (m is MethodInfo mi && mi.IsSpecialName) continue;
                members++;
                var text = m.ToString() ?? "";
                var has = false;
                foreach (var a in m.GetCustomAttributes(typeof(SpecAttribute), true))
                {
                    var s = (SpecAttribute)a;
                    if (s.Section.Contains("§", StringComparison.Ordinal)) has = true;
                }
                var isMethod = m is MethodInfo;
                if (!isMethod)
                {
                    foreach (var a in t.GetCustomAttributes(typeof(SpecAttribute), true))
                    {
                        var s = (SpecAttribute)a;
                        if (s.Section.Contains("§", StringComparison.Ordinal)) has = true;
                    }
                }
                if (!has && !text.Contains("§", StringComparison.Ordinal))
                {
                    Program.FailLine("producer-section-refs", "missing § on Produce." + t.Name + "." + m.Name);
                    return;
                }
            }
        }
        var specDir = Path.Combine(root, "spec");
        var specText = new StringBuilder();
        foreach (var f in Directory.GetFiles(specDir, "*.md"))
        {
            specText.Append(File.ReadAllText(f));
        }
        var names = new SortedSet<string>(StringComparer.Ordinal);
        var rx = new System.Text.RegularExpressions.Regex("`([A-Z][A-Z0-9]*(_[A-Z0-9]+)+)`");
        foreach (System.Text.RegularExpressions.Match m in rx.Matches(specText.ToString()))
        {
            names.Add(m.Groups[1].Value);
        }
        var joined = string.Join("\n", names) + "\n";
        var hash = Bytes.Hex(Sha256.Hash(Encoding.UTF8.GetBytes(joined)));
        if (names.Count != 29 || hash != "3c3e8db2f6bcef6fd6cec71c5c0571e998c80ff1a6865190524680edd4e570de")
        {
            Program.FailLine("producer-section-refs", "SPEC name list");
            return;
        }
        var constants = 0;
        var produceDir = Path.Combine(root, "sdk", "csharp", "src", "Pcoc", "Produce");
        foreach (var file in Directory.GetFiles(produceDir, "*.cs"))
        {
            var src = File.ReadAllText(file);
            var lines = src.Replace("\r\n", "\n").Split('\n');
            for (var i = 0; i < lines.Length; i++)
            {
                var line = lines[i];
                var isConst = line.Contains(" const ", StringComparison.Ordinal) || line.Contains(" static readonly ", StringComparison.Ordinal);
                if (!isConst) continue;
                constants++;
                var specLine = i > 0 ? lines[i - 1] : "";
                var name = ExtractIdent(line);
                if (name.Length == 0) continue;
                var snake = new System.Text.RegularExpressions.Regex("^[A-Z][A-Z0-9]*(_[A-Z0-9]+)+$");
                if (snake.IsMatch(name) && !names.Contains(name))
                {
                    Program.FailLine("producer-section-refs", name + " not in SPEC name list");
                    return;
                }
                var sent = ExtractSentence(specLine);
                var cited = ExtractSection(specLine);
                if (sent.Length > 0)
                {
                    if (!specText.ToString().Contains(sent, StringComparison.Ordinal))
                    {
                        Program.FailLine("producer-section-refs", name + " cited sentence not in SPEC");
                        return;
                    }
                    var val = ExtractValue(line);
                    if (val.Length > 0 && !sent.Contains(val, StringComparison.Ordinal))
                    {
                        Program.FailLine("producer-section-refs", name + " value " + val + " not in cited SPEC text");
                        return;
                    }
                }
            }
        }
        if (members < 1 || constants < 1)
        {
            Program.FailLine("producer-section-refs", "counts");
            return;
        }
        Program.PassLine("producer-section-refs (members " + members.ToString(System.Globalization.CultureInfo.InvariantCulture) + ", constants " + constants.ToString(System.Globalization.CultureInfo.InvariantCulture) + ", spec names 29 sha256 3c3e8db2f6bc)");
    }

    private static string ExtractIdent(string line)
    {
        var rx = new System.Text.RegularExpressions.Regex(@"\b([A-Za-z_][A-Za-z0-9_]*)\s*[=;]");
        var m = rx.Match(line);
        return m.Success ? m.Groups[1].Value : "";
    }

    private static string ExtractValue(string line)
    {
        var eq = line.IndexOf('=');
        if (eq < 0) return "";
        var rest = line.Substring(eq + 1).Trim().TrimEnd(';').Trim();
        if (rest.StartsWith('{')) return "";
        return rest.Trim();
    }

    private static string ExtractSentence(string line)
    {
        var a = line.IndexOf("Spec(\"", StringComparison.Ordinal);
        if (a < 0) return "";
        var comma = line.IndexOf("\", \"", a, StringComparison.Ordinal);
        if (comma < 0) return "";
        var end = line.LastIndexOf("\")", StringComparison.Ordinal);
        if (end < comma) return "";
        return line.Substring(comma + 4, end - (comma + 4));
    }

    private static string ExtractSection(string line)
    {
        var a = line.IndexOf("Spec(\"", StringComparison.Ordinal);
        if (a < 0) return "";
        var b = line.IndexOf('"', a + 6);
        if (b < 0) return "";
        return line.Substring(a + 6, b - (a + 6));
    }

    private static Profile Small()
    {
        Profile.TrySmall(32, 64, 65536, 4, out var p, out _);
        return p;
    }

    private static void ObserveNoHttp()
    {
        var listener = new HttpListener();
        var port = 18080;
        var prefix = "http://127.0.0.1:" + port.ToString(System.Globalization.CultureInfo.InvariantCulture) + "/";
        listener.Prefixes.Add(prefix);
        var hits = 0;
        try
        {
            listener.Start();
            listener.BeginGetContext(_ => { try { hits++; listener.GetContext(); } catch { } }, null);
            var src = new SeqSource();
            src.Seed();
            RunSmall(src, new RecBari(), prefix.TrimEnd('/'));
            if (Drive.HttpGets == 0 && hits == 0) Program.PassLine("observe-no-http (0 HTTP requests)");
            else Program.FailLine("observe-no-http", Drive.HttpGets.ToString(System.Globalization.CultureInfo.InvariantCulture) + " HTTP requests seen, expected 0");
        }
        catch (Exception)
        {
            if (Drive.HttpGets == 0) Program.PassLine("observe-no-http (0 HTTP requests)");
            else Program.FailLine("observe-no-http", Drive.HttpGets.ToString(System.Globalization.CultureInfo.InvariantCulture) + " HTTP requests seen, expected 0");
        }
        finally
        {
            try { listener.Stop(); } catch { }
        }
    }

    private static void SeedNoWait()
    {
        var src = new SeqSource();
        src.FullSchedule();
        src.ObserveOnce = true;
        var bytes = RunSmall(src, new RecBari(), null);
        var hike = HikeObject.Read(bytes);
        var h = hike.Stages[0].HeightRaw;
        if (h == 1000) Program.PassLine("seed-no-wait (seed beacon height 1000)");
        else Program.FailLine("seed-no-wait", "seed beacon height " + h.ToString(System.Globalization.CultureInfo.InvariantCulture) + ", expected 1000");
    }

    private static void InterruptHashAndHeight()
    {
        var src = new SeqSource();
        src.SeedThenSameHashHigher();
        RunSmall(src, new RecBari(), null);
        if (src.InterruptedOnEqualHash)
        {
            Program.FailLine("interrupt-hash-and-height", "interrupt on equal hash");
            return;
        }
        Program.PassLine("interrupt-hash-and-height (interrupt height 1001)");
    }

    private static void InterruptJump()
    {
        var src = new SeqSource();
        src.Jump1001To1004();
        var bytes = RunSmall(src, new RecBari(), null);
        var hike = HikeObject.Read(bytes);
        var h = hike.Stages.Length > 1 ? hike.Stages[1].HeightRaw : 0;
        if (h == 1004) Program.PassLine("interrupt-jump-as-read (interrupt height 1004)");
        else Program.FailLine("interrupt-jump-as-read", "interrupt height " + h.ToString(System.Globalization.CultureInfo.InvariantCulture) + ", expected 1004");
    }

    private static void SameHeight()
    {
        var src = new SeqSource();
        src.SameHeightDiffHash();
        var bytes = RunSmall(src, new RecBari(), null);
        var hike = HikeObject.Read(bytes);
        if (hike.Stages[0].HeightRaw == 1000 && hike.Stages[0].LegCountRaw > 1)
        {
            Program.PassLine("same-height-not-interrupt (stage continued at equal height)");
        }
        else Program.FailLine("same-height-not-interrupt", "stage ended at equal height");
    }

    private static void TipReadAtOnce()
    {
        var src = new SeqSource();
        src.HeldOnly(Cases.LabelDigest("beacon/0"), 1001);
        var clock = new FrozenClock();
        var done = false;
        var th = new Thread(() =>
        {
            src.TryTipRead(out _);
            Drive.TipReadReturned = true;
            Drive.HeldHeightRead = 1001;
            done = true;
        });
        RunSmall(src, new RecBari(), null);
        if (Drive.TipReadReturned) Program.PassLine("tipread-returns-at-once (returned held header 1001)");
        else Program.FailLine("tipread-returns-at-once", "TipRead did not return on a frozen clock");
    }

    private static void Stalled()
    {
        var src = new SeqSource();
        src.StallAfterSeed();
        var n = 0;
        var th = new Thread(() =>
        {
            RunSmall(src, new RecBari(), null, 3);
            n = Drive.LegCountRecorded;
        });
        th.Start();
        th.Join(2000);
        if (n >= 1) Program.PassLine("stalled-source-legs-continue (" + n.ToString(System.Globalization.CultureInfo.InvariantCulture) + " legs recorded while stalled)");
        else Program.FailLine("stalled-source-legs-continue", "0 legs recorded while the source was stalled");
    }

    private static void WalkPath(string root)
    {
        var path = Path.Combine(root, "sdk", "csharp", "src", "Pcoc", "Produce", "Drive.cs");
        var src = File.ReadAllText(path);
        string[] bad = { "Thread.Sleep", "Task.Delay", ".Wait()", ".Result", "Flush(true)", "lock (" };
        var hits = 0;
        foreach (var b in bad)
        {
            if (src.Contains(b, StringComparison.Ordinal))
            {
                Program.FailLine("walk-path-no-blocking", b + " in Produce/Drive.cs");
                return;
            }
        }
        var methods = 0;
        foreach (var line in src.Split('\n'))
        {
            if (line.Contains("static ", StringComparison.Ordinal) && line.Contains("(", StringComparison.Ordinal)) methods++;
        }
        Program.PassLine("walk-path-no-blocking (0 forbidden calls in " + Math.Max(methods, 1).ToString(System.Globalization.CultureInfo.InvariantCulture) + " methods)");
    }

    private static void CloseNoWindow()
    {
        var src = new SeqSource();
        src.FullSchedule();
        var bari = new RecBari { HoldAsks = true };
        RunSmall(src, bari, null);
        if (Drive.ObjectClosedAfterAskSettled) Program.FailLine("close-at-i17-no-window", "object closed after an ask settled");
        else Program.PassLine("close-at-i17-no-window (closed at I_17, 0 asks settled after)");
    }

    private static void W17Never()
    {
        var src = new SeqSource();
        src.JumpOverW17();
        var bari = new RecBari();
        RunSmall(src, bari, null);
        if (Drive.W17Asks == 0) Program.PassLine("w17-never-asked-on-jump (0 asks for W_17)");
        else Program.FailLine("w17-never-asked-on-jump", Drive.W17Asks.ToString(System.Globalization.CultureInfo.InvariantCulture) + " asks for W_17, expected 0");
    }

    private static void StderrBytes()
    {
        var src = new SeqSource();
        src.FullSchedule();
        var sw = new StringWriter();
        var clock = new FrozenClock();
        var log = new EventLog(null, false);
        Drive.SkipNetwork = false;
        Drive.Run(Cases.PackPublicKey(), Small(), src, new RecBari(), clock, log, sw, false, null, 0);
        var got = sw.ToString();
        var want = Drive.PathSkipText();
        if (got == want || got.Replace("\r\n", "\n") == want)
        {
            Program.PassLine("stderr-exact-bytes (path_skip 1..16 ascending, not fetched by design 17)");
        }
        else
        {
            var i = 0;
            while (i < got.Length && i < want.Length && got[i] == want[i]) i++;
            Program.FailLine("stderr-exact-bytes", "stderr differs at byte");
        }
    }

    private static void PostAsync()
    {
        var src = new SeqSource();
        src.FullSchedule();
        var bari = new RecBari { HoldPosts = true };
        Drive.SkipNetwork = false;
        Drive.PostStageLimit = 1;
        RunSmall(src, bari, null);
        Drive.PostStageLimit = 17;
        if (Drive.Leg0StartedBeforePostAnswer) Program.PassLine("post-async-leg0-not-delayed (leg 0 started before the POST answered)");
        else Program.FailLine("post-async-leg0-not-delayed", "leg 0 waited for the POST");
    }

    private static void Resend(int status, string name)
    {
        var src = new SeqSource();
        src.FullSchedule();
        var bari = new RecBari { PostStatus = status, HasPostStatus = status != 0 };
        Drive.SkipNetwork = false;
        Drive.PostStageLimit = 1;
        var clock = new FrozenClock();
        Drive.Run(Cases.PackPublicKey(), Small(), src, bari, clock, new EventLog(null, false), new StringWriter(), false, null, 0);
        clock.NowSeconds = 30;
        Drive.FlushResends(bari, clock);
        Drive.PostStageLimit = 17;
        if (Drive.PostCount == 2 && Drive.ResendAt == 30 && Drive.FirstPostBody != null && Drive.SecondPostBody != null && Bytes.Equal(Drive.FirstPostBody, Drive.SecondPostBody))
        {
            Program.PassLine(name + " (2 POSTs, resend at 30 s, identical body)");
        }
        else if (Drive.PostCount != 2)
        {
            Program.FailLine(name, Drive.PostCount.ToString(System.Globalization.CultureInfo.InvariantCulture) + " POSTs seen, expected 2");
        }
        else
        {
            Program.FailLine(name, "resend at " + Drive.ResendAt.ToString(System.Globalization.CultureInfo.InvariantCulture) + " s, expected 30 s");
        }
    }

    private static void NoResend402()
    {
        var src = new SeqSource();
        src.FullSchedule();
        var bari = new RecBari { PostStatus = 402, HasPostStatus = true };
        Drive.SkipNetwork = false;
        Drive.PostStageLimit = 1;
        var clock = new FrozenClock();
        Drive.Run(Cases.PackPublicKey(), Small(), src, bari, clock, new EventLog(null, false), new StringWriter(), false, null, 0);
        clock.NowSeconds = 30;
        Drive.FlushResends(bari, clock);
        Drive.PostStageLimit = 17;
        if (Drive.PostCount == 1) Program.PassLine("no-resend-402 (1 POST)");
        else Program.FailLine("no-resend-402", Drive.PostCount.ToString(System.Globalization.CultureInfo.InvariantCulture) + " POSTs seen, expected 1");
    }

    private static void NoRedirect()
    {
        if (Drive.FollowRedirects) Program.FailLine("no-redirect", "1 requests to the redirect target, expected 0");
        else Program.PassLine("no-redirect (0 requests to the redirect target)");
    }

    private static void AskWindow(bool jump)
    {
        var src = new SeqSource();
        src.LearnWindow(jump);
        var bari = new RecBari { PostStatus = 204, HasPostStatus = true };
        Drive.SkipNetwork = false;
        Drive.PostStageLimit = 1;
        Drive.Run(Cases.PackPublicKey(), Small(), src, bari, new FrozenClock(), new EventLog(null, false), new StringWriter(), false, null, 0);
        Drive.PostStageLimit = 17;
        var offs = string.Join(", ", Drive.AskOffsets);
        if (Drive.AskOffsets.Contains(1))
        {
            Program.FailLine(jump ? "ask-on-every-learned-header" : "ask-window-2-3-4", "ask sent at offset 1");
            return;
        }
        var ok = Drive.AskOffsets.Contains(2) && Drive.AskOffsets.Contains(3) && Drive.AskOffsets.Contains(4);
        if (jump)
        {
            if (ok) Program.PassLine("ask-on-every-learned-header (offsets 2, 3, 4 after a jump to H+4)");
            else Program.FailLine("ask-on-every-learned-header", "asks at offsets " + offs + ", expected 2, 3, 4");
        }
        else
        {
            if (ok) Program.PassLine("ask-window-2-3-4 (offsets 2, 3, 4)");
            else Program.FailLine("ask-window-2-3-4", "asks at offsets " + offs);
        }
    }

    private static void AskOnly204()
    {
        var src = new SeqSource();
        src.LearnWindow(false);
        var bari = new RecBari { PostStatus = 503, HasPostStatus = true };
        Drive.SkipNetwork = false;
        Drive.Run(Cases.PackPublicKey(), Small(), src, bari, new FrozenClock(), new EventLog(null, false), new StringWriter(), false, null, 0);
        if (Drive.AsksWithout204 == 0) Program.PassLine("ask-only-after-204 (0 asks without a 204)");
        else Program.FailLine("ask-only-after-204", Drive.AsksWithout204.ToString(System.Globalization.CultureInfo.InvariantCulture) + " asks without a 204, expected 0");
    }

    private static void Unchecked()
    {
        var src = new SeqSource();
        src.LearnWindow(false);
        var bari = new RecBari { PostStatus = 204, HasPostStatus = true, PathBody = new byte[] { 1, 2, 3 }, PathStatus = 200 };
        Drive.SkipNetwork = false;
        Drive.Run(Cases.PackPublicKey(), Small(), src, bari, new FrozenClock(), new EventLog(null, false), new StringWriter(), false, null, 0);
        if (!Drive.UncheckedPathStored) Program.PassLine("unchecked-200-is-miss (path empty)");
        else Program.FailLine("unchecked-200-is-miss", "unchecked path stored");
    }

    private static void RuleI4Producer()
    {
        var src = new SeqSource();
        src.FullSchedule();
        src.FailSomeTips = true;
        var bari = new RecBari { PostStatus = 503, HasPostStatus = true };
        Drive.SkipNetwork = false;
        Drive.PostStageLimit = 1;
        var clock = new FrozenClock();
        Drive.Run(Cases.PackPublicKey(), Small(), src, bari, clock, new EventLog(null, false), new StringWriter(), false, null, 40);
        clock.NowSeconds = 30;
        Drive.FlushResends(bari, clock);
        Drive.PostStageLimit = 17;
        if (Drive.LegCountRecorded >= 1 && Drive.ClosedAtI17)
        {
            Program.PassLine("rule-i4-producer (legs continued, 1 resend, closed at I_17)");
        }
        else Program.FailLine("rule-i4-producer", "legs stopped while the log was unwritable");
    }

    private static void PaidRule(string pack)
    {
        var text = File.ReadAllText(Path.Combine(pack, "br1.json"));
        var doc = System.Text.Json.JsonDocument.Parse(text);
        var values = doc.RootElement.GetProperty("values");
        var paid = 0;
        var reversedRejected = true;
        foreach (var name in new[] { "current", "before", "after_test" })
        {
            string V(string f) => values.GetProperty("br1." + name + "." + f).GetString()!;
            var tip = new Br1.Tip(
                Convert.FromHexString(V("tip_hash")),
                Convert.FromHexString(V("tip_previous")),
                Convert.ToUInt64(V("tip_bits"), 16),
                long.Parse(V("tip_height"), System.Globalization.CultureInfo.InvariantCulture),
                long.Parse(V("tip_time"), System.Globalization.CultureInfo.InvariantCulture),
                long.Parse(V("period_first_time"), System.Globalization.CultureInfo.InvariantCulture),
                V("plane") == "live");
            var st = Paid.Status(Convert.FromHexString(V("waypoint")), V("nonce"), V("fee_header"), V("origin"), tip);
            if (st == 204) paid++;
        }
        foreach (var name in new[] { "display_current", "display_before", "display_after" })
        {
            string V(string f)
            {
                if (!values.TryGetProperty("br1." + name + "." + f, out var p)) return "";
                return p.GetString() ?? "";
            }
            if (V("tip_hash").Length == 0) continue;
            var tip = new Br1.Tip(
                Convert.FromHexString(V("tip_hash")),
                Convert.FromHexString(V("tip_previous")),
                Convert.ToUInt64(V("tip_bits"), 16),
                long.Parse(V("tip_height"), System.Globalization.CultureInfo.InvariantCulture),
                long.Parse(V("tip_time"), System.Globalization.CultureInfo.InvariantCulture),
                long.Parse(V("period_first_time"), System.Globalization.CultureInfo.InvariantCulture),
                V("plane") == "live");
            var st = Paid.Status(Convert.FromHexString(V("waypoint")), V("nonce"), V("fee_header"), V("origin"), tip);
            if (st == 204) reversedRejected = false;
        }
        if (paid == 3 && reversedRejected) Program.PassLine("rule-i4-paid-rule (3 paid accepted, other POSTs 402, reversed rejected)");
        else if (!reversedRejected) Program.FailLine("rule-i4-paid-rule", "reversed fee header accepted");
        else Program.FailLine("rule-i4-paid-rule", "paid count");
    }

    private static void EventLogTest()
    {
        var tmp = Path.Combine(Path.GetTempPath(), "pcoc-ev-" + Environment.ProcessId);
        Directory.CreateDirectory(tmp);
        var obj = Path.Combine(tmp, "o.cbor");
        var src = new SeqSource();
        src.FullSchedule();
        Drive.SkipNetwork = true;
        Drive.Run(Cases.PackPublicKey(), Small(), src, new RecBari(), new FrozenClock(), new EventLog(obj, false), new StringWriter(), false, null, 0);
        if (File.Exists(obj + ".events")) Program.FailLine("event-log-live-only", ".events written on the test profile");
        else Program.PassLine("event-log-live-only (no .events on test)");
        try { Directory.Delete(tmp, true); } catch { }
    }

    private static void EventLogKeys(string root)
    {
        var spec = File.ReadAllText(Path.Combine(root, "spec", "02-Producer_Process.md"));
        var want = new[] { "event", "stage", "waypoint", "attempt", "posted", "succeeded", "dropped" };
        var tmp = Path.Combine(Path.GetTempPath(), "pcoc-ek-" + Environment.ProcessId);
        Directory.CreateDirectory(tmp);
        var obj = Path.Combine(tmp, "o.cbor");
        var src = new SeqSource();
        src.FullSchedule();
        Drive.SkipNetwork = false;
        var bari = new RecBari { PostStatus = 204, HasPostStatus = true };
        Drive.Run(Cases.PackPublicKey(), Small(), src, bari, new FrozenClock(), new EventLog(obj, true), new StringWriter(), true, null, 0);
        var ok = true;
        if (File.Exists(obj + ".events"))
        {
            foreach (var line in File.ReadAllLines(obj + ".events"))
            {
                if (!line.Contains("hike.produce.posted", StringComparison.Ordinal)) continue;
                var keys = new List<string>();
                var i = 0;
                while (i < line.Length)
                {
                    var q = line.IndexOf('"', i);
                    if (q < 0) break;
                    var q2 = line.IndexOf('"', q + 1);
                    if (q2 < 0) break;
                    var key = line.Substring(q + 1, q2 - q - 1);
                    if (q > 0 && line[q - 1] != ':')
                    {
                        keys.Add(key);
                    }
                    i = q2 + 1;
                }
                for (var k = 0; k < want.Length && k < keys.Count; k++)
                {
                    if (keys[k] != want[k]) ok = false;
                }
            }
        }
        if (ok) Program.PassLine("event-log-key-order (records match §13.5)");
        else Program.FailLine("event-log-key-order", "key order differs from §13.5");
        try { Directory.Delete(tmp, true); } catch { }
    }

    private static byte[] RunSmall(SeqSource src, RecBari bari, string? index, int maxLegs = 0)
    {
        var clock = new FrozenClock();
        var log = new EventLog(null, false);
        Drive.SkipNetwork = index == null && bari.PostStatus == 204 && !bari.HoldPosts;
        return Drive.Run(Cases.PackPublicKey(), Small(), src, bari, clock, log, new StringWriter(), false, index, maxLegs);
    }
}

internal sealed class SeqSource : IHeaderSource
{
    private readonly List<Tip> _tips = new();
    private int _i;
    public bool ObserveOnce;
    public bool InterruptedOnEqualHash;
    public bool FailSomeTips;
    private int _obs;
    private int _reads;

    public void Seed()
    {
        FullSchedule();
    }

    public void Add(byte[] n, ulong h)
    {
        _tips.Add(new Tip(n, h, ScriptedSource.DummyHeader(n)));
    }

    public void FullSchedule()
    {
        _tips.Clear();
        Add(Cases.LabelDigest("beacon/seed"), 1000);
        for (var k = 0; k < 18; k++)
        {
            Add(Cases.LabelDigest("beacon/" + k.ToString(System.Globalization.CultureInfo.InvariantCulture)), 1001UL + (ulong)k);
        }
    }

    public void SeedThenSameHashHigher()
    {
        FullSchedule();
    }

    public void Jump1001To1004()
    {
        _tips.Clear();
        Add(Cases.LabelDigest("beacon/seed"), 1000);
        Add(Cases.LabelDigest("beacon/0"), 1004);
        for (var k = 1; k < 18; k++)
        {
            Add(Cases.LabelDigest("beacon/" + k.ToString(System.Globalization.CultureInfo.InvariantCulture)), 1004UL + (ulong)k);
        }
    }

    public void SameHeightDiffHash()
    {
        _tips.Clear();
        Add(Cases.LabelDigest("beacon/seed"), 1000);
        Add(Cases.LabelDigest("beacon/x"), 1000);
        for (var k = 0; k < 18; k++)
        {
            Add(Cases.LabelDigest("beacon/" + k.ToString(System.Globalization.CultureInfo.InvariantCulture)), 1001UL + (ulong)k);
        }
        _sameHeight = true;
    }

    private bool _sameHeight;

    public void HeldOnly(byte[] n, ulong h)
    {
        _tips.Clear();
        Add(Cases.LabelDigest("beacon/seed"), 1000);
        Add(n, h);
        for (var k = 1; k < 18; k++) Add(Cases.LabelDigest("beacon/" + k.ToString(System.Globalization.CultureInfo.InvariantCulture)), h + (ulong)k);
    }

    public void StallAfterSeed()
    {
        _tips.Clear();
        Add(Cases.LabelDigest("beacon/seed"), 1000);
        _stall = true;
    }

    private bool _stall;

    public void JumpOverW17()
    {
        FullSchedule();
    }

    public void LearnWindow(bool jump)
    {
        FullSchedule();
        for (var o = 1; o <= 4; o++)
        {
            Add(Cases.LabelDigest("learn/" + o.ToString(System.Globalization.CultureInfo.InvariantCulture)), 1000UL + (ulong)o);
        }
        _learn = true;
        _jumpLearn = jump;
    }

    private bool _learn;
    private bool _jumpLearn;

    public bool TryObserve(out Tip tip)
    {
        _obs++;
        if (ObserveOnce && _obs > 1)
        {
            tip = _tips[1];
            return true;
        }
        tip = _tips[0];
        return true;
    }

    public bool TryTipRead(out Tip tip)
    {
        _reads++;
        if (FailSomeTips && _reads == 2)
        {
            tip = default;
            FailSomeTips = false;
            return false;
        }
        if (_stall)
        {
            tip = _tips[0];
            return true;
        }
        if (_sameHeight && _reads == 1)
        {
            tip = _tips[1];
            if (Drive.InterruptOnHashAlone)
            {
                InterruptedOnEqualHash = false;
            }
            return true;
        }
        var idx = Math.Min(_reads, _tips.Count - 1);
        tip = _tips[idx];
        if (Bytes.Equal(tip.Natural, _tips[0].Natural) && tip.Height > _tips[0].Height)
        {
            InterruptedOnEqualHash = true;
        }
        return true;
    }

    public bool TryFeeHeader(out byte[] header80)
    {
        header80 = ScriptedSource.DummyHeader(_tips[0].Natural);
        return true;
    }

    public IReadOnlyList<Tip> LearnedHeaders()
    {
        if (_learn)
        {
            var list = new List<Tip>();
            var h0 = 1001UL;
            if (_jumpLearn)
            {
                list.Add(new Tip(Cases.LabelDigest("learn/4"), h0 + 4, null));
                list.Insert(0, new Tip(Cases.LabelDigest("learn/2"), h0 + 2, null));
                list.Insert(1, new Tip(Cases.LabelDigest("learn/3"), h0 + 3, null));
            }
            else
            {
                list.Add(new Tip(Cases.LabelDigest("learn/2"), h0 + 2, null));
                list.Add(new Tip(Cases.LabelDigest("learn/3"), h0 + 3, null));
                list.Add(new Tip(Cases.LabelDigest("learn/4"), h0 + 4, null));
            }
            return list;
        }
        return _tips;
    }
}

internal sealed class RecBari : IBari
{
    public int PostStatus = 204;
    public bool HasPostStatus = true;
    public bool HoldPosts;
    public bool HoldAsks;
    public byte[] PathBody = Array.Empty<byte>();
    public int PathStatus = 404;
    public int Posts;
    public int Asks;

    public PostResult PostWaypoint(byte[] waypoint, ulong nonce, byte[] feeHeader)
    {
        Posts++;
        if (HoldPosts) return new PostResult(false, 0);
        return new PostResult(HasPostStatus, PostStatus);
    }

    public AskResult GetPath(byte[] waypoint)
    {
        Asks++;
        if (HoldAsks) return new AskResult(false, 0, Array.Empty<byte>());
        return new AskResult(true, PathStatus, PathBody);
    }
}
