using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Pcoc;
using Pcoc.Cli;
using Xunit;

namespace Pcoc.Tests;

/// <summary>
/// Conformance tests against the committed rig artifacts: the candidate
/// files must be reproducible byte-for-byte, the committed objects must
/// verify green, and the rule edges of sections 7.3, 14 and 16 must behave
/// as the SPEC says.
/// </summary>
public class Conformance
{
    private static string RepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null)
        {
            if (Directory.Exists(Path.Combine(dir.FullName, "candidates")) &&
                Directory.Exists(Path.Combine(dir.FullName, "objects")) &&
                Directory.Exists(Path.Combine(dir.FullName, "impl")))
                return dir.FullName;
            dir = dir.Parent;
        }
        throw new InvalidOperationException("repo root not found above " + AppContext.BaseDirectory);
    }

    private static Dictionary<string, string> ReadCandidate(string name)
    {
        var d = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var line in File.ReadAllLines(Path.Combine(RepoRoot(), "candidates", name)))
        {
            int tab = line.IndexOf('\t');
            d[line.Substring(0, tab)] = line.Substring(tab + 1);
        }
        return d;
    }

    private static byte[] Hex(string h) => Convert.FromHexString(h);

    // ---------------- deliverable 6: committed candidates are reproducible ----------------

    [Fact]
    public void CandidatesUnchanged()
    {
        string tmp = Path.Combine(Path.GetTempPath(), "pcoc-candidates-" + Guid.NewGuid().ToString("N"));
        try
        {
            Candidates.Generate(tmp, TextWriter.Null);
            foreach (string name in Candidates.FileNames)
            {
                byte[] committed = File.ReadAllBytes(Path.Combine(RepoRoot(), "candidates", name));
                byte[] regenerated = File.ReadAllBytes(Path.Combine(tmp, name));
                Assert.True(committed.AsSpan().SequenceEqual(regenerated),
                    $"candidate file {name} changed; re-run gen-candidates and commit the result");
            }
        }
        finally
        {
            if (Directory.Exists(tmp)) Directory.Delete(tmp, true);
        }
    }

    // ---------------- section 14 happy paths on the committed objects ----------------

    private static JsonDocument VerifyCommitted(string file, Profile profile)
    {
        byte[] objectBytes = File.ReadAllBytes(Path.Combine(RepoRoot(), "objects", file));
        var outcome = Verifier.Verify(objectBytes, new VerifyOptions
        {
            Profile = profile,
            Index = Fixtures.SyntheticIndex(),
            DrawBytes = new ChainDrawBytes(Candidates.DrawLabel),
        });
        Assert.Equal(VerifyStatus.Record, outcome.Status);
        return JsonDocument.Parse(outcome.Record);
    }

    private static void AssertGreen(JsonDocument doc, int legsPerStage)
    {
        var r = doc.RootElement;
        Assert.Equal("conformance-small", r.GetProperty("profile").GetString());
        Assert.Equal("match", r.GetProperty("shape").GetString());
        Assert.Equal(0, r.GetProperty("shape_item").GetArrayLength());
        Assert.Equal("match", r.GetProperty("seed_value").GetString());

        var headers = r.GetProperty("headers");
        Assert.Equal(19, headers.GetArrayLength());
        foreach (var h in headers.EnumerateArray())
        {
            Assert.Equal("match", h.GetProperty("index_comparison").GetString());
            Assert.Equal(32, Hex(h.GetProperty("stored_hash").GetString()!).Length);
        }

        var checkpoints = r.GetProperty("checkpoints");
        Assert.Equal(18, checkpoints.GetArrayLength());
        foreach (var c in checkpoints.EnumerateArray())
        {
            Assert.Equal(legsPerStage, c.GetProperty("legs").GetArrayLength());
            foreach (var leg in c.GetProperty("legs").EnumerateArray())
                Assert.Equal("match", leg.GetProperty("result").GetString());
        }

        var waypoints = r.GetProperty("waypoints");
        Assert.Equal(17, waypoints.GetArrayLength());
        foreach (var w in waypoints.EnumerateArray())
            Assert.Equal("match", w.GetProperty("result").GetString());

        var paths = r.GetProperty("paths");
        Assert.Equal(17, paths.GetArrayLength());
        for (int i = 0; i < 16; i++)
        {
            Assert.Equal(i + 1, paths[i].GetProperty("g").GetInt32());
            Assert.Equal("skip", paths[i].GetProperty("path_state").GetString());
            Assert.Equal("n/a", paths[i].GetProperty("plant").GetString());
        }
        Assert.Equal("not fetched by design", paths[16].GetProperty("w17_path").GetString());

        var anchors = r.GetProperty("anchors");
        Assert.Equal(16, anchors.GetArrayLength());
        foreach (var a in anchors.EnumerateArray()) Assert.Equal("not anchored", a.GetString());
        Assert.Equal(0, r.GetProperty("anchored_total").GetInt32());
        Assert.Equal(new[] { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16 },
            r.GetProperty("skip_list").EnumerateArray().Select(x => x.GetInt32()).ToArray());

        // bookends: nothing anchored, so every A4.11 field is "not defined"
        foreach (var k in new[] { "a", "b", "N", "S", "delta_h", "P_b", "height_P_b",
                 "index_comparison_P_b", "T_P_b", "height_I_a_minus_1", "T_I_a_minus_1" })
            Assert.Equal("not defined", r.GetProperty(k).GetString());

        var sixLeg = r.GetProperty("six_leg");
        Assert.Equal(6, sixLeg.GetProperty("indices").GetArrayLength());
        foreach (var res in sixLeg.GetProperty("results").EnumerateArray())
            Assert.Equal("match", res.GetString());

        var analysis = r.GetProperty("analysis");
        Assert.Equal(10200, analysis.GetProperty("whole_hike_span").GetInt64());
        Assert.Equal(10800, analysis.GetProperty("hike_span").GetInt64());
        foreach (var stage in analysis.GetProperty("per_stage").EnumerateArray())
        {
            Assert.Equal(legsPerStage, stage.GetProperty("checkpoint_count").GetInt32());
            Assert.Equal(600, stage.GetProperty("span").GetInt64());
        }
    }

    [Fact]
    public void VerifyO1HappyPath()
    {
        using var doc = VerifyCommitted("O1-S1.cbor", Fixtures.S1);
        AssertGreen(doc, 1);
        Assert.Equal(17, doc.RootElement.GetProperty("analysis").GetProperty("whole_hike_legs").GetInt64());
    }

    [Fact]
    public void VerifyO2HappyPath()
    {
        using var doc = VerifyCommitted("O2-S2.cbor", Fixtures.S2);
        AssertGreen(doc, 3);
        Assert.Equal(51, doc.RootElement.GetProperty("analysis").GetProperty("whole_hike_legs").GetInt64());
    }

    [Fact]
    public void VerifyO3HappyPath()
    {
        using var doc = VerifyCommitted("O3-PROD.cbor", Profile.Production);
        var r = doc.RootElement;
        Assert.Equal("production", r.GetProperty("profile").GetString());
        Assert.Equal("match", r.GetProperty("shape").GetString());
        Assert.Equal("match", r.GetProperty("seed_value").GetString());
        foreach (var h in r.GetProperty("headers").EnumerateArray())
            Assert.Equal("match", h.GetProperty("index_comparison").GetString());
        foreach (var c in r.GetProperty("checkpoints").EnumerateArray())
            foreach (var leg in c.GetProperty("legs").EnumerateArray())
                Assert.Equal("match", leg.GetProperty("result").GetString());
        foreach (var w in r.GetProperty("waypoints").EnumerateArray())
            Assert.Equal("match", w.GetProperty("result").GetString());
        foreach (var res in r.GetProperty("six_leg").GetProperty("results").EnumerateArray())
            Assert.Equal("match", res.GetString());
    }

    // ---------------- section 7.3 decode order (erratum 1) and Rule A5 ----------------

    private static VerifyOutcome VerifyRaw(byte[] bytes) =>
        Verifier.Verify(bytes, new VerifyOptions
        {
            Profile = Fixtures.S1,
            Index = Fixtures.SyntheticIndex(),
            DrawBytes = new ChainDrawBytes(Candidates.DrawLabel),
        });

    public static IEnumerable<object[]> MalformedInputs()
    {
        yield return new object[] { Array.Empty<byte>() };                    // empty input
        yield return new object[] { new byte[] { 0xFF } };                    // lone break
        yield return new object[] { new byte[] { 0xA1, 0x64 } };              // truncated inside text head
        yield return new object[] { new byte[] { 0xA0, 0x00 } };              // trailing byte
        yield return new object[] { new byte[] { 0xBF, 0x00, 0x00, 0xFF } };  // indefinite-length map
        yield return new object[] { new byte[] { 0x5F, 0x40, 0xFF } };        // indefinite-length bstr
        yield return new object[] { new byte[] { 0xC0, 0x00 } };              // tag
        yield return new object[] { new byte[] { 0xF9, 0x3C, 0x00 } };        // float16
        yield return new object[] { new byte[] { 0x1C } };                    // reserved additional info 28
        // duplicate key in a map
        yield return new object[] { new byte[] { 0xA2, 0x61, 0x61, 0x00, 0x61, 0x61, 0x01 } };
    }

    [Theory]
    [MemberData(nameof(MalformedInputs))]
    public void MalformedCborYieldsShapeRecord(byte[] bytes)
    {
        var outcome = VerifyRaw(bytes);
        Assert.Equal(VerifyStatus.Record, outcome.Status);
        using var doc = JsonDocument.Parse(outcome.Record);
        Assert.Equal("mismatch", doc.RootElement.GetProperty("shape").GetString());
        var items = doc.RootElement.GetProperty("shape_item");
        Assert.Equal(1, items.GetArrayLength());
        Assert.Equal("malformed cbor", items[0].GetString());
    }

    public static IEnumerable<object[]> DecodeFailureInputs()
    {
        yield return new object[] { new byte[] { 0x01 } };                    // uint, not a map
        yield return new object[] { new byte[] { 0x40 } };                    // bstr, not a map
        yield return new object[] { new byte[] { 0xA0 } };                    // map without kind
        // map with a non-text kind value: {"kind": 1}
        yield return new object[] { new byte[] { 0xA1, 0x64, 0x6B, 0x69, 0x6E, 0x64, 0x01 } };
    }

    [Theory]
    [MemberData(nameof(DecodeFailureInputs))]
    public void NonMapOrMissingKindIsDecodeFailure(byte[] bytes)
    {
        var outcome = VerifyRaw(bytes);
        Assert.Equal(VerifyStatus.DecodeFailure, outcome.Status);
    }

    [Fact]
    public void UnknownKindMatchesCommittedVectors()
    {
        var d = ReadCandidate("unknown-kind.txt");
        for (int i = 0; i < 4; i++)
        {
            byte[] kindBytes = d[$"unknown-kind.{i}.kind_text_hex"].Length == 0
                ? Array.Empty<byte>()
                : Hex(d[$"unknown-kind.{i}.kind_text_hex"]);
            // hand-encode {"kind": <text>}
            var map = new List<byte> { 0xA1, 0x64, 0x6B, 0x69, 0x6E, 0x64 };
            if (kindBytes.Length < 24) map.Add((byte)(0x60 + kindBytes.Length));
            else { map.Add(0x78); map.Add((byte)kindBytes.Length); }
            map.AddRange(kindBytes);
            var outcome = VerifyRaw(map.ToArray());
            Assert.Equal(VerifyStatus.UnknownKind, outcome.Status);
            Assert.Equal(d[$"unknown-kind.{i}.record_hex"],
                Convert.ToHexString(Encoding.ASCII.GetBytes(outcome.Record)).ToLowerInvariant());
        }
    }

    // ---------------- hashcash candidate vector ----------------

    [Fact]
    public void HashcashVectorRecomputes()
    {
        var d = ReadCandidate("hashcash.txt");
        string origin = Hashcash.BariOrigin("https://example.com");
        Assert.Equal(d["hashcash.bari_origin"], origin);
        byte[] headerHash = SHA256.HashData(SHA256.HashData(Hex(d["hashcash.fee_header"])));
        Assert.Equal(d["hashcash.header_hash"], Convert.ToHexString(headerHash).ToLowerInvariant());
        byte[] challenge = Hashcash.Challenge(origin, headerHash);
        Assert.Equal(d["hashcash.challenge"], Convert.ToHexString(challenge).ToLowerInvariant());
        ulong nonce = Hashcash.Solve(challenge, Hex(d["hashcash.waypoint"]));
        Assert.Equal(ulong.Parse(d["hashcash.nonce"]), nonce);
        byte[] digest = Hashcash.Digest(challenge, Hex(d["hashcash.waypoint"]), nonce);
        Assert.Equal(d["hashcash.digest"], Convert.ToHexString(digest).ToLowerInvariant());
        Assert.Equal(int.Parse(d["hashcash.leading_zero_bits"]), Hashcash.LeadingZeroBits(digest));
        Assert.True(Hashcash.LeadingZeroBits(digest) >= int.Parse(d["hashcash.difficulty_bits"]));
    }

    // ---------------- draw candidate vectors ----------------

    [Fact]
    public void DrawVectorsRecompute()
    {
        var d = ReadCandidate("draw.txt");
        string label = d["draw.b0_label"];
        int rounds = int.Parse(d["draw.thorough_rounds"]);
        foreach (int total in new[] { 6, 18, 54 })
        {
            Assert.True(Draw.SixLeg(new ChainDrawBytes(label), (ulong)total, out ulong[] six));
            Assert.Equal(d[$"draw.legs.{total}.six_leg.indices"], string.Join(',', six));
            Assert.True(Draw.Thorough(new ChainDrawBytes(label), (ulong)total, rounds, out ulong[] phase));
            Assert.Equal(d[$"draw.legs.{total}.thorough.indices"], string.Join(',', phase));
        }
    }

    // ---------------- anchor window and rate floor candidates ----------------

    [Fact]
    public void AnchorWindowMatches()
    {
        var d = ReadCandidate("anchor.txt");
        for (long h = 100; h <= 106; h++)
        {
            bool anchored = h - 100 is >= 2 and <= 4;
            Assert.Equal(anchored ? "anchored" : "not anchored", d[$"anchor.H.100.height.{h}"]);
        }
    }

    [Fact]
    public void RateFloorsMatch()
    {
        var d = ReadCandidate("rate.txt");
        foreach (var (key, value) in d)
        {
            // rate.legs.<l>.span.<s>.floor.<p>_<q> = meets iff q·l >= p·s
            var parts = key.Split('.');
            long l = long.Parse(parts[2]), s = long.Parse(parts[4]);
            var pq = parts[6].Split('_');
            long p = long.Parse(pq[0]), q = long.Parse(pq[1]);
            Assert.Equal(q * l >= p * s ? "meets" : "refuse", value);
        }
    }

    // ---------------- negatives: every committed negative differs from honest ----------------

    [Fact]
    public void NegativesAllDifferFromHonest()
    {
        var d = ReadCandidate("negatives.txt");
        var flags = d.Where(kv => kv.Key.EndsWith(".matches_honest", StringComparison.Ordinal)).ToList();
        Assert.Equal(13, flags.Count);
        foreach (var (key, value) in flags)
            Assert.Equal("false", value);
        // the honest leg result must not equal any committed leg-negative value
        string honest = d["negatives.honest.legs_le_hex"];
        foreach (var (key, value) in d)
            if (key.EndsWith(".legs_le_hex", StringComparison.Ordinal) && key != "negatives.honest.legs_le_hex")
                Assert.NotEqual(honest, value);
    }

    // ---------------- canonical path body (section 14.3) ----------------

    private static byte[] Node(byte tag, ReadOnlySpan<byte> a, ReadOnlySpan<byte> b)
    {
        var msg = new byte[1 + a.Length + b.Length];
        msg[0] = tag;
        a.CopyTo(msg.AsSpan(1));
        b.CopyTo(msg.AsSpan(1 + a.Length));
        return SHA256.HashData(msg);
    }

    private static byte[] LeafHash(byte[] leaf) => Node(0x1A, leaf, ReadOnlySpan<byte>.Empty);

    private static byte[] BuildBody(byte version, byte[] leaf, uint leafIndex, uint treeSize,
        byte[][] siblings, byte[] root, byte[] txid, uint vout, byte[] blockHash, uint blockHeight)
    {
        using var ms = new MemoryStream();
        ms.WriteByte(version);
        ms.Write(leaf);
        ms.Write(BitConverter.GetBytes(System.Buffers.Binary.BinaryPrimitives.ReverseEndianness(leafIndex)));
        ms.Write(BitConverter.GetBytes(System.Buffers.Binary.BinaryPrimitives.ReverseEndianness(treeSize)));
        ms.WriteByte((byte)siblings.Length);
        foreach (var s in siblings) ms.Write(s);
        ms.Write(root);
        ms.Write(txid);
        ms.Write(BitConverter.GetBytes(System.Buffers.Binary.BinaryPrimitives.ReverseEndianness(vout)));
        ms.Write(blockHash);
        ms.Write(BitConverter.GetBytes(System.Buffers.Binary.BinaryPrimitives.ReverseEndianness(blockHeight)));
        return ms.ToArray();
    }

    [Fact]
    public void PathBodySingleLeafTreeValidates()
    {
        byte[] waypoint = SHA256.HashData("waypoint"u8.ToArray());
        byte[] root = LeafHash(waypoint);
        byte[] txid = SHA256.HashData("txid"u8.ToArray());
        byte[] blockHash = SHA256.HashData("block"u8.ToArray());
        byte[] body = BuildBody(0x01, waypoint, 0, 1, Array.Empty<byte[]>(), root, txid, 0, blockHash, 1000);
        Assert.Equal(146, body.Length);
        var decoded = PathBody.Decode(body);
        Assert.NotNull(decoded);
        Assert.True(decoded!.ValidateFor(waypoint));
        Assert.False(decoded.ValidateFor(SHA256.HashData("other"u8.ToArray())));
        // claimed-block partial read agrees with the full decode
        Assert.True(PathBody.TryReadClaimedBlock(body, out byte[]? cb, out uint ch));
        Assert.Equal(blockHash, cb);
        Assert.Equal(1000u, ch);
    }

    [Fact]
    public void PathBodyOddWidthTreeValidates()
    {
        // three leaves: the odd-width level duplicates the last node
        byte[][] leaves = { SHA256.HashData("l0"u8.ToArray()), SHA256.HashData("l1"u8.ToArray()), SHA256.HashData("l2"u8.ToArray()) };
        byte[][] lh = { LeafHash(leaves[0]), LeafHash(leaves[1]), LeafHash(leaves[2]) };
        byte[] n0 = Node(0x1B, lh[0], lh[1]);
        byte[] n1 = Node(0x1B, lh[2], lh[2]);
        byte[] root = Node(0x1B, n0, n1);
        var txid = new byte[32];
        var blockHash = new byte[32];

        // path for leaf 2: siblings [lh[2] (self), n0]
        byte[] body = BuildBody(0x01, leaves[2], 2, 3, new[] { lh[2], n0 }, root, txid, 0, blockHash, 7);
        var decoded = PathBody.Decode(body);
        Assert.NotNull(decoded);
        Assert.True(decoded!.ValidateFor(leaves[2]));

        // path for leaf 0: siblings [lh[1], n1]
        byte[] body0 = BuildBody(0x01, leaves[0], 0, 3, new[] { lh[1], n1 }, root, txid, 0, blockHash, 7);
        var decoded0 = PathBody.Decode(body0);
        Assert.NotNull(decoded0);
        Assert.True(decoded0!.ValidateFor(leaves[0]));

        // a wrong self-sibling at the duplication level must fail, not pass
        byte[] forged = BuildBody(0x01, leaves[2], 2, 3, new[] { lh[0], n0 }, root, txid, 0, blockHash, 7);
        Assert.False(PathBody.Decode(forged)!.ValidateFor(leaves[2]));
    }

    [Fact]
    public void PathBodyDecodeRejectsNonCanonical()
    {
        byte[] waypoint = SHA256.HashData("w"u8.ToArray());
        byte[] good = BuildBody(0x01, waypoint, 0, 1, Array.Empty<byte[]>(),
            LeafHash(waypoint), new byte[32], 0, new byte[32], 1);
        Assert.Null(PathBody.Decode(good.AsSpan(0, good.Length - 1)));   // truncated
        Assert.Null(PathBody.Decode(good.Concat(new byte[1]).ToArray())); // overlong
        var badVersion = (byte[])good.Clone(); badVersion[0] = 0x02;
        Assert.Null(PathBody.Decode(badVersion));
        var badTree = (byte[])good.Clone();                              // tree_size = 0
        Array.Clear(badTree, 37, 4);
        Assert.Null(PathBody.Decode(badTree));
        var badIndex = (byte[])good.Clone();                             // leaf_index 1 >= tree_size 1
        badIndex[36] = 1;
        Assert.Null(PathBody.Decode(badIndex));
        // tree_size 2 needs exactly one sibling; zero siblings is not canonical
        byte[] wrongCount = BuildBody(0x01, waypoint, 0, 2, Array.Empty<byte[]>(),
            new byte[32], new byte[32], 0, new byte[32], 1);
        Assert.Null(PathBody.Decode(wrongCount));
    }
}
