namespace Pcoc;

public static class Rate
{
/// §15.5
    public static string Compare(long hikeLegs, long hikeSpan, long p, long q)
    {
        if (hikeSpan <= 0)
        {
            return "refuse";
        }
        var left = (System.Numerics.BigInteger)q * hikeLegs;
        var right = (System.Numerics.BigInteger)p * hikeSpan;
        return left >= right ? "meets" : "refuse";
    }
}
