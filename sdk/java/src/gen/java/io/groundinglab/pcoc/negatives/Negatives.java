package io.groundinglab.pcoc.negatives;

import io.groundinglab.pcoc.Bytes;
import io.groundinglab.pcoc.Const;
import io.groundinglab.pcoc.Mix;
import io.groundinglab.pcoc.Profile;
import io.groundinglab.pcoc.Seed;
import io.groundinglab.pcoc.Sha256;
import io.groundinglab.pcoc.Step;
import io.groundinglab.pcoc.Terrain;

/**
 * §16 permanent negatives. This module is not on a release path.
 * Each method is the construction its entry names, not §11.1, except where the entry
 * says the honest step runs on a different terrain.
 */
public final class Negatives {
    private Negatives() {}

    /** The bad per-step accumulator. The seal around it stays the §11.1 seal. */
    public interface Fold {
        long fold(long[] terrain, int wordsPerSlot, int slot, long s);
    }

    /** A leg that uses {@code fold} instead of §11.1, then the honest seal. */
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

    /** §11.1. {@code S XOR (XOR over w of (word(w) XOR w))}. */
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
     * §16 {@code prefold_hoist}. {@code pre(i)} is computed once per slot.
     * Each step uses {@code (pre(i) + S) mod 2^64} in place of the §11.1 accumulator.
     */
    public static long prefoldHoist(long[] terrain, Profile profile, Sha256 sha, long s0) {
        int words = profile.wordsPerSlot;
        int slots = profile.nSlots;
        long[] pre = new long[slots];
        for (int i = 0; i < slots; i++) {
            int base = i * words;
            long sum = 0L;
            for (int w = 0; w < words; w++) {
                sum += Mix.mix64(terrain[base + w] ^ (long) w);
            }
            pre[i] = sum;
        }
        return leg(terrain, profile, sha, s0, new Fold() {
            public long fold(long[] t, int wordsPerSlot, int slot, long s) {
                return pre[slot] + s;
            }
        });
    }

    /**
     * §16 {@code final_stage_hoist}. {@code S_folded = x XOR (x >> 31)}, where {@code x} is the
     * XOR of {@code mix64} without its last line applied to {@code word(w) XOR S}.
     */
    public static final Fold FINAL_STAGE = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            int base = slot * wordsPerSlot;
            long x = 0L;
            for (int w = 0; w < wordsPerSlot; w++) {
                x ^= mix64BeforeLast(terrain[base + w] ^ s);
            }
            return x ^ (x >>> 31);
        }
    };

    /** XOR accumulator of the full {@code mix64(word(w) XOR S)}. Equals {@link #FINAL_STAGE}. */
    public static final Fold XOR_OF_MIX64 = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            int base = slot * wordsPerSlot;
            long acc = 0L;
            for (int w = 0; w < wordsPerSlot; w++) {
                acc ^= Mix.mix64(terrain[base + w] ^ s);
            }
            return acc;
        }
    };

    /** §10.1 lines before {@code return y XOR (y >> 31)}. */
    static long mix64BeforeLast(long x) {
        long y = x + Const.MIX_ADD;
        y = (y ^ (y >>> 30)) * Const.MIX_MUL1;
        y = (y ^ (y >>> 27)) * Const.MIX_MUL2;
        return y;
    }

    /** §16. Word 0 only. */
    public static final Fold PARTIAL_SLOT = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            long word = terrain[slot * wordsPerSlot];
            return Mix.mix64(word ^ s);
        }
    };

    /** §16. Words {@code 0 … WORDS_PER_SLOT − 2}. The last word is not read. */
    public static final Fold PARTIAL_2047 = new Fold() {
        public long fold(long[] terrain, int wordsPerSlot, int slot, long s) {
            int base = slot * wordsPerSlot;
            long acc = 0L;
            int last = wordsPerSlot - 1;
            for (int w = 0; w < last; w++) {
                acc += Mix.mix64(terrain[base + w] ^ s);
            }
            return acc;
        }
    };

    /**
     * §16 {@code cached_leg}. A step that reads a slot already read in this leg reuses that
     * earlier {@code S_folded}. The generator stops when the leg never revisits a slot.
     */
    public static long cachedLeg(long[] terrain, Profile profile, Sha256 sha, long s0) {
        byte[] buf = new byte[20];
        System.arraycopy(Const.SEAL_DOMAIN, 0, buf, 0, Const.SEAL_DOMAIN.length);
        Long[] seen = new Long[profile.nSlots];
        long s = s0;
        boolean revisit = false;
        int words = profile.wordsPerSlot;
        for (int t = 0; t < profile.stepsPerLeg; t++) {
            int slot = Step.slotIndex(s, profile.nSlots);
            long acc;
            if (seen[slot] != null) {
                acc = seen[slot];
                revisit = true;
            } else {
                acc = Step.foldSlot(terrain, words, slot, s);
                seen[slot] = acc;
            }
            s = Step.seal(sha, buf, acc);
        }
        if (!revisit) {
            throw new IllegalStateException("cached_leg: the leg did not revisit a slot");
        }
        return s;
    }

    /** §10.4 one-lane map, then the same parents and slot mix. */
    public static long[] oneLane(Profile profile, long[] lanes) {
        return fill(profile, lanes, new Base() {
            public long at(long[] p, int i, int j) {
                long c = ((long) i << 32) ^ (long) j ^ Const.WORD_XOR;
                return Mix.mix64(p[Math.floorMod(j, 4)] ^ c);
            }
        });
    }

    /** §10.4 linear map, one mix64, then the same parents and slot mix. */
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

    /** §10.2 squeeze: every lane is {@code P[0] XOR P[1] XOR P[2] XOR P[3]}. */
    public static long[] squeezed(Profile profile, long[] lanes) {
        long z = lanes[0] ^ lanes[1] ^ lanes[2] ^ lanes[3];
        return Terrain.fill(profile, new long[] {z, z, z, z});
    }

    /** Terrain of {@code otherPublicKey} with the same seed beacon. Honest §10.4 fill. */
    public static long[] otherTerrain(Profile profile, Sha256 sha, byte[] otherPublicKey, byte[] seedBeacon) {
        byte[] d = Seed.derive(sha, otherPublicKey, seedBeacon);
        return Terrain.fill(profile, Seed.lanes(d));
    }

    public static String legsLeHex(long legResult) {
        return Bytes.hex(Bytes.le64(legResult));
    }

    private interface Base {
        long at(long[] lanes, int i, int j);
    }

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
}
