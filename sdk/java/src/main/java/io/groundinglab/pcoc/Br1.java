package io.groundinglab.pcoc;

import java.math.BigInteger;
import java.util.Arrays;

/**
 * §13.6 Rule BR1, the paid-waypoint check.
 * Block-hash comparisons are natural order on both sides (§9). This class does not
 * reverse a hash. A tip hash learned as Bitcoin display hex is reversed once before the call.
 */
public final class Br1 {
    /** §9. Every synth header carries this compact {@code nBits}. */
    public static final int SYNTH_NBITS = 0x207fffff;

    /** Bitcoin mainnet proof-of-work limit, compact form. */
    public static final int MAINNET_POW_LIMIT_BITS = 0x1d00ffff;

    private static final int RETARGET_INTERVAL = 2016;
    private static final long TARGET_TIMESPAN = 14L * 24 * 60 * 60;
    private static final long MIN_TIMESPAN = TARGET_TIMESPAN / 4;
    private static final long MAX_TIMESPAN = TARGET_TIMESPAN * 4;
    private static final BigInteger MASK_256 = BigInteger.ONE.shiftLeft(256).subtract(BigInteger.ONE);

    private Br1() {}

    /** §9. Which plane's {@code nBits} rule an after header uses. */
    public enum Plane {
        /** Fixed synth {@code nBits} {@code 0x207fffff} and the little-endian proof-of-work check. */
        TEST,
        /** Bitcoin mainnet consensus target for the block after the tip, including a retarget. */
        LIVE
    }

    /** §13.6. Where the fee header sits against the tip. {@code OUTSIDE} is not paid. */
    public enum Window {
        CURRENT, BEFORE, AFTER, OUTSIDE
    }

    /**
     * Bari's tip in the form Rule BR1 compares.
     * {@code hash} and {@code previous} are natural order. {@code hash} is
     * {@code SHA-256(SHA-256(tip header))}. {@code previous} is the 32 bytes at offset 4
     * of the tip header. {@code periodFirstTime} is the {@code nTime} at height
     * {@code height - 2015}, and is read only when {@code height + 1} is a multiple of 2016.
     * Height, time, and bits are {@code u32} values in the low 32 bits.
     */
    public static final class Tip {
        public final byte[] hash;
        public final byte[] previous;
        public final int height;
        public final int time;
        public final int bits;
        public final int periodFirstTime;

        public Tip(byte[] hash, byte[] previous, int height, int time, int bits, int periodFirstTime) {
            if (hash == null || previous == null || hash.length != 32 || previous.length != 32) {
                throw new IllegalArgumentException("tip");
            }
            this.hash = hash.clone();
            this.previous = previous.clone();
            this.height = height;
            this.time = time;
            this.bits = bits;
            this.periodFirstTime = periodFirstTime;
        }
    }

    /**
     * §13.1. Shortest decimal {@code u64}, no sign, no leading zeros. Zero is {@code 0}.
     * Returns null when the spelling is not that decimal. The bits are the unsigned value:
     * a magnitude above {@code Long.MAX_VALUE} is negative if read as a signed long.
     */
    public static Long parseNonce(String raw) {
        if (raw == null || raw.isEmpty()) {
            return null;
        }
        for (int i = 0; i < raw.length(); i++) {
            char c = raw.charAt(i);
            if (c < '0' || c > '9') {
                return null;
            }
        }
        if (raw.length() > 1 && raw.charAt(0) == '0') {
            return null;
        }
        try {
            return Long.valueOf(Long.parseUnsignedLong(raw));
        } catch (NumberFormatException e) {
            return null;
        }
    }

    /**
     * §13.1. 160 lowercase hex digits, the 80 fee-header bytes, byte 0 first.
     * Returns null when the spelling is not that.
     */
    public static byte[] parseFeeHeader(String raw) {
        if (raw == null || raw.length() != 160) {
            return null;
        }
        for (int i = 0; i < raw.length(); i++) {
            char c = raw.charAt(i);
            boolean digit = c >= '0' && c <= '9';
            boolean lower = c >= 'a' && c <= 'f';
            if (!digit && !lower) {
                return null;
            }
        }
        return Bytes.parseHex(raw);
    }

    /**
     * §13.6 Rule BR1. {@code 204} when the post is a paid waypoint, {@code 402} otherwise.
     * {@code nonceDecimal} is the nonce-header spelling. {@code feeHeaderHex} is the
     * fee-header spelling. {@code bariOrigin} is Bari's own public origin, already
     * serialized as §13.1 states.
     */
    public static int status(
            Sha256 sha,
            byte[] body,
            String nonceDecimal,
            String feeHeaderHex,
            String bariOrigin,
            Tip tip,
            Plane plane) {
        byte[] header = parseFeeHeader(feeHeaderHex);
        if (header == null) {
            return 402;
        }
        Long nonce = parseNonce(nonceDecimal);
        if (nonce == null) {
            return 402;
        }
        if (body == null || body.length != 32) {
            return 402;
        }
        if (window(sha, header, tip, plane) == Window.OUTSIDE) {
            return 402;
        }
        byte[] hash = headerHash(sha, header);
        byte[] challenge = Hashcash.challenge(sha, bariOrigin, hash);
        byte[] digest = Hashcash.digest(sha, challenge, body, nonce.longValue());
        if (Hashcash.leadingZeroBits(digest) < Const.HASHCASH_ZERO_BITS) {
            return 402;
        }
        return 204;
    }

    /**
     * §13.6 Rule BR1 step 2, before the Hashcash check.
     * A header whose length is not 80 is {@link Window#OUTSIDE}.
     */
    public static Window window(Sha256 sha, byte[] header, Tip tip, Plane plane) {
        if (header == null || header.length != 80) {
            return Window.OUTSIDE;
        }
        byte[] hash = headerHash(sha, header);
        if (Arrays.equals(hash, tip.hash)) {
            return Window.CURRENT;
        }
        if (Arrays.equals(hash, tip.previous)) {
            return Window.BEFORE;
        }
        byte[] previous = Arrays.copyOfRange(header, 4, 36);
        if (Arrays.equals(previous, tip.hash)
                && bitsOf(header) == requiredBits(tip, plane)
                && powMeets(sha, header)) {
            return Window.AFTER;
        }
        return Window.OUTSIDE;
    }

    /** §9 and §13.6. The compact target an after header must carry. */
    public static int requiredBits(Tip tip, Plane plane) {
        if (plane == Plane.TEST) {
            return SYNTH_NBITS;
        }
        return nextBits(tip.height, tip.bits, tip.time, tip.periodFirstTime);
    }

    /**
     * §9. Bitcoin mainnet {@code GetNextWorkRequired} for the block after {@code height}.
     * Between retargets the compact target is the tip's {@code nBits}. On a 2016-block
     * boundary the timespan is {@code tipTime - periodFirstTime}, clamped to a factor of
     * four around two weeks, and the new target is clamped to the mainnet proof-of-work
     * limit. The multiply keeps 256 bits, as Bitcoin {@code arith_uint256} does.
     */
    public static int nextBits(int height, int tipBits, int tipTime, int periodFirstTime) {
        // height, tipTime, and periodFirstTime are u32. The height sum wraps mod 2^32.
        if (Integer.remainderUnsigned(height + 1, RETARGET_INTERVAL) != 0) {
            return tipBits;
        }
        long span = (tipTime & 0xffffffffL) - (periodFirstTime & 0xffffffffL);
        if (span < MIN_TIMESPAN) {
            span = MIN_TIMESPAN;
        }
        if (span > MAX_TIMESPAN) {
            span = MAX_TIMESPAN;
        }
        BigInteger target = setCompact(tipBits).target;
        target = target.multiply(BigInteger.valueOf(span)).and(MASK_256);
        target = target.divide(BigInteger.valueOf(TARGET_TIMESPAN));
        BigInteger limit = setCompact(MAINNET_POW_LIMIT_BITS).target;
        if (target.compareTo(limit) > 0) {
            target = limit;
        }
        return getCompact(target);
    }

    /** §13.1. {@code SHA-256(SHA-256(header))}, natural order. */
    public static byte[] headerHash(Sha256 sha, byte[] header) {
        return sha.hash(sha.hash(header));
    }

    /**
     * 80-byte header. Version is 1. {@code previous} is natural order at offset 4.
     * {@code nTime} is at 68, {@code nBits} at 72, nonce at 76. Multi-byte fields are little-endian.
     */
    public static byte[] assembleHeader(byte[] previous, int nTime, int bits, int nonce) {
        if (previous == null || previous.length != 32) {
            throw new IllegalArgumentException("previous");
        }
        byte[] header = new byte[80];
        putLe32(header, 0, 1);
        System.arraycopy(previous, 0, header, 4, 32);
        putLe32(header, 68, nTime);
        putLe32(header, 72, bits);
        putLe32(header, 76, nonce);
        return header;
    }

    /** Bitcoin {@code arith_uint256::SetCompact}. The flags are {@code (negative, overflow)}. */
    static final class Compact {
        final BigInteger target;
        final boolean negative;
        final boolean overflow;

        Compact(BigInteger target, boolean negative, boolean overflow) {
            this.target = target;
            this.negative = negative;
            this.overflow = overflow;
        }
    }

    static Compact setCompact(int bits) {
        int size = bits >>> 24;
        long word = bits & 0x007fffffL;
        BigInteger value;
        if (word == 0) {
            value = BigInteger.ZERO;
        } else if (size <= 3) {
            value = BigInteger.valueOf(word >>> (8 * (3 - size)));
        } else {
            value = BigInteger.valueOf(word).shiftLeft(8 * (size - 3)).and(MASK_256);
        }
        boolean negative = word != 0 && (bits & 0x00800000) != 0;
        boolean overflow = word != 0
                && (size > 34 || (word > 0xff && size > 33) || (word > 0xffff && size > 32));
        return new Compact(value, negative, overflow);
    }

    /** Bitcoin {@code arith_uint256::GetCompact} for a non-negative value. */
    static int getCompact(BigInteger value) {
        if (value.signum() == 0) {
            return 0;
        }
        int size = (value.bitLength() + 7) / 8;
        long compact;
        if (size <= 3) {
            compact = value.longValue() << (8 * (3 - size));
        } else {
            compact = value.shiftRight(8 * (size - 3)).longValue();
        }
        if ((compact & 0x00800000L) != 0) {
            compact >>>= 8;
            size += 1;
        }
        return (int) ((compact & 0x007fffffL) | ((long) size << 24));
    }

    /** A negative compact, an overflow compact, or a zero target does not prove work. */
    private static boolean powMeets(Sha256 sha, byte[] header) {
        Compact decoded = setCompact(bitsOf(header));
        if (decoded.negative || decoded.overflow || decoded.target.signum() == 0) {
            return false;
        }
        return le256(headerHash(sha, header)).compareTo(decoded.target) <= 0;
    }

    private static BigInteger le256(byte[] le) {
        byte[] be = new byte[33];
        for (int i = 0; i < 32; i++) {
            be[32 - i] = le[i];
        }
        return new BigInteger(be);
    }

    private static int bitsOf(byte[] header) {
        return (header[72] & 0xff)
                | ((header[73] & 0xff) << 8)
                | ((header[74] & 0xff) << 16)
                | ((header[75] & 0xff) << 24);
    }

    private static void putLe32(byte[] b, int off, int v) {
        b[off] = (byte) v;
        b[off + 1] = (byte) (v >>> 8);
        b[off + 2] = (byte) (v >>> 16);
        b[off + 3] = (byte) (v >>> 24);
    }
}
