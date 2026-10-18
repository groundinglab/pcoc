package pcoc.hike;

import java.security.MessageDigest;
import java.util.Locale;

public final class Bytes {
    static final long MASK = -1L;

    private Bytes() {}

    public static byte[] sha256(byte[] one) {
        return digest().digest(one);
    }

    public static byte[] sha256(byte[]... parts) {
        MessageDigest md = digest();
        for (byte[] part : parts) {
            md.update(part);
        }
        return md.digest();
    }

    static byte[] sha256(byte[] a, byte[] b, byte[] blob, int off, int len, byte[] d) {
        MessageDigest md = digest();
        md.update(a);
        md.update(b);
        md.update(blob, off, len);
        md.update(d);
        return md.digest();
    }

    public static MessageDigest digest() {
        try {
            return MessageDigest.getInstance("SHA-256");
        } catch (Exception ex) {
            throw new IllegalStateException(ex);
        }
    }

    public static long u64le(byte[] data) {
        return u64le(data, 0);
    }

    static long u64le(byte[] data, int off) {
        long value = 0;
        for (int i = 7; i >= 0; i--) {
            value = (value << 8) | (data[off + i] & 255L);
        }
        return value;
    }

    public static byte[] le64(long value) {
        byte[] out = new byte[8];
        putLe64(out, 0, value);
        return out;
    }

    public static void putLe64(byte[] out, int off, long value) {
        for (int i = 0; i < 8; i++) {
            out[off + i] = (byte) (value >>> (8 * i));
        }
    }

    public static byte[] concat(byte[] left, byte[] right) {
        byte[] out = new byte[left.length + right.length];
        System.arraycopy(left, 0, out, 0, left.length);
        System.arraycopy(right, 0, out, left.length, right.length);
        return out;
    }

    static boolean eq(byte[] left, byte[] right) {
        if (left == null || right == null || left.length != right.length) {
            return false;
        }
        int diff = 0;
        for (int i = 0; i < left.length; i++) {
            diff |= left[i] ^ right[i];
        }
        return diff == 0;
    }

    public static String hex(byte[] raw) {
        StringBuilder out = new StringBuilder(raw.length * 2);
        for (byte b : raw) {
            out.append(String.format("%02x", b & 255));
        }
        return out.toString();
    }

    /**
     * SPEC §9. Bitcoin display order is the natural bytes reversed:
     * {@code display[i] = natural[natural.length - 1 - i]}. Verify
     * {@code block_hash} on {@code P_g}, {@code P_b}, and {@code claimed_block}
     * uses this. {@code stored_hash} stays {@link #hex}, byte 0 first.
     */
    public static String displayHex(byte[] natural) {
        byte[] display = new byte[natural.length];
        for (int i = 0; i < natural.length; i++) {
            display[i] = natural[natural.length - 1 - i];
        }
        return hex(display);
    }

    public static byte[] unhex(String spelling) {
        String text = spelling;
        if (text.startsWith("0x") || text.startsWith("0X")) {
            text = text.substring(2);
        }
        if ((text.length() & 1) != 0) {
            return null;
        }
        byte[] out = new byte[text.length() / 2];
        for (int i = 0; i < out.length; i++) {
            int hi = fromHex(text.charAt(i * 2));
            int lo = fromHex(text.charAt(i * 2 + 1));
            if (hi < 0 || lo < 0) {
                return null;
            }
            out[i] = (byte) ((hi << 4) | lo);
        }
        return out;
    }

    private static int fromHex(char c) {
        if (c >= '0' && c <= '9') {
            return c - '0';
        }
        char lower = Character.toLowerCase(c);
        if (lower >= 'a' && lower <= 'f') {
            return lower - 'a' + 10;
        }
        return -1;
    }

    public static String pin(long value) {
        return String.format(Locale.ROOT, "%016x", value);
    }
}
