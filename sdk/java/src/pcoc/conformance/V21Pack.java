package pcoc.conformance;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import pcoc.hike.Admit;
import pcoc.hike.Bytes;
import pcoc.hike.Cbor;
import pcoc.hike.Hike;
import pcoc.hike.Measure;
import pcoc.hike.Profile;
import pcoc.hike.Spec6Pins;
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
        checkManifest(pack, load(pack.resolve("manifest.json")));
        checkConstants(load(pack.resolve("constants.json")));
        checkSeed(load(pack.resolve("seed.json")));
        checkWords(load(pack.resolve("nl4-words.json")));
        checkTerrain(load(pack.resolve("terrain-small.json")));
        checkStep(load(pack.resolve("step-c1.json")));
        checkHike(load(pack.resolve("kat-small.json")), "kat-small");
        checkHike(load(pack.resolve("test-3leg.json")), "test-3leg");
        checkRate(load(pack.resolve("rate.json")));
        checkAnchors(load(pack.resolve("anchors.json")));
        checkNegatives(load(pack.resolve("negatives.json")));
        checkKind(load(pack.resolve("kind.json")));
        checkDeadline(load(pack.resolve("deadline.json")));
        checkMtp(load(pack.resolve("mtp.json")));
        checkRounds(load(pack.resolve("rounds.json")));
        checkProd(load(pack.resolve("prod-leg0.json")));
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
            String outcome = optionalText(row, "outcome");
            if (outcome != null && longNum(row, "hike_span") <= 0 && !"refuse".equals(outcome)) {
                throw new AssertionError(text(row, "name") + " span outcome");
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
                case "v2_0_fold" -> rejectFold(name, publicKey, beacon, row, Adversarial.Kind.V2_0);
                case "xor_accumulator_fold" -> rejectFold(name, publicKey, beacon, row, Adversarial.Kind.XOR_ACC);
                case "xor_w_fold" -> rejectFold(name, publicKey, beacon, row, Adversarial.Kind.XOR_W);
                case "partial_slot_read", "partial_slot_read_2047" -> rejectFold(name, publicKey, beacon, row, Adversarial.Kind.PARTIAL);
                case "one_lane_flip" -> rejectLaneFlip(name, publicKey, beacon, row);
                case "all4_kernel" -> rejectAll4(name, publicKey, beacon, row);
                case "forged_window" -> rejectWindow(name, publicKey, beacon, row);
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

    private static void rejectLaneFlip(String name, byte[] publicKey, byte[] beacon, Json.Obj row) {
        long[] lane = Terrain.lanes(Terrain.seed(publicKey, beacon));
        Profile profile = Profile.conformanceSmall(1, (int) longNum(row, "slot_bytes"), (int) longNum(row, "terrain_bytes"), 1);
        String conforming = Bytes.hex(Bytes.sha256(Terrain.fillLanes(lane, profile).blob));
        reject(name, conforming, text(row, "conforming_sha256"), text(row, "forbidden_sha256"));
        int laneIndex = (int) longNum(row, "lane");
        long[] flipped = lane.clone();
        flipped[laneIndex] ^= Long.parseUnsignedLong(text(row, "flip_u64_hex"), 16);
        int words = profile.nSlots * profile.wordsPerSlot;
        if (words != longNum(row, "words")) {
            throw new AssertionError(name + " words");
        }
        int nl4 = 0;
        int ref = 0;
        int pair = 0;
        for (int i = 0; i < profile.nSlots; i++) {
            for (int j = 0; j < profile.wordsPerSlot; j++) {
                if (Terrain.baseWord(lane, i, j) != Terrain.baseWord(flipped, i, j)) {
                    nl4++;
                }
                if (Adversarial.refWord(lane, i, j) != Adversarial.refWord(flipped, i, j)) {
                    ref++;
                }
                if (Adversarial.pairWord(lane, i, j) != Adversarial.pairWord(flipped, i, j)) {
                    pair++;
                }
            }
        }
        if (nl4 != longNum(row, "nl4_changed") || ref != longNum(row, "ref_changed") || pair != longNum(row, "pair_changed")) {
            throw new AssertionError(name + " changed nl4 " + nl4 + " ref " + ref + " pair " + pair);
        }
        String forbidden = Bytes.hex(Bytes.sha256(Terrain.fillLanes(flipped, profile).blob));
        if (!forbidden.equals(text(row, "forbidden_sha256")) || forbidden.equals(conforming)) {
            throw new AssertionError(name + " flipped terrain " + forbidden);
        }
    }

    private static void rejectAll4(String name, byte[] publicKey, byte[] beacon, Json.Obj row) {
        long[] lane = Terrain.lanes(Terrain.seed(publicKey, beacon));
        Profile profile = Profile.conformanceSmall(1, (int) longNum(row, "slot_bytes"), (int) longNum(row, "terrain_bytes"), 1);
        String conforming = Bytes.hex(Bytes.sha256(Terrain.fillLanes(lane, profile).blob));
        reject(name, conforming, text(row, "conforming_sha256"), text(row, "forbidden_sha256"));
        long mask = Long.parseUnsignedLong(text(row, "xor_u64_hex"), 16);
        long[] flipped = lane.clone();
        for (Json item : Json.asArr(row.fields().get("lanes")).items()) {
            flipped[(int) Long.parseLong(Json.asNumber(item))] ^= mask;
        }
        int words = profile.nSlots * profile.wordsPerSlot;
        int nl4 = 0;
        for (int i = 0; i < profile.nSlots; i++) {
            for (int j = 0; j < profile.wordsPerSlot; j++) {
                if (Terrain.baseWord(lane, i, j) != Terrain.baseWord(flipped, i, j)) {
                    nl4++;
                }
                if (Adversarial.all4Word(lane, i, j) != Adversarial.all4Word(flipped, i, j)) {
                    throw new AssertionError(name + " linear word moved");
                }
            }
        }
        if (nl4 != longNum(row, "nl4_changed") || words != longNum(row, "words")) {
            throw new AssertionError(name + " nl4 " + nl4);
        }
        String linear = Bytes.hex(Bytes.sha256(Adversarial.linearTerrain(lane, profile)));
        String linearFlipped = Bytes.hex(Bytes.sha256(Adversarial.linearTerrain(flipped, profile)));
        eq(name + " all4", linear, text(row, "all4_sha256"));
        eq(name + " all4 flipped", linearFlipped, text(row, "all4_sha256_flipped"));
        if (!linear.equals(linearFlipped)) {
            throw new AssertionError(name + " linear terrain moved");
        }
        String forbidden = Bytes.hex(Bytes.sha256(Terrain.fillLanes(flipped, profile).blob));
        if (!forbidden.equals(text(row, "forbidden_sha256")) || forbidden.equals(conforming)) {
            throw new AssertionError(name + " nl4 flipped " + forbidden);
        }
    }

    private static void rejectWindow(String name, byte[] publicKey, byte[] beacon, Json.Obj row) {
        if (!"shifted-one".equals(text(row, "forbidden_window"))) {
            throw new AssertionError(name + " window");
        }
        if (bool(row, "fake_leg_ignore")) {
            throw new AssertionError(name + " fake leg");
        }
        Profile profile = Profile.conformanceSmall(
                (int) longNum(row, "sample_bytes"),
                (int) longNum(row, "slot_bytes"),
                (int) longNum(row, "terrain_bytes"),
                1);
        byte[] seed = Terrain.seed(publicKey, beacon);
        Terrain terrain = Terrain.fillLanes(Terrain.lanes(seed), profile);
        int offset = (int) longNum(row, "sample_offset");
        int derived = (int) Long.remainderUnsigned(Bytes.u64le(seed), profile.offsetModulus);
        if (derived != offset) {
            throw new AssertionError(name + " offset " + derived);
        }
        long leg = Long.parseUnsignedLong(text(row, "leg_u64_hex"), 16);
        byte[] conforming = Adversarial.checkpointAt(seed, seed, terrain.blob, offset, profile.sampleBytes, leg);
        byte[] shifted = Adversarial.checkpointAt(seed, seed, terrain.blob, offset + 1, profile.sampleBytes, leg);
        eq(name + " window", Bytes.hex(conforming), text(row, "conforming_hex"));
        eq(name + " shifted", Bytes.hex(shifted), text(row, "forbidden_hex"));
        if (Arrays.equals(conforming, shifted)) {
            throw new AssertionError(name + " windows matched");
        }
    }

    private static void checkManifest(Path pack, Json.Obj doc) throws Exception {
        if (!"e2451caf9df3cb70debb0d6d99dc11cab20bca3729f7996f8f6d4f521484d50b".equals(text(doc, "vector_sha256"))) {
            throw new AssertionError("manifest pin");
        }
        eq("manifest", packHash(pack), text(doc, "vector_sha256"));
        pass("manifest");
    }

    /** Section 6 names in {@code constants.json}, including the strings {@code Spec6Pins} already formats. */
    private static void checkConstants(Json.Obj doc) {
        if (!"constants".equals(text(doc, "id"))) {
            throw new AssertionError("constants id");
        }
        java.util.Map<String, Object> want = Spec6Pins.section6();
        if (doc.fields().size() != want.size() + 1) {
            throw new AssertionError("constants keys");
        }
        for (java.util.Map.Entry<String, Object> entry : want.entrySet()) {
            Json value = doc.fields().get(entry.getKey());
            if (value == null) {
                throw new AssertionError("constants missing " + entry.getKey());
            }
            Object expected = entry.getValue();
            if (expected instanceof String text) {
                if (!text.equals(Json.asString(value))) {
                    throw new AssertionError("constants " + entry.getKey());
                }
            } else if (Long.parseLong(Json.asNumber(value)) != (Long) expected) {
                throw new AssertionError("constants " + entry.getKey());
            }
        }
        pass("constants");
    }

    /** SHA-256 over the pack except manifest.json: repo-relative path, NUL, sha256(bytes), sorted by path. */
    private static String packHash(Path pack) throws Exception {
        List<Path> files = new ArrayList<>();
        try (var stream = Files.list(pack)) {
            stream.filter(path -> !"manifest.json".equals(path.getFileName().toString())).forEach(files::add);
        }
        files.sort(Comparator.comparing(path -> path.getFileName().toString()));
        MessageDigest digest = Bytes.digest();
        for (Path file : files) {
            String rel = "spec/conformance/v2.1/" + file.getFileName();
            digest.update(rel.getBytes(StandardCharsets.UTF_8));
            digest.update((byte) 0);
            digest.update(Bytes.sha256(Files.readAllBytes(file)));
        }
        return Bytes.hex(digest.digest());
    }

    private static void checkKind(Json.Obj doc) {
        if (!Hike.KIND.equals(text(doc, "kind")) || Hike.KIND.length() != longNum(doc, "kind_bytes")) {
            throw new AssertionError("kind");
        }
        if (!Admit.EARLIER_VERSION_REASON.equals(text(doc, "earlier_reason"))) {
            throw new AssertionError("earlier reason");
        }
        for (Json item : Json.asArr(doc.fields().get("earlier")).items()) {
            String kind = Json.asString(item);
            if (!Hike.isEarlierKind(kind)) {
                throw new AssertionError("not earlier " + kind);
            }
            Admit.Gate gate = Admit.earlierVersion(kind);
            if (!"refuse".equals(gate.outcome) || !Admit.EARLIER_VERSION_REASON.equals(gate.reason) || gate.fakeLegIgnore) {
                throw new AssertionError("earlier gate " + kind);
            }
            byte[] raw = Cbor.encodeMap(List.of(Map.entry("kind", Cbor.encodeText(kind))));
            String line = Measure.record(raw, Profile.CONFORMANCE_SMALL_V2, height -> {
                throw new AssertionError("index read before the earlier-kind return");
            }, null, true);
            String want = "{\"version\":\"earlier version\",\"kind\":\"" + kind + "\"}\n";
            if (!want.equals(line)) {
                throw new AssertionError("earlier record " + line);
            }
            System.out.println("REJECT earlier_version " + kind);
        }
        if (Hike.isEarlierKind(Hike.KIND)) {
            throw new AssertionError("current kind refused");
        }
        pass("kind");
    }

    private static void checkDeadline(Json.Obj doc) {
        if (longNum(doc, "K") != Admit.PLACEHOLDER_DEADLINE_K || !"placeholder, not ratified".equals(text(doc, "K_mark"))) {
            throw new AssertionError("deadline placeholder");
        }
        if (!Admit.SUBMISSION_DEADLINE.equals(text(doc, "reason"))) {
            throw new AssertionError("deadline reason");
        }
        for (Json item : Json.asArr(doc.fields().get("cases")).items()) {
            Json.Obj row = Json.asObj(item);
            Long tip = optionalLong(row, "tip_height");
            Admit.Gate gate = Admit.submissionDeadline(tip, longNum(row, "last_beacon_height"), (int) longNum(row, "K"));
            if (!text(row, "outcome").equals(gate.outcome) || gate.fakeLegIgnore != bool(row, "fake_leg_ignore")) {
                throw new AssertionError(text(row, "name") + " deadline " + gate.outcome);
            }
            String reason = optionalText(row, "reason");
            if (reason != null && !reason.equals(gate.reason)) {
                throw new AssertionError(text(row, "name") + " reason");
            }
            if ("refuse".equals(gate.outcome) && gate.reason == null) {
                throw new AssertionError(text(row, "name") + " missing reason");
            }
            if (bool(row, "must_reject") && !"refuse".equals(gate.outcome)) {
                throw new AssertionError(text(row, "name") + " was not rejected");
            }
            if ("refuse".equals(gate.outcome) || "held".equals(gate.outcome)) {
                System.out.println("REJECT deadline " + text(row, "name"));
            }
        }
        pass("deadline");
    }

    private static void checkMtp(Json.Obj doc) {
        if (!"A4.10".equals(text(doc, "rule"))) {
            throw new AssertionError("mtp rule");
        }
        for (Json item : Json.asArr(doc.fields().get("cases")).items()) {
            Json.Obj row = Json.asObj(item);
            String name = text(row, "name");
            Long first = checkBeacon(name, Json.asObj(row.fields().get("first")));
            Json lastValue = row.fields().get("last");
            Long last = lastValue == null || lastValue instanceof Json.Null ? null : checkBeacon(name, Json.asObj(lastValue));
            Long span = Admit.hikeSpan(first, last);
            Json spanValue = row.fields().get("hike_span");
            if (spanValue instanceof Json.Str) {
                if (span != null || !"not read".equals(Json.asString(spanValue))) {
                    throw new AssertionError(name + " hike_span");
                }
            } else if (spanValue != null) {
                if (span == null || span != Long.parseLong(Json.asNumber(spanValue))) {
                    throw new AssertionError(name + " hike_span " + span);
                }
            }
            Json raw = row.fields().get("raw_span");
            if (raw instanceof Json.Str) {
                if (!"not read".equals(Json.asString(raw))) {
                    throw new AssertionError(name + " raw");
                }
            } else if (raw != null && first != null && last != null) {
                long rawSpan = rawNtime(Json.asObj(row.fields().get("last"))) - rawNtime(Json.asObj(row.fields().get("first")));
                if (rawSpan != Long.parseLong(Json.asNumber(raw))) {
                    throw new AssertionError(name + " raw_span " + rawSpan);
                }
                if (span != null && rawSpan == span) {
                    throw new AssertionError(name + " raw ntime used as hike_span");
                }
            }
            String outcome = text(row, "outcome");
            if (span != null && span > 0 && !"read".equals(outcome)) {
                throw new AssertionError(name + " positive span");
            }
            if (span != null && span <= 0 && !"refuse".equals(outcome)) {
                throw new AssertionError(name + " non-positive span");
            }
            if (span == null && !"held".equals(outcome) && row.fields().get("hike_span") != null) {
                throw new AssertionError(name + " missing span");
            }
            if (bool(row, "must_reject") && span != null && span > 0 && Admit.placeholderRateMeets(1, span)) {
                throw new AssertionError(name + " was not rejected");
            }
            if ("refuse".equals(outcome) || "held".equals(outcome)) {
                System.out.println("REJECT mtp " + name);
            }
        }
        pass("mtp");
    }

    private static Long checkBeacon(String name, Json.Obj beacon) {
        long height = longNum(beacon, "height");
        int n = (int) longNum(beacon, "n");
        if (n != Admit.mtpCount(height)) {
            throw new AssertionError(name + " n");
        }
        Json.Arr reads = Json.asArr(beacon.fields().get("reads"));
        if (reads.items().size() != n) {
            throw new AssertionError(name + " reads");
        }
        long[] times = new long[n];
        boolean missing = false;
        for (int i = 0; i < n; i++) {
            long want = height - n + 1 + i;
            if (Long.parseLong(Json.asNumber(reads.items().get(i))) != want) {
                throw new AssertionError(name + " read " + i);
            }
            Json time = Json.asArr(beacon.fields().get("times")).items().get(i);
            if (time instanceof Json.Null) {
                missing = true;
            } else {
                times[i] = Long.parseLong(Json.asNumber(time));
            }
        }
        Json raw = beacon.fields().get("raw_ntime");
        Json lastTime = Json.asArr(beacon.fields().get("times")).items().get(n - 1);
        if (raw instanceof Json.Null) {
            if (!(lastTime instanceof Json.Null)) {
                throw new AssertionError(name + " raw ntime");
            }
        } else if (Long.parseLong(Json.asNumber(raw)) != Long.parseLong(Json.asNumber(lastTime))) {
            throw new AssertionError(name + " raw ntime");
        }
        Json wantMtp = beacon.fields().get("mtp");
        if (missing || wantMtp instanceof Json.Null) {
            if (!missing || !(wantMtp instanceof Json.Null)) {
                throw new AssertionError(name + " mtp missing");
            }
            return null;
        }
        long got = Admit.medianTimePast(times);
        if (got != Long.parseLong(Json.asNumber(wantMtp))) {
            throw new AssertionError(name + " mtp " + got);
        }
        return got;
    }

    private static long rawNtime(Json.Obj beacon) {
        return Long.parseLong(Json.asNumber(beacon.fields().get("raw_ntime")));
    }

    private static void checkRounds(Json.Obj doc) {
        if (longNum(doc, "thorough_rounds") != Admit.THOROUGH_ROUNDS || longNum(doc, "per_round") != Admit.PER_ROUND) {
            throw new AssertionError("rounds size");
        }
        for (Json item : Json.asArr(doc.fields().get("cases")).items()) {
            Json.Obj row = Json.asObj(item);
            String name = text(row, "name");
            long[] draws = draws(row);
            Integer mismatch = row.fields().containsKey("mismatch_on_round") ? (int) longNum(row, "mismatch_on_round") : null;
            Admit.Thorough got = Admit.thoroughRounds(longNum(row, "leg_total"), draws, mismatch);
            if (!text(row, "outcome").equals(got.outcome) || longNum(row, "rounds_started") != got.roundsStarted) {
                throw new AssertionError(name + " outcome " + got.outcome + " started " + got.roundsStarted);
            }
            String draw = optionalText(row, "draw");
            if (draw != null && !draw.equals(got.draw)) {
                throw new AssertionError(name + " draw");
            }
            Json.Arr rounds = Json.asArr(row.fields().get("rounds"));
            if (rounds.items().size() != got.rounds.length) {
                throw new AssertionError(name + " round count");
            }
            for (int r = 0; r < got.rounds.length; r++) {
                Json.Arr group = Json.asArr(rounds.items().get(r));
                if (group.items().size() != got.rounds[r].length) {
                    throw new AssertionError(name + " group");
                }
                for (int j = 0; j < got.rounds[r].length; j++) {
                    if (got.rounds[r][j] != Long.parseLong(Json.asNumber(group.items().get(j)))) {
                        throw new AssertionError(name + " index " + r + "," + j);
                    }
                }
            }
            if (bool(row, "must_reject") && !"refuse".equals(got.outcome)) {
                throw new AssertionError(name + " was not rejected");
            }
            if ("refuse".equals(got.outcome) || "not read".equals(got.outcome)) {
                System.out.println("REJECT rounds " + name);
            }
        }
        pass("rounds");
    }

    private static long[] draws(Json.Obj row) {
        Json value = row.fields().get("draws_u64_hex");
        if (value == null) {
            return new long[0];
        }
        Json.Arr arr = Json.asArr(value);
        long[] draws = new long[arr.items().size()];
        for (int i = 0; i < draws.length; i++) {
            draws[i] = Long.parseUnsignedLong(Json.asString(arr.items().get(i)), 16);
        }
        return draws;
    }

    private static void checkProd(Json.Obj doc) throws Exception {
        if (!"production".equals(text(doc, "profile")) || longNum(doc, "steps") != Profile.PRODUCTION.stepsPerLeg) {
            throw new AssertionError("prod profile");
        }
        byte[] publicKey = Bytes.unhex(text(doc, "public_key_hex"));
        byte[] beacon = Bytes.unhex(text(doc, "seed_beacon_hex"));
        long fillStart = System.nanoTime();
        Terrain terrain = Terrain.fill(publicKey, beacon, Profile.PRODUCTION);
        long filled = System.nanoTime();
        eq("prod terrain", Bytes.hex(Bytes.sha256(terrain.blob)), text(doc, "terrain_sha256"));
        byte[] seed = Terrain.seed(publicKey, beacon);
        int offset = terrain.sampleOffset(seed);
        if (offset != longNum(doc, "sample_offset")) {
            throw new AssertionError("prod offset " + offset);
        }
        long legStart = System.nanoTime();
        long leg = terrain.leg(seed);
        long legDone = System.nanoTime();
        eq("prod leg", Bytes.pin(leg), text(doc, "leg0_u64_hex"));
        byte[] sample = terrain.sample(seed);
        byte[] checkpoint = Bytes.sha256(seed, seed, sample, Bytes.le64(leg));
        eq("prod checkpoint", Bytes.hex(checkpoint), text(doc, "checkpoint0_hex"));
        System.out.printf(java.util.Locale.ROOT, "prod-leg0 fill_s %.9f leg_s %.9f%n", (filled - fillStart) / 1e9, (legDone - legStart) / 1e9);
        pass("prod-leg0");
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

    private static String optionalText(Json.Obj obj, String key) {
        Json value = obj.fields().get(key);
        if (value == null || value instanceof Json.Null) {
            return null;
        }
        return Json.asString(value);
    }

    private static Long optionalLong(Json.Obj obj, String key) {
        Json value = obj.fields().get(key);
        if (value == null || value instanceof Json.Null) {
            return null;
        }
        return Long.parseLong(Json.asNumber(value));
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
