package pcoc;

import java.util.Arrays;

/** Byte helpers for §3 and §7.1. Not a second encoding. */
public final class Bytes {
    private static final char[] HEX = "0123456789abcdef".toCharArray();

    private Bytes() {}

    public static void putLe64(byte[] b, int off, long v) {
        for (int i = 0; i < 8; i++) {
            b[off + i] = (byte) (v >>> (8 * i));
        }
    }

    public static long getLe64(byte[] b, int off) {
        long v = 0L;
        for (int i = 0; i < 8; i++) {
            v |= (b[off + i] & 0xffL) << (8 * i);
        }
        return v;
    }

    public static byte[] le64(long v) {
        byte[] b = new byte[8];
        putLe64(b, 0, v);
        return b;
    }

    public static byte[] concat(byte[]... parts) {
        int n = 0;
        for (byte[] p : parts) {
            n += p.length;
        }
        byte[] o = new byte[n];
        int i = 0;
        for (byte[] p : parts) {
            System.arraycopy(p, 0, o, i, p.length);
            i += p.length;
        }
        return o;
    }

    public static String hex(byte[] b) {
        char[] c = new char[b.length * 2];
        for (int i = 0; i < b.length; i++) {
            int v = b[i] & 0xff;
            c[i * 2] = HEX[v >>> 4];
            c[i * 2 + 1] = HEX[v & 0xf];
        }
        return new String(c);
    }

    /** 16 lowercase hex digits, big-endian spelling of the u64 (§7 / CASES). */
    public static String hexU64(long v) {
        char[] c = new char[16];
        for (int i = 0; i < 16; i++) {
            int shift = (15 - i) * 4;
            c[i] = HEX[(int) ((v >>> shift) & 0xf)];
        }
        return new String(c);
    }

    public static byte[] parseHex(String s) {
        if ((s.length() & 1) != 0) {
            throw new IllegalArgumentException("odd hex length");
        }
        byte[] o = new byte[s.length() / 2];
        for (int i = 0; i < o.length; i++) {
            int hi = Character.digit(s.charAt(i * 2), 16);
            int lo = Character.digit(s.charAt(i * 2 + 1), 16);
            if (hi < 0 || lo < 0) {
                throw new IllegalArgumentException("bad hex");
            }
            o[i] = (byte) ((hi << 4) | lo);
        }
        return o;
    }

    public static byte[] reverse(byte[] b) {
        byte[] o = new byte[b.length];
        for (int i = 0; i < b.length; i++) {
            o[i] = b[b.length - 1 - i];
        }
        return o;
    }

    public static boolean eq(byte[] a, byte[] b) {
        return Arrays.equals(a, b);
    }
}
