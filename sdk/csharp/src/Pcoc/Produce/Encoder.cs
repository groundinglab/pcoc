namespace Pcoc.Produce;

/// §7.3
[Spec("§7.3", "Produce emits the core deterministic encoding:")]
public static class Encoder
{
    public static bool SwapStageKeys;

    [Spec("§7.3", "Produce emits the core deterministic encoding:")]
    public static byte[] Encode(
        string fill,
        ulong slotBytes,
        ulong nSlots,
        ulong sampleBytes,
        ulong stepsPerLeg,
        byte[] publicKey,
        byte[] seedBeacon,
        StageRecord[] stages,
        byte[] lastInterrupt,
        ulong lastInterruptHeight)
    {
        var encodedStages = new byte[stages.Length][];
        for (var g = 0; g < stages.Length; g++)
        {
            encodedStages[g] = EncodeStage(stages[g], g);
        }
        var pairs = new (string, byte[])[]
        {
            ("kind", Cbor.EncodeText(Const.KindText)),
            ("fill", Cbor.EncodeText(fill)),
            ("slot_bytes", Cbor.EncodeUint(slotBytes)),
            ("n_slots", Cbor.EncodeUint(nSlots)),
            ("sample_bytes", Cbor.EncodeUint(sampleBytes)),
            ("steps_per_leg", Cbor.EncodeUint(stepsPerLeg)),
            ("public_key", Cbor.EncodeBytes(publicKey)),
            ("seed_beacon", Cbor.EncodeBytes(seedBeacon)),
            ("stages", Cbor.EncodeArray(encodedStages)),
            ("last_interrupt", Cbor.EncodeBytes(lastInterrupt)),
            ("last_interrupt_height", Cbor.EncodeUint(lastInterruptHeight)),
        };
        return Cbor.EncodeMap(pairs);
    }

    [Spec("§7.3", "Stage 0 has no path key (Rule A3.3).")]
    public static byte[] EncodeStage(StageRecord stage, int index)
    {
        var cps = new byte[stage.Checkpoints.Length][];
        for (var i = 0; i < stage.Checkpoints.Length; i++)
        {
            cps[i] = Cbor.EncodeBytes(stage.Checkpoints[i]);
        }
        var lrs = new byte[stage.LegResults.Length][];
        for (var i = 0; i < stage.LegResults.Length; i++)
        {
            lrs[i] = Cbor.EncodeBytes(stage.LegResults[i]);
        }
        var beaconKey = SwapStageKeys ? "waypoint" : "beacon";
        var waypointKey = SwapStageKeys ? "beacon" : "waypoint";
        if (index == 0)
        {
            return Cbor.EncodeMap(new (string, byte[])[]
            {
                (beaconKey, Cbor.EncodeBytes(stage.Beacon)),
                (waypointKey, Cbor.EncodeBytes(stage.Waypoint)),
                ("leg_count", Cbor.EncodeUint(stage.LegCount)),
                ("checkpoints", Cbor.EncodeArray(cps)),
                ("leg_results", Cbor.EncodeArray(lrs)),
                ("beacon_height", Cbor.EncodeUint(stage.BeaconHeight)),
            });
        }
        return Cbor.EncodeMap(new (string, byte[])[]
        {
            ("path", Cbor.EncodeBytes(stage.Path ?? System.Array.Empty<byte>())),
            (beaconKey, Cbor.EncodeBytes(stage.Beacon)),
            (waypointKey, Cbor.EncodeBytes(stage.Waypoint)),
            ("leg_count", Cbor.EncodeUint(stage.LegCount)),
            ("checkpoints", Cbor.EncodeArray(cps)),
            ("leg_results", Cbor.EncodeArray(lrs)),
            ("beacon_height", Cbor.EncodeUint(stage.BeaconHeight)),
        });
    }
}

/// §8
[Spec("§8", "Per stage the object stores beacon, waypoint, leg_count, checkpoints, leg_results, beacon_height, and path on stages 1–17.")]
public sealed class StageRecord
{
    public byte[] Beacon { get; set; } = System.Array.Empty<byte>();
    public ulong BeaconHeight { get; set; }
    public byte[] Waypoint { get; set; } = System.Array.Empty<byte>();
    public ulong LegCount { get; set; }
    public byte[][] Checkpoints { get; set; } = System.Array.Empty<byte[]>();
    public byte[][] LegResults { get; set; } = System.Array.Empty<byte[]>();
    public byte[]? Path { get; set; }
}
