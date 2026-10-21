package pcoc.hike;

/** Names fixed by sections 6 and 8. The leg-rate pairs are §15.5 examples. */
public final class Hike {
    public static final int STAGES = 18;
    public static final int HASH_BYTES = 32;
    public static final int LEG_RESULT_BYTES = 8;
    public static final int PUBLIC_KEY_BYTES = 32;
    public static final int BEACON_BYTES = 32;
    /** Earlier kind string. Rule A5. Not a profile. */
    public static final String KIND_V135 = "satoshi-hike-v1.35-object";
    /** Earlier kind string. Rule A5. A v2.0 object is not measured under v2.1. */
    public static final String KIND_V20 = "satoshi-hike-v2.0-object";
    /** SPEC v2.1 P6.15. */
    public static final String KIND = "satoshi-hike-v2.1-object";
    public static final String FILL = "dag2";
    /**
     * Rule A5 measurement. The record key is {@code version}.
     * Gate reason {@code earlier_version} is section 15 and is not written here.
     */
    public static final String EARLIER_VERSION = "earlier version";
    /**
     * How many Rule A4.3 checkpoint comparisons run at once when the caller
     * does not set a count. Not {@code STEPS_PER_LEG}. One at a time is
     * conforming. The verify record does not depend on this number.
     */
    public static final int DEFAULT_BIND_THREADS = 6;
    public static final long JSON_INT_MAX = Index.JSON_INT_MAX;
    /** Count of blocks in the path-ask window. */
    public static final int ANCHOR_WINDOW = 3;
    /**
     * SPEC v2.0 P13.2, written once. A plant posted at H is asked at H+2,
     * H+3, and H+4.
     */
    private static final int[] ANCHOR_OFFSETS_AFTER_POST = {2, 3, 4};
    // No ratified leg-rate floor (v2.1 Rule B2). The value is TBD.
    // Admit holds the placeholder 1/5. It is not a section 6 constant.
    // Bookend (N, S) is not the admission rate. Rule B4 is anchored_total >= 6.

    private Hike() {}

    public static int[] anchorOffsetsAfterPost() {
        return ANCHOR_OFFSETS_AFTER_POST.clone();
    }

    /**
     * SPEC section 13.1. The one height H is the stage's stored beacon height.
     * The Hashcash tip on the POST is not H. The object does not store a
     * second post height.
     */
    public static long anchorWindowOrigin(long stageBeaconHeight) {
        return stageBeaconHeight;
    }

    /**
     * Rule A5. Kind {@code satoshi-hike-v1.35-object} or {@code satoshi-hike-v2.0-object}.
     * The check is the kind text. It runs before any terrain fill.
     */
    public static boolean isEarlierKind(String kind) {
        return KIND_V135.equals(kind) || KIND_V20.equals(kind);
    }

    /**
     * {@code PCOC_BIND_THREADS}. Blank, unset, and non-positive text use
     * {@link #DEFAULT_BIND_THREADS}. The count is not {@code STEPS_PER_LEG}.
     */
    public static int bindThreadsFrom(String text) {
        if (text == null) {
            return DEFAULT_BIND_THREADS;
        }
        text = text.trim();
        if (text.isEmpty()) {
            return DEFAULT_BIND_THREADS;
        }
        try {
            int count = Integer.parseInt(text);
            if (count < 1) {
                return DEFAULT_BIND_THREADS;
            }
            return count;
        } catch (NumberFormatException ex) {
            return DEFAULT_BIND_THREADS;
        }
    }

    public static boolean aboveJson(long value) {
        return Long.compareUnsigned(value, JSON_INT_MAX) > 0;
    }
}
