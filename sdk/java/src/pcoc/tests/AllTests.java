package pcoc.tests;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;

import pcoc.Bytes;
import pcoc.Cbor;
import pcoc.Const;
import pcoc.Draw;
import pcoc.Hashcash;
import pcoc.Hike;
import pcoc.HikeObject;
import pcoc.Index;
import pcoc.Mix;
import pcoc.Pin;
import pcoc.Profile;
import pcoc.Seed;
import pcoc.Sha256;
import pcoc.Step;
import pcoc.Terrain;
import pcoc.Verify;
import pcoc.Version;
import pcoc.cli.Main;
import pcoc.gen.GenCandidates;
import pcoc.negatives.Negatives;

/** The acceptance checks. One process, no sleep. */
public final class AllTests {
    private static int failures;

    private AllTests() {}

    public static void main(String[] args) throws Exception {
        Path repo = Path.of(".");
        boolean core = false;
        boolean pinOnly = false;
        boolean full = true;
        for (int i = 0; i < args.length; i++) {
            if ("--repo".equals(args[i])) {
                repo = Path.of(args[++i]);
            } else if ("--core".equals(args[i])) {
                core = true;
                full = false;
            } else if ("--pin-only".equals(args[i])) {
                pinOnly = true;
                full = false;
            }
        }
        if (pinOnly) {
            pin(repo, false);
            finish();
            return;
        }
        known();
        version();
        pin(repo, true);
        imports(repo);
        foldSource(repo);
        Profile s1 = Profile.conformanceSmall(64, 32, 65536, 100);
        Hike.Result hike = hikeS1(s1);
        roundTrip(hike);
        unknown(s1);
        honest(s1, hike, "S1");
        negatives(s1, hike);
        Profile s2 = Profile.conformanceSmall(256, 64, 1048576, 1000);
        honest(s2, hikeS2(s2), "S2");
        cli(s1);
        if (full && !core) {
            regen(repo);
        }
        finish();
    }

    private static void known() {
        Sha256 sha = new Sha256();
        check(Arrays.equals(Bytes.le64(1L), Bytes.parseHex("0100000000000000")), "le64 1");
        check(Arrays.equals(Bytes.le64(0x0102030405060708L), Bytes.parseHex("0807060504030201")), "le64 wide");
        check(Bytes.hex(sha.hash("abc".getBytes(StandardCharsets.US_ASCII)))
                .equals("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"), "sha abc");
        byte[] buf = new byte[20];
        System.arraycopy(Const.SEAL_DOMAIN, 0, buf, 0, 12);
        byte[] seal = sha.hash(Arrays.copyOf(buf, 20));
        check(Bytes.hex(seal).equals("4816bdaf23240476ca57e168301eceee8eb8170ec5446863855e4ceebb55b533"), "seal 0");
        check(Step.seal(sha, buf, 0L) == 0x76042423afbd1648L, "S prime");

        byte[] preA = new byte[32];
        byte[] preB = new byte[32];
        Arrays.fill(preA, (byte) 0x11);
        Arrays.fill(preB, (byte) 0x22);
        byte[] seed = Seed.derive(sha, preA, preB);
        check(Bytes.hex(seed).equals("5189c77d29fe5d546a045ec46986852785fea5c13ac7da9c115ff5fb6edf817c"), "seed");
        long[] lanes = Seed.lanes(seed);
        check(lanes[0] == 0x545dfe297dc78951L, "P0");
        check(lanes[1] == 0x27858669c45e046aL, "P1");
        check(lanes[2] == 0x9cdac73ac1a5fe85L, "P2");
        check(lanes[3] == 0x7c81df6efbf55f11L, "P3");
        check(Terrain.baseWord(lanes, 0, 0) == 0xc7582082a632c2c8L, "nl4 base");

        check(Mix.bitrevParent(1) == 0 && Mix.mulParent(1) == 0, "parents 1");
        check(Mix.bitrevParent(2) == 0 && Mix.mulParent(2) == 0, "parents 2");
        check(Mix.bitrevParent(3) == 1 && Mix.mulParent(3) == 0, "parents 3");
        check(Mix.bitrevParent(5) == 2 && Mix.mulParent(5) == 4, "parents 5");
        check(Mix.bitrevParent(1023) == 511 && Mix.mulParent(1023) == 26, "parents 1023");

        Profile tiny = Profile.conformanceSmall(32, 8, 128, 1);
        long[] terrain = Terrain.fill(tiny, lanes);
        check(terrain[0] == 0xc7582082a632c2c8L, "slot0 j0");
        check(terrain[1 * 4] == 0x5890986ef9a9b67bL, "slot1 j0");
        check(terrain[1 * 4 + 1] == 0x2e57a9045417eb71L, "slot1 j1");
        check(terrain[2 * 4] == 0xa83f4dcd64f4e7d7L, "slot2 j0");
        check(terrain[3 * 4 + 3] == 0x3076d49c39f47d13L, "slot3 j3");

        long s = 1L;
        long manual = 0L;
        for (int w = 0; w < tiny.wordsPerSlot; w++) {
            manual += Mix.mix64(terrain[w] ^ s);
        }
        check(Step.foldSlot(terrain, tiny.wordsPerSlot, 0, s) == manual, "fold all words");

        byte[] zeros = new byte[32];
        check(Hashcash.leadingZeroBits(zeros) == 256, "zeros");
        byte[] one = new byte[32];
        one[0] = 0x01;
        check(Hashcash.leadingZeroBits(one) == 7, "seven zeros");
        check(Version.STRING.equals("2.2.0"), "constant");
    }

    private static void version() throws Exception {
        java.io.ByteArrayOutputStream bos = new java.io.ByteArrayOutputStream();
        java.io.PrintStream old = System.out;
        System.setOut(new java.io.PrintStream(bos, true, StandardCharsets.UTF_8));
        int code;
        try {
            code = Main.run(new String[] {"version"});
        } finally {
            System.setOut(old);
        }
        check(code == 0, "version exit");
        check("2.2.0\n".equals(bos.toString(StandardCharsets.UTF_8)), "version text");
        int usage = Main.run(new String[] {"verify", "--profile", "conformance-small"});
        check(usage == 2, "usage exit");
    }

    private static void pin(Path repo, boolean copyFlip) throws Exception {
        Pin pin = Pin.read(repo.resolve("sdk/SPEC_PIN"));
        check("spec-2.2.0".equals(pin.tag), "pin tag");
        Path manifest = repo.resolve("spec/conformance/pack/manifest.json");
        check(pin.matches(manifest), "pin match");
        if (!copyFlip) {
            return;
        }
        byte[] raw = Files.readAllBytes(manifest);
        raw[raw.length - 1] = (byte) ((raw[raw.length - 1] & 0xff) ^ 0x01);
        Path tmp = Files.createTempFile("pcoc-manifest", ".json");
        try {
            Files.write(tmp, raw);
            check(!pin.matches(tmp), "pin mismatch on one flipped byte");
        } finally {
            Files.deleteIfExists(tmp);
        }
    }

    private static void imports(Path repo) throws Exception {
        Path src = repo.resolve("sdk/java/src");
        try (var walk = Files.walk(src)) {
            for (Path p : (Iterable<Path>) walk.filter(Files::isRegularFile)::iterator) {
                String rel = src.relativize(p).toString().replace('\\', '/');
                if (!rel.endsWith(".java")) {
                    continue;
                }
                String text = Files.readString(p);
                boolean allowed = rel.startsWith("pcoc/negatives/") || rel.startsWith("pcoc/gen/")
                        || rel.startsWith("pcoc/tests/");
                if (!allowed) {
                    check(!text.contains("pcoc.negatives"), "import " + rel);
                }
                check(!text.contains("accept") || rel.startsWith("pcoc/tests/") || text.contains("does not"), rel);
            }
        }
    }

    private static void foldSource(Path repo) throws Exception {
        String step = Files.readString(repo.resolve("sdk/java/src/pcoc/Step.java"));
        check(step.contains("for (int w = 0; w < wordsPerSlot; w++)"), "fold loop");
        check(step.contains("acc += Mix.mix64(word ^ s)"), "fold add");
        Path src = repo.resolve("sdk/java/src");
        try (var walk = Files.walk(src)) {
            for (Path p : (Iterable<Path>) walk.filter(Files::isRegularFile)::iterator) {
                String rel = src.relativize(p).toString().replace('\\', '/');
                if (!rel.endsWith(".java") || rel.equals("pcoc/Step.java")
                        || rel.startsWith("pcoc/negatives/") || rel.startsWith("pcoc/tests/")) {
                    continue;
                }
                String text = Files.readString(p);
                check(!text.contains("acc += Mix.mix64(word ^ s)"), "fold only in Step " + rel);
            }
        }
    }

    private static Hike.Result hikeS1(Profile s1) {
        return hike(s1, 1);
    }

    private static Hike.Result hikeS2(Profile s2) {
        return hike(s2, 3);
    }

    private static Hike.Result hike(Profile profile, int legs) {
        Sha256 sha = new Sha256();
        byte[] pk = Bytes.parseHex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
        byte[] beacon = sha.hash("pcoc-pack-2.2/beacon/seed".getBytes(StandardCharsets.US_ASCII));
        byte[][] interrupts = new byte[18][];
        long[] heights = new long[18];
        Set<String> seen = new HashSet<String>();
        seen.add(Bytes.hex(beacon));
        for (int k = 0; k < 18; k++) {
            interrupts[k] = sha.hash(("pcoc-pack-2.2/beacon/" + k).getBytes(StandardCharsets.US_ASCII));
            heights[k] = 101L + k;
            seen.add(Bytes.hex(interrupts[k]));
        }
        check(seen.size() == 19, "distinct beacons");
        int[] counts = new int[18];
        Arrays.fill(counts, legs);
        return Hike.run(profile, pk, beacon, 100L, interrupts, heights, counts);
    }

    private static void roundTrip(Hike.Result hike) throws Exception {
        HikeObject obj = HikeObject.fromHike(hike);
        byte[] once = obj.encode();
        HikeObject back = HikeObject.decode(once);
        byte[] twice = back.encode();
        check(Arrays.equals(once, twice), "encode round trip");
        check(Const.KIND.equals(back.kind), "kind");
        check(back.stages.length == 18, "stages");
        check(back.stages[0].path == null, "stage 0 omits path");
        check(back.stages[17].path.length == 0, "W17 empty");
    }

    private static void unknown(Profile profile) {
        String[] labels = {"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"};
        for (String label : labels) {
            byte[] map = Cbor.kindMap(label);
            Verify.Report r = Verify.verify(map, profile, Index.EMPTY, null);
            check(r.decoded && r.unknownKind, "a5 " + label);
            check(Verify.unknownKindJson(label).equals(r.json), "a5 bytes " + label);
            check(r.json.indexOf("shape") < 0, "a5 has no shape");
            byte[] extra = Arrays.copyOf(map, map.length + 1);
            Verify.Report trail = Verify.verify(extra, profile, Index.EMPTY, null);
            check(trail.unknownKind && Verify.unknownKindJson(label).equals(trail.json), "a5 trailing");
        }
        Verify.Report bad = Verify.verify(new byte[] {0x01}, profile, Index.EMPTY, null);
        check(!bad.decoded, "decode failure");
    }

    private static void honest(Profile profile, Hike.Result hike, String name) {
        byte[] enc = HikeObject.fromHike(hike).encode();
        Verify.Report r = Verify.verify(enc, profile, Index.EMPTY, chain());
        if (!"match".equals(r.shape) || !r.checkpointsMatch || !r.waypointsMatch || !"match".equals(r.sixLeg)) {
            System.err.println(r.json);
        }
        check("match".equals(r.shape), name + " shape " + r.shape);
        check("match".equals(r.seedValue), name + " seed");
        check(r.checkpointsMatch, name + " checkpoints");
        check(r.waypointsMatch, name + " waypoints");
        check("match".equals(r.sixLeg), name + " six " + r.sixLeg);
        check(r.sixResults.size() == 6, name + " six count");
        check(r.anchoredTotal == 0, name + " anchored");
        check(r.anchorsNotAnchored, name + " anchors");
        check(r.pathsSkip, name + " paths");
        check(r.json.indexOf("\"accept\"") < 0 && r.json.indexOf("\"refuse\"") < 0, name + " no judgment");
        check(r.json.startsWith("{\"profile\":\"" + profile.name + "\""), name + " profile");
    }

    private static void negatives(Profile profile, Hike.Result hike) {
        Negatives.Fold[] folds = {Negatives.SLOT_SUMMARY, Negatives.XOR_ACCUMULATOR, Negatives.XOR_W,
                Negatives.S_PLUS_W, Negatives.PREFOLD_HOIST, Negatives.PARTIAL_SLOT, Negatives.PARTIAL_2047};
        String[] names = {"slot_summary_fold", "xor_accumulator_fold", "xor_w_fold", "s_plus_w_fold",
                "prefold_hoist", "partial_slot_read", "partial_slot_read_2047"};
        for (int i = 0; i < folds.length; i++) {
            byte[] enc = replaced(hike, profile, folds[i], false);
            Verify.Report r = Verify.verify(enc, profile, Index.EMPTY, chain());
            if (!"mismatch".equals(r.sixLeg)) {
                System.err.println(names[i] + " " + r.sixResults);
            }
            check("mismatch".equals(r.sixLeg), names[i]);
            for (String one : r.sixResults) {
                check("mismatch".equals(one), names[i] + " leg");
            }
        }
        byte[] cached = replaced(hike, profile, null, true);
        Verify.Report c = Verify.verify(cached, profile, Index.EMPTY, chain());
        check("mismatch".equals(c.sixLeg), "cached_leg");
        Sha256 sha = new Sha256();
        byte[] honest = Terrain.digest(sha, hike.terrain);
        long[] lanes = Seed.lanes(hike.seed);
        check(!Arrays.equals(honest, Terrain.digest(sha, Negatives.oneLane(profile, lanes))), "one_lane_flip");
        check(!Arrays.equals(honest, Terrain.digest(sha, Negatives.all4(profile, lanes))), "all4_kernel");
        byte[] pkB = Bytes.parseHex("dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659");
        check(!Arrays.equals(honest, Terrain.digest(sha, Negatives.shared(profile, sha, pkB, hike.seedBeacon))),
                "f3_shared_terrain");
        check(!Arrays.equals(honest, Terrain.digest(sha, Negatives.squeezed(profile, lanes))), "squeezed_seed_terrain");
    }

    private static byte[] replaced(Hike.Result hike, Profile profile, Negatives.Fold fold, boolean cached) {
        HikeObject obj = HikeObject.fromHike(hike);
        obj.stages = new Hike.Stage[hike.stages.length];
        Sha256 sha = new Sha256();
        for (int g = 0; g < hike.stages.length; g++) {
            Hike.Stage src = hike.stages[g];
            Hike.Stage s = new Hike.Stage();
            s.beacon = src.beacon;
            s.beaconHeight = src.beaconHeight;
            s.waypoint = src.waypoint;
            s.checkpoints = src.checkpoints;
            s.path = src.path;
            s.legResults = src.legResults.clone();
            byte[] prev = s.waypoint;
            for (int j = 0; j < s.legResults.length; j++) {
                long s0 = Bytes.getLe64(prev, 0);
                s.legResults[j] = cached ? Negatives.cachedLeg(s0) : Negatives.leg(hike.terrain, profile, sha, s0, fold);
                prev = s.checkpoints[j];
            }
            obj.stages[g] = s;
        }
        return obj.encode();
    }

    private static Draw.Source chain() {
        Sha256 sha = new Sha256();
        byte[] block = sha.hash("pcoc-pack-2.2/draw".getBytes(StandardCharsets.US_ASCII));
        return new Draw.HashChain(sha, block);
    }

    private static void cli(Profile profile) throws Exception {
        Path file = Files.createTempFile("pcoc-a5", ".cbor");
        try {
            Files.write(file, Cbor.kindMap("v2.1-pcoc-object"));
            java.io.ByteArrayOutputStream bos = new java.io.ByteArrayOutputStream();
            java.io.PrintStream old = System.out;
            System.setOut(new java.io.PrintStream(bos, true, StandardCharsets.UTF_8));
            int code;
            try {
                code = Main.run(new String[] {"verify", "--profile", "conformance-small",
                        "--slot-bytes", "64", "--sample-bytes", "32", "--terrain-bytes", "65536",
                        "--steps-per-leg", "100", file.toString()});
            } finally {
                System.setOut(old);
            }
            check(code == 0, "cli a5 exit");
            String out = bos.toString(StandardCharsets.UTF_8);
            check(out.equals(Verify.unknownKindJson("v2.1-pcoc-object") + "\n"), "cli a5 body");
        } finally {
            Files.deleteIfExists(file);
        }
        int missing = Main.run(new String[] {"verify", "no-such-object.cbor"});
        check(missing == 1, "cli missing");
    }

    private static void regen(Path repo) throws Exception {
        Path committed = repo.resolve("sdk/java/candidates");
        Path fresh = Files.createTempDirectory("pcoc-candidates");
        try {
            GenCandidates.generate(fresh);
            String[] names = {"seed.txt", "nl4.txt", "terrain.txt", "fold.txt", "kat-small.txt", "test-3leg.txt",
                    "prod.txt", "wes.txt", "rate.txt", "anchor.txt", "draw.txt", "unknown-kind.txt", "hashcash.txt",
                    "negatives.txt"};
            for (String name : names) {
                Path a = committed.resolve(name);
                Path b = fresh.resolve(name);
                check(Files.isRegularFile(a), "missing " + name);
                byte[] left = Files.readAllBytes(a);
                byte[] right = Files.readAllBytes(b);
                if (!Arrays.equals(left, right)) {
                    System.err.println("diff " + name + " committed " + left.length + " fresh " + right.length);
                }
                check(Arrays.equals(left, right), "unchanged " + name);
            }
        } finally {
            try (var walk = Files.walk(fresh)) {
                walk.sorted(java.util.Comparator.reverseOrder()).forEach(p -> {
                    try {
                        Files.deleteIfExists(p);
                    } catch (Exception e) {
                        throw new RuntimeException(e);
                    }
                });
            }
        }
    }

    private static void check(boolean ok, String msg) {
        if (!ok) {
            failures++;
            System.err.println("FAIL " + msg);
        }
    }

    private static void finish() {
        if (failures != 0) {
            System.err.println(failures + " failed");
            System.exit(1);
        }
        System.out.println("ok");
    }
}
