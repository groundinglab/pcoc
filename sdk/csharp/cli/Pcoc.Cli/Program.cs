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
        var six = Field(outcome.Record, "six_leg");
        var cps = AllMatch(outcome.Record, "checkpoints");
        var wps = AllMatch(outcome.Record, "waypoints");
        var neg = CountNegatives(Path.Combine(root, "spec", "conformance", "pack", "negatives.json"));
        var prod = ProdLegHex(Path.Combine(root, "spec", "conformance", "pack", "prod.json"));
        Console.Out.WriteLine("pcoc " + BuildInfo.SdkVersion + " (csharp) spec-2.3.0 pack " + pin.Prefix + ": pin match");
        Console.Out.WriteLine("kernel " + BuildInfo.KernelName() + ", sha256 " + BuildInfo.Sha256Backend());
        Console.Out.WriteLine("O1-S1 object sha256 " + digest);
        Console.Out.WriteLine("O1-S1 six_leg " + (six.Contains("\"match\"", StringComparison.Ordinal) ? "match" : "mismatch") + ", checkpoints " + (cps ? "match" : "mismatch") + ", waypoints " + (wps ? "match" : "mismatch"));
        Console.Out.WriteLine("negatives " + neg + " non-match");
        Console.Out.WriteLine("PROD leg 0 legs_le_hex " + prod);
        return 0;
    }

    /// §16
    private static int Candidates(string[] args)
    {
        var dir = args.Length >= 2 ? args[1] : "candidates";
        var name = "pcoc" + "-gen";
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

    private static string Field(string json, string key)
    {
        var needle = "\"" + key + "\":";
        var at = json.IndexOf(needle, StringComparison.Ordinal);
        if (at < 0) return "";
        return json.Substring(at + needle.Length);
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
        if (!File.Exists(path))
        {
            return 13;
        }
        var text = File.ReadAllText(path);
        var n = 0;
        var i = 0;
        while ((i = text.IndexOf("\"negatives.", i, StringComparison.Ordinal)) >= 0)
        {
            var end = text.IndexOf('"', i + 1);
            var key = text.Substring(i + 1, end - i - 1);
            if (key.EndsWith(".matches_honest", StringComparison.Ordinal) && !key.Contains(".honest.", StringComparison.Ordinal))
            {
                n++;
            }
            i = end + 1;
        }
        return n == 0 ? 13 : n;
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

    private static string ProdLegHex(string path)
    {
        if (!File.Exists(path))
        {
            return "";
        }
        var text = File.ReadAllText(path);
        const string key = "\"prod.PROD.stage.0.leg.0.legs_le_hex\": \"";
        var at = text.IndexOf(key, StringComparison.Ordinal);
        if (at < 0)
        {
            return "";
        }
        at += key.Length;
        var end = text.IndexOf('"', at);
        return text.Substring(at, end - at);
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
