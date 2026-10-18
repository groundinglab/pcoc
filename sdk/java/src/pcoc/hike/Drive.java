package pcoc.hike;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * Section 13 produce for an explicit tip sequence.
 * This port does not Observe gossip and does not post to Bari.
 * Empty path bytes are the section 13.3 record of that miss.
 */
public final class Drive {
    public static final class Tip {
        public final byte[] natural;
        public final long height;

        public Tip(byte[] natural, long height) {
            this.natural = natural;
            this.height = height;
        }
    }

    public static final class Result {
        public final byte[] object;
        public final String stderr;

        Result(byte[] object, String stderr) {
            this.object = object;
            this.stderr = stderr;
        }
    }

    public static final class Clock extends Exception {
        private static final long serialVersionUID = 1L;

        Clock() {}
    }

    private Drive() {}

    public static Result run(byte[] publicKey, Profile profile, List<Tip> tips) throws Clock {
        if (publicKey.length != 32 || tips == null || tips.isEmpty()) {
            throw new Clock();
        }
        Cursor clock = new Cursor(tips);
        Tip seed = clock.next();
        byte[] seedBeacon = seed.natural.clone();
        long height = seed.height;
        byte[] seedValue = Bytes.sha256(publicKey, seedBeacon);
        Terrain terrain = Terrain.fill(publicKey, seedBeacon, profile);
        byte[] waypoint = seedValue;
        byte[] beacon = seedBeacon;
        long beaconHeight = height;
        List<byte[]> stages = new ArrayList<>();
        byte[] lastInterrupt = null;
        long lastHeight = 0;
        for (int g = 0; g < Hike.STAGES; g++) {
            byte[] stageWaypoint = waypoint;
            byte[] stageBeacon = beacon;
            long stageHeight = beaconHeight;
            byte[] previous = stageWaypoint;
            List<byte[]> checkpoints = new ArrayList<>();
            List<byte[]> legResults = new ArrayList<>();
            while (true) {
                long leg = terrain.leg(previous);
                byte[] legBytes = Bytes.le64(leg);
                int offset = terrain.sampleOffset(previous);
                byte[] checkpoint = Bytes.sha256(
                        previous,
                        stageWaypoint,
                        terrain.blob,
                        offset,
                        profile.sampleBytes,
                        legBytes);
                checkpoints.add(checkpoint);
                legResults.add(legBytes);
                previous = checkpoint;
                Tip tip = clock.next();
                if (!Bytes.eq(tip.natural, stageBeacon) && Long.compareUnsigned(tip.height, stageHeight) > 0) {
                    stages.add(encodeStage(g, stageBeacon, stageHeight, stageWaypoint, checkpoints, legResults));
                    if (g == Hike.STAGES - 1) {
                        lastInterrupt = tip.natural.clone();
                        lastHeight = tip.height;
                    } else {
                        byte[] fold = fold(checkpoints);
                        byte[] slot = terrain.slot(tip.natural);
                        waypoint = Bytes.sha256(tip.natural, slot, stageWaypoint, fold);
                        beacon = tip.natural.clone();
                        beaconHeight = tip.height;
                    }
                    break;
                }
            }
        }
        if (stages.size() != Hike.STAGES || lastInterrupt == null) {
            throw new Clock();
        }
        byte[] object = encodeObject(profile, publicKey, seedBeacon, stages, lastInterrupt, lastHeight);
        return new Result(object, stderr());
    }

    private static byte[] fold(List<byte[]> checkpoints) {
        byte[] state = new byte[32];
        for (byte[] checkpoint : checkpoints) {
            state = Bytes.sha256(state, checkpoint);
        }
        return state;
    }

    private static String stderr() {
        // Stage 0 has no Bari post and no stderr line (SPEC section 13, Rule A3.4).
        // This port does not fetch Bari. Every W_1 … W_16 path is empty, so the
        // final stderr step is path_skip 1 through path_skip 16, then
        // "not fetched by design 17". That last line is only for W_17.
        // The historical produce check reads these same bytes.
        StringBuilder out = new StringBuilder();
        for (int g = 1; g <= 16; g++) {
            out.append("path_skip ").append(g).append('\n');
        }
        out.append("not fetched by design 17\n");
        return out.toString();
    }

    private static byte[] encodeObject(
            Profile profile,
            byte[] publicKey,
            byte[] seedBeacon,
            List<byte[]> stages,
            byte[] lastInterrupt,
            long lastHeight) {
        List<Map.Entry<String, byte[]>> fields = new ArrayList<>();
        fields.add(Map.entry("fill", Cbor.encodeText(Hike.FILL)));
        fields.add(Map.entry("kind", Cbor.encodeText(profile.kind)));
        fields.add(Map.entry("stages", Cbor.encodeArray(stages)));
        fields.add(Map.entry("n_slots", Cbor.encodeUint(profile.nSlots)));
        fields.add(Map.entry("public_key", Cbor.encodeBytes(publicKey)));
        fields.add(Map.entry("slot_bytes", Cbor.encodeUint(profile.slotBytes)));
        fields.add(Map.entry("seed_beacon", Cbor.encodeBytes(seedBeacon)));
        fields.add(Map.entry("sample_bytes", Cbor.encodeUint(profile.sampleBytes)));
        fields.add(Map.entry("steps_per_leg", Cbor.encodeUint(profile.stepsPerLeg)));
        fields.add(Map.entry("last_interrupt", Cbor.encodeBytes(lastInterrupt)));
        fields.add(Map.entry("last_interrupt_height", Cbor.encodeUint(lastHeight)));
        return Cbor.encodeMap(fields);
    }

    private static byte[] encodeStage(
            int g,
            byte[] beacon,
            long beaconHeight,
            byte[] waypoint,
            List<byte[]> checkpoints,
            List<byte[]> legResults) {
        List<byte[]> checkpointItems = new ArrayList<>();
        for (byte[] checkpoint : checkpoints) {
            checkpointItems.add(Cbor.encodeBytes(checkpoint));
        }
        List<byte[]> resultItems = new ArrayList<>();
        for (byte[] result : legResults) {
            resultItems.add(Cbor.encodeBytes(result));
        }
        List<Map.Entry<String, byte[]>> fields = new ArrayList<>();
        if (g > 0) {
            fields.add(Map.entry("path", Cbor.encodeBytes(new byte[0])));
        }
        fields.add(Map.entry("beacon", Cbor.encodeBytes(beacon)));
        fields.add(Map.entry("waypoint", Cbor.encodeBytes(waypoint)));
        fields.add(Map.entry("leg_count", Cbor.encodeUint(checkpoints.size())));
        fields.add(Map.entry("checkpoints", Cbor.encodeArray(checkpointItems)));
        fields.add(Map.entry("leg_results", Cbor.encodeArray(resultItems)));
        fields.add(Map.entry("beacon_height", Cbor.encodeUint(beaconHeight)));
        return Cbor.encodeMap(fields);
    }

    /**
     * Window heights at or below {@code cutOff}. {@code cutOff} is
     * {@code last_interrupt_height}. A height above it is not asked.
     * The window is omitted when an offset does not fit in a Java long
     * interpreted as an unsigned height that wrapped.
     */
    public static long[] pathAskHeights(long postHeight, long cutOff) {
        long origin = Hike.anchorWindowOrigin(postHeight);
        long[] window = new long[Hike.ANCHOR_WINDOW];
        int[] offsets = Hike.anchorOffsetsAfterPost();
        for (int i = 0; i < offsets.length; i++) {
            if (offsets[i] < 0) {
                return new long[0];
            }
            long sum = origin + Integer.toUnsignedLong(offsets[i]);
            if (Long.compareUnsigned(sum, origin) < 0) {
                return new long[0];
            }
            window[i] = sum;
        }
        int n = 0;
        for (long height : window) {
            if (Long.compareUnsigned(height, cutOff) <= 0) {
                n++;
            }
        }
        long[] asked = new long[n];
        int at = 0;
        for (long height : window) {
            if (Long.compareUnsigned(height, cutOff) <= 0) {
                asked[at++] = height;
            }
        }
        return asked;
    }

    /**
     * The body when it validates for {@code waypoint}, the inclusion height
     * is in the anchor window, and that height is at or below {@code cutOff}.
     * Otherwise null. Null is a miss: the stage stores empty path bytes and
     * the hike still returns the object.
     */
    public static byte[] acceptPath(byte[] body, byte[] waypoint, long postHeight, long cutOff) {
        if (waypoint == null || waypoint.length != 32 || body == null || body.length == 0) {
            return null;
        }
        PathBody.Locators loc = PathBody.locatorsFor(body, waypoint);
        if (loc == null) {
            return null;
        }
        if (!heightInWindow(postHeight, loc.blockHeight) || Long.compareUnsigned(loc.blockHeight, cutOff) > 0) {
            return null;
        }
        return body.clone();
    }

    private static boolean heightInWindow(long beaconHeight, long inclusion) {
        long origin = Hike.anchorWindowOrigin(beaconHeight);
        for (int off : Hike.anchorOffsetsAfterPost()) {
            long sum = origin + Integer.toUnsignedLong(off);
            if (Long.compareUnsigned(sum, origin) < 0) {
                return false;
            }
            if (sum == inclusion) {
                return true;
            }
        }
        return false;
    }

    /** Section 4 tokens. present means the object was written. not read means it was not. */
    public static String report(String output, boolean written) {
        if (!written) {
            return "{\"schema\":\"satoshi-hike-cli-report-v0.2\",\"command\":\"produce\",\"result\":\"not read\",\"rung\":null}\n";
        }
        String quoted = output == null ? "" : output.replace("\\", "\\\\").replace("\"", "\\\"");
        return "{\"schema\":\"satoshi-hike-cli-report-v0.2\",\"command\":\"produce\",\"result\":\"present\",\"rung\":null,\"output\":\"" + quoted + "\"}\n";
    }

    private static final class Cursor {
        final List<Tip> tips;
        int at;

        Cursor(List<Tip> tips) {
            this.tips = tips;
        }

        Tip next() throws Clock {
            if (at >= tips.size()) {
                throw new Clock();
            }
            Tip tip = tips.get(at++);
            if (tip.natural == null || tip.natural.length != 32) {
                throw new Clock();
            }
            return tip;
        }
    }
}
