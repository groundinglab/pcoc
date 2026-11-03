namespace Pcoc;

/// §16.2
public static class Pin
{
    public const string Tag = "spec-2.3.0";
    public const string VectorSha = "91ded57e5c05b7cb4cd74d313a51d83d2373fa86dc625df41faed9586e9032af";

    /// §16.2
    public readonly struct Result
    {
        public bool Match { get; }
        public string Tag { get; }
        public string Vector { get; }
        public string Prefix { get; }

        public Result(bool match, string tag, string vector, string prefix)
        {
            Match = match;
            Tag = tag;
            Vector = vector;
            Prefix = prefix;
        }
    }

    /// §16.2
    public static Result Check(string pinText, byte[] manifest)
    {
        var tag = "";
        var vector = "";
        foreach (var raw in pinText.Replace("\r\n", "\n").Split('\n'))
        {
            var line = raw.Trim();
            if (line.StartsWith("tag=", System.StringComparison.Ordinal))
            {
                tag = line.Substring(4).Trim();
            }
            else if (line.StartsWith("vector-sha256=", System.StringComparison.Ordinal))
            {
                vector = line.Substring(14).Trim();
            }
        }
        var digest = Bytes.Hex(Sha256.Hash(manifest));
        var match = tag == Tag && vector == VectorSha && digest == VectorSha;
        var prefix = digest.Length >= 8 ? digest.Substring(0, 8) : digest;
        return new Result(match, tag, vector, prefix);
    }

    /// §16.2
    public static string? FindFile(string name)
    {
        var dir = System.IO.Directory.GetCurrentDirectory();
        for (var i = 0; i < 8 && dir != null; i++)
        {
            var candidate = System.IO.Path.Combine(dir, "sdk", name);
            if (System.IO.File.Exists(candidate))
            {
                return candidate;
            }
            var flat = System.IO.Path.Combine(dir, name);
            if (name.StartsWith("SPEC", System.StringComparison.Ordinal) && System.IO.File.Exists(flat) && System.IO.File.Exists(System.IO.Path.Combine(dir, "VERSION")))
            {
                return flat;
            }
            var parent = System.IO.Directory.GetParent(dir);
            dir = parent?.FullName;
        }
        return null;
    }
}
