namespace Pcoc;

public static class Hashcash
{
/// §13.1
    public static string Origin(string url)
    {
        var s = url.Trim();
        var schemeEnd = s.IndexOf("://", System.StringComparison.Ordinal);
        if (schemeEnd < 0)
        {
            return s;
        }
        var scheme = s.Substring(0, schemeEnd).ToLowerInvariant();
        var rest = s.Substring(schemeEnd + 3);
        var cut = rest.IndexOfAny(new[] { '/', '?', '#' });
        var hostport = cut < 0 ? rest : rest.Substring(0, cut);
        var at = hostport.LastIndexOf('@');
        if (at >= 0)
        {
            hostport = hostport.Substring(at + 1);
        }
        var host = hostport;
        string? port = null;
        if (hostport.StartsWith('['))
        {
            var rb = hostport.IndexOf(']');
            host = rb < 0 ? hostport : hostport.Substring(0, rb + 1);
            if (rb >= 0 && rb + 1 < hostport.Length && hostport[rb + 1] == ':')
            {
                port = hostport.Substring(rb + 2);
            }
        }
        else
        {
            var colon = hostport.LastIndexOf(':');
            if (colon >= 0 && hostport.IndexOf(':') == colon)
            {
                host = hostport.Substring(0, colon);
                port = hostport.Substring(colon + 1);
            }
        }
        host = host.ToLowerInvariant();
        var def = scheme == "http" ? "80" : scheme == "https" ? "443" : null;
        var withPort = port != null && port.Length > 0 && port != def;
        return withPort ? scheme + "://" + host + ":" + port : scheme + "://" + host;
    }

    /// §13.1
    public static byte[] HeaderHash(byte[] header80)
    {
        return Sha256.Hash(Sha256.Hash(header80));
    }

    /// §13.1
    public static byte[] Challenge(string bariOrigin, byte[] headerHash)
    {
        var origin = System.Text.Encoding.ASCII.GetBytes(bariOrigin);
        var pre = new byte[Const.HashcashPrefix.Length + origin.Length + 32];
        System.Buffer.BlockCopy(Const.HashcashPrefix, 0, pre, 0, Const.HashcashPrefix.Length);
        System.Buffer.BlockCopy(origin, 0, pre, Const.HashcashPrefix.Length, origin.Length);
        System.Buffer.BlockCopy(headerHash, 0, pre, Const.HashcashPrefix.Length + origin.Length, 32);
        return Sha256.Hash(pre);
    }

    /// §13.1
    public static byte[] Digest(byte[] challenge, byte[] waypoint, ulong nonce)
    {
        var pre = new byte[32 + 32 + 8];
        System.Buffer.BlockCopy(challenge, 0, pre, 0, 32);
        System.Buffer.BlockCopy(waypoint, 0, pre, 32, 32);
        Bytes.WriteLe64(pre, 64, nonce);
        return Sha256.Hash(pre);
    }

    /// §13.1
    public static int LeadingZeroBits(byte[] digest)
    {
        var n = 0;
        foreach (var b in digest)
        {
            if (b == 0)
            {
                n += 8;
                continue;
            }
            var v = b;
            for (var bit = 7; bit >= 0; bit--)
            {
                if (((v >> bit) & 1) == 0)
                {
                    n++;
                }
                else
                {
                    return n;
                }
            }
        }
        return n;
    }

    /// §13.1
    public static bool Meets(byte[] digest, int bits) => LeadingZeroBits(digest) >= bits;

    /// §13.1
    public static ulong? Solve(byte[] challenge, byte[] waypoint, int bits)
    {
        for (ulong nonce = 0; nonce < ulong.MaxValue; nonce++)
        {
            if (Meets(Digest(challenge, waypoint, nonce), bits))
            {
                return nonce;
            }
        }
        if (Meets(Digest(challenge, waypoint, ulong.MaxValue), bits))
        {
            return ulong.MaxValue;
        }
        return null;
    }
}
