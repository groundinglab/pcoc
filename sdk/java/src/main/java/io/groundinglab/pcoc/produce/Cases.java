package io.groundinglab.pcoc.produce;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

import io.groundinglab.pcoc.Bytes;
import io.groundinglab.pcoc.Const;
import io.groundinglab.pcoc.HikeObject;
import io.groundinglab.pcoc.Profile;
import io.groundinglab.pcoc.Sha256;

/**
 * Scripted rig for the round-2 fixtures (§13.2 with a fixed tip schedule, §13.3 empty paths).
 * No network, no post, and no wait. Heights are stored and are not hashed (§9).
 */
public final class Cases {
    public static final byte[] PK_A = Bytes.parseHex(
            "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
    public static final byte[] PK_B = Bytes.parseHex(
            "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659");
    public static final long SEED_HEIGHT = 1000L;

    private Cases() {}

    public static byte[] asciiHash(Sha256 sha, String ascii) {
        return sha.hash(ascii.getBytes(StandardCharsets.US_ASCII));
    }

    public static byte[] seedBeacon(Sha256 sha) {
        return asciiHash(sha, "pcoc-pack-2.2/beacon/seed");
    }

    public static byte[] interrupt(Sha256 sha, int k) {
        return asciiHash(sha, "pcoc-pack-2.2/beacon/" + k);
    }

    /** {@code I_k} is at height {@code 1001 + k}. */
    public static long interruptHeight(int k) {
        return 1001L + k;
    }

    public static int[] legs(int n) {
        int[] v = new int[Const.STAGES];
        Arrays.fill(v, n);
        return v;
    }

    public static Profile s1() {
        return Profile.conformanceSmall(64, 32, 65536, 100);
    }

    public static Profile s2() {
        return Profile.conformanceSmall(256, 64, 1048576, 1000);
    }

    /**
     * §13.2. {@code legsPerStage} is fixture metadata (§16.1), one count per stage.
     * Stage 0 has no path key. Stages 1–17 store empty path bytes.
     */
    public static Hike.Result run(Profile profile, byte[] publicKey, int[] legsPerStage) {
        Sha256 sha = new Sha256();
        byte[] beacon = seedBeacon(sha);
        byte[][] interrupts = new byte[Const.STAGES][];
        long[] heights = new long[Const.STAGES];
        for (int k = 0; k < Const.STAGES; k++) {
            interrupts[k] = interrupt(sha, k);
            heights[k] = interruptHeight(k);
        }
        return Hike.run(profile, publicKey, beacon, SEED_HEIGHT, interrupts, heights, legsPerStage);
    }

    public static byte[] encode(Hike.Result hike) {
        HikeObject o = new HikeObject();
        o.kind = Const.KIND;
        o.fill = Const.FILL;
        o.slotBytes = hike.profile.slotBytes;
        o.nSlots = hike.profile.nSlots;
        o.sampleBytes = hike.profile.sampleBytes;
        o.stepsPerLeg = hike.profile.stepsPerLeg;
        o.publicKey = hike.publicKey;
        o.seedBeacon = hike.seedBeacon;
        o.lastInterrupt = hike.lastInterrupt;
        o.lastInterruptHeight = hike.lastInterruptHeight;
        o.stages = hike.stages;
        return o.encode();
    }

    /**
     * Writes {@code O1-S1.cbor}, {@code O2-S2.cbor}, and {@code O3-PROD.cbor}.
     * Stderr is the §13.3 close step: {@code path_skip} for 1…16, then {@code not fetched by design 17}.
     */
    public static void writeObjects(Path dir) throws IOException {
        Files.createDirectories(dir);
        Files.write(dir.resolve("O1-S1.cbor"), encode(run(s1(), PK_A, legs(1))));
        Files.write(dir.resolve("O2-S2.cbor"), encode(run(s2(), PK_A, legs(3))));
        Files.write(dir.resolve("O3-PROD.cbor"), encode(run(Profile.production(), PK_A, legs(1))));
        for (int g = 1; g <= 16; g++) {
            System.err.print("path_skip " + g + "\n");
        }
        System.err.print("not fetched by design 17\n");
    }
}
