using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text.Json;
using Pcoc;

namespace Pcoc.Check;

public static class Program
{
    private static int _failed;

    /// §16
    public static int Main(string[] args)
    {
        string? through = null;
        if (args.Length == 2 && args[0] == "--through")
        {
            through = args[1];
        }
        else if (args.Length != 0)
        {
            Console.Error.WriteLine("usage: Pcoc.Check [--through CASE]");
            return 2;
        }
        var root = FindRoot();
        var pack = Path.Combine(root, "spec", "conformance", "pack");
        PinCheck(root, pack);
        if (through == null)
        {
            var work = Path.Combine(Path.GetTempPath(), "pcoc-csharp-cand-" + Environment.ProcessId);
            Directory.CreateDirectory(work);
            try
            {
                Generate(root, work);
                foreach (var name in new[]
                {
                    "anchor.json", "br1.json", "draw.json", "fold.json", "hashcash.json", "kat-small.json",
                    "negatives.json", "nl4.json", "prod.json", "rate.json", "seed.json", "terrain.json",
                    "test-3leg.json", "unknown-kind.json", "wes.json"
                })
                {
                    Compare(pack, work, name);
                }
                Blockhash(Path.Combine(pack, "blockhash-display.json"));
                SlotMutation();
                ShaBackend();
                Excluded();
                QuickstartCases(root);
                NegativesCanary(root);
            }
            finally
            {
                try { Directory.Delete(work, true); } catch (Exception) { }
            }
        }
        else
        {
            Blockhash(Path.Combine(pack, "blockhash-display.json"));
            SlotMutation();
            ShaBackend();
        }
        ProducerCases.Run(root, pack, through);
        if (_failed != 0)
        {
            Console.Error.WriteLine("failed " + _failed);
            return 1;
        }
        return 0;
    }

    internal static void PassLine(string name) => Pass(name);

    internal static void FailLine(string name, string why) => Fail(name, why);

    private static void Pass(string name)
    {
        Console.Out.WriteLine("pass " + name);
    }

    private static void Fail(string name, string why)
    {
        Console.Error.WriteLine("fail " + name + ": " + why);
        _failed++;
    }

    private static void PinCheck(string root, string pack)
    {
        var pin = File.ReadAllText(Path.Combine(root, "sdk", "SPEC_PIN"));
        var manifest = File.ReadAllBytes(Path.Combine(pack, "manifest.json"));
        var result = Pin.Check(pin, manifest);
        if (!result.Match)
        {
            Fail("manifest.json", "pin mismatch");
            return;
        }
        Pass("manifest.json");
        var copyDir = Path.Combine(Path.GetTempPath(), "pcoc-pin-" + Environment.ProcessId);
        Directory.CreateDirectory(copyDir);
        var copy = Path.Combine(copyDir, "manifest.json");
        var mutated = (byte[])manifest.Clone();
        mutated[mutated.Length / 2] ^= 0x01;
        File.WriteAllBytes(copy, mutated);
        var bad = Pin.Check(pin, File.ReadAllBytes(copy));
        try { Directory.Delete(copyDir, true); } catch (Exception) { }
        if (bad.Match)
        {
            Fail("manifest-mutation", "mutated manifest still matched");
            return;
        }
        Pass("manifest-mutation (mutated copy rejected)");
    }

    private static void Generate(string root, string work)
    {
        var genDir = Path.Combine(root, "sdk", "csharp", "gen");
        var projDir = Directory.GetDirectories(genDir)[0];
        var proj = Directory.GetFiles(projDir, "*.csproj")[0];
        var built = FirstDll(Path.Combine(projDir, "bin", "Release", "net10.0"));
        if (built == null)
        {
            var psiBuild = new ProcessStartInfo("dotnet", "build \"" + proj + "\" -c Release --source " + EmptySource() + " --nologo")
            {
                WorkingDirectory = root,
                UseShellExecute = false,
            };
            var builtProc = Process.Start(psiBuild)!;
            builtProc.WaitForExit();
            built = FirstDll(Path.Combine(projDir, "bin", "Release", "net10.0"));
            if (builtProc.ExitCode != 0 || built == null)
            {
                throw new InvalidOperationException("generator build");
            }
        }
        var dll = built;
        var psi = new ProcessStartInfo("dotnet", "exec \"" + dll + "\" \"" + work + "\"")
        {
            WorkingDirectory = root,
            UseShellExecute = false,
        };
        var proc = Process.Start(psi)!;
        proc.WaitForExit();
        if (proc.ExitCode != 0)
        {
            throw new InvalidOperationException("generator exit " + proc.ExitCode);
        }
    }

    private static void Compare(string pack, string work, string jsonName)
    {
        var doc = JsonDocument.Parse(File.ReadAllText(Path.Combine(pack, jsonName)));
        var values = doc.RootElement.GetProperty("values");
        var file = doc.RootElement.GetProperty("candidate").GetString()!;
        var got = LoadTsv(Path.Combine(work, file));
        if (Count(values) != got.Count)
        {
            Fail(jsonName, "row count");
            return;
        }
        foreach (var prop in values.EnumerateObject())
        {
            if (!got.TryGetValue(prop.Name, out var have) || have != prop.Value.GetString())
            {
                Fail(jsonName, prop.Name);
                return;
            }
        }
        Pass(jsonName);
    }

    private static int Count(JsonElement values)
    {
        var n = 0;
        foreach (var _ in values.EnumerateObject()) n++;
        return n;
    }

    private static Dictionary<string, string> LoadTsv(string path)
    {
        var map = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var line in File.ReadAllLines(path))
        {
            var tab = line.IndexOf('\t');
            map[line.Substring(0, tab)] = line.Substring(tab + 1);
        }
        return map;
    }

    private static void Blockhash(string path)
    {
        var doc = JsonDocument.Parse(File.ReadAllText(path));
        var root = doc.RootElement;
        var header = Convert.FromHexString(root.GetProperty("header_hex").GetString()!);
        var natural = Sha256.Hash(Sha256.Hash(header));
        var display = Bytes.Reverse32(natural);
        var ok = Bytes.Hex(natural) == root.GetProperty("natural_block_hash").GetString()
            && Bytes.Hex(display) == root.GetProperty("display_block_hash").GetString();
        if (ok) Pass("blockhash-display.json");
        else Fail("blockhash-display.json", "reversal");
    }

    /// §11.1
    private static void SlotMutation()
    {
        var pk = Convert.FromHexString("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
        var beacon = Convert.FromHexString("bac91a1b52b8ce663404ef4c275df559246083d86ce925ab52d46c67735fb4db");
        if (!Profile.TrySmall(32, 64, 65536, 4, out var profile, out _))
        {
            Fail("slot-mutation", "profile");
            return;
        }
        var terrain = Terrain.Fill(pk, beacon, profile);
        var s = terrain.Lanes[0];
        var honest = Step.Apply(terrain, s);
        int[] offs = { 0, profile.SlotBytes / 2, profile.SlotBytes - 1 };
        var changed = true;
        foreach (var off in offs)
        {
            var at = honest.SlotIndex * profile.SlotBytes + off;
            terrain.Data[at] ^= 0x01;
            var next = Step.Apply(terrain, s);
            if (next.Next == honest.Next && next.Folded == honest.Folded)
            {
                changed = false;
            }
            terrain.Data[at] ^= 0x01;
        }
        if (changed) Pass("slot-mutation");
        else Fail("slot-mutation", "step unchanged");
        if (!Profile.TrySmall(32, 16384, 32768, 1, out var wide, out _))
        {
            Fail("slot-mutation-16384", "profile");
            return;
        }
        var wideTerrain = Terrain.Fill(pk, beacon, wide);
        var wideStep = Step.Apply(wideTerrain, wideTerrain.Lanes[0]);
        int[] wordIndex = { 0, wide.WordsPerSlot / 2, wide.WordsPerSlot - 1 };
        var wordsChanged = true;
        foreach (var w in wordIndex)
        {
            var at = wideStep.SlotIndex * wide.SlotBytes + w * 8;
            wideTerrain.Data[at] ^= 0x01;
            var next = Step.Apply(wideTerrain, wideTerrain.Lanes[0]);
            if (next.Folded == wideStep.Folded)
            {
                wordsChanged = false;
            }
            wideTerrain.Data[at] ^= 0x01;
        }
        if (wordsChanged) Pass("slot-mutation-16384");
        else Fail("slot-mutation-16384", "fold unchanged");
    }

    /// §11.1
    private static void ShaBackend()
    {
        var name = Sha256.BackendName();
        if (name == "sha-ni")
        {
            Fail("sha256-backend", name);
            return;
        }
        var abc = Bytes.Hex(Sha256.Hash(System.Text.Encoding.ASCII.GetBytes("abc")));
        if (abc != "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        {
            Fail("sha256-backend", "abc");
            return;
        }
        Pass("sha256-backend");
    }

    private static void Excluded()
    {
        Console.Out.WriteLine("excluded sign-in.json");
        Console.Out.WriteLine("excluded sign-in-origins.json");
    }

    private static void QuickstartCases(string root)
    {
        var dll = BuildCli(root);
        if (dll == null)
        {
            Fail("quickstart-honest", "build");
            Fail("quickstart-object-tamper", "build");
            Fail("quickstart-tamper", "build");
            return;
        }
        QuickstartHonest(root, dll);
        QuickstartObjectTamper(root, dll);
        QuickstartTamper(root, dll);
    }

    private static string? BuildCli(string root)
    {
        var proj = Path.Combine(root, "sdk", "csharp", "cli", "Pcoc.Cli", "Pcoc.Cli.csproj");
        var build = new ProcessStartInfo("dotnet", "build \"" + proj + "\" -c Release --source " + EmptySource() + " --nologo")
        {
            WorkingDirectory = root,
            UseShellExecute = false,
        };
        var built = Process.Start(build)!;
        built.WaitForExit();
        var dll = Path.Combine(root, "sdk", "csharp", "cli", "Pcoc.Cli", "bin", "Release", "net10.0", "pcoc.dll");
        if (built.ExitCode != 0 || !File.Exists(dll))
        {
            return null;
        }
        return dll;
    }

    private static void QuickstartHonest(string root, string dll)
    {
        var tmp = NewTree("honest");
        try
        {
            CopyPin(root, tmp);
            CopyPack(root, tmp, false);
            File.Copy(ObjectPath(root), Path.Combine(tmp, "sdk", "csharp", "objects", "O1-S1.cbor"), true);
            var run = RunQuickstart(dll, tmp);
            string[] want =
            {
                "pcoc 2.3.2 (csharp) spec-2.3.0 pack 91ded57e: pin match",
                "kernel scalar, sha256 openssl",
                "O1-S1 object sha256 800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246",
                "O1-S1 six_leg match, checkpoints match, waypoints match",
                "negatives 13 non-match",
                "PROD leg 0 legs_le_hex 88a04ac5beacb8a7",
            };
            var got = Lines(run.Stdout);
            if (run.Code != 0 || got.Length != want.Length)
            {
                Fail("quickstart-honest", "exit " + run.Code.ToString(System.Globalization.CultureInfo.InvariantCulture));
                return;
            }
            for (var i = 0; i < want.Length; i++)
            {
                if (got[i] != want[i])
                {
                    Fail("quickstart-honest", "line " + (i + 1).ToString(System.Globalization.CultureInfo.InvariantCulture));
                    return;
                }
            }
            Pass("quickstart-honest");
        }
        finally
        {
            try { Directory.Delete(tmp, true); } catch (Exception) { }
        }
    }

    private static void QuickstartObjectTamper(string root, string dll)
    {
        var tmp = NewTree("object");
        try
        {
            CopyPin(root, tmp);
            CopyPack(root, tmp, false);
            var original = File.ReadAllBytes(ObjectPath(root));
            var offset = DrawnLegPayload(original);
            if (offset == null)
            {
                Fail("quickstart-object-tamper", "locate");
                return;
            }
            var copy = (byte[])original.Clone();
            // Payload byte of an 8-byte leg_results bstr (header 0x48 stays). Decode still reaches six-leg comparison.
            copy[offset.Value] ^= 0x01;
            if (!Cbor.TryDecodeOne(copy, out _) || !Decoded(copy))
            {
                Fail("quickstart-object-tamper", "decode");
                return;
            }
            File.WriteAllBytes(Path.Combine(tmp, "sdk", "csharp", "objects", "O1-S1.cbor"), copy);
            var run = RunQuickstart(dll, tmp);
            var combined = run.Stdout + run.Stderr;
            if (combined.Contains("pin mismatch", StringComparison.Ordinal))
            {
                Fail("quickstart-object-tamper", "pin mismatch");
                return;
            }
            var six = SixLegValue(run.Stdout);
            if (run.Code == 0 || six == null || six == "match" || !six.Contains("mismatch", StringComparison.Ordinal))
            {
                Fail("quickstart-object-tamper", "exit " + run.Code.ToString(System.Globalization.CultureInfo.InvariantCulture));
                return;
            }
            Pass("quickstart-object-tamper (six_leg " + six + ")");
        }
        finally
        {
            try { Directory.Delete(tmp, true); } catch (Exception) { }
        }
    }

    private static void QuickstartTamper(string root, string dll)
    {
        var tmp = NewTree("pin");
        try
        {
            CopyPin(root, tmp);
            CopyPack(root, tmp, true);
            var run = RunQuickstart(dll, tmp);
            if (run.Code != 0
                && run.Stderr.Contains("pin mismatch", StringComparison.Ordinal)
                && !run.Stdout.Contains("six_leg", StringComparison.Ordinal))
            {
                Pass("quickstart-tamper (pin mismatch)");
            }
            else
            {
                Fail("quickstart-tamper", "exit " + run.Code.ToString(System.Globalization.CultureInfo.InvariantCulture));
            }
        }
        finally
        {
            try { Directory.Delete(tmp, true); } catch (Exception) { }
        }
    }

    private static void NegativesCanary(string root)
    {
        var tmp = Path.Combine(Path.GetTempPath(), "pcoc-canary-" + Environment.ProcessId);
        try
        {
            var sdk = Path.Combine(root, "sdk", "csharp");
            CopyTree(Path.Combine(sdk, "src", "Pcoc"), Path.Combine(tmp, "src", "Pcoc"));
            CopyTree(Path.Combine(sdk, "cli", "Pcoc.Cli"), Path.Combine(tmp, "cli", "Pcoc.Cli"));
            Directory.CreateDirectory(Path.Combine(tmp, "check"));
            File.Copy(Path.Combine(sdk, "check", "negatives.sh"), Path.Combine(tmp, "check", "negatives.sh"), true);
            File.Copy(Path.Combine(sdk, "global.json"), Path.Combine(tmp, "global.json"), true);
            File.Copy(Path.Combine(sdk, "NuGet.Config"), Path.Combine(tmp, "NuGet.Config"), true);
            var constPath = Path.Combine(tmp, "src", "Pcoc", "Const.cs");
            var text = File.ReadAllText(constPath);
            var nl = text.Contains("\r\n", StringComparison.Ordinal) ? "\r\n" : "\n";
            var marker = "public static class Const" + nl + "{";
            if (!text.Contains(marker, StringComparison.Ordinal))
            {
                Fail("negatives-canary", "anchor");
                return;
            }
            var planted = marker + nl + "    internal const string CanaryX = \"CachedLeg\";";
            File.WriteAllText(constPath, text.Replace(marker, planted));
            var script = Path.Combine(tmp, "check", "negatives.sh");
            var psi = new ProcessStartInfo("sh", "\"" + script + "\"")
            {
                WorkingDirectory = tmp,
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
            };
            psi.Environment["PCOC_ROOT"] = root;
            var proc = Process.Start(psi)!;
            var errTask = proc.StandardError.ReadToEndAsync();
            var outTask = proc.StandardOutput.ReadToEndAsync();
            proc.WaitForExit();
            var stderr = errTask.GetAwaiter().GetResult();
            outTask.GetAwaiter().GetResult();
            if (proc.ExitCode == 1 && stderr.Contains("cached_leg", StringComparison.Ordinal))
            {
                Pass("negatives-canary (cached_leg)");
            }
            else
            {
                Fail("negatives-canary", "exit " + proc.ExitCode.ToString(System.Globalization.CultureInfo.InvariantCulture));
            }
        }
        finally
        {
            try { Directory.Delete(tmp, true); } catch (Exception) { }
        }
    }

    private readonly struct RunOut
    {
        public RunOut(int code, string stdout, string stderr)
        {
            Code = code;
            Stdout = stdout;
            Stderr = stderr;
        }

        public int Code { get; }
        public string Stdout { get; }
        public string Stderr { get; }
    }

    private static RunOut RunQuickstart(string dll, string cwd)
    {
        var run = new ProcessStartInfo("dotnet", "exec \"" + dll + "\" quickstart")
        {
            WorkingDirectory = cwd,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };
        var proc = Process.Start(run)!;
        var errTask = proc.StandardError.ReadToEndAsync();
        var outTask = proc.StandardOutput.ReadToEndAsync();
        proc.WaitForExit();
        return new RunOut(proc.ExitCode, outTask.GetAwaiter().GetResult(), errTask.GetAwaiter().GetResult());
    }

    private static string NewTree(string tag)
    {
        var tmp = Path.Combine(Path.GetTempPath(), "pcoc-qs-" + tag + "-" + Environment.ProcessId);
        Directory.CreateDirectory(Path.Combine(tmp, "sdk", "csharp", "objects"));
        Directory.CreateDirectory(Path.Combine(tmp, "spec", "conformance", "pack"));
        return tmp;
    }

    private static void CopyPin(string root, string tmp)
    {
        File.Copy(Path.Combine(root, "sdk", "SPEC_PIN"), Path.Combine(tmp, "sdk", "SPEC_PIN"), true);
        File.Copy(Path.Combine(root, "sdk", "csharp", "global.json"), Path.Combine(tmp, "global.json"), true);
    }

    private static void CopyPack(string root, string tmp, bool flipManifest)
    {
        var manifest = File.ReadAllBytes(Path.Combine(root, "spec", "conformance", "pack", "manifest.json"));
        if (flipManifest)
        {
            manifest[manifest.Length / 2] ^= 0x01;
        }
        File.WriteAllBytes(Path.Combine(tmp, "spec", "conformance", "pack", "manifest.json"), manifest);
        if (!flipManifest)
        {
            File.Copy(Path.Combine(root, "spec", "conformance", "pack", "negatives.json"), Path.Combine(tmp, "spec", "conformance", "pack", "negatives.json"), true);
            File.Copy(Path.Combine(root, "spec", "conformance", "pack", "prod.json"), Path.Combine(tmp, "spec", "conformance", "pack", "prod.json"), true);
        }
    }

    private static string ObjectPath(string root)
    {
        return Path.Combine(root, "sdk", "csharp", "objects", "O1-S1.cbor");
    }

    private static string[] Lines(string text)
    {
        var norm = text.Replace("\r\n", "\n").Replace('\r', '\n');
        if (norm.EndsWith('\n'))
        {
            norm = norm.Substring(0, norm.Length - 1);
        }
        if (norm.Length == 0)
        {
            return System.Array.Empty<string>();
        }
        return norm.Split('\n');
    }

    private static string? SixLegValue(string stdout)
    {
        const string head = "O1-S1 six_leg ";
        foreach (var line in Lines(stdout))
        {
            if (!line.StartsWith(head, StringComparison.Ordinal))
            {
                continue;
            }
            var rest = line.Substring(head.Length);
            var cut = rest.IndexOf(", checkpoints", StringComparison.Ordinal);
            if (cut < 0)
            {
                return null;
            }
            return rest.Substring(0, cut);
        }
        return null;
    }

    private static bool Decoded(byte[] buf)
    {
        var hike = HikeObject.Read(buf);
        return !hike.Malformed && !hike.DecodeFailure;
    }

    private readonly struct LegPayload
    {
        public LegPayload(int payload, byte[] bytes)
        {
            Payload = payload;
            Bytes = bytes;
        }

        public int Payload { get; }
        public byte[] Bytes { get; }
    }

    private static int? DrawnLegPayload(byte[] buf)
    {
        try
        {
            var hike = HikeObject.Read(buf);
            if (hike.Malformed || hike.DecodeFailure || hike.StagesLength < 18)
            {
                return null;
            }
            var hits = new List<LegPayload>();
            var i = 0;
            Walk(buf, ref i, hits);
            if (i != buf.Length)
            {
                return null;
            }
            var flat = new List<byte[]>();
            var total = 0;
            for (var g = 0; g < 18; g++)
            {
                var st = hike.Stages[g];
                if (!st.CheckpointsUsable || !st.LegResultsUsable || st.Checkpoints.Length != st.LegResults.Length)
                {
                    return null;
                }
                total += st.Checkpoints.Length;
                for (var j = 0; j < st.LegResults.Length; j++)
                {
                    flat.Add(st.LegResults[j]);
                }
            }
            if (hits.Count != flat.Count || total < 6)
            {
                return null;
            }
            for (var k = 0; k < flat.Count; k++)
            {
                if (hits[k].Bytes.Length != 8 || !SameBytes(hits[k].Bytes, flat[k]) || buf[hits[k].Payload - 1] != 0x48)
                {
                    return null;
                }
            }
            var drawn = Draw.Six(Draw.LabelSource("pcoc-pack-2.2/draw"), total);
            if (drawn == null || drawn.Length != 6 || drawn[0] < 0 || drawn[0] >= hits.Count)
            {
                return null;
            }
            return hits[drawn[0]].Payload;
        }
        catch (FormatException)
        {
            return null;
        }
    }

    private static bool SameBytes(byte[] a, byte[] b)
    {
        if (a.Length != b.Length)
        {
            return false;
        }
        for (var i = 0; i < a.Length; i++)
        {
            if (a[i] != b[i])
            {
                return false;
            }
        }
        return true;
    }

    private static void Walk(byte[] buf, ref int i, List<LegPayload> hits)
    {
        if (i >= buf.Length)
        {
            throw new FormatException();
        }
        var b = buf[i++];
        var major = b >> 5;
        var add = b & 0x1F;
        if (add == 31)
        {
            throw new FormatException();
        }
        var n = CborLen(buf, ref i, add);
        switch (major)
        {
            case 0:
                return;
            case 2:
            case 3:
                Skip(buf, ref i, n);
                return;
            case 4:
                for (ulong k = 0; k < n; k++)
                {
                    Walk(buf, ref i, hits);
                }
                return;
            case 5:
                for (ulong k = 0; k < n; k++)
                {
                    var key = ReadKey(buf, ref i);
                    if (key == "leg_results")
                    {
                        ReadLegArray(buf, ref i, hits);
                    }
                    else
                    {
                        Walk(buf, ref i, hits);
                    }
                }
                return;
            default:
                throw new FormatException();
        }
    }

    private static void ReadLegArray(byte[] buf, ref int i, List<LegPayload> hits)
    {
        if (i >= buf.Length)
        {
            throw new FormatException();
        }
        var b = buf[i++];
        var major = b >> 5;
        var add = b & 0x1F;
        if (major != 4 || add == 31)
        {
            throw new FormatException();
        }
        var n = CborLen(buf, ref i, add);
        for (ulong k = 0; k < n; k++)
        {
            if (i >= buf.Length)
            {
                throw new FormatException();
            }
            var item = buf[i++];
            var itemMajor = item >> 5;
            var itemAdd = item & 0x1F;
            if (itemMajor != 2 || itemAdd == 31)
            {
                throw new FormatException();
            }
            var len = CborLen(buf, ref i, itemAdd);
            if (len > (ulong)(buf.Length - i))
            {
                throw new FormatException();
            }
            var size = (int)len;
            var data = new byte[size];
            System.Buffer.BlockCopy(buf, i, data, 0, size);
            hits.Add(new LegPayload(i, data));
            i += size;
        }
    }

    private static string ReadKey(byte[] buf, ref int i)
    {
        if (i >= buf.Length)
        {
            throw new FormatException();
        }
        var b = buf[i++];
        var major = b >> 5;
        var add = b & 0x1F;
        if (major != 3 || add == 31)
        {
            throw new FormatException();
        }
        var n = CborLen(buf, ref i, add);
        if (n > (ulong)(buf.Length - i))
        {
            throw new FormatException();
        }
        var len = (int)n;
        var text = System.Text.Encoding.UTF8.GetString(buf, i, len);
        i += len;
        return text;
    }

    private static void Skip(byte[] buf, ref int i, ulong n)
    {
        if (n > (ulong)(buf.Length - i))
        {
            throw new FormatException();
        }
        i += (int)n;
    }

    private static ulong CborLen(byte[] buf, ref int i, int add)
    {
        if (add < 24)
        {
            return (ulong)add;
        }
        var width = add switch
        {
            24 => 1,
            25 => 2,
            26 => 4,
            27 => 8,
            _ => throw new FormatException(),
        };
        if (i + width > buf.Length)
        {
            throw new FormatException();
        }
        ulong n = 0;
        for (var k = 0; k < width; k++)
        {
            n = (n << 8) | buf[i++];
        }
        return n;
    }

    private static void CopyTree(string from, string to)
    {
        foreach (var file in Directory.GetFiles(from, "*", SearchOption.AllDirectories))
        {
            var rel = Path.GetRelativePath(from, file);
            var parts = rel.Split(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            var skip = false;
            foreach (var part in parts)
            {
                if (part == "bin" || part == "obj")
                {
                    skip = true;
                    break;
                }
            }
            if (skip)
            {
                continue;
            }
            var dest = Path.Combine(to, rel);
            var dir = Path.GetDirectoryName(dest);
            if (dir != null)
            {
                Directory.CreateDirectory(dir);
            }
            File.Copy(file, dest, true);
        }
    }

    private static string? FirstDll(string dir)
    {
        if (!Directory.Exists(dir))
        {
            return null;
        }
        var files = Directory.GetFiles(dir, "*.dll");
        foreach (var file in files)
        {
            if (!file.Contains("PcocLib", StringComparison.Ordinal))
            {
                return file;
            }
        }
        return null;
    }

    private static string EmptySource()
    {
        var dir = Path.Combine(Path.GetTempPath(), "pcoc-empty-nuget");
        Directory.CreateDirectory(dir);
        return "\"" + dir + "\"";
    }

    private static string FindRoot()
    {
        var dir = Directory.GetCurrentDirectory();
        for (var i = 0; i < 8 && dir != null; i++)
        {
            if (File.Exists(Path.Combine(dir, "sdk", "SPEC_PIN")) && File.Exists(Path.Combine(dir, "spec", "conformance", "pack", "manifest.json")))
            {
                return dir;
            }
            dir = Directory.GetParent(dir)?.FullName;
        }
        throw new InvalidOperationException("repo root");
    }
}
