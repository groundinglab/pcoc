package pcoc.conformance;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import pcoc.hike.Admit;
import pcoc.hike.Bytes;
import pcoc.hike.Profile;
import pcoc.hike.Terrain;
import pcoc.json.Json;

/**
 * Satoshi Hike v2.1 conformance. Reads {@code spec/conformance/v2.1}.
 * A withdrawn result is a reject. The v2.0 pack is not this target.
 */
public final class V21Pack {
    private static final List<String> ROWS = new ArrayList<>();

    private V21Pack() {}

    public static void main(String[] args) throws Exception {
        Path pack = repoRoot().resolve("spec/conformance/v2.1");
        checkSeed(load(pack.resolve("seed.json")));
        checkWords(load(pack.resolve("nl4-words.json")));
        checkTerrain(load(pack.resolve("terrain-small.json")));
        checkStep(load(pack.resolve("step-c1.json")));
        checkHike(load(pack.resolve("kat-small.json")), "kat-small");
        checkHike(load(pack.resolve("test-3leg.json")), "test-3leg");
        checkRate(load(pack.resolve("rate.json")));
        checkAnchors(load(pack.resolve("anchors.json")));
        checkNegatives(load(pack.resolve("negatives.json")));
        Adversarial.cachedLeg();
        System.out.println("vector match");
        for (String row : ROWS) {
            System.out.println(row);
        }
        System.out.println("v2.1 java ok");
    }

    static Path repoRoot() {
        Path root = Path.of("").toAbsolutePath();
        while (root != null && !Files.isDirectory(root.resolve("spec/conformance/v2.1"))) {
            root = root.getParent();
        }
        if (root == null) {
            throw new IllegalStateException("repo root");
        }
        return root;
    }

    private static void checkSeed(Json.Obj doc) {
        byte[] publicKey = Bytes.unhex(text(doc, "public_key_hex"));
        byte[] beacon = Bytes.unhex(text(doc, "seed_beacon_hex"));
        byte[] digest = Terrain.seed(publicKey, beacon);
        String got = Bytes.hex(digest);
        eq("seed", got, text(doc, "seed_hex"));
        long[] lane = Terrain.lanes(digest);
        Json.Arr want = Json.asArr(doc.fields().get("lanes_u64_hex"));
        if (want.items().size() != 4) {
            throw new AssertionError("lanes");
        }
        for (int k = 0; k < 4; k++) {
            eq("lane " + k, Bytes.pin(lane[k]), Json.asString(want.items().get(k)));
        }
        if (longNum(doc, "waypoint_equals_seed") != 1 || !got.equals(Bytes.hex(digest))) {
            throw new AssertionError("waypoint");
        }
        String seal = Bytes.hex(Terrain.sealDigest(0));
        if (!"4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533".equals(seal)) {
            throw new AssertionError("seal0 " + seal);
        }
        if (Terrain.sealState(0) != 0x76042423afbd1648L) {
            throw new AssertionError("seal0 state");
        }
        pass("seed");
    }

    private static void checkWords(Json.Obj doc) {
        long[] lane = Terrain.lanes(Terrain.seed(Bytes.unhex(text(doc, "public_key_hex")), Bytes.unhex(text(doc, "seed_beacon_hex"))));
        for (Json item : Json.asArr(doc.fields().get("samples")).items()) {
            Json.Obj sample = Json.asObj(item);
            int i = (int) longNum(sample, "i");
            int j = (int) longNum(sample, "j");
            eq("nl4 " + i + "," + j, Bytes.pin(Terrain.baseWord(lane, i, j)), text(sample, "u64_hex"));
        }
        if (longNum(doc, "slot0_words") != 2048) {
            throw new AssertionError("slot0_words");
        }
        Profile one = Profile.conformanceSmall(1, 16384, 16384, 1);
        Terrain terrain = Terrain.fill(Bytes.unhex(text(doc, "public_key_hex")), Bytes.unhex(text(doc, "seed_beacon_hex")), one);
        if (terrain.profile.nSlots != 1 || terrain.blob.length != 16384) {
            throw new AssertionError("slot0 fill");
        }
        eq("slot0", Bytes.hex(Bytes.sha256(terrain.blob)), text(doc, "slot0_sha256"));
        pass("nl4-words");
    }

    private static void checkTerrain(Json.Obj doc) {
        byte[] publicKey = Bytes.unhex(text(doc, "public_key_hex"));
        byte[] beacon = Bytes.unhex(text(doc, "seed_beacon_hex"));
        for (Json item : Json.asArr(doc.fields().get("cases")).items()) {
            Json.Obj row = Json.asObj(item);
            int slot = (int) longNum(row, "slot_bytes");
            int bytes = (int) longNum(row, "terrain_bytes");
            Profile profile = Profile.conformanceSmall(1, slot, bytes, 1);
            Terrain terrain = Terrain.fill(publicKey, beacon, profile);
            eq("terrain " + bytes, Bytes.hex(Bytes.sha256(terrain.blob)), text(row, "sha256"));
        }
        pass("terrain-small");
    }

    private static void checkStep(Json.Obj doc) {
        String seal = Bytes.hex(Terrain.sealDigest(0));
        eq("seal0 digest", seal, text(doc, "seal0_digest_hex"));
        eq("seal0 state", Bytes.pin(Terrain.sealState(0)), text(doc, "seal0_s_prime_u64_hex"));
        Profile one = Profile.conformanceSmall(1, 16384, 16384, 1);
        Terrain terrain = Terrain.fill(Bytes.unhex(text(doc, "public_key_hex")), Bytes.unhex(text(doc, "seed_beacon_hex")), one);
        if (longNum(doc, "words") != 2048 || longNum(doc, "slot_index") != 0) {
            throw new AssertionError("step setup");
        }
        for (Json item : Json.asArr(doc.fields().get("steps")).items()) {
            Json.Obj step = Json.asObj(item);
            long state = Long.parseUnsignedLong(text(step, "S_u64_hex"), 16);
            long folded = terrain.foldSlot(state);
            eq("S_folded", Bytes.pin(folded), text(step, "S_folded_u64_hex"));
            byte[] digest = Terrain.sealDigest(folded);
            eq("digest", Bytes.hex(digest), text(step, "digest_hex"));
            eq("S'", Bytes.pin(Bytes.u64le(digest)), text(step, "S_prime_u64_hex"));
            if (terrain.step(state) != Bytes.u64le(digest)) {
                throw new AssertionError("step diverges from seal");
            }
        }
        pass("step-c1");
    }

    private static void checkHike(Json.Obj doc, String name) {
        Json.Obj sizes = Json.asObj(doc.fields().get("profile"));
        Profile profile = Profile.conformanceSmall(
                (int) longNum(sizes, "sample_bytes"),
                (int) longNum(sizes, "slot_bytes"),
                (int) longNum(sizes, "terrain_bytes"),
                (int) longNum(sizes, "steps_per_leg"));
        byte[] publicKey = Bytes.unhex(text(doc, "public_key_hex"));
        byte[] beacon = Bytes.unhex(text(doc, "seed_beacon_hex"));
        Terrain terrain = Terrain.fill(publicKey, beacon, profile);
        byte[] seed = Terrain.seed(publicKey, beacon);
        eq(name + " seed", Bytes.hex(seed), text(doc, "seed_hex"));
        eq(name + " opening", Bytes.hex(seed), text(doc, "opening_waypoint_hex"));
        if (longNum(doc, "waypoint_equals_seed") != 1) {
            throw new AssertionError(name + " waypoint");
        }
        eq(name + " terrain", Bytes.hex(Bytes.sha256(terrain.blob)), text(doc, "terrain_sha256"));
        byte[] waypoint = seed;
        int index = 0;
        for (Json item : Json.asArr(doc.fields().get("stages")).items()) {
            Json.Obj stage = Json.asObj(item);
            if (longNum(stage, "leg_count") != 1) {
                throw new AssertionError(name + " leg_count");
            }
            byte[] previous = waypoint;
            long leg = terrain.leg(previous);
            String legHex = Json.asString(Json.asArr(stage.fields().get("legs_le_hex")).items().get(0));
            eq(name + " leg " + index, Bytes.hex(Bytes.le64(leg)), legHex);
            byte[] sample = terrain.sample(previous);
            byte[] checkpoint = Bytes.sha256(previous, waypoint, sample, Bytes.le64(leg));
            String checkpointHex = Json.asString(Json.asArr(stage.fields().get("checkpoints_hex")).items().get(0));
            eq(name + " checkpoint " + index, Bytes.hex(checkpoint), checkpointHex);
            String nextBeacon = optionalHex(stage, "next_beacon_hex");
            String nextWaypoint = optionalHex(stage, "next_waypoint_hex");
            if (nextBeacon == null) {
                if (nextWaypoint != null) {
                    throw new AssertionError(name + " dangling waypoint");
                }
            } else {
                byte[] interrupt = Bytes.unhex(nextBeacon);
                byte[] fold = Bytes.sha256(new byte[32], checkpoint);
                byte[] built = Bytes.sha256(interrupt, terrain.slot(interrupt), waypoint, fold);
                eq(name + " waypoint " + (index + 1), Bytes.hex(built), nextWaypoint);
                waypoint = built;
            }
            index++;
        }
        pass(name);
    }

    private static void checkRate(Json.Obj doc) {
        Json.Obj floor = Json.asObj(doc.fields().get("floor"));
        long p = longNum(floor, "p");
        long q = longNum(floor, "q");
        if (p != Admit.PLACEHOLDER_FLOOR_P || q != Admit.PLACEHOLDER_FLOOR_Q) {
            throw new AssertionError("floor placeholder");
        }
        if (!"placeholder, not ratified".equals(text(floor, "mark"))) {
            throw new AssertionError("floor mark");
        }
        for (Json item : Json.asArr(doc.fields().get("cases")).items()) {
            Json.Obj row = Json.asObj(item);
            boolean meets = Admit.legRateMeets(longNum(row, "hike_legs"), longNum(row, "hike_span"), p, q);
            boolean want = bool(row, "meets");
            if (meets != want) {
                throw new AssertionError(text(row, "name") + " rate " + meets);
            }
            if (bool(row, "must_reject") && meets) {
                throw new AssertionError(text(row, "name") + " was not rejected");
            }
            if (!meets) {
                System.out.println("REJECT rate " + text(row, "name"));
            }
        }
        pass("rate");
    }

    private static void checkAnchors(Json.Obj doc) {
        if (longNum(doc, "minimum") != Admit.MIN_ANCHORED_PATHS) {
            throw new AssertionError("minimum");
        }
        for (Json item : Json.asArr(doc.fields().get("cases")).items()) {
            Json.Obj row = Json.asObj(item);
            Json.Arr indices = Json.asArr(row.fields().get("indices"));
            int[] got = new int[indices.items().size()];
            for (int i = 0; i < got.length; i++) {
                got[i] = (int) Long.parseLong(Json.asNumber(indices.items().get(i)));
            }
            boolean meets = Admit.anchorsMeet(got);
            if (meets != bool(row, "meets") || meets != Admit.anchorsMeetCount(got.length)) {
                throw new AssertionError(text(row, "name") + " anchors");
            }
            if (bool(row, "must_reject") && meets) {
                throw new AssertionError(text(row, "name") + " was not rejected");
            }
            if (!meets) {
                System.out.println("REJECT anchors " + text(row, "name"));
            }
        }
        // No first-planted rule and no last-planted rule: six indices that skip 1 and 16 still meet.
        if (!Admit.anchorsMeet(2, 4, 6, 8, 10, 12) || Admit.anchorsMeet(1, 2, 3, 4, 5)) {
            throw new AssertionError("anchor placement");
        }
        pass("anchors");
    }

    private static void checkNegatives(Json.Obj doc) {
        byte[] publicKey = Bytes.unhex(text(doc, "public_key_hex"));
        byte[] beacon = Bytes.unhex(text(doc, "seed_beacon_hex"));
        for (Json item : Json.asArr(doc.fields().get("cases")).items()) {
            Json.Obj row = Json.asObj(item);
            String name = text(row, "name");
            switch (name) {
                case "fill_pin_terrain" -> rejectTerrain(name, publicKey, beacon, row);
                case "ref_word", "all4_word" -> rejectWord(name, publicKey, beacon, row);
                case "mix64_without_mix_add" -> rejectFold(name, publicKey, beacon, row, Adversarial.Kind.NO_ADD);
                case "fatslot_fold" -> rejectFold(name, publicKey, beacon, row, Adversarial.Kind.FATSLOT);
                case "partial_slot_read" -> rejectFold(name, publicKey, beacon, row, Adversarial.Kind.PARTIAL);
                case "f3_shared_terrain" -> rejectShared(name, publicKey, beacon, row);
                default -> throw new AssertionError("unknown negative " + name);
            }
            if (!bool(row, "must_reject")) {
                throw new AssertionError(name + " must_reject");
            }
            System.out.println("REJECT " + name);
        }
        pass("negatives");
    }

    private static void rejectTerrain(String name, byte[] publicKey, byte[] beacon, Json.Obj row) {
        Profile profile = Profile.conformanceSmall(1, (int) longNum(row, "slot_bytes"), (int) longNum(row, "terrain_bytes"), 1);
        String got = Bytes.hex(Bytes.sha256(Terrain.fill(publicKey, beacon, profile).blob));
        reject(name, got, text(row, "conforming_sha256"), text(row, "forbidden_sha256"));
    }

    private static void rejectWord(String name, byte[] publicKey, byte[] beacon, Json.Obj row) {
        long[] lane = Terrain.lanes(Terrain.seed(publicKey, beacon));
        int i = (int) longNum(row, "i");
        int j = (int) longNum(row, "j");
        String got = Bytes.pin(Terrain.baseWord(lane, i, j));
        reject(name, got, text(row, "conforming_u64_hex"), text(row, "forbidden_u64_hex"));
        long withdrawn = "ref_word".equals(name) ? Adversarial.refWord(lane, i, j) : Adversarial.all4Word(lane, i, j);
        if (!Bytes.pin(withdrawn).equals(text(row, "forbidden_u64_hex"))) {
            throw new AssertionError(name + " withdrawn reconstruction " + Bytes.pin(withdrawn));
        }
        if (withdrawn == Terrain.baseWord(lane, i, j)) {
            throw new AssertionError(name + " verifier matched the withdrawn word");
        }
    }

    private static void rejectFold(String name, byte[] publicKey, byte[] beacon, Json.Obj row, Adversarial.Kind kind) {
        Profile one = Profile.conformanceSmall(1, 16384, 16384, 1);
        Terrain terrain = Terrain.fill(publicKey, beacon, one);
        long state = Long.parseUnsignedLong(text(row, "S_u64_hex"), 16);
        if (longNum(row, "words") != terrain.profile.wordsPerSlot) {
            throw new AssertionError(name + " words");
        }
        String got = Bytes.pin(terrain.foldSlot(state));
        reject(name, got, text(row, "conforming_S_folded_u64_hex"), text(row, "forbidden_S_folded_u64_hex"));
        String withdrawn = Bytes.pin(Adversarial.withdrawnFold(terrain, state, kind, (int) longNum(row, "words_read")));
        if (!withdrawn.equals(text(row, "forbidden_S_folded_u64_hex"))) {
            throw new AssertionError(name + " withdrawn reconstruction " + withdrawn);
        }
        if (withdrawn.equals(got)) {
            throw new AssertionError(name + " verifier matched the withdrawn fold");
        }
    }

    private static void rejectShared(String name, byte[] publicKey, byte[] beacon, Json.Obj row) {
        Profile profile = Profile.conformanceSmall(1, (int) longNum(row, "slot_bytes"), (int) longNum(row, "terrain_bytes"), 1);
        String first = Bytes.hex(Bytes.sha256(Terrain.fill(publicKey, beacon, profile).blob));
        String second = Bytes.hex(Bytes.sha256(Terrain.fill(Bytes.unhex(text(row, "pk2_hex")), beacon, profile).blob));
        reject(name + " pk1", first, text(row, "conforming_sha256_pk1"), text(row, "forbidden_sha256_pk1"));
        reject(name + " pk2", second, text(row, "conforming_sha256_pk2"), text(row, "forbidden_sha256_pk2"));
        if (first.equals(second)) {
            throw new AssertionError(name + " shared terrain");
        }
    }

    /** The recomputed value verifies. The withdrawn value does not. */
    private static void reject(String name, String got, String conforming, String forbidden) {
        if (!conforming.equals(got)) {
            throw new AssertionError(name + " got " + got + " want " + conforming);
        }
        if (forbidden.equals(got)) {
            throw new AssertionError(name + " emitted the withdrawn result");
        }
    }

    private static void pass(String name) {
        ROWS.add(name + " match");
    }

    private static void eq(String name, String got, String want) {
        if (!want.equals(got)) {
            throw new AssertionError(name + " got " + got + " want " + want);
        }
    }

    private static Json.Obj load(Path path) throws Exception {
        return Json.asObj(Json.parse(Files.readString(path)));
    }

    private static String text(Json.Obj obj, String key) {
        return Json.asString(obj.fields().get(key));
    }

    private static String optionalHex(Json.Obj obj, String key) {
        Json value = obj.fields().get(key);
        if (value == null || value instanceof Json.Null) {
            return null;
        }
        return Json.asString(value);
    }

    private static long longNum(Json.Obj obj, String key) {
        Json value = obj.fields().get(key);
        if (value == null) {
            return 0;
        }
        return Long.parseLong(Json.asNumber(value));
    }

    private static boolean bool(Json.Obj obj, String key) {
        Json value = obj.fields().get(key);
        if (value == null) {
            return false;
        }
        if (value instanceof Json.Bool flag) {
            return flag.value;
        }
        throw new AssertionError(key);
    }
}
