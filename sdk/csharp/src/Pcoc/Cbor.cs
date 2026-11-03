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
}
