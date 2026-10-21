package pcoc.hike;

import java.math.BigInteger;

/**
 * Section 15 comparisons. Rule B2 reads {@code (hike_legs, hike_span)}.
 * Rule B4 reads {@code anchored_total}. The section 14 record does not
 * carry the judgment. The floor {@code 1/5} is the test placeholder in
 * section 15.5. It is not a ratified value and it is not a section 6 constant.
 */
public final class Admit {
    /** Placeholder numerator. Not ratified. */
    public static final long PLACEHOLDER_FLOOR_P = 1;
    /** Placeholder denominator. Not ratified. */
    public static final long PLACEHOLDER_FLOOR_Q = 5;
    /** Rule B4. Settled. Anywhere among {@code W_1} … {@code W_16}. */
    public static final int MIN_ANCHORED_PATHS = 6;

    private Admit() {}

    /**
     * Rule B2. Meets when {@code hike_span > 0} and {@code q · hike_legs ≥ p · hike_span}.
     * The comparison does not round. A span that is not a positive integer does not meet.
     */
    public static boolean legRateMeets(long hikeLegs, long hikeSpan, long p, long q) {
        if (hikeSpan <= 0 || hikeLegs < 0 || p < 0 || q <= 0) {
            return false;
        }
        BigInteger left = BigInteger.valueOf(q).multiply(BigInteger.valueOf(hikeLegs));
        BigInteger right = BigInteger.valueOf(p).multiply(BigInteger.valueOf(hikeSpan));
        return left.compareTo(right) >= 0;
    }

    /** Rule B2 with the placeholder floor {@code 1/5}. */
    public static boolean placeholderRateMeets(long hikeLegs, long hikeSpan) {
        return legRateMeets(hikeLegs, hikeSpan, PLACEHOLDER_FLOOR_P, PLACEHOLDER_FLOOR_Q);
    }

    /**
     * Rule B4. Each index in {@code 1 … 16} counts once. {@code W_17} does not count.
     * There is no first-planted rule and no last-planted rule.
     */
    public static boolean anchorsMeet(int... indices) {
        boolean[] seen = new boolean[17];
        int count = 0;
        if (indices != null) {
            for (int g : indices) {
                if (g >= 1 && g <= 16 && !seen[g]) {
                    seen[g] = true;
                    count++;
                }
            }
        }
        return count >= MIN_ANCHORED_PATHS;
    }

    /** Rule B4 on a count already taken over {@code W_1} … {@code W_16}. */
    public static boolean anchorsMeetCount(int anchoredTotal) {
        return anchoredTotal >= MIN_ANCHORED_PATHS;
    }
}
