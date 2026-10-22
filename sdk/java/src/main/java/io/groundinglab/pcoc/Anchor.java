package io.groundinglab.pcoc;

/**
 * §13.1 anchor window. A height is in the window only at H+2, H+3, or H+4.
 * The outcome word is {@code anchored} or {@code not anchored}. The full §14.5 token
 * also requires a stored interrupt and plant match; this method is the integer window check.
 */
public final class Anchor {
    private Anchor() {}

    public static String window(long plantHeightH, long headerHeight) {
        long d = headerHeight - plantHeightH;
        for (int off : Const.ANCHOR_WINDOW_OFFSETS) {
            if (d == off) {
                return "anchored";
            }
        }
        return "not anchored";
    }
}
