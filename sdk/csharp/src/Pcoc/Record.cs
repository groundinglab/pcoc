namespace Pcoc;

public static class Record
{
/// §14
    public static string UnknownKind(string kindText)
    {
        return "{\"kind\":\"unknown kind\",\"kind_text\":" + JsonString(kindText) + "}";
    }

    /// §14
    public static string Num(long n) => n.ToString(System.Globalization.CultureInfo.InvariantCulture);

    /// §14
    public static string Tok(string s) => JsonString(s);

    /// §14
    public static string JsonString(string s)
    {
        var sb = new System.Text.StringBuilder(s.Length + 2);
        sb.Append('"');
        foreach (var c in s)
        {
            switch (c)
            {
                case '"': sb.Append("\\\""); break;
                case '\\': sb.Append("\\\\"); break;
                case '\b': sb.Append("\\b"); break;
                case '\f': sb.Append("\\f"); break;
                case '\n': sb.Append("\\n"); break;
                case '\r': sb.Append("\\r"); break;
                case '\t': sb.Append("\\t"); break;
                default:
                    if (c < 0x20)
                    {
                        sb.Append("\\u");
                        sb.Append(((int)c).ToString("x4"));
                    }
                    else
                    {
                        sb.Append(c);
                    }
                    break;
            }
        }
        sb.Append('"');
        return sb.ToString();
    }
}
