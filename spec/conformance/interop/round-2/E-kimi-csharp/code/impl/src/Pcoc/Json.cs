using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text;

namespace Pcoc;

/// <summary>A parsed JSON value, DOM for the section 9 body rules.</summary>
public abstract record JVal
{
    public sealed record Obj(List<(string Name, JVal Value)> Members) : JVal
    {
        public JVal? Find(string name)
        {
            foreach (var (n, v) in Members)
                if (n == name) return v;
            return null;
        }
    }
    public sealed record Arr(List<JVal> Items) : JVal;
    public sealed record Str(string Value) : JVal;
    /// <summary>A JSON number, kept as its exact source spelling.</summary>
    public sealed record Num(string Raw) : JVal;
    public sealed record Bool(bool Value) : JVal;
    public sealed record Null() : JVal;
}

/// <summary>
/// Section 9 body rules: one UTF-8 JSON object and no second value; a byte
/// order mark is not a successful read; bytes that are not UTF-8 are not a
/// successful read; a repeated member name in any object in the document,
/// including nested, is not a successful read. This parser accepts the JSON
/// grammar only (RFC 8259) and enforces those rules.
/// </summary>
public static class JsonDoc
{
    /// <summary>Parses the body; returns null when the read is not successful.</summary>
    public static JVal.Obj? Parse(byte[] body)
    {
        if (body.Length >= 3 && body[0] == 0xEF && body[1] == 0xBB && body[2] == 0xBF)
            return null; // BOM: not a successful read, not stripped
        string text;
        try { text = new UTF8Encoding(false, true).GetString(body); }
        catch (DecoderFallbackException) { return null; }
        var p = new P(text);
        p.Ws();
        JVal? v = p.Value(0);
        if (v is not JVal.Obj obj) return null; // one JSON object
        p.Ws();
        if (!p.End) return null; // a second value is not a successful read
        return obj;
    }

    /// <summary>
    /// Reads a JSON number as an integer in [lo, hi]. A spelling with a
    /// fractional part or an exponent is not an integer for section 9.
    /// </summary>
    public static bool TryInteger(JVal? v, long lo, long hi, out long value)
    {
        value = 0;
        if (v is not JVal.Num n) return false;
        string r = n.Raw;
        if (r.IndexOfAny(new[] { '.', 'e', 'E' }) >= 0) return false;
        if (!long.TryParse(r, NumberStyles.None, CultureInfo.InvariantCulture, out long x)) return false;
        if (x < lo || x > hi) return false;
        value = x;
        return true;
    }

    /// <summary>Reads a JSON string of exactly 64 hexadecimal digits into 32 bytes (letter case free).</summary>
    public static bool TryHex64(JVal? v, out byte[] bytes)
    {
        bytes = Array.Empty<byte>();
        if (v is not JVal.Str s || s.Value.Length != 64) return false;
        var b = new byte[32];
        for (int i = 0; i < 32; i++)
        {
            int hi = HexVal(s.Value[2 * i]);
            int lo = HexVal(s.Value[2 * i + 1]);
            if (hi < 0 || lo < 0) return false;
            b[i] = (byte)((hi << 4) | lo);
        }
        bytes = b;
        return true;
    }

    public static int HexVal(char c) =>
        c >= '0' && c <= '9' ? c - '0' :
        c >= 'a' && c <= 'f' ? c - 'a' + 10 :
        c >= 'A' && c <= 'F' ? c - 'A' + 10 : -1;

    private ref struct P
    {
        private readonly string _s;
        private int _i;
        public P(string s) { _s = s; _i = 0; }
        public bool End => _i >= _s.Length;

        public void Ws()
        {
            while (_i < _s.Length && (_s[_i] == ' ' || _s[_i] == '\t' || _s[_i] == '\n' || _s[_i] == '\r'))
                _i++;
        }

        public JVal? Value(int depth)
        {
            if (depth > 100 || End) return null;
            char c = _s[_i];
            switch (c)
            {
                case '{': return Object(depth);
                case '[': return Array(depth);
                case '"':
                    if (!String(out string? s)) return null;
                    return new JVal.Str(s!);
                case 't': return Lit("true", new JVal.Bool(true));
                case 'f': return Lit("false", new JVal.Bool(false));
                case 'n': return Lit("null", new JVal.Null());
                default:
                    if (c == '-' || (c >= '0' && c <= '9')) return Number();
                    return null;
            }
        }

        private JVal? Lit(string w, JVal v)
        {
            if (_i + w.Length > _s.Length) return null;
            if (string.CompareOrdinal(_s, _i, w, 0, w.Length) != 0) return null;
            _i += w.Length;
            return v;
        }

        private JVal? Number()
        {
            int start = _i;
            if (!End && _s[_i] == '-') _i++;
            if (End) return null;
            if (_s[_i] == '0') { _i++; }
            else if (_s[_i] >= '1' && _s[_i] <= '9') { while (!End && _s[_i] >= '0' && _s[_i] <= '9') _i++; }
            else return null;
            if (!End && _s[_i] == '.')
            {
                _i++;
                if (End || _s[_i] < '0' || _s[_i] > '9') return null;
                while (!End && _s[_i] >= '0' && _s[_i] <= '9') _i++;
            }
            if (!End && (_s[_i] == 'e' || _s[_i] == 'E'))
            {
                _i++;
                if (!End && (_s[_i] == '+' || _s[_i] == '-')) _i++;
                if (End || _s[_i] < '0' || _s[_i] > '9') return null;
                while (!End && _s[_i] >= '0' && _s[_i] <= '9') _i++;
            }
            return new JVal.Num(_s.Substring(start, _i - start));
        }

        private bool String(out string? result)
        {
            result = null;
            _i++; // opening quote
            var sb = new StringBuilder();
            while (true)
            {
                if (End) return false;
                char c = _s[_i++];
                if (c == '"') { result = sb.ToString(); return true; }
                if (c == '\\')
                {
                    if (End) return false;
                    char e = _s[_i++];
                    switch (e)
                    {
                        case '"': sb.Append('"'); break;
                        case '\\': sb.Append('\\'); break;
                        case '/': sb.Append('/'); break;
                        case 'b': sb.Append('\b'); break;
                        case 'f': sb.Append('\f'); break;
                        case 'n': sb.Append('\n'); break;
                        case 'r': sb.Append('\r'); break;
                        case 't': sb.Append('\t'); break;
                        case 'u':
                            if (!Hex4(out char u1)) return false;
                            if (u1 >= 0xD800 && u1 <= 0xDBFF)
                            {
                                // high surrogate must be followed by 〈 low surrogate
                                if (_i + 1 >= _s.Length || _s[_i] != '\\' || _s[_i + 1] != 'u') return false;
                                _i += 2;
                                if (!Hex4(out char u2)) return false;
                                if (u2 < 0xDC00 || u2 > 0xDFFF) return false;
                                sb.Append(u1); sb.Append(u2);
                            }
                            else if (u1 >= 0xDC00 && u1 <= 0xDFFF)
                            {
                                return false; // lone low surrogate
                            }
                            else sb.Append(u1);
                            break;
                        default: return false;
                    }
                }
                else
                {
                    if (c < 0x20) return false; // raw control in a string
                    sb.Append(c);
                }
            }
        }

        private bool Hex4(out char c)
        {
            c = '\0';
            if (_i + 4 > _s.Length) return false;
            int v = 0;
            for (int k = 0; k < 4; k++)
            {
                int h = HexVal(_s[_i + k]);
                if (h < 0) return false;
                v = (v << 4) | h;
            }
            _i += 4;
            c = (char)v;
            return true;
        }

        private JVal? Object(int depth)
        {
            _i++; // {
            var members = new List<(string, JVal)>();
            var seen = new HashSet<string>(StringComparer.Ordinal);
            Ws();
            if (!End && _s[_i] == '}') { _i++; return new JVal.Obj(members); }
            while (true)
            {
                Ws();
                if (End || _s[_i] != '"') return null;
                if (!String(out string? name)) return null;
                if (!seen.Add(name!)) return null; // duplicate member name: not a successful read
                Ws();
                if (End || _s[_i] != ':') return null;
                _i++;
                Ws();
                JVal? v = Value(depth + 1);
                if (v is null) return null;
                members.Add((name!, v));
                Ws();
                if (End) return null;
                if (_s[_i] == ',') { _i++; continue; }
                if (_s[_i] == '}') { _i++; return new JVal.Obj(members); }
                return null;
            }
        }

        private JVal? Array(int depth)
        {
            _i++; // [
            var items = new List<JVal>();
            Ws();
            if (!End && _s[_i] == ']') { _i++; return new JVal.Arr(items); }
            while (true)
            {
                Ws();
                JVal? v = Value(depth + 1);
                if (v is null) return null;
                items.Add(v);
                Ws();
                if (End) return null;
                if (_s[_i] == ',') { _i++; continue; }
                if (_s[_i] == ']') { _i++; return new JVal.Arr(items); }
                return null;
            }
        }
    }
}

/// <summary>Compact JSON writer for the section 14 record: no insignificant whitespace.</summary>
public sealed class JsonWriter
{
    private readonly StringBuilder _sb = new();
    private bool _first = true;

    public override string ToString() => _sb.ToString();

    private void Sep()
    {
        if (_first) _first = false;
        else _sb.Append(',');
    }

    public JsonWriter BeginObject()
    {
        Sep();
        _sb.Append('{');
        _first = true;
        return this;
    }

    public JsonWriter EndObject()
    {
        _sb.Append('}');
        _first = false;
        return this;
    }

    public JsonWriter BeginArray()
    {
        Sep();
        _sb.Append('[');
        _first = true;
        return this;
    }

    public JsonWriter EndArray()
    {
        _sb.Append(']');
        _first = false;
        return this;
    }

    public JsonWriter Key(string name)
    {
        Sep();
        Rfc8785.WriteString(_sb, name);
        _sb.Append(':');
        _first = true;
        return this;
    }

    public JsonWriter Str(string value)
    {
        Sep();
        Rfc8785.WriteString(_sb, value);
        return this;
    }

    public JsonWriter Num(long value)
    {
        Sep();
        _sb.Append(value.ToString(CultureInfo.InvariantCulture));
        return this;
    }

    public JsonWriter Num(ulong value)
    {
        Sep();
        _sb.Append(value.ToString(CultureInfo.InvariantCulture));
        return this;
    }

    /// <summary>Writes the token as a JSON string (all section 4 tokens are plain strings).</summary>
    public JsonWriter Token(string token) => Str(token);
}
