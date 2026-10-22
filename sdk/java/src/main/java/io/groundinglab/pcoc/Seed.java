package io.groundinglab.pcoc;

/** §10 seed preimage, seed, and lanes. */
public final class Seed {
    private Seed() {}

    /** §10. {@code D = SHA-256(public_key ‖ seed_beacon)}. No label. */
    public static byte[] derive(Sha256 sha, byte[] publicKey, byte[] seedBeacon) {
        if (publicKey.length != Const.PUBLIC_KEY_BYTES || seedBeacon.length != Const.BEACON_BYTES) {
            throw new IllegalArgumentException("seed preimage");
        }
        return sha.hash(publicKey, seedBeacon);
    }

    /** §10. {@code P[k] = u64_le(D[8k : 8k+8])} for k = 0 … 3. */
    public static long[] lanes(byte[] d) {
        if (d.length != Const.HASH_BYTES) {
            throw new IllegalArgumentException("seed");
        }
        long[] p = new long[4];
        for (int k = 0; k < 4; k++) {
            p[k] = Bytes.getLe64(d, 8 * k);
        }
        return p;
    }
}
