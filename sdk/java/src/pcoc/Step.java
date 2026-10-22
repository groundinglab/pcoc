package pcoc;

/** §11.1 step. Every step loads the full slot and folds every word with S. */
public final class Step {
    private Step() {}

    /** §11.1. {@code i = S mod N_SLOTS}. */
    public static int slotIndex(long s, int nSlots) {
        return (int) Long.remainderUnsigned(s, nSlots);
    }

    /**
     * §11.1 fold. {@code acc} starts at 0. Each word is {@code mix64(word(w) XOR S)},
     * added mod 2^64. The loop bound is this profile's WORDS_PER_SLOT.
     */
    public static long foldSlot(long[] terrain, int wordsPerSlot, int slot, long s) {
        int base = slot * wordsPerSlot;
        long acc = 0L;
        for (int w = 0; w < wordsPerSlot; w++) {
            long word = terrain[base + w];
            acc += Mix.mix64(word ^ s);
        }
        return acc;
    }

    /** §11.1 seal. {@code buf20[0:12]} is SEAL_DOMAIN. Returns S'. */
    public static long seal(Sha256 sha, byte[] buf20, long folded) {
        Bytes.putLe64(buf20, 12, folded);
        sha.start();
        sha.update(buf20, 0, 20);
        byte[] e = sha.finish();
        return Bytes.getLe64(e, 0);
    }

    /** §11.1. One step. Returns S'. */
    public static long step(long[] terrain, int wordsPerSlot, int nSlots, Sha256 sha, byte[] buf20, long s) {
        int slot = slotIndex(s, nSlots);
        long acc = foldSlot(terrain, wordsPerSlot, slot, s);
        return seal(sha, buf20, acc);
    }
}
