package io.groundinglab.pcoc;

/** §10.1 mix64, §10.3 rotation and parent indices. */
public final class Mix {
    private Mix() {}

    /** §10.1. Includes the initial addition of MIX_ADD. */
    public static long mix64(long x) {
        long y = x + Const.MIX_ADD;
        y = (y ^ (y >>> 30)) * Const.MIX_MUL1;
        y = (y ^ (y >>> 27)) * Const.MIX_MUL2;
        return y ^ (y >>> 31);
    }

    /** §10.3. Left rotation. {@code r} is in 0 … 63. */
    public static long rotl(long w, int r) {
        if (r == 0) {
            return w;
        }
        return (w << r) | (w >>> (64 - r));
    }

    /** §10.3. Greatest b with 2^b ≤ n, for n ≥ 1. */
    public static int floorLog2(int n) {
        return 31 - Integer.numberOfLeadingZeros(n);
    }

    /**
     * §10.3. Bit k of the result equals bit 31−k of v, bit 0 the least significant.
     */
    public static long reverse32(long v) {
        return Integer.reverse((int) v) & 0xffffffffL;
    }

    /** §10.3. Parent in 0 … i−1. */
    public static int bitrevParent(int i) {
        int bits = Math.max(floorLog2(i), 1);
        long shifted = reverse32(i) >>> (32 - bits);
        return (int) (shifted % i);
    }

    /** §10.3. Parent in 0 … i−1. */
    public static int mulParent(int i) {
        long h = (long) i * Const.MIX_ADD;
        long hi = h >>> 32;
        return (int) (hi % i);
    }
}
