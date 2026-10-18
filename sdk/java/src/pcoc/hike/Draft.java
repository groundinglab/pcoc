package pcoc.hike;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

/**
 * A v2.0 fixture the compact summary can measure. Encode follows
 * {@code encode_hike}: stage 0 has no path key unless {@code pathKey} is set.
 */
public final class Draft {
    public byte[] publicKey = new byte[32];
    public byte[] seedBeacon = new byte[32];
    public byte[] lastInterrupt;
    public Long lastInterruptHeight;
    public final List<Stage> stages = new ArrayList<>();

    /** Structural object with paths, checkpoints, and leg results cleared. */
    public static Draft quiet() {
        Draft draft = new Draft();
        Arrays.fill(draft.publicKey, (byte) 0x11);
        for (int g = 0; g < Hike.STAGES; g++) {
            Stage stage = new Stage();
            stage.beacon[0] = (byte) (g + 1);
            stage.beaconHeight = g + 1L;
            if (g != 0) {
                Arrays.fill(stage.waypoint, (byte) g);
            }
            draft.stages.add(stage);
        }
        draft.seedBeacon = draft.stages.get(0).beacon.clone();
        draft.stages.get(0).waypoint = Bytes.sha256(draft.publicKey, draft.seedBeacon);
        draft.lastInterrupt = new byte[32];
        draft.lastInterrupt[0] = 99;
        draft.lastInterruptHeight = (long) Hike.STAGES + 2;
        return draft;
    }

    public byte[] encode() {
        return encode(Profile.PRODUCTION);
    }

    public byte[] encode(Profile profile) {
        List<byte[]> encoded = new ArrayList<>();
        for (int g = 0; g < stages.size(); g++) {
            Stage stage = stages.get(g);
            List<byte[]> checks = new ArrayList<>();
            for (byte[] item : stage.checkpoints) {
                checks.add(Cbor.encodeBytes(item));
            }
            List<byte[]> legs = new ArrayList<>();
            for (byte[] item : stage.legResults) {
                legs.add(Cbor.encodeBytes(item));
            }
            List<Map.Entry<String, byte[]>> stageFields = new ArrayList<>();
            // Stage 0 omits path. A present key, including empty bytes, is the
            // shape item "stage-0 path key".
            if (g != 0 || stage.pathKey) {
                byte[] path = stage.path == null ? new byte[0] : stage.path;
                stageFields.add(Map.entry("path", Cbor.encodeBytes(path)));
            }
            stageFields.add(Map.entry("beacon", Cbor.encodeBytes(stage.beacon)));
            stageFields.add(Map.entry("waypoint", Cbor.encodeBytes(stage.waypoint)));
            stageFields.add(Map.entry("leg_count", Cbor.encodeUint(stage.legCount)));
            stageFields.add(Map.entry("checkpoints", Cbor.encodeArray(checks)));
            stageFields.add(Map.entry("leg_results", Cbor.encodeArray(legs)));
            stageFields.add(Map.entry("beacon_height", Cbor.encodeUint(stage.beaconHeight)));
            encoded.add(Cbor.encodeMap(stageFields));
        }
        byte[] last = lastInterrupt == null ? new byte[32] : lastInterrupt;
        long lastHeight = lastInterruptHeight == null ? 0L : lastInterruptHeight;
        List<Map.Entry<String, byte[]>> fields = new ArrayList<>();
        fields.add(Map.entry("fill", Cbor.encodeText(Hike.FILL)));
        fields.add(Map.entry("kind", Cbor.encodeText(Hike.KIND)));
        fields.add(Map.entry("stages", Cbor.encodeArray(encoded)));
        fields.add(Map.entry("n_slots", Cbor.encodeUint(profile.nSlots)));
        fields.add(Map.entry("public_key", Cbor.encodeBytes(publicKey)));
        fields.add(Map.entry("slot_bytes", Cbor.encodeUint(profile.slotBytes)));
        fields.add(Map.entry("seed_beacon", Cbor.encodeBytes(seedBeacon)));
        fields.add(Map.entry("sample_bytes", Cbor.encodeUint(profile.sampleBytes)));
        fields.add(Map.entry("steps_per_leg", Cbor.encodeUint(profile.stepsPerLeg)));
        fields.add(Map.entry("last_interrupt", Cbor.encodeBytes(last)));
        fields.add(Map.entry("last_interrupt_height", Cbor.encodeUint(lastHeight)));
        return Cbor.encodeMap(fields);
    }

    public static final class Stage {
        public byte[] beacon = new byte[32];
        public long beaconHeight;
        public long legCount;
        public byte[] waypoint = new byte[32];
        public byte[] path = new byte[0];
        /** Forces a path key on stage 0. Later stages always carry the key. */
        public boolean pathKey;
        public final List<byte[]> checkpoints = new ArrayList<>();
        public final List<byte[]> legResults = new ArrayList<>();
    }
}
