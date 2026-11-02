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
    public static int Main()
    {
        var root = FindRoot();
        var pack = Path.Combine(root, "spec", "conformance", "pack");
        PinCheck(root, pack);
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
        }
        finally
        {
            try { Directory.Delete(work, true); } catch (Exception) { }
        }
        if (_failed != 0)
        {
            Console.Error.WriteLine("failed " + _failed);
            return 1;
        }
        return 0;
    }

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
        Console.Out.WriteLine("fail manifest-mutation (expected)");
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
