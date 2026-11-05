namespace Pcoc.Produce;

/// §13.6
[Spec("§13.6", "An accepted waypoint is answered 204 and Bari holds it.")]
public static class Paid
{
    public static bool DisplayOrder;

    [Spec("§9", "Every Rule BR1 block-hash comparison (current, before, after) is natural order on both sides")]
    public static int Status(byte[] body, string nonceText, string headerHex, string origin, Br1.Tip tip)
    {
        if (DisplayOrder)
        {
            var reversed = new Br1.Tip(Bytes.Reverse32(tip.Hash), Bytes.Reverse32(tip.Previous), tip.Bits, tip.Height, tip.Time, tip.PeriodFirstTime, tip.Live);
            return Br1.Status(body, nonceText, headerHex, origin, reversed);
        }
        return Br1.Status(body, nonceText, headerHex, origin, tip);
    }
}
