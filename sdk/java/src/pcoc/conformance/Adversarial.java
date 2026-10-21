package pcoc.conformance;

import java.lang.reflect.Field;
import java.util.Arrays;
import pcoc.hike.Bytes;
import pcoc.hike.Profile;
import pcoc.hike.Terrain;

/**
 * Withdrawn constructions. Each one is recomputed here and rejected by the
 * v2.1 verifier. These methods are not the producer.
 */
public final class Adversarial {
    enum Kind {
        NO_ADD,
        V2_0,
        XOR_ACC,
        PARTIAL
    }

    private Adversarial() {}

    /**
     * A leg-result cache returns an earlier leg for a later previous checkpoint.
     * The verifier walks that later checkpoint and rejects the cached result.
     */
    public static void cachedLeg() {
        for (Field field : Terrain.class.getDeclaredFields()) {
            String name = field.getName().toLowerCase();
            if (name.contains("cache") || name.contains("slotxor") || name.contains("fillpin") || name.contains("seeddomain")) {
                throw new AssertionError("terrain kept " + field.getName());
            }
        }
        byte[] publicKey = new byte[32];
        byte[] beacon = new byte[32];
        publicKey[0] = 0x11;
        beacon[0] = 0x22;
        Terrain terrain = Terrain.fill(publicKey, beacon, Profile.CONFORMANCE_SMALL_V2);
        byte[] first = new byte[32];
        byte[] second = new byte[32];
        first[0] = 1;
        second[0] = 2;
        long cached = terrain.leg(first);
        long fresh = terrain.leg(second);
        if (cached == fresh) {
            throw new AssertionError("cached-leg fixture collided");
        }
        if (terrain.leg(second) == cached) {
            throw new AssertionError("cached leg accepted");
        }
        if (terrain.leg(second) != fresh) {
            throw new AssertionError("recompute diverged");
        }
        System.out.println("REJECT cached_leg");
    }

    /**
     * The withdrawn fold on a stored slot. {@code wordsRead} is used only for a partial read.
     * A missing {@code words_read} in the vector is zero and is ignored except for {@link Kind#PARTIAL}.
     */
    static long withdrawnFold(Terrain terrain, long state, Kind kind, int wordsRead) {
        int words = terrain.profile.wordsPerSlot;
        int limit = kind == Kind.PARTIAL ? wordsRead : words;
        if (limit < 1 || limit > words) {
            throw new AssertionError("partial bound");
        }
        if (kind == Kind.V2_0) {
            long folded = state;
            for (int w = 0; w < words; w++) {
                folded ^= word(terrain, state, w) ^ (w & 0xffffffffL);
            }
            return folded;
        }
        long acc = 0;
        for (int w = 0; w < limit; w++) {
            long mixed = word(terrain, state, w) ^ (w & 0xffffffffL) ^ state;
            long term = kind == Kind.NO_ADD ? mix64WithoutAdd(mixed) : Terrain.mix64(mixed);
            if (kind == Kind.XOR_ACC) {
                acc ^= term;
            } else {
                acc += term;
            }
        }
        return acc;
    }

    private static long word(Terrain terrain, long state, int index) {
        int slot = (int) Long.remainderUnsigned(state, terrain.profile.nSlots);
        int offset = slot * terrain.profile.slotBytes + index * 8;
        byte[] eight = new byte[8];
        System.arraycopy(terrain.blob, offset, eight, 0, 8);
        return Bytes.u64le(eight);
    }

    /**
     * One lane, which section 10.4 says is not NL4: only the first {@code mix64} of the base word.
     */
    static long refWord(long[] lane, int i, int j) {
        long c = ((long) i << 32) ^ (j & 0xffffffffL) ^ Terrain.WORD_XOR;
        return Terrain.mix64(lane[Math.floorMod(j, 4)] ^ c);
    }

    /**
     * A linear mix of the four lanes, which section 10.4 says is not NL4.
     * The NL4 rotations are XORed together and mixed once. Complementing two
     * lanes cancels under XOR, so the word does not move.
     */
    static long all4Word(long[] lane, int i, int j) {
        long c = ((long) i << 32) ^ (j & 0xffffffffL) ^ Terrain.WORD_XOR;
        long mixed = lane[Math.floorMod(j, 4)]
                ^ rotl(lane[Math.floorMod(j + 1, 4)], 17)
                ^ rotl(lane[Math.floorMod(j + 2, 4)], 31)
                ^ rotl(lane[Math.floorMod(j + 3, 4)], 47)
                ^ c;
        return Terrain.mix64(mixed);
    }

    private static long rotl(long value, int bits) {
        if (bits == 0) {
            return value;
        }
        return (value << bits) | (value >>> (64 - bits));
    }

    /**
     * The first two NL4 steps. Not the full chain. Complementing one lane moves
     * half of these words on the small slot.
     */
    static long pairWord(long[] lane, int i, int j) {
        long c = ((long) i << 32) ^ (j & 0xffffffffL) ^ Terrain.WORD_XOR;
        long t = Terrain.mix64(lane[Math.floorMod(j, 4)] ^ c);
        return Terrain.mix64(t ^ rotl(lane[Math.floorMod(j + 1, 4)], 17));
    }

    /**
     * Terrain whose base word is the linear four-lane mix. Parents and
     * {@code slot_mix} stay the section 10.4 steps. This is not the producer.
     */
    static byte[] linearTerrain(long[] lane, Profile profile) {
        int slots = profile.nSlots;
        int words = profile.wordsPerSlot;
        int slotBytes = profile.slotBytes;
        byte[] blob = new byte[profile.terrainBytes];
        long[] word = new long[words];
        for (int i = 0; i < slots; i++) {
            for (int j = 0; j < words; j++) {
                word[j] = all4Word(lane, i, j);
            }
            if (i > 0) {
                int[] parents = new int[] {i - 1, Terrain.bitrevParent(i), Terrain.mulParent(i)};
                for (int k = 1; k <= 3; k++) {
                    int parent = parents[k - 1];
                    int parentBase = parent * slotBytes;
                    long parentMix = (long) k * Terrain.PARENT_MUL;
                    for (int j = 0; j < words; j++) {
                        long stored = wordAt(blob, parentBase + j * 8);
                        int rot = (j + k) % 64;
                        word[j] = Terrain.mix64((word[j] ^ rotl(stored, rot)) ^ parentMix);
                    }
                }
                long slotMix = (long) i * Terrain.SLOT_MUL;
                for (int j = 0; j < words; j++) {
                    word[j] = Terrain.mix64(word[j] ^ slotMix);
                }
            }
            int base = i * slotBytes;
            for (int j = 0; j < words; j++) {
                Bytes.putLe64(blob, base + j * 8, word[j]);
            }
        }
        return blob;
    }

    /** Checkpoint preimage with limb 3 taken at {@code offset}, not at {@code offset + 1}. */
    static byte[] checkpointAt(byte[] previous, byte[] limb, byte[] blob, int offset, int sample, long leg) {
        byte[] window = Arrays.copyOfRange(blob, offset, offset + sample);
        return Bytes.sha256(previous, limb, window, Bytes.le64(leg));
    }

    private static long wordAt(byte[] blob, int offset) {
        byte[] eight = new byte[8];
        System.arraycopy(blob, offset, eight, 0, 8);
        return Bytes.u64le(eight);
    }

    /** Section 10.1 with the initial {@code + MIX_ADD} omitted. Not {@code mix64}. */
    private static long mix64WithoutAdd(long x) {
        long y = x;
        y = (y ^ (y >>> 30)) * Terrain.MIX_MUL1;
        y = (y ^ (y >>> 27)) * Terrain.MIX_MUL2;
        return y ^ (y >>> 31);
    }
}
