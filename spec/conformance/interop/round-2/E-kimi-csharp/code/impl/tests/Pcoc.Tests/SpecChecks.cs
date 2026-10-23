using System;
using System.Linq;
using System.Security.Cryptography;
using Pcoc;
using Xunit;

namespace Pcoc.Tests;

/// <summary>SPEC 7.4 informative checks, which are exact byte vectors.</summary>
public class SpecChecks
{
    private static byte[] Hex(string h) => Convert.FromHexString(h);

    [Fact]
    public void Le64Checks()
    {
        Assert.Equal(Hex("0100000000000000"), BitConverter.GetBytes(1UL));
        var b = new byte[8];
        Prims.Le64(0x0102030405060708UL, b);
        Assert.Equal(Hex("0807060504030201"), b);
    }

    [Fact]
    public void Sha256Abc()
    {
        Assert.Equal("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            Convert.ToHexString(SHA256.HashData("abc"u8.ToArray())).ToLowerInvariant());
    }

    [Fact]
    public void SealOfZero()
    {
        // SPEC 7.4: SHA-256(SEAL_DOMAIN ‖ le64(0)) and S' = u64_le(E[0:8]).
        var msg = Profile.SealDomain.Concat(new byte[8]).ToArray();
        var digest = SHA256.HashData(msg);
        Assert.Equal("4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533",
            Convert.ToHexString(digest).ToLowerInvariant());
        Assert.Equal(0x76042423afbd1648UL, Prims.U64Le(digest));
        // and through the seal entry point used by every step:
        Assert.Equal(0x76042423afbd1648UL, Leg.Seal(0));
    }

    [Fact]
    public void SeedLanesAndNl4()
    {
        // SPEC 7.4: preimage 32 × 0x11 ‖ 32 × 0x22.
        var pk = Enumerable.Repeat((byte)0x11, 32).ToArray();
        var beacon = Enumerable.Repeat((byte)0x22, 32).ToArray();
        var d = Terrain.Seed(pk, beacon);
        Assert.Equal("5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c",
            Convert.ToHexString(d).ToLowerInvariant());
        Span<ulong> lanes = stackalloc ulong[4];
        Terrain.Lanes(d, lanes);
        Assert.Equal(0x545dfe297dc78951UL, lanes[0]);
        Assert.Equal(0x27858669c45e046aUL, lanes[1]);
        Assert.Equal(0x9cdac73ac1a5fe85UL, lanes[2]);
        Assert.Equal(0x7c81df6efbf55f11UL, lanes[3]);
        Assert.Equal(0xc7582082a632c2c8UL, Terrain.Nl4Base(0, 0, lanes));
    }

    [Fact]
    public void ParentIndexSmoke()
    {
        // For i = 1 all three parents must be slot 0.
        Assert.Equal(0, Prims.BitrevParent(1));
        Assert.Equal(0, Prims.MulParent(1));
        // All parents in range for a spread of i.
        foreach (long i in new long[] { 2, 3, 4, 5, 7, 8, 15, 16, 100, 1023, 1024, 4095, 16383 })
        {
            Assert.InRange(Prims.BitrevParent(i), 0, i - 1);
            Assert.InRange(Prims.MulParent(i), 0, i - 1);
        }
    }
}

/// <summary>The three engines must produce bit-identical terrain and folds.</summary>
public class EngineEquivalence
{
    private static Profile Small => Profile.ConformanceSmall(slotBytes: 64, sampleBytes: 32, terrainBytes: 65536, stepsPerLeg: 100);

    [Fact]
    public void TerrainIdenticalAcrossEngines()
    {
        var profile = Small;
        var seed = Terrain.Seed(Enumerable.Repeat((byte)0xA5, 32).ToArray(), Enumerable.Repeat((byte)0x5A, 32).ToArray());
        Span<ulong> lanes = stackalloc ulong[4];
        Terrain.Lanes(seed, lanes);
        int total = (int)(profile.NSlots * profile.WordsPerSlot);
        var a = new ulong[total];
        var b = new ulong[total];
        var c = new ulong[total];
        Terrain.FillScalar(profile, a, lanes);
        Terrain.FillAvx2(profile, b, lanes);
        Terrain.FillAvx512(profile, c, lanes);
        Assert.Equal(a, b);
        Assert.Equal(a, c);
    }

    [Fact]
    public void FoldIdenticalAcrossEngines()
    {
        var rng = new Random(42);
        var slot = new ulong[2048];
        for (int i = 0; i < slot.Length; i++) slot[i] = (ulong)rng.NextInt64();
        foreach (ulong s in new ulong[] { 0, 1, 0xFFFFFFFFFFFFFFFF, 0x123456789ABCDEF0 })
        {
            ulong expect = Leg.FoldScalar(slot, s);
            Assert.Equal(expect, Leg.FoldAvx2(slot, s));
            Assert.Equal(expect, Leg.FoldAvx512(slot, s));
        }
        // Odd lengths exercise the scalar tails.
        foreach (int len in new[] { 1, 3, 5, 7, 9, 33 })
        {
            var odd = slot.Take(len).ToArray();
            ulong expect = Leg.FoldScalar(odd, 0xDEADBEEF);
            Assert.Equal(expect, Leg.FoldAvx2(odd, 0xDEADBEEF));
            Assert.Equal(expect, Leg.FoldAvx512(odd, 0xDEADBEEF));
        }
    }

    [Fact]
    public void Nl4BaseDependsOnEveryLane()
    {
        // SPEC 10: every terrain word depends on all 256 bits of the seed.
        var seed = Terrain.Seed(new byte[32], new byte[32]);
        Span<ulong> lanes = stackalloc ulong[4];
        Terrain.Lanes(seed, lanes);
        ulong w0 = Terrain.Nl4Base(3, 2, lanes);
        for (int k = 0; k < 4; k++)
        {
            Span<ulong> other = stackalloc ulong[4];
            lanes.CopyTo(other);
            other[k] ^= 1UL;
            Assert.NotEqual(w0, Terrain.Nl4Base(3, 2, other));
        }
    }
}
