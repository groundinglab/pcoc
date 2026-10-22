package pcoc;

import java.math.BigInteger;

/** §14.6 unbiased six-leg index draw from a supplied byte source. */
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
        BigInteger modulus = BigInteger.ONE.shiftLeft(64);
        BigInteger n = BigInteger.valueOf(legTotal);
        BigInteger limit = modulus.subtract(modulus.mod(n));
        int[] got = new int[Const.SIX_LEG];
        int filled = 0;
        while (filled < Const.SIX_LEG) {
            byte[] raw = source.read8();
            if (raw == null || raw.length != 8) {
                return new Six(null, true);
            }
            long u = Bytes.getLe64(raw, 0);
            if (!below(u, limit)) {
                continue;
            }
            int q = (int) Long.remainderUnsigned(u, legTotal);
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
