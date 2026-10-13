package pcoc.hike;

/**
 * Anchor window for v2.0. Offsets are SPEC v2.0 P13.2. A canonical path
 * body yields its block height. The compact verify line is {@link Summary}.
 */
public final class Anchor {
    public interface HeaderBody {
        String body(long height);
    }

    private Anchor() {}

    /** block_height of a canonical path body, or null when the body does not validate. */
    public static Long pathInclusionHeight(byte[] path) {
        return PathBody.inclusionHeight(path);
    }

    /**
     * True when inclusion is one of the offsets after H and is also one of
     * the interrupt heights. H is {@link Hike#anchorWindowOrigin} of the
     * stage's stored beacon height. offsets must name {@link Hike#ANCHOR_WINDOW}
     * entries. A short list is not a window.
     */
    public static boolean inclusionInWindow(long inclusion, long stageBeaconHeight, int[] offsets, long[] interruptHeights) {
        if (offsets == null || offsets.length != Hike.ANCHOR_WINDOW) {
            return false;
        }
        long origin = Hike.anchorWindowOrigin(stageBeaconHeight);
        boolean matched = false;
        for (int off : offsets) {
            if (off < 0) {
                return false;
            }
            long sum = origin + Integer.toUnsignedLong(off);
            if (Long.compareUnsigned(sum, origin) < 0) {
                continue;
            }
            if (sum == inclusion) {
                matched = true;
            }
        }
        if (!matched || interruptHeights == null) {
            return false;
        }
        for (long height : interruptHeights) {
            if (height == inclusion) {
                return true;
            }
        }
        return false;
    }

    /**
     * Reads the index at inclusion once, and only when inclusion is inside
     * the window and is one of the interrupt heights. A miss reads nothing.
     */
    public static String bindInclusion(HeaderBody index, long inclusion, long stageBeaconHeight, int[] offsets, long[] interruptHeights) {
        if (index == null || !inclusionInWindow(inclusion, stageBeaconHeight, offsets, interruptHeights)) {
            return null;
        }
        return index.body(inclusion);
    }
}
