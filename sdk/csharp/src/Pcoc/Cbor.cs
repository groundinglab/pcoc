namespace Pcoc;

public static class Cbor
{
/// §7.3
    public abstract class Value
    {
    }

    /// §7.3
    public sealed class Unsigned : Value
    {
        public ulong N { get; }
        public Unsigned(ulong n) { N = n; }
    }

    /// §7.3
    public sealed class BytesVal : Value
    {
        public byte[] B { get; }
        public BytesVal(byte[] b) { B = b; }
    }

    /// §7.3
    public sealed class Text : Value
    {
        public string S { get; }
        public Text(string s) { S = s; }
    }

    /// §7.3
    public sealed class ArrayVal : Value
    {
        public Value[] Items { get; }
        public ArrayVal(Value[] items) { Items = items; }
    }

    /// §7.3
    public sealed class MapVal : Value
    {
        public (string Key, Value Value)[] Pairs { get; }
        public MapVal((string, Value)[] pairs) { Pairs = pairs; }

        public bool TryGet(string key, out Value value)
        {
            foreach (var (k, v) in Pairs)
            {
                if (k == key)
                {
                    value = v;
                    return true;
                }
            }
            value = new Unsigned(0);
            return false;
        }
    }

    /// §7.3
    public static bool TryDecodeOne(byte[] input, out Value value)
    {
        value = new Unsigned(0);
        if (input.Length == 0)
        {
            return false;
        }
        try
        {
            var i = 0;
            value = Read(input, ref i);
            return i == input.Length;
        }
        catch (System.FormatException)
        {
            return false;
        }
    }

    private static Value Read(byte[] buf, ref int i)
    {
        if (i >= buf.Length)
        {
            throw new System.FormatException();
        }
        var b = buf[i++];
        var major = b >> 5;
        var add = b & 0x1F;
        if (add == 31)
        {
            throw new System.FormatException();
        }
        var n = ReadLen(buf, ref i, add);
        switch (major)
        {
            case 0:
                return new Unsigned(n);
            case 1:
                throw new System.FormatException();
            case 2:
                return new BytesVal(Take(buf, ref i, n));
            case 3:
                var raw = Take(buf, ref i, n);
                if (!IsUtf8(raw))
                {
                    throw new System.FormatException();
                }
                return new Text(System.Text.Encoding.UTF8.GetString(raw));
            case 4:
                if (n > int.MaxValue)
                {
                    throw new System.FormatException();
                }
                var items = new Value[n];
                for (ulong k = 0; k < n; k++)
                {
                    items[k] = Read(buf, ref i);
                }
                return new ArrayVal(items);
            case 5:
                if (n > int.MaxValue)
                {
                    throw new System.FormatException();
                }
                var pairs = new (string, Value)[n];
                var seen = new System.Collections.Generic.HashSet<string>();
                for (ulong k = 0; k < n; k++)
                {
                    var key = Read(buf, ref i);
                    if (key is not Text text)
                    {
                        throw new System.FormatException();
                    }
                    if (!seen.Add(text.S))
                    {
                        throw new System.FormatException();
                    }
                    pairs[k] = (text.S, Read(buf, ref i));
                }
                return new MapVal(pairs);
            default:
                throw new System.FormatException();
        }
    }

    private static ulong ReadLen(byte[] buf, ref int i, int add)
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
            _ => throw new System.FormatException(),
        };
        if (i + width > buf.Length)
        {
            throw new System.FormatException();
        }
        ulong n = 0;
        for (var k = 0; k < width; k++)
        {
            n = (n << 8) | buf[i++];
        }
        return n;
    }

    private static byte[] Take(byte[] buf, ref int i, ulong n)
    {
        if (n > (ulong)(buf.Length - i))
        {
            throw new System.FormatException();
        }
        var len = (int)n;
        var d = new byte[len];
        System.Buffer.BlockCopy(buf, i, d, 0, len);
        i += len;
        return d;
    }

    private static bool IsUtf8(byte[] raw)
    {
        var i = 0;
        while (i < raw.Length)
        {
            var b = raw[i];
            int need;
            uint cp;
            if (b <= 0x7F)
            {
                i++;
                continue;
            }
            if (b >= 0xC2 && b <= 0xDF)
            {
                need = 1;
                cp = (uint)(b & 0x1F);
            }
            else if (b >= 0xE0 && b <= 0xEF)
            {
                need = 2;
                cp = (uint)(b & 0x0F);
            }
            else if (b >= 0xF0 && b <= 0xF4)
            {
                need = 3;
                cp = (uint)(b & 0x07);
            }
            else
            {
                return false;
            }
            if (i + need >= raw.Length)
            {
                return false;
            }
            for (var k = 1; k <= need; k++)
            {
                var c = raw[i + k];
                if ((c & 0xC0) != 0x80)
                {
                    return false;
                }
                cp = (cp << 6) | (uint)(c & 0x3F);
            }
            if (need == 2 && cp < 0x800)
            {
                return false;
            }
            if (need == 3 && cp < 0x10000)
            {
                return false;
            }
            if (cp >= 0xD800 && cp <= 0xDFFF)
            {
                return false;
            }
            if (cp > 0x10FFFF)
            {
                return false;
            }
            i += need + 1;
        }
        return true;
    }

    /// §7.3
    public static byte[] EncodeUint(ulong n)
    {
        return Head(0, n);
    }

    /// §7.3
    public static byte[] EncodeBytes(byte[] b)
    {
        var head = Head(2, (ulong)b.Length);
        var d = new byte[head.Length + b.Length];
        System.Buffer.BlockCopy(head, 0, d, 0, head.Length);
        System.Buffer.BlockCopy(b, 0, d, head.Length, b.Length);
        return d;
    }

    /// §7.3
    public static byte[] EncodeText(string s)
    {
        var raw = System.Text.Encoding.UTF8.GetBytes(s);
        var head = Head(3, (ulong)raw.Length);
        var d = new byte[head.Length + raw.Length];
        System.Buffer.BlockCopy(head, 0, d, 0, head.Length);
        System.Buffer.BlockCopy(raw, 0, d, head.Length, raw.Length);
        return d;
    }

    /// §7.3
    public static byte[] EncodeArray(byte[][] items)
    {
        var n = 0;
        foreach (var it in items) n += it.Length;
        var head = Head(4, (ulong)items.Length);
        var d = new byte[head.Length + n];
        System.Buffer.BlockCopy(head, 0, d, 0, head.Length);
        var o = head.Length;
        foreach (var it in items)
        {
            System.Buffer.BlockCopy(it, 0, d, o, it.Length);
            o += it.Length;
        }
        return d;
    }

    /// §7.3
    public static byte[] EncodeMap((string Key, byte[] Value)[] pairs)
    {
        var enc = new (byte[] K, byte[] V)[pairs.Length];
        for (var i = 0; i < pairs.Length; i++)
        {
            enc[i] = (EncodeText(pairs[i].Key), pairs[i].Value);
        }
        System.Array.Sort(enc, (a, b) => CompareBytes(a.K, b.K));
        var n = 0;
        foreach (var (k, v) in enc) n += k.Length + v.Length;
        var head = Head(5, (ulong)enc.Length);
        var d = new byte[head.Length + n];
        System.Buffer.BlockCopy(head, 0, d, 0, head.Length);
        var o = head.Length;
        foreach (var (k, v) in enc)
        {
            System.Buffer.BlockCopy(k, 0, d, o, k.Length);
            o += k.Length;
            System.Buffer.BlockCopy(v, 0, d, o, v.Length);
            o += v.Length;
        }
        return d;
    }

    private static int CompareBytes(byte[] a, byte[] b)
    {
        var n = a.Length < b.Length ? a.Length : b.Length;
        for (var i = 0; i < n; i++)
        {
            if (a[i] != b[i])
            {
                return a[i] - b[i];
            }
        }
        return a.Length - b.Length;
    }

    private static byte[] Head(int major, ulong n)
    {
        var m = (byte)(major << 5);
        if (n < 24)
        {
            return new[] { (byte)(m | (byte)n) };
        }
        if (n <= 0xff)
        {
            return new[] { (byte)(m | 24), (byte)n };
        }
        if (n <= 0xffff)
        {
            return new[] { (byte)(m | 25), (byte)(n >> 8), (byte)n };
        }
        if (n <= 0xffffffffUL)
        {
            return new[]
            {
                (byte)(m | 26),
                (byte)(n >> 24),
                (byte)(n >> 16),
                (byte)(n >> 8),
                (byte)n
            };
        }
        return new[]
        {
            (byte)(m | 27),
            (byte)(n >> 56),
            (byte)(n >> 48),
            (byte)(n >> 40),
            (byte)(n >> 32),
            (byte)(n >> 24),
            (byte)(n >> 16),
            (byte)(n >> 8),
            (byte)n
        };
    }
}
