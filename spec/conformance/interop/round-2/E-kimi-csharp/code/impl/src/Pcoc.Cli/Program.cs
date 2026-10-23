using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;

namespace Pcoc.Cli;

/// <summary>
/// pcoc — the Satoshi Hike 2.2 CLI: candidate generation, offline test-plane
/// production, and the section 18 verify CLI (exit 0 record produced, exit 1
/// unreadable object or decode failure, exit 2 usage).
/// </summary>
public static class Program
{
    public static int Main(string[] args)
    {
        if (args.Length == 0) return Usage();
        try
        {
            switch (args[0])
            {
                case "gen-candidates": return GenCandidates(args);
                case "produce": return ProduceCmd(args);
                case "verify": return VerifyCmd(args);
                case "bench": return BenchCmd(args);
                default: return Usage();
            }
        }
        catch (UsageError e)
        {
            Console.Error.WriteLine(e.Message);
            return 2;
        }
    }

    private sealed class UsageError : Exception
    {
        public UsageError(string message) : base(message) { }
    }

    private static int Usage()
    {
        Console.Error.WriteLine("usage:");
        Console.Error.WriteLine("  pcoc gen-candidates --out <dir>");
        Console.Error.WriteLine("  pcoc produce --out <dir>");
        Console.Error.WriteLine("  pcoc verify <object> (--index-url <u> | --synthetic-index)");
        Console.Error.WriteLine("       [--profile production|conformance-small]");
        Console.Error.WriteLine("       [--slot-bytes N --sample-bytes N --terrain-bytes N --steps-per-leg N]");
        Console.Error.WriteLine("       [--draw-label <ascii> | --draw-bytes-hex <hex>]");
        Console.Error.WriteLine("  pcoc bench [--engine scalar|avx2|avx512] [--skip-fill] [--skip-leg]");
        return 2;
    }

    private static string OptValue(string[] args, string name)
    {
        for (int i = 1; i < args.Length - 1; i++)
            if (args[i] == name) return args[i + 1];
        throw new UsageError($"missing value for {name}");
    }

    private static bool HasOpt(string[] args, string name)
    {
        foreach (var a in args) if (a == name) return true;
        return false;
    }

    private static int GenCandidates(string[] args)
    {
        string outDir = OptValue(args, "--out");
        Candidates.Generate(outDir, Console.Error);
        return 0;
    }

    private static int ProduceCmd(string[] args)
    {
        string outDir = OptValue(args, "--out");
        Directory.CreateDirectory(outDir);
        // O1: S1 18×1, O2: S2 18×3, O3: PROD 18×1 (production profile, one leg per stage).
        (string file, Pcoc.Profile profile, int[] legs)[] jobs =
        {
            ("O1-S1.cbor", Fixtures.S1, Fill(18, 1)),
            ("O2-S2.cbor", Fixtures.S2, Fill(18, 3)),
            ("O3-PROD.cbor", Pcoc.Profile.Production, Fill(18, 1)),
        };
        foreach (var (file, profile, legs) in jobs)
        {
            Console.Error.WriteLine($"produce: {file} ({profile.Name})");
            var obj = Producer.Produce(profile, Fixtures.PkA, legs, Fixtures.SeedBeacon,
                Fixtures.AllInterrupts(), Fixtures.SeedHeight, Console.Error);
            File.WriteAllBytes(Path.Combine(outDir, file), obj.Encode());
        }
        return 0;
    }

    private static int[] Fill(int n, int v) { var a = new int[n]; Array.Fill(a, v); return a; }

    private static int VerifyCmd(string[] args)
    {
        if (args.Length < 2 || args[1].StartsWith("--")) return Usage();
        string objectPath = args[1];

        string profileName = HasOpt(args, "--profile") ? OptValue(args, "--profile") : "production";
        Pcoc.Profile profile;
        if (profileName == "production" || profileName == "live" || profileName == "test")
        {
            profile = Pcoc.Profile.Production;
        }
        else if (profileName == "conformance-small")
        {
            // all four integers are required inputs; none has a default (section 16.1)
            bool all = HasOpt(args, "--slot-bytes") && HasOpt(args, "--sample-bytes") &&
                       HasOpt(args, "--terrain-bytes") && HasOpt(args, "--steps-per-leg");
            if (!all)
                throw new UsageError("conformance-small requires --slot-bytes, --sample-bytes, --terrain-bytes and --steps-per-leg");
            profile = Pcoc.Profile.ConformanceSmall(
                int.Parse(OptValue(args, "--slot-bytes")),
                int.Parse(OptValue(args, "--sample-bytes")),
                long.Parse(OptValue(args, "--terrain-bytes")),
                long.Parse(OptValue(args, "--steps-per-leg")));
        }
        else
        {
            throw new UsageError($"unknown profile: {profileName}");
        }

        bool synth = HasOpt(args, "--synthetic-index");
        bool hasUrl = HasOpt(args, "--index-url");
        if (synth == hasUrl)
            throw new UsageError("exactly one of --index-url and --synthetic-index is required");
        Pcoc.IIndexSource index = synth
            ? Fixtures.SyntheticIndex()
            : new Pcoc.HttpIndexSource(OptValue(args, "--index-url"));

        bool hasLabel = HasOpt(args, "--draw-label");
        bool hasHex = HasOpt(args, "--draw-bytes-hex");
        if (hasLabel && hasHex)
            throw new UsageError("at most one of --draw-label and --draw-bytes-hex");
        Pcoc.IDrawByteSource? draw = null;
        if (hasLabel) draw = new Pcoc.ChainDrawBytes(OptValue(args, "--draw-label"));
        if (hasHex) draw = new Pcoc.FixedDrawBytes(Fixtures.Hex(OptValue(args, "--draw-bytes-hex")));

        byte[] objectBytes;
        try
        {
            objectBytes = File.ReadAllBytes(objectPath);
        }
        catch (Exception)
        {
            Console.Error.WriteLine($"cannot read object file: {objectPath}");
            return 1;
        }

        var outcome = Pcoc.Verifier.Verify(objectBytes, new Pcoc.VerifyOptions
        {
            Profile = profile,
            Index = index,
            DrawBytes = draw,
        });
        if (outcome.Status == Pcoc.VerifyStatus.DecodeFailure)
        {
            Console.Error.WriteLine("decode failure: not one well-formed CBOR map carrying a kind text");
            return 1;
        }
        // section 18: the one record, keys in order, one trailing newline
        Console.Out.Write(outcome.Record);
        Console.Out.Write('\n');
        return 0;
    }

    // ---------------- bench ----------------

    private static int BenchCmd(string[] args)
    {
        string? engineName = HasOpt(args, "--engine") ? OptValue(args, "--engine") : null;
        bool skipFill = HasOpt(args, "--skip-fill");
        bool skipLeg = HasOpt(args, "--skip-leg");
        var engines = new List<Pcoc.Engine>();
        if (engineName is not null)
        {
            engines.Add(engineName switch
            {
                "scalar" => Pcoc.Engine.Scalar,
                "avx2" => Pcoc.Engine.Avx2,
                "avx512" => Pcoc.Engine.Avx512,
                _ => throw new UsageError($"unknown engine {engineName}"),
            });
        }
        else
        {
            engines.Add(Pcoc.Engine.Scalar);
            if (System.Runtime.Intrinsics.X86.Avx2.IsSupported) engines.Add(Pcoc.Engine.Avx2);
            if (System.Runtime.Intrinsics.Vector512.IsHardwareAccelerated &&
                System.Runtime.Intrinsics.X86.Avx512DQ.IsSupported) engines.Add(Pcoc.Engine.Avx512);
        }

        var prod = Pcoc.Profile.Production;
        byte[] seed = Pcoc.Terrain.Seed(Fixtures.PkA, Fixtures.SeedBeacon);
        Console.WriteLine($"host engine in force: {Pcoc.EngineSelection.Current}; seal backend: {Pcoc.Sha256Fast.Backend}");
        foreach (var e in engines)
        {
            if (e == Pcoc.Engine.Avx2 && !System.Runtime.Intrinsics.X86.Avx2.IsSupported)
            {
                Console.WriteLine($"{e}: not supported on this host");
                continue;
            }
            if (e == Pcoc.Engine.Avx512 && !(System.Runtime.Intrinsics.Vector512.IsHardwareAccelerated &&
                System.Runtime.Intrinsics.X86.Avx512DQ.IsSupported))
            {
                Console.WriteLine($"{e}: not supported on this host");
                continue;
            }
            Pcoc.Terrain? terrain = null;
            if (!skipFill)
            {
                var sw = Stopwatch.StartNew();
                terrain = Pcoc.Terrain.Fill(prod, seed, e);
                sw.Stop();
                double mib = prod.TerrainBytes / 1048576.0;
                Console.WriteLine($"engine {e}: terrain fill {sw.Elapsed.TotalSeconds:F3}s ({mib / sw.Elapsed.TotalSeconds:F0} MiB/s)");
            }
            if (!skipLeg)
            {
                terrain ??= Pcoc.Terrain.Fill(prod, seed, e);
                var sw = Stopwatch.StartNew();
                ulong result = Pcoc.Leg.RunLeg(terrain, seed, prod.StepsPerLeg, e);
                sw.Stop();
                Console.WriteLine($"engine {e}: one production leg {sw.Elapsed.TotalSeconds:F3}s " +
                    $"({prod.StepsPerLeg / sw.Elapsed.TotalSeconds:F0} steps/s, result {result:x16})");
            }
        }

        // seal micro-benchmark: the single-block section 11.1 seal
        const int sealReps = 400_000;
        Span<byte> msg = stackalloc byte[20];
        Pcoc.Profile.SealDomain.CopyTo(msg);
        Pcoc.Prims.Le64(0xDEADBEEF, msg.Slice(12));
        Span<byte> digest = stackalloc byte[32];
        var swSeal = Stopwatch.StartNew();
        ulong acc = 0;
        for (int i = 0; i < sealReps; i++)
        {
            System.Security.Cryptography.SHA256.TryHashData(msg, digest, out _);
            acc += Pcoc.Prims.U64Le(digest);
        }
        swSeal.Stop();
        double managedNs = swSeal.Elapsed.TotalMilliseconds * 1e6 / sealReps;
        Console.WriteLine($"seal via managed SHA256.TryHashData: {managedNs:F0} ns/hash ({acc:x})");
        if (Pcoc.Sha256Fast.NativeAvailable)
        {
            swSeal.Restart();
            acc = 0;
            for (int i = 0; i < sealReps; i++)
            {
                Pcoc.Sha256Fast.Hash(msg, digest);
                acc += Pcoc.Prims.U64Le(digest);
            }
            swSeal.Stop();
            double nativeNs = swSeal.Elapsed.TotalMilliseconds * 1e6 / sealReps;
            Console.WriteLine($"seal via libcrypto SHA256:        {nativeNs:F0} ns/hash ({acc:x})");
        }
        return 0;
    }
}
