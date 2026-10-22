package pcoc.gen;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import pcoc.Bytes;
import pcoc.Hashcash;
import pcoc.Hike;
import pcoc.Leg;
import pcoc.Profile;
import pcoc.Seed;
import pcoc.Sha256;
import pcoc.Step;
import pcoc.Terrain;
import pcoc.Verify;
import pcoc.negatives.Negatives;

/** CASES.md candidate files. Deterministic. Timing lines go to stderr, not into the files. */
public final class GenCandidates {
    private static final byte[] PK_A = Bytes.parseHex(
            "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
    private static final byte[] PK_B = Bytes.parseHex(
            "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659");

    private GenCandidates() {}

    public static void main(String[] args) throws IOException {
        Path out = null;
        for (int i = 0; i < args.length; i++) {
            if ("--out".equals(args[i]) && i + 1 < args.length) {
                out = Path.of(args[++i]);
            }
        }
        if (out == null) {
            throw new IllegalArgumentException("--out");
        }
        generate(out);
    }

    public static void generate(Path dir) throws IOException {
        Files.createDirectories(dir);
        Sha256 sha = new Sha256();
        byte[] seedBeacon = asciiHash(sha, "pcoc-pack-2.2/beacon/seed");
        byte[][] interrupts = new byte[18][];
        long[] heights = new long[18];
        for (int k = 0; k < 18; k++) {
            interrupts[k] = asciiHash(sha, "pcoc-pack-2.2/beacon/" + k);
            heights[k] = 101L + k;
        }
        Map<String, Lines> files = new LinkedHashMap<String, Lines>();
        String[] names = {"seed", "nl4", "terrain", "fold", "kat-small", "test-3leg", "prod", "wes", "rate",
                "anchor", "draw", "unknown-kind", "hashcash", "negatives"};
        for (String n : names) {
            files.put(n, new Lines());
        }

        seedLines(sha, files.get("seed"), "pk_A", PK_A, seedBeacon);
        seedLines(sha, files.get("seed"), "pk_B", PK_B, seedBeacon);

        Profile s1 = Profile.conformanceSmall(64, 32, 65536, 100);
        int[] one = legs(1);
        Hike.Result hike1 = Hike.run(s1, PK_A, seedBeacon, 100L, interrupts, heights, one);
        nl4(files.get("nl4"), s1, hike1);
        files.get("terrain").add("terrain.S1.pk_A.digest", Bytes.hex(Terrain.digest(sha, hike1.terrain)));
        fold(sha, files.get("fold"), s1, hike1);
        hikeLines(files.get("kat-small"), "kat-small.S1.pk_A", hike1);
        wes(files.get("wes"), hike1);

        Profile s2 = Profile.conformanceSmall(256, 64, 1048576, 1000);
        Hike.Result hike2 = Hike.run(s2, PK_A, seedBeacon, 100L, interrupts, heights, legs(3));
        files.get("terrain").add("terrain.S2.pk_A.digest", Bytes.hex(Terrain.digest(sha, hike2.terrain)));
        hikeLines(files.get("test-3leg"), "test-3leg.S2.pk_A", hike2);

        negatives(sha, files.get("negatives"), s1, hike1, seedBeacon);

        rate(files.get("rate"));
        anchor(files.get("anchor"));
        draw(sha, files.get("draw"));
        unknown(files.get("unknown-kind"));
        hashcash(sha, files.get("hashcash"));

        Profile prod = Profile.production();
        byte[] seed = Seed.derive(sha, PK_A, seedBeacon);
        long[] lanes = Seed.lanes(seed);
        long t0 = System.nanoTime();
        long[] terrain = Terrain.fill(prod, lanes);
        long t1 = System.nanoTime();
        long s0 = lanes[0];
        long leg = Leg.legResult(terrain, prod, sha, s0);
        long t2 = System.nanoTime();
        System.err.println("prod.terrain_ms " + ((t1 - t0) / 1_000_000L));
        System.err.println("prod.leg_ms " + ((t2 - t1) / 1_000_000L));
        byte[] digest = Terrain.digest(sha, terrain);
        String digestHex = Bytes.hex(digest);
        files.get("terrain").add("terrain.PROD.pk_A.digest", digestHex);
        files.get("prod").add("prod.PROD.pk_A.digest", digestHex);
        files.get("prod").add("prod.PROD.pk_A.stage0.leg0.legs_le_hex", Bytes.hex(Bytes.le64(leg)));
        int off = Leg.sampleOffset(seed, prod.offsetModulus);
        byte[] sample = Terrain.window(terrain, off, prod.sampleBytes);
        byte[] cp = pcoc.Checkpoint.digest(sha, seed, seed, sample, Bytes.le64(leg));
        files.get("prod").add("prod.PROD.pk_A.stage0.leg0.checkpoint", Bytes.hex(cp));

        for (Map.Entry<String, Lines> e : files.entrySet()) {
            e.getValue().write(dir.resolve(e.getKey() + ".txt"));
        }
    }

    private static void seedLines(Sha256 sha, Lines lines, String name, byte[] pk, byte[] beacon) {
        byte[] pre = Bytes.concat(pk, beacon);
        byte[] d = Seed.derive(sha, pk, beacon);
        long[] p = Seed.lanes(d);
        String k = "seed." + name + ".";
        lines.add(k + "preimage", Bytes.hex(pre));
        lines.add(k + "D", Bytes.hex(d));
        for (int i = 0; i < 4; i++) {
            lines.add(k + "P" + i + "_u64", Bytes.hexU64(p[i]));
        }
    }

    private static void nl4(Lines lines, Profile profile, Hike.Result hike) {
        long[] lanes = Seed.lanes(hike.seed);
        int[] slots = {0, 1, profile.nSlots - 1};
        for (int slot : slots) {
            for (int j = 0; j < 16; j++) {
                String key = "nl4.S1.pk_A.slot" + slot + ".j" + j + ".";
                lines.add(key + "base_u64", Bytes.hexU64(Terrain.baseWord(lanes, slot, j)));
                if (j < profile.wordsPerSlot) {
                    lines.add(key + "stored_u64", Bytes.hexU64(hike.terrain[slot * profile.wordsPerSlot + j]));
                }
            }
        }
    }

    private static void fold(Sha256 sha, Lines lines, Profile profile, Hike.Result hike) {
        byte[] buf = new byte[20];
        System.arraycopy(pcoc.Const.SEAL_DOMAIN, 0, buf, 0, pcoc.Const.SEAL_DOMAIN.length);
        long s = Bytes.getLe64(hike.seed, 0);
        for (int t = 0; t < 4; t++) {
            int slot = Step.slotIndex(s, profile.nSlots);
            long acc = Step.foldSlot(hike.terrain, profile.wordsPerSlot, slot, s);
            long sp = Step.seal(sha, buf, acc);
            String key = "fold.S1.pk_A.stage0.leg0.step" + t + ".";
            lines.add(key + "S_u64", Bytes.hexU64(s));
            lines.add(key + "i", Integer.toString(slot));
            lines.add(key + "acc_u64", Bytes.hexU64(acc));
            lines.add(key + "S_prime_u64", Bytes.hexU64(sp));
            s = sp;
        }
        long leg = hike.stages[0].legResults[0];
        lines.add("fold.S1.pk_A.stage0.leg0.leg_result_u64", Bytes.hexU64(leg));
        lines.add("fold.S1.pk_A.stage0.leg0.legs_le_hex", Bytes.hex(Bytes.le64(leg)));
    }

    private static void hikeLines(Lines lines, String prefix, Hike.Result hike) {
        for (int g = 0; g < hike.stages.length; g++) {
            Hike.Stage stage = hike.stages[g];
            lines.add(prefix + ".stage" + g + ".waypoint", Bytes.hex(stage.waypoint));
            for (int j = 0; j < stage.legResults.length; j++) {
                String key = prefix + ".stage" + g + ".leg" + j + ".";
                lines.add(key + "legs_le_hex", Bytes.hex(Bytes.le64(stage.legResults[j])));
                lines.add(key + "checkpoint", Bytes.hex(stage.checkpoints[j]));
            }
        }
    }

    private static void wes(Lines lines, Hike.Result hike) {
        lines.add("wes.S1.pk_A.stage0.waypoint", Bytes.hex(hike.stages[0].waypoint));
        lines.add("wes.S1.pk_A.D", Bytes.hex(hike.seed));
        lines.add("wes.S1.pk_A.equal", Bytes.eq(hike.stages[0].waypoint, hike.seed) ? "true" : "false");
    }

    private static void negatives(Sha256 sha, Lines lines, Profile profile, Hike.Result hike, byte[] seedBeacon) {
        long honest = hike.stages[0].legResults[0];
        String honestHex = Bytes.hex(Bytes.le64(honest));
        long s0 = Bytes.getLe64(hike.seed, 0);
        legNeg(sha, lines, profile, hike, "slot_summary_fold", Negatives.SLOT_SUMMARY, s0, honestHex);
        legNeg(sha, lines, profile, hike, "xor_accumulator_fold", Negatives.XOR_ACCUMULATOR, s0, honestHex);
        legNeg(sha, lines, profile, hike, "xor_w_fold", Negatives.XOR_W, s0, honestHex);
        legNeg(sha, lines, profile, hike, "s_plus_w_fold", Negatives.S_PLUS_W, s0, honestHex);
        legNeg(sha, lines, profile, hike, "prefold_hoist", Negatives.PREFOLD_HOIST, s0, honestHex);
        legNeg(sha, lines, profile, hike, "partial_slot_read", Negatives.PARTIAL_SLOT, s0, honestHex);
        legNeg(sha, lines, profile, hike, "partial_slot_read_2047", Negatives.PARTIAL_2047, s0, honestHex);
        String cached = Negatives.legsLeHex(Negatives.cachedLeg(s0));
        putLeg(lines, "cached_leg", cached, honestHex);

        long[] lanes = Seed.lanes(hike.seed);
        byte[] honestDigest = Terrain.digest(sha, hike.terrain);
        putTerrain(lines, sha, "one_lane_flip", Negatives.oneLane(profile, lanes), honestDigest);
        putTerrain(lines, sha, "all4_kernel", Negatives.all4(profile, lanes), honestDigest);
        putTerrain(lines, sha, "f3_shared_terrain", Negatives.shared(profile, sha, PK_B, seedBeacon), honestDigest);
        putTerrain(lines, sha, "squeezed_seed_terrain", Negatives.squeezed(profile, lanes), honestDigest);
    }

    private static void legNeg(Sha256 sha, Lines lines, Profile profile, Hike.Result hike, String name,
                               Negatives.Fold fold, long s0, String honestHex) {
        long bad = Negatives.leg(hike.terrain, profile, sha, s0, fold);
        putLeg(lines, name, Negatives.legsLeHex(bad), honestHex);
    }

    private static void putLeg(Lines lines, String name, String hex, String honestHex) {
        lines.add("negatives.S1.pk_A." + name + ".legs_le_hex", hex);
        lines.add("negatives.S1.pk_A." + name + ".matches_honest", hex.equals(honestHex) ? "true" : "false");
    }

    private static void putTerrain(Lines lines, Sha256 sha, String name, long[] terrain, byte[] honest) {
        String hex = Bytes.hex(Terrain.digest(sha, terrain));
        lines.add("negatives.S1.pk_A." + name + ".terrain_digest", hex);
        lines.add("negatives.S1.pk_A." + name + ".matches_honest", hex.equals(Bytes.hex(honest)) ? "true" : "false");
    }

    private static void rate(Lines lines) {
        int[] legs = {0, 1, 5, 6, 1000};
        int[] spans = {1, 300, 3600};
        int[][] floors = {{1, 5}, {1, 4}, {7, 20}};
        for (int leg : legs) {
            for (int span : spans) {
                for (int[] pq : floors) {
                    String key = "rate.legs_" + leg + ".span_" + span + ".floor_" + pq[0] + "_" + pq[1];
                    lines.add(key, pcoc.Rate.compare(leg, span, pq[0], pq[1]));
                }
            }
        }
    }

    private static void anchor(Lines lines) {
        for (int h = 100; h <= 106; h++) {
            lines.add("anchor.H100.height" + h, pcoc.Anchor.window(100, h));
        }
    }

    private static void draw(Sha256 sha, Lines lines) {
        int[] counts = {6, 18, 54};
        for (int n : counts) {
            byte[] block = asciiHash(sha, "pcoc-pack-2.2/draw");
            pcoc.Draw.Six six = pcoc.Draw.six(n, new pcoc.Draw.HashChain(sha, block));
            for (int i = 0; i < six.indices.length; i++) {
                lines.add("draw.legs" + n + ".index" + i, Integer.toString(six.indices[i]));
            }
        }
    }

    private static void unknown(Lines lines) {
        String[] labels = {"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"};
        String[] keys = {"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "empty-label", "v2.2-pcoc-objecT"};
        for (int i = 0; i < labels.length; i++) {
            byte[] rec = Verify.unknownKindJson(labels[i]).getBytes(StandardCharsets.UTF_8);
            lines.add("unknown-kind." + keys[i] + ".record_hex", Bytes.hex(rec));
        }
    }

    private static void hashcash(Sha256 sha, Lines lines) {
        String origin = "https://bari.example";
        String headerName = "pcoc-pack-2.2/hashcash/fee-header";
        String wpName = "pcoc-pack-2.2/hashcash/waypoint";
        byte[] header = asciiHash(sha, headerName);
        byte[] waypoint = asciiHash(sha, wpName);
        byte[] challenge = Hashcash.challenge(sha, origin, header);
        long nonce = Hashcash.solve(sha, challenge, waypoint);
        byte[] digest = Hashcash.digest(sha, challenge, waypoint, nonce);
        lines.add("hashcash.origin", origin);
        lines.add("hashcash.fee_header_hash_name", headerName);
        lines.add("hashcash.header_hash", Bytes.hex(header));
        lines.add("hashcash.waypoint_name", wpName);
        lines.add("hashcash.waypoint", Bytes.hex(waypoint));
        lines.add("hashcash.challenge", Bytes.hex(challenge));
        lines.add("hashcash.nonce", Long.toString(nonce));
        lines.add("hashcash.digest", Bytes.hex(digest));
    }

    private static int[] legs(int n) {
        int[] v = new int[18];
        Arrays.fill(v, n);
        return v;
    }

    private static byte[] asciiHash(Sha256 sha, String ascii) {
        return sha.hash(ascii.getBytes(StandardCharsets.US_ASCII));
    }

    private static final class Lines {
        final List<String> rows = new ArrayList<String>();

        void add(String key, String value) {
            if (key.indexOf('\t') >= 0 || key.indexOf('\n') >= 0 || value.indexOf('\n') >= 0 || value.indexOf('\t') >= 0) {
                throw new IllegalArgumentException(key);
            }
            if (!value.equals(value.trim()) || key.endsWith(" ") || value.endsWith(" ")) {
                throw new IllegalArgumentException("space " + key);
            }
            rows.add(key + "\t" + value);
        }

        void write(Path file) throws IOException {
            List<String> sorted = new ArrayList<String>(rows);
            Collections.sort(sorted);
            for (int i = 1; i < sorted.size(); i++) {
                if (sorted.get(i).equals(sorted.get(i - 1))) {
                    throw new IllegalStateException("duplicate " + sorted.get(i));
                }
                String a = sorted.get(i - 1);
                String b = sorted.get(i);
                if (a.substring(0, a.indexOf('\t')).equals(b.substring(0, b.indexOf('\t')))) {
                    throw new IllegalStateException("duplicate key " + a);
                }
            }
            StringBuilder sb = new StringBuilder();
            for (String row : sorted) {
                sb.append(row).append('\n');
            }
            for (int i = 0; i < sb.length(); i++) {
                if (sb.charAt(i) > 127) {
                    throw new IllegalArgumentException("non-ascii");
                }
            }
            byte[] raw = sb.toString().getBytes(StandardCharsets.US_ASCII);
            Files.write(file, raw);
        }
    }
}
