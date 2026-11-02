namespace Pcoc;

public static class Anchor
{
/// §16.1
    public static string Compare(long storedHeight, long inclusionHeight)
    {
        var delta = inclusionHeight - storedHeight;
        if (delta == Const.AnchorOffset0 || delta == Const.AnchorOffset1 || delta == Const.AnchorOffset2)
        {
            return "anchored";
        }
        return "not anchored";
    }
}
