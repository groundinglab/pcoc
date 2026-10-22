package io.groundinglab.pcoc.gen;

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

import io.groundinglab.pcoc.Anchor;
import io.groundinglab.pcoc.Bytes;
import io.groundinglab.pcoc.Checkpoint;
import io.groundinglab.pcoc.Const;
import io.groundinglab.pcoc.Draw;
import io.groundinglab.pcoc.Hashcash;
import io.groundinglab.pcoc.Leg;
import io.groundinglab.pcoc.Profile;
import io.groundinglab.pcoc.Rate;
import io.groundinglab.pcoc.Seed;
import io.groundinglab.pcoc.Sha256;
import io.groundinglab.pcoc.Step;
import io.groundinglab.pcoc.Terrain;
import io.groundinglab.pcoc.Verify;
import io.groundinglab.pcoc.negatives.Negatives;
import io.groundinglab.pcoc.produce.Cases;
import io.groundinglab.pcoc.produce.Hike;

/** CASES.md candidate files. Deterministic. No timings. */
public final class GenCandidates {
    private static final String[] LEG_NEG = {
            "slot_summary_fold", "xor_accumulator_fold", "xor_w_fold", "s_plus_w_fold",
            "prefold_hoist", "final_stage_hoist", "partial_slot_read", "partial_slot_read_2047",
            "f3_shared_terrain", "cached_leg"
    };
    private static final String[] TERRAIN_NEG = {
            "one_lane_flip", "all4_kernel", "squeezed_seed_terrain"
    };

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
        Map<String, Lines> files = new LinkedHashMap<String, Lines>();
        String[] names = {"seed", "nl4", "terrain", "fold", "kat-small", "test-3leg", "prod", "wes",
                "rate", "anchor", "draw", "unknown-kind", "hashcash", "negatives"};
        for (String n : names) {
            files.put(n, new Lines());
        }

        byte[] seedBeacon = Cases.seedBeacon(sha);
        seedLines(sha, files.get("seed"), "pk_A", Cases.PK_A, seedBeacon);
        seedLines(sha, files.get("seed"), "pk_B", Cases.PK_B, seedBeacon);

        Hike.Result hike1 = Cases.run(Cases.s1(), Cases.PK_A, Cases.legs(1));
        nl4(files.get("nl4"), Cases.s1(), hike1);
        String s1Digest = Bytes.hex(Terrain.digest(sha, hike1.terrain));
        files.get("terrain").add("terrain.S1.terrain_sha256", s1Digest);
        files.get("kat-small").add("kat-small.S1.terrain_sha256", s1Digest);
        fold(sha, files.get("fold"), Cases.s1(), hike1);
        hikeLines(files.get("kat-small"), "kat-small.S1", hike1);
        wes(files.get("wes"), hike1);

        Hike.Result hike2 = Cases.run(Cases.s2(), Cases.PK_A, Cases.legs(3));
        String s2Digest = Bytes.hex(Terrain.digest(sha, hike2.terrain));
        files.get("terrain").add("terrain.S2.terrain_sha256", s2Digest);
        files.get("test-3leg").add("test-3leg.S2.terrain_sha256", s2Digest);
        hikeLines(files.get("test-3leg"), "test-3leg.S2", hike2);

        negatives(sha, files.get("negatives"), Cases.s1(), hike1, seedBeacon);
        rate(files.get("rate"));
        anchor(files.get("anchor"));
        draw(sha, files.get("draw"));
        unknown(files.get("unknown-kind"));
        hashcash(sha, files.get("hashcash"));

        Profile prod = Profile.production();
        byte[] seed = Seed.derive(sha, Cases.PK_A, seedBeacon);
        long[] lanes = Seed.lanes(seed);
        long[] terrain = Terrain.fill(prod, lanes);
        long leg = Leg.legResult(terrain, prod, sha, lanes[0]);
        String prodDigest = Bytes.hex(Terrain.digest(sha, terrain));
        files.get("terrain").add("terrain.PROD.terrain_sha256", prodDigest);
        files.get("prod").add("prod.PROD.terrain_sha256", prodDigest);
        files.get("prod").add("prod.PROD.stage.0.leg.0.legs_le_hex", Bytes.hex(Bytes.le64(leg)));
        int off = Leg.sampleOffset(seed, prod.offsetModulus);
        byte[] sample = Terrain.window(terrain, off, prod.sampleBytes);
        byte[] cp = Checkpoint.digest(sha, seed, seed, sample, Bytes.le64(leg));
        files.get("prod").add("prod.PROD.stage.0.leg.0.checkpoint", Bytes.hex(cp));

        for (Map.Entry<String, Lines> e : files.entrySet()) {
            e.getValue().write(dir.resolve(e.getKey() + ".txt"));
        }
    }

    /** §13.1 fee header for the hashcash fixture. 80 bytes, not a Bitcoin header. */
    public static byte[] feeHeader(Sha256 sha) {
        byte[] a = Cases.asciiHash(sha, "pcoc-pack-2.2/hashcash/fee_header/0");
        byte[] b = Cases.asciiHash(sha, "pcoc-pack-2.2/hashcash/fee_header/1");
        byte[] c = Cases.asciiHash(sha, "pcoc-pack-2.2/hashcash/fee_header/2");
        byte[] o = new byte[80];
        System.arraycopy(a, 0, o, 0, 32);
        System.arraycopy(b, 0, o, 32, 32);
        System.arraycopy(c, 0, o, 64, 16);
        return o;
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
            for (int j = 0; j < profile.wordsPerSlot; j++) {
                String key = "nl4.S1.slot." + slot + ".word." + j + ".";
                lines.add(key + "base_u64", Bytes.hexU64(Terrain.baseWord(lanes, slot, j)));
                lines.add(key + "stored_u64", Bytes.hexU64(hike.terrain[slot * profile.wordsPerSlot + j]));
            }
        }
    }

    private static void fold(Sha256 sha, Lines lines, Profile profile, Hike.Result hike) {
        byte[] buf = new byte[20];
        System.arraycopy(Const.SEAL_DOMAIN, 0, buf, 0, Const.SEAL_DOMAIN.length);
        long s = Bytes.getLe64(hike.seed, 0);
        for (int t = 0; t < 4; t++) {
            int slot = Step.slotIndex(s, profile.nSlots);
            long acc = Step.foldSlot(hike.terrain, profile.wordsPerSlot, slot, s);
            long next = Step.seal(sha, buf, acc);
            String key = "fold.S1.step." + t + ".";
            lines.add(key + "S_u64", Bytes.hexU64(s));
            lines.add(key + "slot_index", Integer.toString(slot));
            lines.add(key + "acc_u64", Bytes.hexU64(acc));
            lines.add(key + "S_next_u64", Bytes.hexU64(next));
            s = next;
        }
        lines.add("fold.S1.leg_result_u64", Bytes.hexU64(hike.stages[0].legResults[0]));
    }

    private static void hikeLines(Lines lines, String prefix, Hike.Result hike) {
        for (int g = 0; g < hike.stages.length; g++) {
            var stage = hike.stages[g];
            lines.add(prefix + ".stage." + g + ".waypoint", Bytes.hex(stage.waypoint));
            for (int j = 0; j < stage.legResults.length; j++) {
                String key = prefix + ".stage." + g + ".leg." + j + ".";
                lines.add(key + "legs_le_hex", Bytes.hex(Bytes.le64(stage.legResults[j])));
                lines.add(key + "checkpoint", Bytes.hex(stage.checkpoints[j]));
            }
        }
    }

    private static void wes(Lines lines, Hike.Result hike) {
        lines.add("wes.S1.D", Bytes.hex(hike.seed));
        lines.add("wes.S1.stage.0.waypoint", Bytes.hex(hike.stages[0].waypoint));
        lines.add("wes.S1.equal", Bytes.eq(hike.stages[0].waypoint, hike.seed) ? "true" : "false");
    }

    private static void negatives(Sha256 sha, Lines lines, Profile profile, Hike.Result hike, byte[] seedBeacon) {
        long honest = hike.stages[0].legResults[0];
        String honestHex = Bytes.hex(Bytes.le64(honest));
        String honestDigest = Bytes.hex(Terrain.digest(sha, hike.terrain));
        lines.add("negatives.honest.legs_le_hex", honestHex);
        lines.add("negatives.honest.terrain_sha256", honestDigest);
        long s0 = Bytes.getLe64(hike.seed, 0);
        long[] lanes = Seed.lanes(hike.seed);

        Map<String, String> legs = new LinkedHashMap<String, String>();
        legs.put("slot_summary_fold", hexLeg(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.SLOT_SUMMARY)));
        legs.put("xor_accumulator_fold", hexLeg(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.XOR_ACCUMULATOR)));
        legs.put("xor_w_fold", hexLeg(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.XOR_W)));
        legs.put("s_plus_w_fold", hexLeg(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.S_PLUS_W)));
        legs.put("prefold_hoist", hexLeg(Negatives.prefoldHoist(hike.terrain, profile, sha, s0)));
        long hoist = Negatives.leg(hike.terrain, profile, sha, s0, Negatives.FINAL_STAGE);
        long xorMix = Negatives.leg(hike.terrain, profile, sha, s0, Negatives.XOR_OF_MIX64);
        if (hoist != xorMix) {
            throw new IllegalStateException("final_stage_hoist is not the XOR accumulator of mix64(word XOR S)");
        }
        legs.put("final_stage_hoist", hexLeg(hoist));
        legs.put("partial_slot_read", hexLeg(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.PARTIAL_SLOT)));
        legs.put("partial_slot_read_2047", hexLeg(Negatives.leg(hike.terrain, profile, sha, s0, Negatives.PARTIAL_2047)));
        long[] shared = Negatives.otherTerrain(profile, sha, Cases.PK_B, seedBeacon);
        legs.put("f3_shared_terrain", hexLeg(Leg.legResult(shared, profile, sha, s0)));
        legs.put("cached_leg", hexLeg(Negatives.cachedLeg(hike.terrain, profile, sha, s0)));

        Map<String, String> terrains = new LinkedHashMap<String, String>();
        terrains.put("one_lane_flip", Bytes.hex(Terrain.digest(sha, Negatives.oneLane(profile, lanes))));
        terrains.put("all4_kernel", Bytes.hex(Terrain.digest(sha, Negatives.all4(profile, lanes))));
        terrains.put("squeezed_seed_terrain", Bytes.hex(Terrain.digest(sha, Negatives.squeezed(profile, lanes))));

        List<String> all = new ArrayList<String>();
        for (String name : LEG_NEG) {
            String hex = legs.get(name);
            if (hex == null) {
                throw new IllegalStateException(name);
            }
            if (hex.equals(honestHex)) {
                throw new IllegalStateException(name + " equals honest");
            }
            all.add(hex);
            lines.add("negatives." + name + ".legs_le_hex", hex);
            lines.add("negatives." + name + ".matches_honest", "false");
        }
        for (String name : TERRAIN_NEG) {
            String hex = terrains.get(name);
            if (hex == null) {
                throw new IllegalStateException(name);
            }
            if (hex.equals(honestDigest)) {
                throw new IllegalStateException(name + " equals honest");
            }
            all.add(hex);
            lines.add("negatives." + name + ".terrain_sha256", hex);
            lines.add("negatives." + name + ".matches_honest", "false");
        }
        for (int i = 0; i < all.size(); i++) {
            for (int j = i + 1; j < all.size(); j++) {
                if (all.get(i).equals(all.get(j))) {
                    throw new IllegalStateException("negatives not pairwise distinct");
                }
            }
        }
    }

    private static String hexLeg(long leg) {
        return Bytes.hex(Bytes.le64(leg));
    }

    private static void rate(Lines lines) {
        int[] legs = {0, 1, 5, 6, 1000};
        int[] spans = {1, 300, 3600};
        int[][] floors = {{1, 5}, {1, 4}, {7, 20}};
        for (int leg : legs) {
            for (int span : spans) {
                for (int[] pq : floors) {
                    String key = "rate.legs." + leg + ".span." + span + ".floor." + pq[0] + "_" + pq[1];
                    lines.add(key, Rate.compare(leg, span, pq[0], pq[1]));
                }
            }
        }
    }

    private static void anchor(Lines lines) {
        for (int h = 100; h <= 106; h++) {
            lines.add("anchor.H.100.height." + h, Anchor.window(100, h));
        }
    }

    private static void draw(Sha256 sha, Lines lines) {
        lines.add("draw.b0_label", "pcoc-pack-2.2/draw");
        lines.add("draw.thorough_rounds", "3");
        int[] counts = {6, 18, 54};
        for (int n : counts) {
            byte[] b0 = Cases.asciiHash(sha, "pcoc-pack-2.2/draw");
            Draw.Six six = Draw.six(n, new Draw.HashChain(sha, Arrays.copyOf(b0, b0.length)));
            lines.add("draw.legs." + n + ".six_leg.indices", join(six.indices));
            byte[] b0b = Cases.asciiHash(sha, "pcoc-pack-2.2/draw");
            Draw.Thorough th = Draw.thorough(n, 3, new Draw.HashChain(sha, b0b));
            if (th.notDefined || th.notRead || th.indices == null || th.indices.length != 18) {
                throw new IllegalStateException("thorough draw");
            }
            lines.add("draw.legs." + n + ".thorough.indices", join(th.indices));
        }
    }

    private static String join(int[] xs) {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < xs.length; i++) {
            if (i > 0) {
                b.append(',');
            }
            b.append(xs[i]);
        }
        return b.toString();
    }

    private static void unknown(Lines lines) {
        String[] labels = {"v2.1-pcoc-object", "satoshi-hike-v2.0-object", "", "v2.2-pcoc-objecT"};
        for (int n = 0; n < labels.length; n++) {
            byte[] rec = Verify.unknownKindJson(labels[n]).getBytes(StandardCharsets.UTF_8);
            lines.add("unknown-kind." + n + ".kind_text_hex", Bytes.hex(labels[n].getBytes(StandardCharsets.UTF_8)));
            lines.add("unknown-kind." + n + ".record_hex", Bytes.hex(rec));
        }
    }

    private static void hashcash(Sha256 sha, Lines lines) {
        String origin = "https://example.com";
        byte[] header = feeHeader(sha);
        byte[] once = sha.hash(header);
        byte[] headerHash = sha.hash(once);
        if (Arrays.equals(once, headerHash)) {
            throw new IllegalStateException("header_hash");
        }
        byte[] waypoint = Cases.asciiHash(sha, "pcoc-pack-2.2/hashcash/waypoint");
        byte[] challenge = Hashcash.challenge(sha, origin, headerHash);
        long nonce = Hashcash.solve(sha, challenge, waypoint);
        byte[] digest = Hashcash.digest(sha, challenge, waypoint, nonce);
        int zeros = Hashcash.leadingZeroBits(digest);
        lines.add("hashcash.bari_origin", origin);
        lines.add("hashcash.fee_header", Bytes.hex(header));
        lines.add("hashcash.header_hash", Bytes.hex(headerHash));
        lines.add("hashcash.waypoint", Bytes.hex(waypoint));
        lines.add("hashcash.difficulty_bits", "21");
        lines.add("hashcash.challenge", Bytes.hex(challenge));
        lines.add("hashcash.nonce", Long.toString(nonce));
        lines.add("hashcash.digest", Bytes.hex(digest));
        lines.add("hashcash.leading_zero_bits", Integer.toString(zeros));
    }

    private static final class Lines {
        final List<String> rows = new ArrayList<String>();

        void add(String key, String value) {
            if (key.indexOf('\t') >= 0 || value.indexOf('\n') >= 0 || value.indexOf('\t') >= 0) {
                throw new IllegalArgumentException(key);
            }
            if (!value.equals(value.trim())) {
                throw new IllegalArgumentException("space " + key);
            }
            rows.add(key + "\t" + value);
        }

        void write(Path file) throws IOException {
            List<String> sorted = new ArrayList<String>(rows);
            Collections.sort(sorted);
            for (int i = 1; i < sorted.size(); i++) {
                String a = sorted.get(i - 1);
                String b = sorted.get(i);
                String ka = a.substring(0, a.indexOf('\t'));
                String kb = b.substring(0, b.indexOf('\t'));
                if (ka.equals(kb)) {
                    throw new IllegalStateException("duplicate key " + ka);
                }
            }
            StringBuilder sb = new StringBuilder();
            for (String row : sorted) {
                sb.append(row).append('\n');
            }
            byte[] raw = sb.toString().getBytes(StandardCharsets.US_ASCII);
            Files.write(file, raw);
        }
    }
}
