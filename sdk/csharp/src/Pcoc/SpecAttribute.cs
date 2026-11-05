namespace Pcoc;

/// §16.2
[System.AttributeUsage(System.AttributeTargets.All, AllowMultiple = true)]
internal sealed class SpecAttribute : System.Attribute
{
    public string Section { get; }
    public string Sentence { get; }

    public SpecAttribute(string section, string sentence)
    {
        Section = section;
        Sentence = sentence;
    }
}
