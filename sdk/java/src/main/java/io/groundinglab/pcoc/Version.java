package io.groundinglab.pcoc;

/** Rule I5. This SDK reports 2.3.1 from its own constant. */
public final class Version {
    public static final String STRING = "2.3.1";

    private Version() {}

    public static String version() {
        return STRING;
    }
}
