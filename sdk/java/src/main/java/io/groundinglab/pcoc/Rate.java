package io.groundinglab.pcoc;

import java.math.BigInteger;

/**
 * Rule B2 integer comparison (§15.4). The comparison result is {@code meets} when
 * {@code hike_span} is an integer greater than 0 and {@code q · hike_legs ≥ p · hike_span},
 * and {@code refuse} otherwise (§4 Comparison result; erratum E05).
 */
public final class Rate {
    private Rate() {}

    /** §15.4 Rule B2. {@code p} and {@code q} are the floor ratio {@code p/q}. */
    public static String compare(long hikeLegs, long hikeSpan, long p, long q) {
        if (p < 1 || q < 1) {
            throw new IllegalArgumentException("floor");
        }
        if (hikeSpan <= 0) {
            return "refuse";
        }
        BigInteger left = BigInteger.valueOf(q).multiply(BigInteger.valueOf(hikeLegs));
        BigInteger right = BigInteger.valueOf(p).multiply(BigInteger.valueOf(hikeSpan));
        return left.compareTo(right) >= 0 ? "meets" : "refuse";
    }
}
