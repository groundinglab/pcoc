package pcoc;

/**
 * §10.4 dag2 fill. The terrain is the array of little-endian u64 words.
 * Slot {@code i} is words {@code [i * WORDS_PER_SLOT, (i+1) * WORDS_PER_SLOT)}.
 * Filled once per seed. The step does not write it.
 */
public final class Terrain {
    private Terrain() {}

    /** §10.4 NL4 base word, before parent mixing and slot_mix. */
    public static long baseWord(long[] lanes, int i, int j) {
        long c = ((long) i << 32) ^ (long) j ^ Const.WORD_XOR;
        long t = Mix.mix64(lanes[Math.floorMod(j, 4)] ^ c);
        t = Mix.mix64(t ^ Mix.rotl(lanes[Math.floorMod(j + 1, 4)], 17));
        t = Mix.mix64(t ^ Mix.rotl(lanes[Math.floorMod(j + 2, 4)], 31));
        t = Mix.mix64(t ^ Mix.rotl(lanes[Math.floorMod(j + 3, 4)], 47));
        return t;
    }

    /** §10.4. One terrain for this seed. */
    public static long[] fill(Profile profile, long[] lanes) {
        if (lanes.length != 4) {
            throw new IllegalArgumentException("lanes");
        }
        int words = profile.wordsPerSlot;
        int nSlots = profile.nSlots;
        long[] terrain = new long[nSlots * words];
        long[] w = new long[words];
        for (int i = 0; i < nSlots; i++) {
            for (int j = 0; j < words; j++) {
                w[j] = baseWord(lanes, i, j);
            }
            if (i > 0) {
                int p0 = i - 1;
                int p1 = Mix.bitrevParent(i);
                int p2 = Mix.mulParent(i);
                mixParent(w, terrain, p0, 1, words);
                mixParent(w, terrain, p1, 2, words);
                mixParent(w, terrain, p2, 3, words);
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

    /** §11.3. Contiguous window. The offset may fall mid-word. */
    public static byte[] window(long[] terrain, int offset, int length) {
        byte[] out = new byte[length];
        for (int i = 0; i < length; i++) {
            int abs = offset + i;
            long word = terrain[abs >>> 3];
            int shift = (abs & 7) << 3;
            out[i] = (byte) (word >>> shift);
        }
        return out;
    }

    /** §12.2 slot sample: the whole slot, SLOT_BYTES bytes. */
    public static byte[] slotBytes(long[] terrain, int slot, int wordsPerSlot) {
        int base = slot * wordsPerSlot;
        byte[] out = new byte[wordsPerSlot * 8];
        int p = 0;
        for (int j = 0; j < wordsPerSlot; j++) {
            Bytes.putLe64(out, p, terrain[base + j]);
            p += 8;
        }
        return out;
    }

    /** SHA-256 of the whole terrain in slot order, each word little-endian. */
    public static byte[] digest(Sha256 sha, long[] terrain) {
        sha.start();
        byte[] buf = new byte[8192];
        int p = 0;
        for (long v : terrain) {
            if (p == buf.length) {
                sha.update(buf, 0, p);
                p = 0;
            }
            Bytes.putLe64(buf, p, v);
            p += 8;
        }
        if (p > 0) {
            sha.update(buf, 0, p);
        }
        return sha.finish();
    }
}
