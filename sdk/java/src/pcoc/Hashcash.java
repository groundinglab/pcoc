package pcoc;

/** §13.1 Hashcash challenge, digest, and nonce search. Not the Bari paid rule. */
public final class Hashcash {
    private Hashcash() {}

    /**
     * §13.1. {@code SHA-256("bari-hc-v2" ‖ bari_origin ‖ header_hash)}.
     * {@code bariOrigin} is the ASCII origin serialization. {@code headerHash} is 32 natural bytes.
     */
    public static byte[] challenge(Sha256 sha, String bariOrigin, byte[] headerHash) {
        if (headerHash.length != 32) {
            throw new IllegalArgumentException("header_hash");
        }
        return sha.hash(Const.HASHCASH_PREFIX, bariOrigin.getBytes(java.nio.charset.StandardCharsets.US_ASCII), headerHash);
    }

    /** §13.1. {@code SHA-256(challenge ‖ waypoint ‖ le64(nonce))}. */
    public static byte[] digest(Sha256 sha, byte[] challenge, byte[] waypoint, long nonce) {
        byte[] n = Bytes.le64(nonce);
        return sha.hash(challenge, waypoint, n);
    }

    /**
     * §13.1. Leading zero bits from the first digest byte. Inside a byte the high bit is first.
     */
    public static int leadingZeroBits(byte[] digest) {
        int n = 0;
        for (byte b : digest) {
            int v = b & 0xff;
            if (v == 0) {
                n += 8;
                continue;
            }
            n += Integer.numberOfLeadingZeros(v) - 24;
            break;
        }
        return n;
    }

    /**
     * §13.1. The first nonce from 0 whose digest has at least 21 leading zero bits.
     */
    public static long solve(Sha256 sha, byte[] challenge, byte[] waypoint) {
        byte[] nonce = new byte[8];
        for (long n = 0L; n >= 0L; n++) {
            Bytes.putLe64(nonce, 0, n);
            byte[] d = sha.hash(challenge, waypoint, nonce);
            if (leadingZeroBits(d) >= Const.HASHCASH_ZERO_BITS) {
                return n;
            }
        }
        throw new IllegalStateException("nonce space exhausted");
    }
}
