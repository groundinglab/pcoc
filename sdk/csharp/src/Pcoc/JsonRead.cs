namespace Pcoc;

public static class JsonRead
{
/// §9
    public static IndexDoc? Parse(byte[] body)
    {
        if (body.Length >= 3 && body[0] == 0xEF && body[1] == 0xBB && body[2] == 0xBF)
        {
            return null;
        }
        string text;
        try
        {
            text = System.Text.Encoding.UTF8.GetString(body);
        }
        catch (System.ArgumentException)
        {
            return null;
        }
        if (!IsUtf8(body))
        {
            return null;
        }
        try
        {
            var i = 0;
            Skip(text, ref i);
            if (i >= text.Length || text[i] != '{')
            {
                return null;
            }
            var obj = ReadObject(text, ref i, out var dup);
            Skip(text, ref i);
            if (dup || i != text.Length || obj == null)
            {
                return null;
            }
            return ToDoc(obj);
        }
        catch (System.FormatException)
        {
            return null;
        }
    }

    private static IndexDoc ToDoc(System.Collections.Generic.Dictionary<string, object?> obj)
    {
        var doc = new IndexDoc
        {
            Document = true,
            Network = obj.TryGetValue("network", out var net) && net is string ns ? ns : null,
        };
        if (obj.TryGetValue("header", out var header) && header is System.Collections.Generic.Dictionary<string, object?> hm)
        {
            if (hm.TryGetValue("hash", out var hash) && hash is string hs
                && hm.TryGetValue("height", out var height) && height is long hn
                && hs.Length == 64 && Bytes.TryParseHex(hs, out var display) && display.Length == 32
                && hn >= 0 && hn <= ((1L << 53) - 1))
            {
                doc = new IndexDoc
                {
                    Document = true,
                    HeaderOk = true,
                    NaturalHash = Bytes.Reverse32(display),
                    Height = hn,
                    TimeOk = hm.TryGetValue("time", out var time) && time is long tn && tn >= 0 && tn <= uint.MaxValue,
                    Time = hm.TryGetValue("time", out var t2) && t2 is long tv ? tv : 0,
                    Network = doc.Network,
                    Plants = doc.Plants,
                };
            }
        }
        var plants = new System.Collections.Generic.List<IndexDoc.PlantFact>();
        if (obj.TryGetValue("plants", out var parr) && parr is System.Collections.Generic.List<object?> list)
        {
            foreach (var item in list)
            {
                if (item is System.Collections.Generic.Dictionary<string, object?> pm)
                {
                    var fact = ReadPlant(pm);
                    if (fact.Ok)
                    {
                        plants.Add(fact);
                    }
                    else
                    {
                        plants.Add(fact);
                    }
                }
            }
        }
        else if (obj.TryGetValue("plant", out var one) && one is System.Collections.Generic.Dictionary<string, object?> pm)
        {
            plants.Add(ReadPlant(pm));
        }
        return new IndexDoc
        {
            Document = true,
            HeaderOk = doc.HeaderOk,
            NaturalHash = doc.NaturalHash,
            Height = doc.Height,
            TimeOk = doc.TimeOk,
            Time = doc.Time,
            Network = doc.Network,
            Plants = plants,
        };
    }

    private static IndexDoc.PlantFact ReadPlant(System.Collections.Generic.Dictionary<string, object?> pm)
    {
        var ok = pm.TryGetValue("root", out var root) && root is string rs && rs.Length == 64 && Bytes.TryParseHex(rs, out var rootb) && rootb.Length == 32
            && pm.TryGetValue("txid", out var tx) && tx is string ts && ts.Length == 64 && Bytes.TryParseHex(ts, out var txd) && txd.Length == 32
            && pm.TryGetValue("block_hash", out var bh) && bh is string bhs && bhs.Length == 64 && Bytes.TryParseHex(bhs, out var bhd) && bhd.Length == 32
            && pm.TryGetValue("vout", out var vo) && vo is long vout && vout >= 0 && vout <= uint.MaxValue
            && pm.TryGetValue("block_height", out var bhe) && bhe is long height && height >= 0 && height <= uint.MaxValue;
        if (!ok)
        {
            return new IndexDoc.PlantFact(System.Array.Empty<byte>(), System.Array.Empty<byte>(), 0, System.Array.Empty<byte>(), 0, false);
        }
        Bytes.TryParseHex((string)pm["root"]!, out var r);
        Bytes.TryParseHex((string)pm["txid"]!, out var t);
        Bytes.TryParseHex((string)pm["block_hash"]!, out var b);
        return new IndexDoc.PlantFact(r, Bytes.Reverse32(t), (long)pm["vout"]!, Bytes.Reverse32(b), (long)pm["block_height"]!, true);
    }

    private static System.Collections.Generic.Dictionary<string, object?>? ReadObject(string text, ref int i, out bool dup)
    {
        dup = false;
        if (text[i] != '{')
        {
            throw new System.FormatException();
        }
        i++;
        var map = new System.Collections.Generic.Dictionary<string, object?>();
        Skip(text, ref i);
        if (i < text.Length && text[i] == '}')
        {
            i++;
            return map;
        }
        while (true)
        {
            Skip(text, ref i);
            var key = ReadString(text, ref i);
            Skip(text, ref i);
            if (i >= text.Length || text[i] != ':')
            {
                throw new System.FormatException();
            }
            i++;
            var value = ReadValue(text, ref i, ref dup);
            if (map.ContainsKey(key))
            {
                dup = true;
            }
            map[key] = value;
            Skip(text, ref i);
            if (i >= text.Length)
            {
                throw new System.FormatException();
            }
            if (text[i] == ',')
            {
                i++;
                continue;
            }
            if (text[i] == '}')
            {
                i++;
                return map;
            }
            throw new System.FormatException();
        }
    }

    private static object? ReadValue(string text, ref int i, ref bool dup)
    {
        Skip(text, ref i);
        if (i >= text.Length)
        {
            throw new System.FormatException();
        }
        var c = text[i];
        if (c == '"')
        {
            return ReadString(text, ref i);
        }
        if (c == '{')
        {
            var nested = ReadObject(text, ref i, out var inner);
            if (inner)
            {
                dup = true;
            }
            return nested;
        }
        if (c == '[')
        {
            return ReadArray(text, ref i, ref dup);
        }
        if (c == 'n' && Match(text, i, "null"))
        {
            i += 4;
            return null;
        }
        if (c == 't' && Match(text, i, "true"))
        {
            i += 4;
            return true;
        }
        if (c == 'f' && Match(text, i, "false"))
        {
            i += 5;
            return false;
        }
        return ReadNumber(text, ref i);
    }

    private static System.Collections.Generic.List<object?> ReadArray(string text, ref int i, ref bool dup)
    {
        i++;
        var list = new System.Collections.Generic.List<object?>();
        Skip(text, ref i);
        if (i < text.Length && text[i] == ']')
        {
            i++;
            return list;
        }
        while (true)
        {
            list.Add(ReadValue(text, ref i, ref dup));
            Skip(text, ref i);
            if (i >= text.Length)
            {
                throw new System.FormatException();
            }
            if (text[i] == ',')
            {
                i++;
                continue;
            }
            if (text[i] == ']')
            {
                i++;
                return list;
            }
            throw new System.FormatException();
        }
    }

    private static object ReadNumber(string text, ref int i)
    {
        var start = i;
        if (text[i] == '-')
        {
            i++;
        }
        if (i >= text.Length || text[i] < '0' || text[i] > '9')
        {
            throw new System.FormatException();
        }
        if (text[i] == '0')
        {
            i++;
        }
        else
        {
            while (i < text.Length && text[i] >= '0' && text[i] <= '9')
            {
                i++;
            }
        }
        var frac = false;
        if (i < text.Length && (text[i] == '.' || text[i] == 'e' || text[i] == 'E'))
        {
            frac = true;
            i++;
            while (i < text.Length && ((text[i] >= '0' && text[i] <= '9') || text[i] == '+' || text[i] == '-' || text[i] == 'e' || text[i] == 'E'))
            {
                i++;
            }
        }
        var s = text.Substring(start, i - start);
        if (frac)
        {
            return s;
        }
        if (long.TryParse(s, System.Globalization.NumberStyles.AllowLeadingSign, System.Globalization.CultureInfo.InvariantCulture, out var n))
        {
            return n;
        }
        return s;
    }

    private static string ReadString(string text, ref int i)
    {
        if (text[i] != '"')
        {
            throw new System.FormatException();
        }
        i++;
        var sb = new System.Text.StringBuilder();
        while (i < text.Length)
        {
            var c = text[i++];
            if (c == '"')
            {
                return sb.ToString();
            }
            if (c == '\\')
            {
                if (i >= text.Length)
                {
                    throw new System.FormatException();
                }
                var e = text[i++];
                switch (e)
                {
                    case '"':
                    case '\\':
                    case '/':
                        sb.Append(e);
                        break;
                    case 'b': sb.Append('\b'); break;
                    case 'f': sb.Append('\f'); break;
                    case 'n': sb.Append('\n'); break;
                    case 'r': sb.Append('\r'); break;
                    case 't': sb.Append('\t'); break;
                    case 'u':
                        if (i + 4 > text.Length)
                        {
                            throw new System.FormatException();
                        }
                        var hex = text.Substring(i, 4);
                        i += 4;
                        sb.Append((char)int.Parse(hex, System.Globalization.NumberStyles.HexNumber, System.Globalization.CultureInfo.InvariantCulture));
                        break;
                    default:
                        throw new System.FormatException();
                }
            }
            else if (c < 0x20)
            {
                throw new System.FormatException();
            }
            else
            {
                sb.Append(c);
            }
        }
        throw new System.FormatException();
    }

    private static void Skip(string text, ref int i)
    {
        while (i < text.Length && (text[i] == ' ' || text[i] == '\n' || text[i] == '\r' || text[i] == '\t'))
        {
            i++;
        }
    }

    private static bool Match(string text, int i, string word)
    {
        return i + word.Length <= text.Length && text.Substring(i, word.Length) == word;
    }

    private static bool IsUtf8(byte[] raw)
    {
        try
        {
            var utf8 = new System.Text.UTF8Encoding(false, true);
            utf8.GetString(raw);
            return true;
        }
        catch (System.ArgumentException)
        {
            return false;
        }
    }
}
