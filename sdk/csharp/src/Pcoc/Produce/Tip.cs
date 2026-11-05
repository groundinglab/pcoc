namespace Pcoc.Produce;

/// §9
[Spec("§9", "Observe performs the read in section 9 and returns (natural, height).")]
public readonly struct Tip
{
    public byte[] Natural { get; }
    public ulong Height { get; }
    public byte[]? Header80 { get; }

    [Spec("§9", "Observe performs the read in section 9 and returns (natural, height).")]
    public Tip(byte[] natural, ulong height, byte[]? header80)
    {
        Natural = natural;
        Height = height;
        Header80 = header80;
    }
}
