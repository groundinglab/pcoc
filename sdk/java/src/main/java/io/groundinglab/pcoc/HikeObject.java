package io.groundinglab.pcoc;

import java.io.ByteArrayOutputStream;

/** §8 hike object. Encode is the core deterministic encoding of §7.3. */
public final class HikeObject {
    public String kind = Const.KIND;
    public String fill = Const.FILL;
    public long slotBytes;
    public long nSlots;
    public long sampleBytes;
    public long stepsPerLeg;
    public byte[] publicKey;
    public byte[] seedBeacon;
    public byte[] lastInterrupt;
    public long lastInterruptHeight;
    public Stage[] stages;

    /** §8 stage map. Stage 0 has a null path; stages 1–17 store the path bytes. */
    public static final class Stage {
        public byte[] beacon;
        public long beaconHeight;
        public byte[] waypoint;
        public byte[][] checkpoints;
        public long[] legResults;
        /** Null on stage 0, where the key is omitted. Empty on stages 1–17 for a skip. */
        public byte[] path;
    }

    /** §7.3 / §8. Stage 0 omits {@code path}. */
    public byte[] encode() {
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        Cbor.writeType(o, 5, 11);
        Cbor.writeText(o, "fill");
        Cbor.writeText(o, fill);
        Cbor.writeText(o, "kind");
        Cbor.writeText(o, kind);
        Cbor.writeText(o, "stages");
        Cbor.writeType(o, 4, stages.length);
        for (int g = 0; g < stages.length; g++) {
            writeStage(o, stages[g], g == 0);
        }
        Cbor.writeText(o, "n_slots");
        Cbor.writeUint(o, nSlots);
        Cbor.writeText(o, "public_key");
        Cbor.writeBytes(o, publicKey);
        Cbor.writeText(o, "slot_bytes");
        Cbor.writeUint(o, slotBytes);
        Cbor.writeText(o, "seed_beacon");
        Cbor.writeBytes(o, seedBeacon);
        Cbor.writeText(o, "sample_bytes");
        Cbor.writeUint(o, sampleBytes);
        Cbor.writeText(o, "steps_per_leg");
        Cbor.writeUint(o, stepsPerLeg);
        Cbor.writeText(o, "last_interrupt");
        Cbor.writeBytes(o, lastInterrupt);
        Cbor.writeText(o, "last_interrupt_height");
        Cbor.writeUint(o, lastInterruptHeight);
        return o.toByteArray();
    }

    private static void writeStage(ByteArrayOutputStream o, Stage stage, boolean stage0) {
        Cbor.writeType(o, 5, stage0 ? 6 : 7);
        if (!stage0) {
            Cbor.writeText(o, "path");
            Cbor.writeBytes(o, stage.path);
        }
        Cbor.writeText(o, "beacon");
        Cbor.writeBytes(o, stage.beacon);
        Cbor.writeText(o, "waypoint");
        Cbor.writeBytes(o, stage.waypoint);
        Cbor.writeText(o, "leg_count");
        Cbor.writeUint(o, stage.legResults.length);
        Cbor.writeText(o, "checkpoints");
        Cbor.writeType(o, 4, stage.checkpoints.length);
        for (byte[] cp : stage.checkpoints) {
            Cbor.writeBytes(o, cp);
        }
        Cbor.writeText(o, "leg_results");
        Cbor.writeType(o, 4, stage.legResults.length);
        for (long lr : stage.legResults) {
            Cbor.writeBytes(o, Bytes.le64(lr));
        }
        Cbor.writeText(o, "beacon_height");
        Cbor.writeUint(o, stage.beaconHeight);
    }

    /** Strict read of a complete object. Used by the encode → decode → encode check. */
    public static HikeObject decode(byte[] input) throws Cbor.Malformed {
        Cbor.Parsed parsed = Cbor.parse(input);
        if (parsed.error != null || parsed.item == null || parsed.item.kind != Cbor.Kind.MAP) {
            throw new Cbor.Malformed(parsed.error == null ? "not a map" : parsed.error);
        }
        HikeObject o = new HikeObject();
        o.kind = text(parsed.item, "kind");
        o.fill = text(parsed.item, "fill");
        o.slotBytes = uint(parsed.item, "slot_bytes");
        o.nSlots = uint(parsed.item, "n_slots");
        o.sampleBytes = uint(parsed.item, "sample_bytes");
        o.stepsPerLeg = uint(parsed.item, "steps_per_leg");
        o.publicKey = bytes(parsed.item, "public_key");
        o.seedBeacon = bytes(parsed.item, "seed_beacon");
        o.lastInterrupt = bytes(parsed.item, "last_interrupt");
        o.lastInterruptHeight = uint(parsed.item, "last_interrupt_height");
        Cbor.Item stages = parsed.item.field("stages");
        if (stages == null || stages.kind != Cbor.Kind.ARRAY) {
            throw new Cbor.Malformed("stages");
        }
        o.stages = new Stage[stages.array.size()];
        for (int g = 0; g < stages.array.size(); g++) {
            o.stages[g] = readStage(stages.array.get(g), g == 0);
        }
        return o;
    }

    private static Stage readStage(Cbor.Item item, boolean stage0) throws Cbor.Malformed {
        if (item.kind != Cbor.Kind.MAP) {
            throw new Cbor.Malformed("stage");
        }
        Stage s = new Stage();
        s.beacon = bytes(item, "beacon");
        s.beaconHeight = uint(item, "beacon_height");
        s.waypoint = bytes(item, "waypoint");
        Cbor.Item cps = item.field("checkpoints");
        Cbor.Item lrs = item.field("leg_results");
        if (cps == null || cps.kind != Cbor.Kind.ARRAY || lrs == null || lrs.kind != Cbor.Kind.ARRAY) {
            throw new Cbor.Malformed("legs");
        }
        s.checkpoints = new byte[cps.array.size()][];
        for (int j = 0; j < cps.array.size(); j++) {
            if (cps.array.get(j).kind != Cbor.Kind.BYTES) {
                throw new Cbor.Malformed("checkpoint");
            }
            s.checkpoints[j] = cps.array.get(j).bytes;
        }
        s.legResults = new long[lrs.array.size()];
        for (int j = 0; j < lrs.array.size(); j++) {
            if (lrs.array.get(j).kind != Cbor.Kind.BYTES || lrs.array.get(j).bytes.length != 8) {
                throw new Cbor.Malformed("leg result");
            }
            s.legResults[j] = Bytes.getLe64(lrs.array.get(j).bytes, 0);
        }
        if (stage0) {
            if (item.field("path") != null) {
                throw new Cbor.Malformed("stage-0 path");
            }
            s.path = null;
        } else {
            s.path = bytes(item, "path");
        }
        return s;
    }

    private static String text(Cbor.Item map, String key) throws Cbor.Malformed {
        Cbor.Item v = map.field(key);
        if (v == null || v.kind != Cbor.Kind.TEXT) {
            throw new Cbor.Malformed(key);
        }
        return v.text;
    }

    private static byte[] bytes(Cbor.Item map, String key) throws Cbor.Malformed {
        Cbor.Item v = map.field(key);
        if (v == null || v.kind != Cbor.Kind.BYTES) {
            throw new Cbor.Malformed(key);
        }
        return v.bytes;
    }

    private static long uint(Cbor.Item map, String key) throws Cbor.Malformed {
        Cbor.Item v = map.field(key);
        if (v == null || v.kind != Cbor.Kind.UINT || v.uint.bitLength() > 63) {
            throw new Cbor.Malformed(key);
        }
        return v.uint.longValue();
    }
}
