using System;
using System.Collections.Generic;
using System.Text;

namespace Pcoc;

/// <summary>A decoded CBOR value, limited to the types the hike object uses.</summary>
public abstract record CborValue
{
    public sealed record Uint(ulong Value) : CborValue;
    public sealed record Bstr(byte[] Value) : CborValue;
    public sealed record Text(string Value) : CborValue;
    public sealed record Array(List<CborValue> Items) : CborValue;
    public sealed record Map(List<(string Key, CborValue Value)> Entries) : CborValue
    {
        public CborValue? Find(string key)
        {
            foreach (var (k, v) in Entries)
                if (k == key) return v;
            return null;
        }
    }
}

/// <summary>SPEC 7.3 produce: core deterministic encoding (definite, shortest, sorted).</summary>
public static class CborWriter
{
    public static byte[] Encode(CborValue value)
    {
        var buf = new List<byte>(1024);
        Write(buf, value);
        return buf.ToArray();
    }

    private static void Write(List<byte> buf, CborValue value)
    {
        switch (value)
        {
            case CborValue.Uint u: WriteUint(buf, u.Value); break;
            case CborValue.Bstr b: WriteHead(buf, 2, (ulong)b.Value.Length); buf.AddRange(b.Value); break;
            case CborValue.Text t:
                byte[] utf = Encoding.UTF8.GetBytes(t.Value);
                WriteHead(buf, 3, (ulong)utf.Length);
                buf.AddRange(utf);
                break;
            case CborValue.Array a:
                WriteHead(buf, 4, (ulong)a.Items.Count);
                foreach (var item in a.Items) Write(buf, item);
                break;
            case CborValue.Map m:
                // Core deterministic encoding: keys sorted by bytewise lexicographic
                // order of their definite-length encodings.
                var sorted = new List<(byte[] Enc, CborValue Val)>(m.Entries.Count);
                foreach (var (k, v) in m.Entries)
                {
                    byte[] keyUtf = Encoding.UTF8.GetBytes(k);
                    var enc = new List<byte>(keyUtf.Length + 9);
                    WriteHead(enc, 3, (ulong)keyUtf.Length);
                    enc.AddRange(keyUtf);
                    sorted.Add((enc.ToArray(), v));
                }
                sorted.Sort((x, y) => Lexicographic(x.Enc, y.Enc));
                WriteHead(buf, 5, (ulong)sorted.Count);
                foreach (var (enc, v) in sorted)
                {
                    buf.AddRange(enc);
                    Write(buf, v);
                }
                break;
        }
    }

    private static int Lexicographic(byte[] a, byte[] b)
    {
        int n = Math.Min(a.Length, b.Length);
        for (int i = 0; i < n; i++)
        {
            if (a[i] != b[i]) return a[i].CompareTo(b[i]);
        }
        return a.Length.CompareTo(b.Length);
    }

    private static void WriteUint(List<byte> buf, ulong v) => WriteHead(buf, 0, v);

    private static void WriteHead(List<byte> buf, int major, ulong v)
    {
        int m = major << 5;
        if (v < 24) buf.Add((byte)(m | (int)v));
        else if (v <= 0xFF) { buf.Add((byte)(m | 24)); buf.Add((byte)v); }
        else if (v <= 0xFFFF) { buf.Add((byte)(m | 25)); buf.Add((byte)(v >> 8)); buf.Add((byte)v); }
        else if (v <= 0xFFFFFFFF)
        {
            buf.Add((byte)(m | 26));
            buf.Add((byte)(v >> 24)); buf.Add((byte)(v >> 16)); buf.Add((byte)(v >> 8)); buf.Add((byte)v);
        }
        else
        {
            buf.Add((byte)(m | 27));
            for (int s = 56; s >= 0; s -= 8) buf.Add((byte)(v >> s));
        }
    }
}

/// <summary>
/// SPEC 7.3 verification: decodes one definite-length CBOR data item.
/// Indefinite lengths, tags, floating-point values, simple values, negative
/// integers, a non-text map key, a duplicate key, trailing bytes, reserved
/// additional information, and invalid UTF-8 in a text string are a malformed
/// CBOR value. Non-shortest definite encodings decode. Key order is free.
/// </summary>
public static class CborReader
{
    public enum Status { WellFormed, Malformed }

    private const int MaxDepth = 64; // the hike object nests 4 deep; far deeper is not a hike object

    public static Status Decode(ReadOnlySpan<byte> input, out CborValue? value)
    {
        value = null;
        var reader = new Reader(input);
        if (!TryRead(ref reader, 0, out value)) return Status.Malformed;
        if (reader.Pos != input.Length) return Status.Malformed; // any byte after the one data item
        return Status.WellFormed;
    }

    private ref struct Reader
    {
        private readonly ReadOnlySpan<byte> _buf;
        public int Pos;
        public Reader(ReadOnlySpan<byte> buf) { _buf = buf; Pos = 0; }

        public bool Take(int n, out ReadOnlySpan<byte> s)
        {
            if (n < 0 || _buf.Length - Pos < n) { s = default; return false; }
            s = _buf.Slice(Pos, n);
            Pos += n;
            return true;
        }

        public bool Byte(out int b)
        {
            if (Pos >= _buf.Length) { b = 0; return false; }
            b = _buf[Pos++];
            return true;
        }
    }

    private static bool TryRead(ref Reader r, int depth, out CborValue? value)
    {
        value = null;
        if (depth > MaxDepth) return false;
        if (!r.Byte(out int first)) return false;
        int major = first >> 5;
        int ai = first & 31;
        switch (major)
        {
            case 0: // unsigned integer; a non-shortest encoding decodes
            {
                if (!TryArg(ref r, ai, out ulong v)) return false;
                value = new CborValue.Uint(v);
                return true;
            }
            case 2: // definite byte string
            {
                if (!TryArg(ref r, ai, out ulong len)) return false;
                if (len > int.MaxValue) return false;
                if (!r.Take((int)len, out var s)) return false;
                value = new CborValue.Bstr(s.ToArray());
                return true;
            }
            case 3: // definite text string, must be valid UTF-8
            {
                if (!TryArg(ref r, ai, out ulong len)) return false;
                if (len > int.MaxValue) return false;
                if (!r.Take((int)len, out var s)) return false;
                string text;
                try { text = new UTF8Encoding(false, true).GetString(s); }
                catch (DecoderFallbackException) { return false; }
                value = new CborValue.Text(text);
                return true;
            }
            case 4: // definite array
            {
                if (!TryArg(ref r, ai, out ulong len)) return false;
                if (len > int.MaxValue) return false;
                var items = new List<CborValue>((int)Math.Min(len, 1024));
                for (ulong k = 0; k < len; k++)
                {
                    if (!TryRead(ref r, depth + 1, out var item)) return false;
                    items.Add(item!);
                }
                value = new CborValue.Array(items);
                return true;
            }
            case 5: // definite map; text keys only; no duplicate keys
            {
                if (!TryArg(ref r, ai, out ulong len)) return false;
                if (len > int.MaxValue) return false;
                var entries = new List<(string, CborValue)>((int)Math.Min(len, 64));
                var seen = new HashSet<string>(StringComparer.Ordinal);
                for (ulong k = 0; k < len; k++)
                {
                    if (!TryRead(ref r, depth + 1, out var key)) return false;
                    if (key is not CborValue.Text kt) return false;
                    if (!seen.Add(kt.Value)) return false; // duplicate key
                    if (!TryRead(ref r, depth + 1, out var val)) return false;
                    entries.Add((kt.Value, val!));
                }
                value = new CborValue.Map(entries);
                return true;
            }
            default:
                // major 1 (negative), 6 (tag), 7 (simple/float), and break are malformed here.
                return false;
        }
    }

    /// <summary>Reads the argument of a head. Additional info 28–31 is not definite-length.</summary>
    private static bool TryArg(ref Reader r, int ai, out ulong v)
    {
        v = 0;
        if (ai < 24) { v = (ulong)ai; return true; }
        int n = ai switch { 24 => 1, 25 => 2, 26 => 4, 27 => 8, _ => -1 };
        if (n < 0) return false; // 28, 29, 30 reserved; 31 indefinite
        if (!r.Take(n, out var s)) return false;
        foreach (byte b in s) v = (v << 8) | b;
        return true;
    }
}
