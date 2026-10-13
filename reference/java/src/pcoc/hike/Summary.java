package pcoc.hike;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import pcoc.json.Json;

/**
 * Compact line the SPEC names only to exclude it, and the Rule A5 line.
 * {@link #packRecord} is the section 14 record the v2.0 pack compares.
 *
 * <p>{@code threads} is the Rule A4.3 pool size. It is not {@code STEPS_PER_LEG}.
 * A non-positive count uses {@link Hike#bindThreadsFrom} on
 * {@code PCOC_BIND_THREADS}. Checkpoint binding hashes the sample.
 * It does not run a leg. {@code six_leg} recomputes six stored legs when
 * the hike has at least six, using the profile's steps per leg.
 * {@code stream} is the draw byte source. A null stream with at least six
 * legs records {@code six_leg} as not read. Fewer than six legs is not defined.
 * An earlier kind returns before the index or the chain bind is called.
 */
public final class Summary {
    public interface Chain {
        /** True when the chain bind accepts the locators. */
        boolean ok(PathBody.Locators locators);
    }

    private static final String[] STAGE_KEYS = {
        "path", "beacon", "waypoint", "leg_count", "checkpoints", "leg_results", "beacon_height"
    };
    private static final String[] STAGE0_KEYS = {
        "beacon", "waypoint", "leg_count", "checkpoints", "leg_results", "beacon_height"
    };
    private static final String[] TOP_KEYS = {
        "fill", "kind", "stages", "n_slots", "public_key", "slot_bytes", "seed_beacon", "sample_bytes", "steps_per_leg",
        "last_interrupt", "last_interrupt_height"
    };

    private Summary() {}

    public static String line(byte[] object, Profile profile, Measure.Reader beacon, Chain chain, int threads, byte[] stream) {
        if (threads < 1) {
            threads = Hike.bindThreadsFrom(System.getenv("PCOC_BIND_THREADS"));
        }
        Cbor.Val top;
        try {
            top = Cbor.decode(object);
        } catch (Cbor.Problem problem) {
            return summary(profile.name, "mismatch", "not read", "not read", "not read", List.of());
        }
        if (top.kind != Cbor.Val.Kind.MAP) {
            return summary(profile.name, "mismatch", "not read", "not read", "not read", List.of());
        }
        String kind = text(top, "kind");
        if (Hike.KIND.equals(profile.kind) && Hike.isEarlierKind(kind)) {
            return Json.obj().put("version", Hike.EARLIER_VERSION).put("kind", kind).compact();
        }
        HikeBody hike = parse(top, profile);
        if (hike == null) {
            return summary(profile.name, "mismatch", "not read", "not read", "not read", List.of());
        }
        return measure(hike, profile, beacon, chain, threads, stream);
    }

    /**
     * Verify line conformance/v2.0 compares.
     * Issue 228 settled that a v2.0 verifier writes one output, the section 14
     * record, and the pack stores those bytes. This method is that switch.
     * {@code threads} is the Rule A4.3 pool size. The record does not depend
     * on it. One at a time is conforming. Checkpoint binding hashes the 1 KB
     * sample. It does not run a leg. {@link #line} is the compact line the
     * SPEC names only to exclude it. It is not a conformance output.
     */
    public static String packRecord(byte[] object, Profile profile, Measure.Reader beacon, Chain chain, int threads, byte[] stream) {
        return Measure.record(object, profile, beacon, chain, stream, false);
    }

    private static String summary(String profile, String shape, String checkpoints, String waypoints, String six, List<String> anchors) {
        Json.Arr anchorArr = Json.arr();
        for (String anchor : anchors) {
            anchorArr.add(anchor);
        }
        return Json.obj()
                .put("profile", profile)
                .put("shape", shape)
                .put("checkpoints", checkpoints)
                .put("waypoints", waypoints)
                .put("leg_rate", "not read")
                .put("six_leg", six)
                .put("anchors", anchorArr)
                .compact();
    }

    private static final class Stage {
        byte[] beacon;
        long beaconHeight;
        long legCount;
        byte[] waypoint;
        byte[] path;
        boolean pathPresent;
        List<byte[]> checkpoints;
        List<byte[]> legResults;
    }

    private static final class HikeBody {
        byte[] publicKey;
        byte[] seedBeacon;
        List<Stage> stages;
        byte[] last;
        Long lastHeight;
        boolean shape;
    }

    private static HikeBody parse(Cbor.Val top, Profile profile) {
        byte[] pub = bytes(top, "public_key", 32);
        byte[] seed = bytes(top, "seed_beacon", 32);
        List<Cbor.Val> rawStages = array(top, "stages");
        if (pub == null || seed == null || rawStages == null) {
            return null;
        }
        List<Stage> stages = new ArrayList<>();
        for (Cbor.Val raw : rawStages) {
            Stage stage = stage(raw);
            if (stage == null) {
                return null;
            }
            stages.add(stage);
        }
        HikeBody hike = new HikeBody();
        hike.publicKey = pub;
        hike.seedBeacon = seed;
        hike.stages = stages;
        hike.last = bytes(top, "last_interrupt", 32);
        hike.lastHeight = uint(top, "last_interrupt_height");
        hike.shape = shape(top, hike, profile);
        return hike;
    }

    private static Stage stage(Cbor.Val raw) {
        boolean pathPresent = keys(raw, STAGE_KEYS);
        if (!pathPresent && !keys(raw, STAGE0_KEYS)) {
            return null;
        }
        byte[] beacon = bytes(raw, "beacon", 32);
        byte[] waypoint = bytes(raw, "waypoint", 32);
        byte[] path = pathPresent ? bytesAny(raw, "path") : new byte[0];
        Long count = uint(raw, "leg_count");
        Long height = uint(raw, "beacon_height");
        List<Cbor.Val> checks = array(raw, "checkpoints");
        List<Cbor.Val> results = array(raw, "leg_results");
        if (beacon == null || waypoint == null || path == null || count == null || height == null || checks == null || results == null) {
            return null;
        }
        if (checks.size() != results.size() || Integer.toUnsignedLong(checks.size()) != count) {
            return null;
        }
        List<byte[]> checkpoints = new ArrayList<>();
        List<byte[]> legs = new ArrayList<>();
        for (int i = 0; i < checks.size(); i++) {
            if (checks.get(i).kind != Cbor.Val.Kind.BYTES || checks.get(i).bytes.length != 32) {
                return null;
            }
            if (results.get(i).kind != Cbor.Val.Kind.BYTES || results.get(i).bytes.length != 8) {
                return null;
            }
            checkpoints.add(checks.get(i).bytes);
            legs.add(results.get(i).bytes);
        }
        Stage stage = new Stage();
        stage.beacon = beacon;
        stage.waypoint = waypoint;
        stage.path = path;
        stage.pathPresent = pathPresent;
        stage.legCount = count;
        stage.beaconHeight = height;
        stage.checkpoints = checkpoints;
        stage.legResults = legs;
        return stage;
    }

    private static boolean shape(Cbor.Val top, HikeBody hike, Profile profile) {
        if (!keys(top, TOP_KEYS)) {
            return false;
        }
        if (!profile.kind.equals(text(top, "kind")) || !Hike.FILL.equals(text(top, "fill"))) {
            return false;
        }
        if (!uintEq(top, "slot_bytes", profile.slotBytes)
                || !uintEq(top, "n_slots", profile.nSlots)
                || !uintEq(top, "sample_bytes", profile.sampleBytes)
                || !uintEq(top, "steps_per_leg", profile.stepsPerLeg)) {
            return false;
        }
        if (hike.stages.size() != Hike.STAGES) {
            return false;
        }
        for (int g = 0; g < hike.stages.size(); g++) {
            Stage stage = hike.stages.get(g);
            if (stage.legCount < 1) {
                return false;
            }
            if (g == 0) {
                if (stage.pathPresent) {
                    return false;
                }
            } else if (!stage.pathPresent) {
                return false;
            }
            if (g == Hike.STAGES - 1 && stage.path.length != 0) {
                return false;
            }
        }
        if (!Bytes.eq(hike.stages.get(0).beacon, hike.seedBeacon)
                || !Bytes.eq(hike.stages.get(0).waypoint, Bytes.sha256(hike.publicKey, hike.seedBeacon))) {
            return false;
        }
        for (int g = 0; g < Hike.STAGES - 1; g++) {
            Stage cur = hike.stages.get(g);
            Stage next = hike.stages.get(g + 1);
            if (Long.compareUnsigned(next.beaconHeight, cur.beaconHeight) <= 0 || Bytes.eq(next.beacon, cur.beacon)) {
                return false;
            }
        }
        Stage closing = hike.stages.get(Hike.STAGES - 1);
        if (hike.last == null || hike.last.length != 32 || hike.lastHeight == null) {
            return false;
        }
        if (Bytes.eq(hike.last, closing.beacon) || Long.compareUnsigned(hike.lastHeight, closing.beaconHeight) <= 0) {
            return false;
        }
        return true;
    }

    private static String measure(HikeBody hike, Profile profile, Measure.Reader beacon, Chain chain, int threads, byte[] stream) {
        boolean beaconMiss = false;
        for (Stage stage : hike.stages) {
            String body = beacon == null ? null : beacon.body(stage.beaconHeight);
            Index read = Index.readDocument(body);
            if (read == null || read.height != stage.beaconHeight || !Bytes.eq(read.natural, stage.beacon)) {
                beaconMiss = true;
            }
        }
        boolean digests = false;
        for (Stage stage : hike.stages) {
            if (!stage.checkpoints.isEmpty()) {
                digests = true;
                break;
            }
        }
        boolean waypointsReady = hike.stages.size() >= Hike.STAGES;
        Terrain terrain = null;
        if (digests || waypointsReady) {
            terrain = Terrain.fill(hike.publicKey, hike.seedBeacon, profile);
        }
        boolean digestMiss = digests && (terrain == null || !bindAll(terrain, hike.stages, threads));
        String checkpoints = beaconMiss || digestMiss ? "mismatch" : "match";
        String six = six(hike, profile, stream, terrain);
        String shape = hike.shape ? "match" : "mismatch";
        if (!waypointsReady || terrain == null) {
            return summary(profile.name, shape, checkpoints, "not read", six, List.of());
        }
        boolean successorDiffers = false;
        for (int g = 0; g < Hike.STAGES - 1; g++) {
            // SPEC section 12.2 and the reference successor: I_g is stages[g+1].beacon.
            byte[] interrupt = hike.stages.get(g + 1).beacon;
            byte[] digest = waypoint(terrain, interrupt, hike.stages.get(g).waypoint, hike.stages.get(g).checkpoints);
            if (!Bytes.eq(digest, hike.stages.get(g + 1).waypoint)) {
                successorDiffers = true;
            }
        }
        List<String> anchors = new ArrayList<>();
        boolean plantDiffers = false;
        if (hike.stages.size() >= 17) {
            for (int g = 1; g <= 16; g++) {
                String token = anchor(hike, g, chain);
                if ("differs".equals(token)) {
                    plantDiffers = true;
                    token = "not anchored";
                }
                anchors.add(token);
            }
        }
        // Opening waypoint is the seed value, SHA-256(public_key ‖ seed_beacon).
        boolean openingIsSeed = Bytes.eq(hike.stages.get(0).waypoint, Bytes.sha256(hike.publicKey, hike.seedBeacon));
        String waypoints = successorDiffers || plantDiffers || !openingIsSeed ? "mismatch" : "match";
        return summary(profile.name, shape, checkpoints, waypoints, six, anchors);
    }

    /** "differs" means not anchored and the stored plant does not match. */
    private static String anchor(HikeBody hike, int g, Chain chain) {
        Stage stage = hike.stages.get(g);
        if (stage.path.length == 0) {
            return "not anchored";
        }
        PathBody.Locators loc = PathBody.locatorsFor(stage.path, stage.waypoint);
        if (loc == null) {
            return "differs";
        }
        if (!inWindow(stage.beaconHeight, loc.blockHeight) || !interrupt(hike, loc.blockHash, loc.blockHeight)) {
            return "not anchored";
        }
        boolean ok = chain == null || chain.ok(loc);
        return ok ? "anchored" : "differs";
    }

    private static boolean inWindow(long stageBeaconHeight, long height) {
        long origin = Hike.anchorWindowOrigin(stageBeaconHeight);
        int[] offsets = Hike.anchorOffsetsAfterPost();
        for (int off : offsets) {
            if (off < 0 || Long.compareUnsigned(origin + Integer.toUnsignedLong(off), origin) < 0) {
                return false;
            }
        }
        for (int off : offsets) {
            if (origin + Integer.toUnsignedLong(off) == height) {
                return true;
            }
        }
        return false;
    }

    /** Stages 1..17, plus last_interrupt (I_17) when both hash and height are stored. */
    private static boolean interrupt(HikeBody hike, byte[] hash, long height) {
        for (int i = 1; i < hike.stages.size(); i++) {
            Stage stage = hike.stages.get(i);
            if (stage.beaconHeight == height && Bytes.eq(stage.beacon, hash)) {
                return true;
            }
        }
        return hike.last != null && hike.lastHeight != null && hike.lastHeight == height && Bytes.eq(hike.last, hash);
    }

    private static byte[] waypoint(Terrain terrain, byte[] interrupt, byte[] limb, List<byte[]> checkpoints) {
        byte[] fold = new byte[32];
        for (byte[] checkpoint : checkpoints) {
            fold = Bytes.sha256(fold, checkpoint);
        }
        return Bytes.sha256(interrupt, terrain.slot(interrupt), limb, fold);
    }

    private static boolean bindAll(Terrain terrain, List<Stage> stages, int threads) {
        boolean match = true;
        for (Stage stage : stages) {
            if (stage.checkpoints.isEmpty()) {
                continue;
            }
            if (!bindStage(terrain, stage, threads)) {
                match = false;
            }
        }
        return match;
    }

    private static boolean bindStage(Terrain terrain, Stage stage, int threads) {
        int n = stage.checkpoints.size();
        int workers = threads < 1 ? 1 : Math.min(threads, n);
        ExecutorService pool = Executors.newFixedThreadPool(workers);
        try {
            List<Future<Boolean>> jobs = new ArrayList<>();
            for (int j = 0; j < n; j++) {
                int index = j;
                jobs.add(pool.submit(() -> {
                    byte[] previous = index == 0 ? stage.waypoint : stage.checkpoints.get(index - 1);
                    int offset = terrain.sampleOffset(previous);
                    byte[] digest = Bytes.sha256(
                            previous, stage.waypoint, terrain.blob, offset, terrain.profile.sampleBytes, stage.legResults.get(index));
                    return Bytes.eq(digest, stage.checkpoints.get(index));
                }));
            }
            for (Future<Boolean> job : jobs) {
                if (!job.get()) {
                    return false;
                }
            }
            return true;
        } catch (Exception ex) {
            return false;
        } finally {
            pool.shutdown();
        }
    }

    /** Compact six_leg token. The first leg on a stage uses the stored waypoint. */
    private static String six(HikeBody hike, Profile profile, byte[] stream, Terrain terrain) {
        long total = 0;
        for (Stage stage : hike.stages) {
            long next = total + stage.legCount;
            if (Long.compareUnsigned(next, total) < 0) {
                return "not read";
            }
            total = next;
        }
        Measure.Drawn drawn = Measure.drawSix(stream, total);
        if (drawn.token != null) {
            return drawn.token;
        }
        if (terrain == null) {
            terrain = Terrain.fill(hike.publicKey, hike.seedBeacon, profile);
        }
        boolean match = true;
        for (long index : drawn.indices) {
            int[] located = locate(hike, index);
            if (located == null) {
                match = false;
                continue;
            }
            Stage stage = hike.stages.get(located[0]);
            int j = located[1];
            byte[] previous = null;
            if (j == 0) {
                previous = stage.waypoint;
            } else if (j - 1 < stage.checkpoints.size()) {
                previous = stage.checkpoints.get(j - 1);
            }
            if (previous == null || previous.length < 8 || j >= stage.legResults.size() || stage.legResults.get(j).length != 8
                    || terrain.leg(previous) != Bytes.u64le(stage.legResults.get(j))) {
                match = false;
            }
        }
        return match ? "match" : "mismatch";
    }

    private static int[] locate(HikeBody hike, long index) {
        long start = 0;
        for (int g = 0; g < hike.stages.size(); g++) {
            long count = hike.stages.get(g).legCount;
            if (Long.compareUnsigned(index, start + count) < 0) {
                return new int[] {g, (int) (index - start)};
            }
            start += count;
        }
        return null;
    }

    private static boolean keys(Cbor.Val map, String[] expect) {
        return map != null && map.kind == Cbor.Val.Kind.MAP && map.map.size() == expect.length && contains(map, expect);
    }

    private static boolean contains(Cbor.Val map, String[] expect) {
        for (String key : expect) {
            if (!map.map.containsKey(key)) {
                return false;
            }
        }
        return true;
    }

    private static boolean uintEq(Cbor.Val map, String key, long want) {
        Long got = uint(map, key);
        return got != null && got == want;
    }

    private static byte[] bytes(Cbor.Val map, String key, int width) {
        byte[] raw = bytesAny(map, key);
        if (raw == null || raw.length != width) {
            return null;
        }
        return raw;
    }

    private static byte[] bytesAny(Cbor.Val map, String key) {
        if (map == null || map.kind != Cbor.Val.Kind.MAP) {
            return null;
        }
        Cbor.Val value = map.map.get(key);
        if (value == null || value.kind != Cbor.Val.Kind.BYTES) {
            return null;
        }
        return value.bytes;
    }

    private static String text(Cbor.Val map, String key) {
        if (map == null || map.kind != Cbor.Val.Kind.MAP) {
            return null;
        }
        Cbor.Val value = map.map.get(key);
        if (value == null || value.kind != Cbor.Val.Kind.TEXT) {
            return null;
        }
        return value.text;
    }

    private static Long uint(Cbor.Val map, String key) {
        if (map == null || map.kind != Cbor.Val.Kind.MAP) {
            return null;
        }
        Cbor.Val value = map.map.get(key);
        if (value == null || value.kind != Cbor.Val.Kind.UINT) {
            return null;
        }
        return value.uint;
    }

    private static List<Cbor.Val> array(Cbor.Val map, String key) {
        if (map == null || map.kind != Cbor.Val.Kind.MAP) {
            return null;
        }
        Cbor.Val value = map.map.get(key);
        if (value == null || value.kind != Cbor.Val.Kind.ARRAY) {
            return null;
        }
        return value.array;
    }
}
