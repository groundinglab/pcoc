package pcoc.hike;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Section 15 comparisons. Rule B2 reads {@code (hike_legs, hike_span)}.
 * Rule B4 reads {@code anchored_total}. The section 14 record does not
 * carry the judgment. The floor {@code 1/5} and the deadline {@code K = 3}
 * are test placeholders. They are not ratified values and they are not
 * section 6 constants.
 */
public final class Admit {
    /** Placeholder numerator. Not ratified. */
    public static final long PLACEHOLDER_FLOOR_P = 1;
    /** Placeholder denominator. Not ratified. */
    public static final long PLACEHOLDER_FLOOR_Q = 5;
    /** Rule B4. Settled. Anywhere among {@code W_1} … {@code W_16}. */
    public static final int MIN_ANCHORED_PATHS = 6;
    /** Submission-deadline placeholder, in blocks. Not ratified. */
    public static final int PLACEHOLDER_DEADLINE_K = 3;
    /** Section 15 reason. Not a section 14 token. */
    public static final String SUBMISSION_DEADLINE = "submission_deadline";
    /** Section 15 reason when Rule A5 records an earlier kind. Not a fake leg. */
    public static final String EARLIER_VERSION_REASON = "earlier_version";
    /** Default policy. One draw, then this many rounds. */
    public static final int THOROUGH_ROUNDS = 3;
    /** Legs started together in one thorough round. */
    public static final int PER_ROUND = 6;
    /** Indices in one thorough draw when {@code leg_total} is at least 18. */
    public static final int THOROUGH_LEGS = 18;

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

    /**
     * Rule A4.10. {@code n = min(11, height + 1)}. Height 0 uses one header.
     */
    public static int mtpCount(long height) {
        if (height < 0) {
            throw new IllegalArgumentException("height");
        }
        if (height >= 10) {
            return 11;
        }
        return (int) height + 1;
    }

    /**
     * BIP113 median. Sort into non-decreasing order and take index {@code n / 2}.
     * The caller supplies every time in the window. A missing time is not a median.
     */
    public static long medianTimePast(long[] times) {
        if (times == null || times.length == 0) {
            throw new IllegalArgumentException("mtp");
        }
        long[] sorted = times.clone();
        java.util.Arrays.sort(sorted);
        return sorted[times.length / 2];
    }

    /**
     * {@code hike_span = MTP(last) − MTP(first)}. A missing median is not read,
     * which this method does not invent as an integer.
     */
    public static Long hikeSpan(Long firstMtp, Long lastMtp) {
        if (firstMtp == null || lastMtp == null) {
            return null;
        }
        return lastMtp - firstMtp;
    }

    /**
     * Submission deadline at receipt. A missing tip is {@code held}. A tip below
     * the last beacon is {@code held}. A gap greater than {@code k} refuses
     * {@code submission_deadline}. That refuse is not a fake leg. This method
     * does not fill terrain.
     */
    public static Gate submissionDeadline(Long tipHeight, long lastBeaconHeight, int k) {
        if (tipHeight == null || Long.compare(tipHeight, lastBeaconHeight) < 0) {
            return new Gate("held", null, false);
        }
        long gap = tipHeight - lastBeaconHeight;
        if (gap > k) {
            return new Gate("refuse", SUBMISSION_DEADLINE, false);
        }
        return new Gate("accept", null, false);
    }

    /**
     * Rule A5 then section 15. The reason is {@code earlier_version}. Not a fake
     * leg. The caller has not filled terrain.
     */
    public static Gate earlierVersion(String kind) {
        if (!Hike.isEarlierKind(kind)) {
            throw new IllegalArgumentException("kind");
        }
        return new Gate("refuse", EARLIER_VERSION_REASON, false);
    }

    /**
     * Section 14.6 unbiased draw. Returns null when {@code u} is at or above the
     * limit. Those bytes are consumed and are not an index.
     */
    public static Long unbiasedIndex(long draw, long legTotal) {
        if (legTotal <= 0) {
            throw new IllegalArgumentException("leg_total");
        }
        long mod = Measure.modPow64(legTotal);
        long limit = mod == 0 ? 0L : -mod;
        if (mod != 0 && Long.compareUnsigned(draw, limit) >= 0) {
            return null;
        }
        return Long.remainderUnsigned(draw, legTotal);
    }

    /**
     * Section 15.5. One draw of up to 18 indices, then rounds of 6.
     * {@code leg_total} below 6 is {@code not defined} and refuses. A mismatch
     * on a 1-based round does not start the next round. {@code draws} are the
     * raw {@code u64} values, in order, including values the limit discards.
     */
    public static Thorough thoroughRounds(long legTotal, long[] draws, Integer mismatchOnRound) {
        if (legTotal < 6) {
            return new Thorough("refuse", 0, new int[0][], "not defined");
        }
        if (draws == null) {
            throw new IllegalArgumentException("draws");
        }
        List<Long> phase = new ArrayList<>();
        Set<Long> seen = new HashSet<>();
        int cursor = 0;
        while (phase.size() < THOROUGH_LEGS) {
            if (cursor >= draws.length) {
                throw new IllegalArgumentException("short draw");
            }
            Long index = unbiasedIndex(draws[cursor], legTotal);
            cursor++;
            if (index == null) {
                continue;
            }
            long q = index;
            int groupStart = phase.size() - (phase.size() % PER_ROUND);
            boolean inGroup = false;
            for (int i = groupStart; i < phase.size(); i++) {
                if (phase.get(i) == q) {
                    inGroup = true;
                    break;
                }
            }
            if (inGroup) {
                continue;
            }
            if (seen.contains(q) && seen.size() < legTotal) {
                continue;
            }
            phase.add(q);
            seen.add(q);
        }
        int groups = phase.size() / PER_ROUND;
        List<int[]> kept = new ArrayList<>();
        int started = 0;
        for (int round = 0; round < groups && started < THOROUGH_ROUNDS; round++) {
            int[] group = new int[PER_ROUND];
            for (int j = 0; j < PER_ROUND; j++) {
                group[j] = phase.get(round * PER_ROUND + j).intValue();
            }
            started++;
            kept.add(group);
            if (mismatchOnRound != null && mismatchOnRound == started) {
                return new Thorough("refuse", started, kept.toArray(int[][]::new), null);
            }
        }
        return new Thorough("recorded", started, kept.toArray(int[][]::new), null);
    }

    /** A section 15 outcome. The section 14 record does not carry it. */
    public static final class Gate {
        public final String outcome;
        public final String reason;
        public final boolean fakeLegIgnore;

        Gate(String outcome, String reason, boolean fakeLegIgnore) {
            this.outcome = outcome;
            this.reason = reason;
            this.fakeLegIgnore = fakeLegIgnore;
        }
    }

    /** The thorough index draw after it has been split into rounds. */
    public static final class Thorough {
        public final String outcome;
        public final int roundsStarted;
        public final int[][] rounds;
        /** {@code not defined} when {@code leg_total} is below 6. Otherwise null. */
        public final String draw;

        Thorough(String outcome, int roundsStarted, int[][] rounds, String draw) {
            this.outcome = outcome;
            this.roundsStarted = roundsStarted;
            this.rounds = rounds;
            this.draw = draw;
        }
    }
}
