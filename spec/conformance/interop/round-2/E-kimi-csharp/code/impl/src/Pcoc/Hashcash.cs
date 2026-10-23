using System;
using System.Globalization;
using System.Security.Cryptography;
using System.Text;

namespace Pcoc;

/// <summary>Section 13.1 Hashcash: origin serialization, challenge, digest, solve.</summary>
public static class Hashcash
{
    public const int DifficultyBits = 21;

    /// <summary>
    /// RFC 6454 section 6.2 serialization of a web origin: lowercase scheme
    /// (http or https), "://", lowercase host, and ":" + shortest-decimal port
    /// only when the port is not the scheme's default. No trailing slash.
    /// </summary>
    public static string BariOrigin(string url)
    {
        var uri = new Uri(url, UriKind.Absolute);
        string scheme = uri.Scheme.ToLowerInvariant();
        if (scheme != "http" && scheme != "https")
            throw new ArgumentException("origin scheme must be http or https");
        string host = uri.Host.ToLowerInvariant(); // Uri gives the A-label (IDNA) form
        int port = uri.IsDefaultPort ? -1 : uri.Port;
        var sb = new StringBuilder();
        sb.Append(scheme).Append("://").Append(host);
        if (port >= 0)
            sb.Append(':').Append(port.ToString(CultureInfo.InvariantCulture));
        return sb.ToString();
    }

    /// <summary>challenge = SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash).</summary>
    public static byte[] Challenge(string bariOrigin, ReadOnlySpan<byte> headerHash32)
    {
        byte[] origin = Encoding.ASCII.GetBytes(bariOrigin);
        var msg = new byte[10 + origin.Length + 32];
        "bari-hc-v2"u8.CopyTo(msg);
        origin.CopyTo(msg, 10);
        headerHash32.CopyTo(msg.AsSpan(10 + origin.Length));
        return SHA256.HashData(msg);
    }

    /// <summary>digest = SHA-256(challenge ‖ waypoint ‖ le64(nonce)).</summary>
    public static byte[] Digest(ReadOnlySpan<byte> challenge32, ReadOnlySpan<byte> waypoint32, ulong nonce)
    {
        Span<byte> msg = stackalloc byte[72];
        challenge32.CopyTo(msg);
        waypoint32.CopyTo(msg.Slice(32));
        Prims.Le64(nonce, msg.Slice(64));
        return SHA256.HashData(msg);
    }

    /// <summary>Leading zero bits from the first digest byte, high bit first.</summary>
    public static int LeadingZeroBits(ReadOnlySpan<byte> digest)
    {
        int n = 0;
        foreach (byte b in digest)
        {
            if (b == 0) { n += 8; continue; }
            n += System.Numerics.BitOperations.LeadingZeroCount(b) - 24;
            break;
        }
        return n;
    }

    /// <summary>The first nonce in the order 0, 1, 2, … whose digest meets the difficulty.</summary>
    public static ulong Solve(ReadOnlySpan<byte> challenge32, ReadOnlySpan<byte> waypoint32,
        int difficultyBits = DifficultyBits)
    {
        Span<byte> msg = stackalloc byte[72];
        challenge32.CopyTo(msg);
        waypoint32.CopyTo(msg.Slice(32));
        Span<byte> digest = stackalloc byte[32];
        for (ulong nonce = 0; ; nonce++)
        {
            Prims.Le64(nonce, msg.Slice(64));
            SHA256.TryHashData(msg, digest, out _);
            if (LeadingZeroBits(digest) >= difficultyBits) return nonce;
        }
    }
}
