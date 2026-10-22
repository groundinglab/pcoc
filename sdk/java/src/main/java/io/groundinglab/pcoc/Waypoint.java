package io.groundinglab.pcoc;

/** §12.2 waypoint preimage. */
public final class Waypoint {
    private Waypoint() {}

    /**
     * §12.2 fold of a stage's checkpoints. {@code F_0 = 0^32}. An empty list returns {@code F_0}
     * and performs no hash.
     */
    public static byte[] fold(Sha256 sha, byte[][] checkpoints) {
        byte[] f = new byte[Const.HASH_BYTES];
        for (byte[] c : checkpoints) {
            f = sha.hash(f, c);
        }
        return f;
    }

    /**
     * §12.2. {@code W = SHA-256(B ‖ slot_sample ‖ stages[g].waypoint ‖ fold)}.
     * {@code B} is {@code I_g}. No waypoint is built from {@code I_17}.
     */
    public static byte[] compute(Sha256 sha, byte[] interrupt, byte[] slotSample, byte[] stageWaypoint, byte[] fold) {
        return sha.hash(interrupt, slotSample, stageWaypoint, fold);
    }

    /** §12.2. {@code slot_index = u64_le(B[0:8]) mod N_SLOTS}. */
    public static int slotIndex(byte[] interrupt, int nSlots) {
        return (int) Long.remainderUnsigned(Bytes.getLe64(interrupt, 0), nSlots);
    }
}
