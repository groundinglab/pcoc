using System;
using System.Collections.Generic;

namespace Pcoc;

/// <summary>One stage of the hike object (Rule A3.2). Path is null when the key is absent.</summary>
public sealed class HikeStage
{
    public byte[] Beacon = Array.Empty<byte>();
    public ulong BeaconHeight;
    public ulong LegCount;
    public byte[] Waypoint = Array.Empty<byte>();
    public byte[]? Path; // stage 0 has no path key; stages 1–17 carry one (possibly empty)
    public List<byte[]> Checkpoints = new();
    public List<byte[]> LegResults = new();
}

/// <summary>The hike object of Rule A3 (section 8).</summary>
public sealed class HikeObject
{
    public string Kind = Profile.Kind;
    public string Fill = Profile.Fill;
    public ulong SlotBytes;
    public ulong NSlots;
    public ulong SampleBytes;
    public ulong StepsPerLeg;
    public byte[] PublicKey = Array.Empty<byte>();
    public byte[] SeedBeacon = Array.Empty<byte>();
    public List<HikeStage> Stages = new();
    public byte[] LastInterrupt = Array.Empty<byte>();
    public ulong LastInterruptHeight;

    /// <summary>Builds the CBOR tree of section 8 (stage 0 without a path key).</summary>
    public CborValue.Map ToCbor()
    {
        var stages = new List<CborValue>();
        for (int g = 0; g < Stages.Count; g++)
        {
            var s = Stages[g];
            var fields = new List<(string, CborValue)>
            {
                ("beacon", new CborValue.Bstr(s.Beacon)),
                ("beacon_height", new CborValue.Uint(s.BeaconHeight)),
                ("leg_count", new CborValue.Uint(s.LegCount)),
                ("waypoint", new CborValue.Bstr(s.Waypoint)),
            };
            if (s.Path is not null)
                fields.Add(("path", new CborValue.Bstr(s.Path)));
            var cps = new List<CborValue>();
            foreach (var c in s.Checkpoints) cps.Add(new CborValue.Bstr(c));
            var lrs = new List<CborValue>();
            foreach (var l in s.LegResults) lrs.Add(new CborValue.Bstr(l));
            fields.Add(("checkpoints", new CborValue.Array(cps)));
            fields.Add(("leg_results", new CborValue.Array(lrs)));
            stages.Add(new CborValue.Map(fields));
        }
        return new CborValue.Map(new List<(string, CborValue)>
        {
            ("kind", new CborValue.Text(Kind)),
            ("fill", new CborValue.Text(Fill)),
            ("slot_bytes", new CborValue.Uint(SlotBytes)),
            ("n_slots", new CborValue.Uint(NSlots)),
            ("sample_bytes", new CborValue.Uint(SampleBytes)),
            ("steps_per_leg", new CborValue.Uint(StepsPerLeg)),
            ("public_key", new CborValue.Bstr(PublicKey)),
            ("seed_beacon", new CborValue.Bstr(SeedBeacon)),
            ("stages", new CborValue.Array(stages)),
            ("last_interrupt", new CborValue.Bstr(LastInterrupt)),
            ("last_interrupt_height", new CborValue.Uint(LastInterruptHeight)),
        });
    }

    /// <summary>Section 7.3 produce encoding: core deterministic CBOR.</summary>
    public byte[] Encode() => CborWriter.Encode(ToCbor());
}
