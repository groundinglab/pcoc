package pcoc.hike;

import java.security.MessageDigest;
import java.util.Arrays;

/** Sections 10 and 11. Terrain is filled once and then only read. */
public final class Terrain {
    static final long MIX_ADD = 0x9E3779B97F4A7C15L;
    static final long MIX_MUL1 = 0xBF58476D1CE4E5B9L;
    static final long MIX_MUL2 = 0x94D049BB133111EBL;
    static final long WORD_XOR = 0xC0FFEEL;
    static final long PARENT_MUL = 0xD1B54A32D192ED03L;
    static final long SLOT_MUL = 0xA24BAED4963EE407L;
    static final byte[] SEAL_DOMAIN = "pcoc-walk-v1".getBytes(java.nio.charset.StandardCharsets.US_ASCII);
    static final byte[] SEED_DOMAIN = new byte[] {0x50, 0x53, 0x45, 0x45, 0x44, 0x36, 0x34, 0x00};

    public final Profile profile;
    public final byte[] blob;
    private final long[] slotXor;
    private byte[] cachedPrevious8;
    private long cachedLeg;

    private Terrain(Profile profile, byte[] blob, long[] slotXor) {
        this.profile = profile;
        this.blob = blob;
        this.slotXor = slotXor;
    }

    public static long fillPin(byte[] seed) {
        if (seed.length != 64) {
            throw new IllegalArgumentException("production_seed");
        }
        long state = Bytes.u64le(SEED_DOMAIN);
        for (int i = 0; i < 8; i++) {
            long lane = Bytes.u64le(seed, 8 * i);
            state = mix64(state ^ lane ^ (i * MIX_ADD));
        }
        return state;
    }

    public static Terrain fill(byte[] publicKey, byte[] seedBeacon, Profile profile) {
        byte[] seed = Bytes.concat(publicKey, seedBeacon);
        long pin = fillPin(seed);
        int slots = profile.nSlots;
        int words = profile.wordsPerSlot;
        byte[] blob = new byte[profile.terrainBytes];
        long[] slotXor = new long[slots];
        long[] stored = new long[slots * words];
        for (int i = 0; i < slots; i++) {
            int base = i * words;
            long mixed = pin ^ (((long) i) << 32) ^ WORD_XOR;
            for (int j = 0; j < words; j++) {
                stored[base + j] = mix64(mixed ^ (long) j);
            }
            if (i > 0) {
                int[] parents = {i - 1, bitrevParent(i), mulParent(i)};
                for (int k = 1; k <= 3; k++) {
                    int parent = parents[k - 1] * words;
                    long mix = k * PARENT_MUL;
                    for (int j = 0; j < words; j++) {
                        int rot = (j + k) & 63;
                        long word = stored[parent + j];
                        if (rot != 0) {
                            word = (word << rot) | (word >>> (64 - rot));
                        }
                        stored[base + j] = mix64((stored[base + j] ^ word) ^ mix);
                    }
                }
                long slotMix = (long) i * SLOT_MUL;
                for (int j = 0; j < words; j++) {
                    stored[base + j] = mix64(stored[base + j] ^ slotMix);
                }
            }
            long folded = 0;
            int at = i * profile.slotBytes;
            for (int j = 0; j < words; j++) {
                long word = stored[base + j];
                folded ^= word ^ (long) j;
                Bytes.putLe64(blob, at + 8 * j, word);
            }
            slotXor[i] = folded;
        }
        return new Terrain(profile, blob, slotXor);
    }

    public static long mix64(long x) {
        long y = x + MIX_ADD;
        y = (y ^ (y >>> 30)) * MIX_MUL1;
        y = (y ^ (y >>> 27)) * MIX_MUL2;
        return y ^ (y >>> 31);
    }

    public long leg(byte[] previous) {
        byte[] key = Arrays.copyOf(previous, 8);
        synchronized (this) {
            if (cachedPrevious8 != null && Arrays.equals(key, cachedPrevious8)) {
                return cachedLeg;
            }
        }
        long state = Bytes.u64le(previous);
        byte[] buf = new byte[20];
        System.arraycopy(SEAL_DOMAIN, 0, buf, 0, 12);
        MessageDigest md = Bytes.digest();
        byte[] out = new byte[32];
        int steps = profile.stepsPerLeg;
        int slots = profile.nSlots;
        for (int t = 0; t < steps; t++) {
            int slot = (int) Long.remainderUnsigned(state, slots);
            long folded = state ^ slotXor[slot];
            Bytes.putLe64(buf, 12, folded);
            md.update(buf);
            try {
                md.digest(out, 0, 32);
            } catch (Exception ex) {
                throw new IllegalStateException(ex);
            }
            state = Bytes.u64le(out);
        }
        synchronized (this) {
            cachedPrevious8 = key;
            cachedLeg = state;
        }
        return state;
    }

    public int sampleOffset(byte[] previous) {
        return (int) Long.remainderUnsigned(Bytes.u64le(previous), profile.offsetModulus);
    }

    public byte[] sample(byte[] previous) {
        int offset = sampleOffset(previous);
        return Arrays.copyOfRange(blob, offset, offset + profile.sampleBytes);
    }

    public int slotIndex(byte[] interrupt) {
        return (int) Long.remainderUnsigned(Bytes.u64le(interrupt), profile.nSlots);
    }

    public byte[] slot(byte[] interrupt) {
        int index = slotIndex(interrupt);
        int start = index * profile.slotBytes;
        return Arrays.copyOfRange(blob, start, start + profile.slotBytes);
    }

    static int bitrevParent(int index) {
        int bits = 31 - Integer.numberOfLeadingZeros(index);
        if (bits < 1) {
            bits = 1;
        }
        int shifted = Integer.reverse(index) >>> (32 - bits);
        return shifted % index;
    }

    static int mulParent(int index) {
        long mixed = (long) index * MIX_ADD;
        return (int) ((mixed >>> 32) % index);
    }
}
