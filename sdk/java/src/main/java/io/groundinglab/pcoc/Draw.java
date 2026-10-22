package io.groundinglab.pcoc;

import java.math.BigInteger;

/**
 * §14.6 six-leg draw and §15.5 thorough draw. Both take a supplied byte source.
 * Those are rig functions. Production verify reads the operating-system CSPRNG
 * through {@link #csprng()} and does not take a byte source.
 */
public final class Draw {
    private Draw() {}

    public interface Source {
        /** Next 8 bytes, or null when the source is short. */
        byte[] read8();
    }

    public static final class Six {
        public final int[] indices;
        /** True when the byte source ran out before six distinct indices. */
        public final boolean notRead;

        private Six(int[] indices, boolean notRead) {
            this.indices = indices;
            this.notRead = notRead;
        }
    }

    /**
     * §14.6. When {@code legTotal < 6} the draw is not made: indices is null and notRead is false.
     * A short source sets notRead. An 8-byte sample at or above the limit is consumed.
     */
    public static Six six(int legTotal, Source source) {
        if (legTotal < Const.SIX_LEG) {
            return new Six(null, false);
        }
        int[] got = new int[Const.SIX_LEG];
        int filled = 0;
        while (filled < Const.SIX_LEG) {
            Integer q = one(legTotal, source);
            if (q == null) {
                return new Six(null, true);
            }
            boolean dup = false;
            for (int i = 0; i < filled; i++) {
                if (got[i] == q) {
                    dup = true;
                    break;
                }
            }
            if (dup) {
                continue;
            }
            got[filled++] = q;
        }
        return new Six(got, false);
    }

    /**
     * §15.5 thorough-round index draw from a supplied byte source. One draw of
     * {@code D = 6 · thoroughRounds} indices, then split into rounds of 6.
     * Returns null when {@code legTotal < 6} (the draw is not defined) and an
     * empty indices array when the byte source is exhausted before {@code D}
     * (that exhaustion is {@code not read}; a trailing group shorter than 6 is not a round).
     */
    public static final class Thorough {
        public final int[] indices;
        public final boolean notDefined;
        public final boolean notRead;

        private Thorough(int[] indices, boolean notDefined, boolean notRead) {
            this.indices = indices;
            this.notDefined = notDefined;
            this.notRead = notRead;
        }
    }

    public static Thorough thorough(int legTotal, int thoroughRounds, Source source) {
        if (legTotal < Const.SIX_LEG) {
            return new Thorough(null, true, false);
        }
        if (thoroughRounds < 1) {
            throw new IllegalArgumentException("thorough_rounds");
        }
        int d = Const.SIX_LEG * thoroughRounds;
        int[] phase = new int[d];
        int filled = 0;
        boolean[] seen = new boolean[legTotal];
        int distinct = 0;
        while (filled < d) {
            Integer q = one(legTotal, source);
            if (q == null) {
                int[] got = new int[filled];
                System.arraycopy(phase, 0, got, 0, filled);
                return new Thorough(got, false, true);
            }
            int groupStart = (filled / Const.SIX_LEG) * Const.SIX_LEG;
            boolean inGroup = false;
            for (int i = groupStart; i < filled; i++) {
                if (phase[i] == q) {
                    inGroup = true;
                    break;
                }
            }
            if (inGroup) {
                continue;
            }
            if (seen[q] && distinct < legTotal) {
                continue;
            }
            phase[filled++] = q;
            if (!seen[q]) {
                seen[q] = true;
                distinct++;
            }
        }
        return new Thorough(phase, false, false);
    }

    /**
     * One §14.6 unbiased draw from {@code 0 … legTotal−1}. Null when the source
     * cannot supply the next 8 bytes. A sample at or above the rejection limit is consumed.
     */
    static Integer one(int legTotal, Source source) {
        if (legTotal < 1) {
            throw new IllegalArgumentException("leg_total");
        }
        BigInteger modulus = BigInteger.ONE.shiftLeft(64);
        BigInteger n = BigInteger.valueOf(legTotal);
        BigInteger limit = modulus.subtract(modulus.mod(n));
        while (true) {
            byte[] raw = source.read8();
            if (raw == null || raw.length != 8) {
                return null;
            }
            long u = Bytes.getLe64(raw, 0);
            if (!below(u, limit)) {
                continue;
            }
            return (int) Long.remainderUnsigned(u, legTotal);
        }
    }

    /** Operating-system CSPRNG (§14.6). The release verify path uses this. */
    public static Source csprng() {
        return new Source() {
            private final java.security.SecureRandom rnd = new java.security.SecureRandom();

            public byte[] read8() {
                byte[] b = new byte[8];
                rnd.nextBytes(b);
                return b;
            }
        };
    }

    private static boolean below(long u, BigInteger limit) {
        byte[] mag = new byte[8];
        for (int i = 0; i < 8; i++) {
            mag[i] = (byte) (u >>> (56 - 8 * i));
        }
        return new BigInteger(1, mag).compareTo(limit) < 0;
    }

    /** SHA-256 chain {@code b0 = SHA-256(label)}, {@code b_{n+1} = SHA-256(b_n)}, read 8 bytes at a time. */
    public static final class HashChain implements Source {
        private final Sha256 sha;
        private byte[] block;
        private int off;

        public HashChain(Sha256 sha, byte[] firstBlock) {
            this.sha = sha;
            this.block = firstBlock;
            this.off = 0;
        }

        public byte[] read8() {
            byte[] o = new byte[8];
            System.arraycopy(block, off, o, 0, 8);
            off += 8;
            if (off == 32) {
                block = sha.hash(block);
                off = 0;
            }
            return o;
        }
    }

    /** Finite injected bytes. A short read returns null. */
    public static final class Finite implements Source {
        private final byte[] bytes;
        private int off;

        public Finite(byte[] bytes) {
            this.bytes = bytes;
            this.off = 0;
        }

        public byte[] read8() {
            if (off + 8 > bytes.length) {
                return null;
            }
            byte[] o = new byte[8];
            System.arraycopy(bytes, off, o, 0, 8);
            off += 8;
            return o;
        }
    }
}
