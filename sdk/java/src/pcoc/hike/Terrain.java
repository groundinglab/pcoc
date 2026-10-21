package pcoc.hike;

import java.security.DigestException;
import java.security.MessageDigest;

/**
 * Section 10 terrain and section 11 step.
 *
 * <p>The seed is {@code SHA-256(public_key ‖ seed_beacon)}, 32 bytes, no label.
 * Every word is the NL4 chain over all four lanes. A step folds every word of
 * the selected slot with {@code mix64}, which includes {@code MIX_ADD}, then
 * seals. The working word array exists only while a slot is being stored.
 * Nothing in this class keeps a slot summary, a word table, or a leg result.
 */
public final class Terrain {
    public static final long MIX_ADD = 0x9E3779B97F4A7C15L;
    public static final long MIX_MUL1 = 0xBF58476D1CE4E5B9L;
    public static final long MIX_MUL2 = 0x94D049BB133111EBL;
    public static final long WORD_XOR = 0xC0FFEEL;
    public static final long PARENT_MUL = 0xD1B54A32D192ED03L;
    public static final long SLOT_MUL = 0xA24BAED4963EE407L;
    public static final byte[] SEAL_DOMAIN = "pcoc-walk-v1".getBytes(java.nio.charset.StandardCharsets.US_ASCII);

    public final Profile profile;
    public final byte[] blob;

    private Terrain(Profile profile, byte[] blob) {
        this.profile = profile;
        this.blob = blob;
    }

    /** Section 10. {@code D = SHA-256(public_key ‖ seed_beacon)}. */
    public static byte[] seed(byte[] publicKey, byte[] seedBeacon) {
        if (publicKey == null || seedBeacon == null || publicKey.length != 32 || seedBeacon.length != 32) {
            throw new IllegalArgumentException("seed preimage");
        }
        return Bytes.sha256(publicKey, seedBeacon);
    }

    /** Four little-endian lanes of the 32-byte seed. */
    public static long[] lanes(byte[] seed) {
        if (seed == null || seed.length != 32) {
            throw new IllegalArgumentException("seed");
        }
        long[] lane = new long[4];
        for (int k = 0; k < 4; k++) {
            lane[k] = Bytes.u64le(seed, k * 8);
        }
        return lane;
    }

    /**
     * Section 10.4. Fills the terrain once from the seed. The step does not write it.
     */
    public static Terrain fill(byte[] publicKey, byte[] seedBeacon, Profile profile) {
        long[] lane = lanes(seed(publicKey, seedBeacon));
        int slots = profile.nSlots;
        int words = profile.wordsPerSlot;
        int slotBytes = profile.slotBytes;
        byte[] blob = new byte[profile.terrainBytes];
        long[] word = new long[words];
        for (int i = 0; i < slots; i++) {
            for (int j = 0; j < words; j++) {
                word[j] = baseWord(lane, i, j);
            }
            if (i > 0) {
                int[] parents = new int[] {i - 1, bitrevParent(i), mulParent(i)};
                for (int k = 1; k <= 3; k++) {
                    int parent = parents[k - 1];
                    int parentBase = parent * slotBytes;
                    long parentMix = (long) k * PARENT_MUL;
                    for (int j = 0; j < words; j++) {
                        long stored = Bytes.u64le(blob, parentBase + j * 8);
                        int rot = (j + k) % 64;
                        word[j] = mix64((word[j] ^ rotl(stored, rot)) ^ parentMix);
                    }
                }
                long slotMix = (long) i * SLOT_MUL;
                for (int j = 0; j < words; j++) {
                    word[j] = mix64(word[j] ^ slotMix);
                }
            }
            int base = i * slotBytes;
            for (int j = 0; j < words; j++) {
                Bytes.putLe64(blob, base + j * 8, word[j]);
            }
        }
        return new Terrain(profile, blob);
    }

    /**
     * NL4 base word, section 10.4. Parent mixing is not part of this word.
     * {@code i} is the slot index and {@code j} is the word index.
     */
    public static long baseWord(long[] lane, int i, int j) {
        long c = ((long) i << 32) ^ (j & 0xffffffffL) ^ WORD_XOR;
        long t = mix64(lane[Math.floorMod(j, 4)] ^ c);
        t = mix64(t ^ rotl(lane[Math.floorMod(j + 1, 4)], 17));
        t = mix64(t ^ rotl(lane[Math.floorMod(j + 2, 4)], 31));
        t = mix64(t ^ rotl(lane[Math.floorMod(j + 3, 4)], 47));
        return t;
    }

    /** Section 10.1. The first step is {@code + MIX_ADD}. */
    public static long mix64(long x) {
        long y = x + MIX_ADD;
        y = (y ^ (y >>> 30)) * MIX_MUL1;
        y = (y ^ (y >>> 27)) * MIX_MUL2;
        return y ^ (y >>> 31);
    }

    /**
     * Section 11.1 fold. {@code acc} starts at 0. Every word of the slot is mixed with the state.
     */
    public long foldSlot(long state) {
        int slot = (int) Long.remainderUnsigned(state, profile.nSlots);
        int words = profile.wordsPerSlot;
        int base = slot * profile.slotBytes;
        long acc = 0;
        for (int w = 0; w < words; w++) {
            long stored = Bytes.u64le(blob, base + w * 8);
            acc ^= mix64(stored ^ (w & 0xffffffffL) ^ state);
        }
        return acc;
    }

    /** Section 11.1. {@code SHA-256(SEAL_DOMAIN ‖ le64(folded))}. */
    public static byte[] sealDigest(long folded) {
        byte[] block = sealBlock(folded);
        return Bytes.sha256(block);
    }

    /** First eight digest bytes as a little-endian integer. Section 7.4. */
    public static long sealState(long folded) {
        return Bytes.u64le(sealDigest(folded), 0);
    }

    /** One step. Reads the slot from the stored terrain and does not write it. */
    public long step(long state) {
        return sealState(foldSlot(state));
    }

    /**
     * Section 11.2. {@code STEPS_PER_LEG} steps from the first eight bytes of the previous checkpoint.
     * The result is not stored.
     */
    public long leg(byte[] previous) {
        if (previous == null || previous.length < 8) {
            throw new IllegalArgumentException("previous");
        }
        long state = Bytes.u64le(previous, 0);
        MessageDigest sha = Bytes.digest();
        byte[] block = new byte[20];
        System.arraycopy(SEAL_DOMAIN, 0, block, 0, SEAL_DOMAIN.length);
        byte[] digest = new byte[32];
        int steps = profile.stepsPerLeg;
        for (int t = 0; t < steps; t++) {
            Bytes.putLe64(block, 12, foldSlot(state));
            sha.update(block);
            try {
                sha.digest(digest, 0, digest.length);
            } catch (DigestException ex) {
                throw new IllegalStateException(ex);
            }
            state = Bytes.u64le(digest, 0);
        }
        return state;
    }

    /** Section 11.3. Offset is {@code u64_le(previous[0:8]) mod OFFSET_MODULUS}. */
    public int sampleOffset(byte[] previous) {
        return (int) Long.remainderUnsigned(Bytes.u64le(previous, 0), profile.offsetModulus);
    }

    public byte[] sample(byte[] previous) {
        int offset = sampleOffset(previous);
        byte[] out = new byte[profile.sampleBytes];
        System.arraycopy(blob, offset, out, 0, out.length);
        return out;
    }

    /** Section 12.2. {@code u64_le(interrupt[0:8]) mod N_SLOTS}. */
    public int slotIndex(byte[] interrupt) {
        return (int) Long.remainderUnsigned(Bytes.u64le(interrupt, 0), profile.nSlots);
    }

    /** The full slot. The caller receives {@code SLOT_BYTES} bytes. */
    public byte[] slot(byte[] interrupt) {
        int index = slotIndex(interrupt);
        byte[] out = new byte[profile.slotBytes];
        System.arraycopy(blob, index * profile.slotBytes, out, 0, out.length);
        return out;
    }

    static int bitrevParent(int index) {
        int bits = Math.max(31 - Integer.numberOfLeadingZeros(index), 1);
        int reversed = Integer.reverse(index) >>> (32 - bits);
        return reversed % index;
    }

    static int mulParent(int index) {
        long mixed = (long) index * MIX_ADD;
        return (int) ((mixed >>> 32) % index);
    }

    private static long rotl(long value, int bits) {
        if (bits == 0) {
            return value;
        }
        return (value << bits) | (value >>> (64 - bits));
    }

    private static byte[] sealBlock(long folded) {
        byte[] block = new byte[20];
        System.arraycopy(SEAL_DOMAIN, 0, block, 0, SEAL_DOMAIN.length);
        Bytes.putLe64(block, 12, folded);
        return block;
    }
}
