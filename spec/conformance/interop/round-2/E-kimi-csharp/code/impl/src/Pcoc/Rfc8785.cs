using System;
using System.Text;

namespace Pcoc;

/// <summary>
/// RFC 8785 (JSON Canonicalization Scheme) string spelling, used by Rule A5
/// for the unknown-kind record. Escapes exactly the set RFC 8785 section
/// 3.2.2.2 names: the two-character escapes for U+0008, U+0009, U+000A,
/// U+000C, U+000D, U+0022 and U+005C, and \u00xx (lowercase hex) for the
/// remaining C0 controls. Every other code point passes through.
/// </summary>
public static class Rfc8785
{
    public static void WriteString(StringBuilder sb, string s)
    {
        sb.Append('"');
        foreach (char ch in s)
        {
            switch (ch)
            {
                case '"': sb.Append("\\\""); break;
                case '\\': sb.Append("\\\\"); break;
                case '\b': sb.Append("\\b"); break;
                case '\t': sb.Append("\\t"); break;
                case '\n': sb.Append("\\n"); break;
                case '\f': sb.Append("\\f"); break;
                case '\r': sb.Append("\\r"); break;
                default:
                    if (ch < 0x20)
                    {
                        sb.Append("\\u00");
                        sb.Append(HexLower(ch >> 4));
                        sb.Append(HexLower(ch & 0xF));
                    }
                    else
                    {
                        sb.Append(ch);
                    }
                    break;
            }
        }
        sb.Append('"');
    }

    public static string String(string s)
    {
        var sb = new StringBuilder(s.Length + 2);
        WriteString(sb, s);
        return sb.ToString();
    }

    private static char HexLower(int v) => (char)(v < 10 ? '0' + v : 'a' + v - 10);
}
