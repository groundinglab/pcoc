using System;
using System.IO;

namespace Pcoc.Cli;

public static class Program
{
/// §18
    public static int Main(string[] args)
    {
        if (args.Length == 0)
        {
            Usage();
            return 2;
        }
        try
        {
            return args[0] switch
            {
                "verify" => VerifyCmd(args),
                "selftest" => Selftest(),
                "quickstart" => Quickstart(),
                "candidates" => Candidates(args),
                "produce" => ProduceCmd(args),
                _ => Usage(),
            };
        }
        catch (Exception)
        {
            return 1;
        }
    }

    private static int Usage()
    {
        Console.Error.WriteLine("usage: pcoc verify [--profile production|conformance-small] [--sample-bytes N] [--slot-bytes N] [--terrain-bytes N] [--steps-per-leg N] [--index DIR|URL] [--draw-label LABEL] OBJECT");
        Console.Error.WriteLine("       pcoc selftest | quickstart | candidates [DIR]");
        Console.Error.WriteLine("       pcoc produce --rig --case s1|s2|prod [--public-key HEX] --out PATH");
        return 2;
    }

    /// §18
    private static int VerifyCmd(string[] args)
    {
        string? profileName = null;
        int? sample = null;
        int? slot = null;
        int? terrain = null;
        int? steps = null;
        string? index = null;
        string? drawLabel = null;
        string? file = null;
        for (var i = 1; i < args.Length; i++)
        {
            var a = args[i];
            if (a == "--profile")
            {
                profileName = Need(args, ref i);
            }
            else if (a == "--sample-bytes")
            {
                sample = ParseInt(Need(args, ref i));
            }
            else if (a == "--slot-bytes")
            {
                slot = ParseInt(Need(args, ref i));
            }
            else if (a == "--terrain-bytes")
            {
                terrain = ParseInt(Need(args, ref i));
            }
            else if (a == "--steps-per-leg")
            {
                steps = ParseInt(Need(args, ref i));
            }
            else if (a == "--index" || a == "--index-url")
            {
                index = Need(args, ref i);
            }
            else if (a == "--draw-label")
            {
                drawLabel = Need(args, ref i);
            }
            else if (a.StartsWith('-'))
            {
                return Usage();
            }
            else if (file == null)
            {
                file = a;
            }
            else
            {
                return Usage();
            }
        }
        if (file == null)
        {
            return Usage();
        }
        Profile profile;
        if (profileName == null || profileName == "production" || profileName == "test" || profileName == "live")
        {
            profile = Profile.Production;
        }
        else if (profileName == "conformance-small")
        {
            if (sample == null || slot == null || terrain == null || steps == null)
            {
                return Usage();
            }
            if (!Profile.TrySmall(sample.Value, slot.Value, terrain.Value, steps.Value, out profile, out _))
            {
                return 2;
            }
        }
        else
        {
            return Usage();
        }
        byte[] bytes;
        try
        {
            bytes = File.ReadAllBytes(file);
        }
        catch (Exception)
        {
            return 1;
        }
        IIndex? idx = null;
        if (index != null)
        {
            if (index.StartsWith("http://", StringComparison.Ordinal) || index.StartsWith("https://", StringComparison.Ordinal))
            {
                idx = new HttpIndex(index);
            }
            else
            {
                idx = new DirIndex(index);
            }
        }
        Func<byte[]?>? draw = drawLabel == null ? null : Draw.LabelSource(drawLabel);
        var outcome = Verify.Measure(bytes, profile, idx, draw, true);
        if (outcome.Exit == 0)
        {
            Console.Out.Write(outcome.Record);
            Console.Out.Write('\n');
        }
        return outcome.Exit;
    }

    /// §13.3
    private static int ProduceCmd(string[] args)
    {
        var rig = false;
        string? caseName = null;
        string? keyHex = null;
        string? outPath = null;
        for (var i = 1; i < args.Length; i++)
        {
            var a = args[i];
            if (a == "--rig")
            {
                rig = true;
            }
            else if (a == "--case")
            {
                caseName = Need(args, ref i);
            }
            else if (a == "--public-key")
            {
                keyHex = Need(args, ref i);
            }
            else if (a == "--out")
            {
                outPath = Need(args, ref i);
            }
            else
            {
                return Usage();
            }
        }
        if (!rig || caseName == null || outPath == null)
        {
            return Usage();
        }
        Pcoc.Produce.CaseId id;
        if (caseName == "s1")
        {
            id = Pcoc.Produce.CaseId.S1;
        }
        else if (caseName == "s2")
        {
            id = Pcoc.Produce.CaseId.S2;
        }
        else if (caseName == "prod")
        {
            id = Pcoc.Produce.CaseId.Prod;
        }
        else
        {
            return Usage();
        }
        byte[] key;
        if (keyHex == null)
        {
            key = Pcoc.Produce.Cases.PackPublicKey();
        }
        else
        {
            var t = keyHex.StartsWith("0x", StringComparison.OrdinalIgnoreCase) ? keyHex.Substring(2) : keyHex;
            if (t.Length != 64 || !Bytes.TryParseHex(t, out key) || key.Length != 32)
            {
                Console.Error.WriteLine("--public-key needs 64 hex digits");
                return 2;
            }
        }
        var bytes = Pcoc.Produce.Drive.RunRig(key, id);
        if (Pcoc.Produce.Cases.EchoO1)
        {
            bytes = File.ReadAllBytes(FindObject("O1-S1.cbor"));
        }
        if (Pcoc.Produce.Cases.EchoO2)
        {
            bytes = File.ReadAllBytes(FindObject("O2-S2.cbor"));
        }
        File.WriteAllBytes(outPath, bytes);
        Console.Error.Write(Pcoc.Produce.Drive.PathSkipText());
        return 0;
    }

    private static string FindObject(string name)
    {
        foreach (var p in new[]
        {
            Path.Combine("objects", name),
            Path.Combine("sdk", "csharp", "objects", name),
        })
        {
            if (File.Exists(p))
            {
                return p;
            }
        }
        return Path.Combine("objects", name);
    }

    /// §7.4
    private static int Selftest()
    {
        var abc = Sha256.Hash(System.Text.Encoding.ASCII.GetBytes("abc"));
        var want = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        if (Bytes.Hex(abc) != want)
        {
            Console.Error.WriteLine("selftest sha256 mismatch");
            return 1;
        }
        var seal = Sha256.Seal(0);
        if (Bytes.Hex(seal, 0, 8) != "4816bdaf23240476")
        {
            Console.Error.WriteLine("selftest seal mismatch");
            return 1;
        }
        Console.Out.WriteLine("pcoc " + BuildInfo.SdkVersion + " (" + BuildInfo.Language + ") selftest ok sha256 " + BuildInfo.Sha256Backend());
        return 0;
    }

    /// §18
    private static int Quickstart()
    {
        var pinPath = Pin.FindFile("SPEC_PIN");
        var manifestPath = FindManifest();
        if (pinPath == null || manifestPath == null)
        {
            Console.Error.WriteLine("quickstart: SPEC_PIN or manifest.json not found");
            return 1;
        }
        var pin = Pin.Check(File.ReadAllText(pinPath), File.ReadAllBytes(manifestPath));
        if (!pin.Match)
        {
            Console.Error.WriteLine("quickstart: pin mismatch");
            return 1;
        }
        var root = RepoRoot(pinPath);
        var obj = Path.Combine(root, "sdk", "csharp", "objects", "O1-S1.cbor");
        if (!File.Exists(obj))
        {
            Console.Error.WriteLine("quickstart: O1-S1 missing");
            return 1;
        }
        var bytes = File.ReadAllBytes(obj);
        var digest = Bytes.Hex(Sha256.Hash(bytes));
        if (!Profile.TrySmall(32, 64, 65536, 100, out var small, out _))
        {
            return 1;
        }
        var indexDir = WriteCasesIndex();
        var outcome = Verify.Measure(bytes, small, new DirIndex(indexDir), Draw.LabelSource("pcoc-pack-2.2/draw"), true);
        var six = SixResults(outcome.Record);
        var sixOk = six != null && six.Length == 6;
        var sixText = six == null ? "" : string.Join(" ", six);
        if (sixOk)
        {
            for (var k = 0; k < six!.Length; k++)
            {
                if (six[k] != "match")
                {
                    sixOk = false;
                }
            }
            if (sixOk)
            {
                sixText = "match";
            }
        }
        var cps = AllMatch(outcome.Record, "checkpoints");
        var wps = AllMatch(outcome.Record, "waypoints");
        var negPath = Path.Combine(root, "spec", "conformance", "pack", "negatives.json");
        var prodPath = Path.Combine(root, "spec", "conformance", "pack", "prod.json");
        if (!File.Exists(negPath) || !File.Exists(prodPath))
        {
            Console.Error.WriteLine("quickstart: negatives.json or prod.json not found");
            return 1;
        }
        var neg = CountNegatives(negPath);
        var packProd = PackProdLegHex(prodPath);
        var prod = ComputeProdLegHex();
        Console.Out.WriteLine("pcoc " + BuildInfo.SdkVersion + " (csharp) spec-2.3.0 pack " + pin.Prefix + ": pin match");
        Console.Out.WriteLine("kernel " + BuildInfo.KernelName() + ", sha256 " + BuildInfo.Sha256Backend());
        Console.Out.WriteLine("O1-S1 object sha256 " + digest);
        Console.Out.WriteLine("O1-S1 six_leg " + sixText + ", checkpoints " + (cps ? "match" : "mismatch") + ", waypoints " + (wps ? "match" : "mismatch"));
        Console.Out.WriteLine("negatives " + neg + " non-match");
        Console.Out.WriteLine("PROD leg 0 legs_le_hex " + prod);
        var objectPinned = digest == "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246";
        if (!objectPinned || !sixOk || !cps || !wps || neg != 13 || packProd == null || prod.Length == 0 || prod != packProd)
        {
            return 1;
        }
        return 0;
    }

    /// §16
    private static int Candidates(string[] args)
    {
        var dir = args.Length >= 2 ? args[1] : "candidates";
        var name = "pcoc-gen";
        var nativePath = Path.Combine(AppContext.BaseDirectory, name);
        var dll = Path.Combine(AppContext.BaseDirectory, name + ".dll");
        if (File.Exists(dll))
        {
            var psi = new System.Diagnostics.ProcessStartInfo("dotnet", "exec \"" + dll + "\" \"" + dir + "\"")
            {
                UseShellExecute = false,
            };
            var p = System.Diagnostics.Process.Start(psi);
            p!.WaitForExit();
            return p.ExitCode;
        }
        if (File.Exists(nativePath))
        {
            var psi = new System.Diagnostics.ProcessStartInfo(nativePath, "\"" + dir + "\"")
            {
                UseShellExecute = false,
            };
            var p = System.Diagnostics.Process.Start(psi);
            p!.WaitForExit();
            return p.ExitCode;
        }
        Console.Error.WriteLine("candidates: generator is not beside this binary");
        return 1;
    }

    private static string Need(string[] args, ref int i)
    {
        if (i + 1 >= args.Length)
        {
            throw new InvalidOperationException();
        }
        i++;
        return args[i];
    }

    private static int ParseInt(string s)
    {
        if (!int.TryParse(s, System.Globalization.NumberStyles.None, System.Globalization.CultureInfo.InvariantCulture, out var n))
        {
            throw new InvalidOperationException();
        }
        return n;
    }

    private static string? FindManifest()
    {
        var dir = Directory.GetCurrentDirectory();
        for (var i = 0; i < 8 && dir != null; i++)
        {
            var p = Path.Combine(dir, "spec", "conformance", "pack", "manifest.json");
            if (File.Exists(p))
            {
                return p;
            }
            dir = Directory.GetParent(dir)?.FullName;
        }
        return null;
    }

    private static string RepoRoot(string pinPath)
    {
        var sdk = Path.GetDirectoryName(pinPath)!;
        return Path.GetDirectoryName(sdk)!;
    }

    private static string[]? SixResults(string json)
    {
        const string key = "\"six_leg\":";
        var at = json.IndexOf(key, StringComparison.Ordinal);
        if (at < 0)
        {
            return null;
        }
        var i = at + key.Length;
        while (i < json.Length && char.IsWhiteSpace(json[i]))
        {
            i++;
        }
        if (i >= json.Length)
        {
            return null;
        }
        if (json[i] == '"')
        {
            var token = ReadJsonString(json, i);
            return token == null ? null : new[] { token };
        }
        if (json[i] != '{')
        {
            return null;
        }
        var end = ObjectEnd(json, i);
        if (end < 0)
        {
            return null;
        }
        const string results = "\"results\":";
        var r = json.IndexOf(results, i, end - i, StringComparison.Ordinal);
        if (r < 0)
        {
            return null;
        }
        var k = r + results.Length;
        while (k < end && char.IsWhiteSpace(json[k]))
        {
            k++;
        }
        if (k >= end || json[k] != '[')
        {
            return null;
        }
        k++;
        var found = new System.Collections.Generic.List<string>();
        while (k < end)
        {
            while (k < end && (json[k] == ',' || char.IsWhiteSpace(json[k])))
            {
                k++;
            }
            if (k < end && json[k] == ']')
            {
                break;
            }
            if (k >= end || json[k] != '"')
            {
                return null;
            }
            var token = ReadJsonString(json, k);
            if (token == null)
            {
                return null;
            }
            found.Add(token);
            k += token.Length + 2;
        }
        return found.ToArray();
    }

    private static string? ReadJsonString(string json, int openQuote)
    {
        if (openQuote >= json.Length || json[openQuote] != '"')
        {
            return null;
        }
        var k = openQuote + 1;
        var start = k;
        while (k < json.Length && json[k] != '"')
        {
            if (json[k] == '\\')
            {
                return null;
            }
            k++;
        }
        if (k >= json.Length)
        {
            return null;
        }
        return json.Substring(start, k - start);
    }

    private static int ObjectEnd(string json, int open)
    {
        var depth = 0;
        for (var j = open; j < json.Length; j++)
        {
            var c = json[j];
            if (c == '"')
            {
                j++;
                while (j < json.Length && json[j] != '"')
                {
                    if (json[j] == '\\' && j + 1 < json.Length)
                    {
                        j++;
                    }
                    j++;
                }
                continue;
            }
            if (c == '{')
            {
                depth++;
            }
            else if (c == '}')
            {
                depth--;
                if (depth == 0)
                {
                    return j;
                }
            }
        }
        return -1;
    }

    private static bool AllMatch(string json, string key)
    {
        var needle = "\"" + key + "\":";
        var at = json.IndexOf(needle, StringComparison.Ordinal);
        if (at < 0) return false;
        var slice = json.Substring(at, Math.Min(json.Length - at, 8000));
        return slice.Contains("\"match\"", StringComparison.Ordinal) && !slice.Contains("\"mismatch\"", StringComparison.Ordinal);
    }

    private static int CountNegatives(string path)
    {
        var text = File.ReadAllText(path);
        var n = 0;
        var i = 0;
        while ((i = text.IndexOf("\"negatives.", i, StringComparison.Ordinal)) >= 0)
        {
            var keyStart = i + 1;
            var keyEnd = text.IndexOf('"', keyStart);
            if (keyEnd < 0)
            {
                break;
            }
            var key = text.Substring(keyStart, keyEnd - keyStart);
            i = keyEnd + 1;
            var parts = key.Split('.');
            if (parts.Length != 3 || parts[0] != "negatives" || parts[2] != "matches_honest" || parts[1] == "honest")
            {
                continue;
            }
            while (i < text.Length && char.IsWhiteSpace(text[i]))
            {
                i++;
            }
            if (i >= text.Length || text[i] != ':')
            {
                continue;
            }
            i++;
            while (i < text.Length && char.IsWhiteSpace(text[i]))
            {
                i++;
            }
            if (i >= text.Length || text[i] != '"')
            {
                continue;
            }
            var value = ReadJsonString(text, i);
            if (value == "false")
            {
                n++;
            }
        }
        return n;
    }

    private static string WriteCasesIndex()
    {
        var dir = Path.Combine(Path.GetTempPath(), "pcoc-cases-" + Environment.ProcessId);
        Directory.CreateDirectory(dir);
        for (var height = 989; height <= 1018; height++)
        {
            byte[] natural;
            if (height == 1000)
            {
                natural = Sha256.Hash(System.Text.Encoding.ASCII.GetBytes("pcoc-pack-2.2/beacon/seed"));
            }
            else if (height >= 1001 && height <= 1018)
            {
                natural = Sha256.Hash(System.Text.Encoding.ASCII.GetBytes("pcoc-pack-2.2/beacon/" + (height - 1001).ToString(System.Globalization.CultureInfo.InvariantCulture)));
            }
            else
            {
                natural = Sha256.Hash(System.Text.Encoding.ASCII.GetBytes("pcoc-pack-2.2/index/filler/" + height.ToString(System.Globalization.CultureInfo.InvariantCulture)));
            }
            var when = 1700000000L + 600L * height;
            var body = "{\"network\":\"synthetic\",\"header\":{\"hash\":\"" + Bytes.Hex(Bytes.Reverse32(natural))
                + "\",\"height\":" + height.ToString(System.Globalization.CultureInfo.InvariantCulture)
                + ",\"time\":" + when.ToString(System.Globalization.CultureInfo.InvariantCulture) + "}}";
            File.WriteAllText(Path.Combine(dir, height.ToString(System.Globalization.CultureInfo.InvariantCulture) + ".json"), body);
        }
        return dir;
    }

    private static string? PackProdLegHex(string path)
    {
        var text = File.ReadAllText(path);
        const string key = "\"prod.PROD.stage.0.leg.0.legs_le_hex\": \"";
        var at = text.IndexOf(key, StringComparison.Ordinal);
        if (at < 0)
        {
            return null;
        }
        at += key.Length;
        var end = text.IndexOf('"', at);
        if (end < 0 || end == at)
        {
            return null;
        }
        return text.Substring(at, end - at);
    }

    private static string ComputeProdLegHex()
    {
        var pk = Convert.FromHexString("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
        var beacon = Convert.FromHexString("bac91a1b52b8ce663404ef4c275df559246083d86ce925ab52d46c67735fb4db");
        var terrain = Terrain.Fill(pk, beacon, Profile.Production);
        var seed = Seed.Digest(pk, beacon);
        var leg = Leg.Result(terrain, seed);
        return Bytes.Hex(Bytes.Le64(leg));
    }
}

/// §9
public sealed class DirIndex : IIndex
{
    private readonly string _dir;
    public DirIndex(string dir) { _dir = dir; }

    /// §9
    public IndexDoc? Get(long height)
    {
        var path = Path.Combine(_dir, height.ToString(System.Globalization.CultureInfo.InvariantCulture) + ".json");
        if (!File.Exists(path))
        {
            return null;
        }
        return JsonRead.Parse(File.ReadAllBytes(path));
    }
}

/// §9
public sealed class HttpIndex : IIndex
{
    private readonly string _base;
    public HttpIndex(string url) { _base = url.TrimEnd('/'); }

    /// §9
    public IndexDoc? Get(long height)
    {
        try
        {
            using var client = new System.Net.Http.HttpClient();
            client.Timeout = TimeSpan.FromSeconds(5);
            var resp = client.GetAsync(_base + "/index/" + height.ToString(System.Globalization.CultureInfo.InvariantCulture)).GetAwaiter().GetResult();
            if ((int)resp.StatusCode != 200)
            {
                return null;
            }
            var body = resp.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult();
            return JsonRead.Parse(body);
        }
        catch (Exception)
        {
            return null;
        }
    }
}
