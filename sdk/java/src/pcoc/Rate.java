package pcoc;

import java.math.BigInteger;

/**
 * Rule B2 integer comparison, the one the leg-rate floor uses.
 * {@code q · hike_legs ≥ p · hike_span} when {@code hike_span > 0}.
 */
public final class Rate {
    private Rate() {}

    /**
     * Outcome word of the comparison. {@code hike_span ≤ 0} is {@code refuse} (Rule B2).
     * A span that meets the inequality is {@code match}. A positive span that falls short is {@code mismatch}.
     * Rule B2 judges that shortfall as {@code refuse}; this method returns the comparison token.
     */
    public static String compare(long hikeLegs, long hikeSpan, long p, long q) {
        if (p < 1 || q < 1) {
            throw new IllegalArgumentException("floor");
        }
        if (hikeSpan <= 0) {
            return "refuse";
        }
        BigInteger left = BigInteger.valueOf(q).multiply(BigInteger.valueOf(hikeLegs));
        BigInteger right = BigInteger.valueOf(p).multiply(BigInteger.valueOf(hikeSpan));
        return left.compareTo(right) >= 0 ? "match" : "mismatch";
    }
}
