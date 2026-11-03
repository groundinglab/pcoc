namespace Pcoc;

public static class Br1
{
/// §13.6
    public readonly struct Tip
    {
        public byte[] Hash { get; }
        public byte[] Previous { get; }
        public ulong Bits { get; }
        public long Height { get; }
        public long Time { get; }
        public long PeriodFirstTime { get; }
        public bool Live { get; }

        public Tip(byte[] hash, byte[] previous, ulong bits, long height, long time, long periodFirstTime, bool live)
        {
            Hash = hash;
            Previous = previous;
            Bits = bits;
            Height = height;
            Time = time;
            PeriodFirstTime = periodFirstTime;
            Live = live;
        }
    }

    /// §13.6
    public static int Status(byte[] body, string nonceText, string headerHex, string origin, Tip tip)
    {
        if (body.Length != 32)
        {
            return 402;
        }
        if (!TryNonce(nonceText, out var nonce))
        {
            return 402;
        }
        if (headerHex.Length != 160 || !Bytes.TryParseHex(headerHex, out var header) || header.Length != 80)
        {
            return 402;
        }
        var headerHash = Hashcash.HeaderHash(header);
        var window = Window(header, headerHash, tip);
        if (window == null)
        {
            return 402;
        }
        var challenge = Hashcash.Challenge(origin, headerHash);
        var digest = Hashcash.Digest(challenge, body, nonce);
        if (!Hashcash.Meets(digest, Const.HashcashBits))
        {
            return 402;
        }
        return 204;
    }

    /// §13.6
    public static string? Window(byte[] header, byte[] headerHash, Tip tip)
    {
        if (Equal32(headerHash, tip.Hash))
        {
            return "current";
        }
        if (Equal32(headerHash, tip.Previous))
        {
            return "before";
        }
        var prevField = new byte[32];
        System.Buffer.BlockCopy(header, 4, prevField, 0, 32);
        if (!Equal32(prevField, tip.Hash))
        {
            return null;
        }
        var required = RequiredBits(tip);
        var feeBits = (ulong)header[72] | ((ulong)header[73] << 8) | ((ulong)header[74] << 16) | ((ulong)header[75] << 24);
        if (feeBits != required)
        {
            return null;
        }
        var pow = Sha256.Hash(Sha256.Hash(header));
        if (Le256(pow) > CompactTarget(required))
        {
            return null;
        }
        return "after";
    }

    /// §13.6
    public static ulong RequiredBits(Tip tip)
    {
        if (!tip.Live)
        {
            return Const.SynthNBits;
        }
        var next = tip.Height + 1;
        if (next % Const.RetargetSpacing != 0)
        {
            return tip.Bits;
        }
        var span = tip.Time - tip.PeriodFirstTime;
        if (span < Const.TargetTimespan / 4)
        {
            span = Const.TargetTimespan / 4;
        }
        if (span > Const.TargetTimespan * 4)
        {
            span = Const.TargetTimespan * 4;
        }
        var target = CompactTarget(tip.Bits);
        target = target * (System.Numerics.BigInteger)span / Const.TargetTimespan;
        var limit = CompactTarget(Const.PowLimitCompact);
        if (target > limit)
        {
            target = limit;
        }
        return ToCompact(target);
    }

    /// §9
    public static System.Numerics.BigInteger CompactTarget(ulong compact)
    {
        var exp = (int)(compact >> 24);
        var mant = compact & 0x007fffffUL;
        if ((compact & 0x00800000UL) != 0)
        {
            return System.Numerics.BigInteger.Zero;
        }
        if (exp <= 3)
        {
            return new System.Numerics.BigInteger(mant >> (8 * (3 - exp)));
        }
        return new System.Numerics.BigInteger(mant) << (8 * (exp - 3));
    }

    /// §9
    public static ulong ToCompact(System.Numerics.BigInteger target)
    {
        if (target.Sign <= 0)
        {
            return 0;
        }
        var size = target.GetByteCount(isUnsigned: true);
        System.Numerics.BigInteger mant;
        if (size <= 3)
        {
            mant = target << (8 * (3 - size));
        }
        else
        {
            mant = target >> (8 * (size - 3));
        }
        var m = (ulong)mant;
        if ((m & 0x00800000UL) != 0)
        {
            m >>= 8;
            size += 1;
        }
        return ((ulong)size << 24) | (m & 0x007fffffUL);
    }

    /// §9
    public static System.Numerics.BigInteger Le256(byte[] digest)
    {
        var le = new byte[33];
        System.Buffer.BlockCopy(digest, 0, le, 0, 32);
        return new System.Numerics.BigInteger(le);
    }

    private static bool TryNonce(string text, out ulong nonce)
    {
        nonce = 0;
        if (text.Length == 0)
        {
            return false;
        }
        if (text.Length > 1 && text[0] == '0')
        {
            return false;
        }
        foreach (var c in text)
        {
            if (c < '0' || c > '9')
            {
                return false;
            }
        }
        return ulong.TryParse(text, System.Globalization.NumberStyles.None, System.Globalization.CultureInfo.InvariantCulture, out nonce);
    }

    private static bool Equal32(byte[] a, byte[] b)
    {
        if (a.Length != 32 || b.Length != 32)
        {
            return false;
        }
        var d = 0;
        for (var i = 0; i < 32; i++)
        {
            d |= a[i] ^ b[i];
        }
        return d == 0;
    }
}
