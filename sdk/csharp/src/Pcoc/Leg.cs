namespace Pcoc;

public static class Leg
{
/// §11.2
    public static ulong Result(Terrain terrain, byte[] previousCheckpoint)
    {
        var s = Bytes.U64Le(previousCheckpoint, 0);
        var steps = terrain.Profile.StepsPerLeg;
        for (var t = 0; t < steps; t++)
        {
            s = Step.Apply(terrain, s).Next;
        }
        return s;
    }
}
