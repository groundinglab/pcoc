package io.groundinglab.pcoc;

/** §11.4 checkpoint preimage. */
public final class Checkpoint {
    private Checkpoint() {}

    /**
     * §11.4. {@code SHA-256(previous ‖ stage limb ‖ terrain sample ‖ le64(leg_result))}.
     * The stage limb is the seed on stage 0 and {@code W_g} on stage {@code g ≥ 1}.
     */
    public static byte[] digest(Sha256 sha, byte[] previous, byte[] stageLimb, byte[] sample, byte[] legResultLe) {
        return sha.hash(previous, stageLimb, sample, legResultLe);
    }
}
