package pcoc.negatives;

import pcoc.Bytes;
import pcoc.Const;
import pcoc.Mix;
import pcoc.Profile;
import pcoc.Seed;
import pcoc.Sha256;
import pcoc.Step;
import pcoc.Terrain;

/**
 * Permanent negatives (§16, §19 F1–F4). This module is not on the hike, step, or verify path.
 * Each method is the shortcut named in section 16, not §11.1.
 */
public final class Negatives {
    private Negatives() {}

    /** The bad per-step accumulator. The seal around it stays the §11.1 seal. */
    public interface Fold {
        long fold(long[] terrain, int wordsPerSlot, int slot, long s);
    }

    /**
     * A leg that uses {@code fold} instead of §11.1, then the honest seal.
     * {@code cachedLeg} does not use this.
     */
    public static long leg(long[] terrain, Profile profile, Sha256 sha, long s0, Fold fold) {
        byte[] buf = new byte[20];
        System.arraycopy(Const.SEAL_DOMAIN, 0, buf, 0, Const.SEAL_DOMAIN.length);
        long s = s0;
        int words = profile.wordsPerSlot;
        int slots = profile.nSlots;
        for (int t = 0; t < profile.stepsPerLeg; t++) {
            int slot = Step.slotIndex(s, slots);
            long acc = fold.fold(terrain, words, slot, s);
            s = Step.seal(sha, buf, acc);
        }
        return s;
    }

    /** §11.1 names this. {@code S XOR (XOR over w of (word(w) XOR w))}. State-free summary. */
    public static final Fold SLOT_SUMMARY = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            int base = slot * wordsPerSlot;
            long x = 0L;
            for (int w = 0; w < wordsPerSlot; w++) {
                x ^= terrain[base + w] ^ (long) w;
            }
            return s ^ x;
        }
    };

    /** §11.1. {@code acc = acc XOR mix64(word(w) XOR w XOR S)}. */
    public static final Fold XOR_ACCUMULATOR = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            int base = slot * wordsPerSlot;
            long acc = 0L;
            for (int w = 0; w < wordsPerSlot; w++) {
                acc ^= Mix.mix64(terrain[base + w] ^ (long) w ^ s);
            }
            return acc;
        }
    };

    /** §11.1. {@code acc = (acc + mix64(word(w) XOR w XOR S)) mod 2^64}. */
    public static final Fold XOR_W = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            int base = slot * wordsPerSlot;
            long acc = 0L;
            for (int w = 0; w < wordsPerSlot; w++) {
                acc += Mix.mix64(terrain[base + w] ^ (long) w ^ s);
            }
            return acc;
        }
    };

    /** §11.1. {@code acc = (acc + mix64(word(w) XOR ((S + w) mod 2^64))) mod 2^64}. */
    public static final Fold S_PLUS_W = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            int base = slot * wordsPerSlot;
            long acc = 0L;
            for (int w = 0; w < wordsPerSlot; w++) {
                acc += Mix.mix64(terrain[base + w] ^ (s + (long) w));
            }
            return acc;
        }
    };

    /**
     * F2 hoist. The state-free term {@code word(w) XOR w} is summed once, then {@code mix64(S)}
     * is added. This is not the §11.1 per-word input {@code word(w) XOR S}.
     */
    public static final Fold PREFOLD_HOIST = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            int base = slot * wordsPerSlot;
            long pre = 0L;
            for (int w = 0; w < wordsPerSlot; w++) {
                pre += Mix.mix64(terrain[base + w] ^ (long) w);
            }
            return pre + Mix.mix64(s);
        }
    };

    /** F4. Only word 0 is read. */
    public static final Fold PARTIAL_SLOT = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            long word = terrain[slot * wordsPerSlot];
            return Mix.mix64(word ^ s);
        }
    };

    /**
     * F4 {@code partial_slot_read_2047}. The production off-by-one reads
     * {@code WORDS_PER_SLOT − 1} words. On a profile whose slot is shorter than 2048,
     * that is still one word short of the slot.
     */
    public static final Fold PARTIAL_2047 = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            int base = slot * wordsPerSlot;
            long acc = 0L;
            for (int w = 0; w < wordsPerSlot - 1; w++) {
                acc += Mix.mix64(terrain[base + w] ^ s);
            }
            return acc;
        }
    };

    /** A cached leg returns the opening state and runs no step. */
    public static long cachedLeg(long s0) {
        return s0;
    }

    /** §10.4 one-lane map, then the same parent mix and slot_mix. Not NL4. */
    public static long[] oneLane(Profile profile, long[] lanes) {
        return fill(profile, lanes, new Base() {
            public long at(long[] p, int i, int j) {
                long c = ((long) i << 32) ^ (long) j ^ Const.WORD_XOR;
                return Mix.mix64(p[Math.floorMod(j, 4)] ^ c);
            }
        });
    }

    /** §10.4 linear four-lane map, one mix64, then parent mix and slot_mix. Not NL4. */
    public static long[] all4(Profile profile, long[] lanes) {
        return fill(profile, lanes, new Base() {
            public long at(long[] p, int i, int j) {
                long c = ((long) i << 32) ^ (long) j ^ Const.WORD_XOR;
                long x = p[Math.floorMod(j, 4)]
                        ^ Mix.rotl(p[Math.floorMod(j + 1, 4)], 17)
                        ^ Mix.rotl(p[Math.floorMod(j + 2, 4)], 31)
                        ^ Mix.rotl(p[Math.floorMod(j + 3, 4)], 47)
                        ^ c;
                return Mix.mix64(x);
            }
        });
    }

    /** F4 shared terrain: another public key's terrain for the same seed beacon. */
    public static long[] shared(Profile profile, Sha256 sha, byte[] otherPublicKey, byte[] seedBeacon) {
        byte[] d = Seed.derive(sha, otherPublicKey, seedBeacon);
        return Terrain.fill(profile, Seed.lanes(d));
    }

    /** F4. Every lane is replaced by {@code P[0]} before the NL4 fill. */
    public static long[] squeezed(Profile profile, long[] lanes) {
        long[] squeezed = new long[] {lanes[0], lanes[0], lanes[0], lanes[0]};
        return Terrain.fill(profile, squeezed);
    }

    private interface Base {
        long at(long[] lanes, int i, int j);
    }

    /** Parent mixing copied here so {@link Terrain} has no base-word hook. */
    private static long[] fill(Profile profile, long[] lanes, Base base) {
        int words = profile.wordsPerSlot;
        int nSlots = profile.nSlots;
        long[] terrain = new long[nSlots * words];
        long[] w = new long[words];
        for (int i = 0; i < nSlots; i++) {
            for (int j = 0; j < words; j++) {
                w[j] = base.at(lanes, i, j);
            }
            if (i > 0) {
                mixParent(w, terrain, i - 1, 1, words);
                mixParent(w, terrain, Mix.bitrevParent(i), 2, words);
                mixParent(w, terrain, Mix.mulParent(i), 3, words);
                long slotMix = (long) i * Const.SLOT_MUL;
                for (int j = 0; j < words; j++) {
                    w[j] = Mix.mix64(w[j] ^ slotMix);
                }
            }
            System.arraycopy(w, 0, terrain, i * words, words);
        }
        return terrain;
    }

    private static void mixParent(long[] w, long[] terrain, int parent, int k, int words) {
        long kMul = (long) k * Const.PARENT_MUL;
        int base = parent * words;
        for (int j = 0; j < words; j++) {
            int rot = (j + k) & 63;
            long pw = terrain[base + j];
            w[j] = Mix.mix64((w[j] ^ Mix.rotl(pw, rot)) ^ kMul);
        }
    }

    /** Little-endian wire hex of a leg result. */
    public static String legsLeHex(long legResult) {
        return Bytes.hex(Bytes.le64(legResult));
    }
}
