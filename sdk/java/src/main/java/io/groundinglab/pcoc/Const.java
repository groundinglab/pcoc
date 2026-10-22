package io.groundinglab.pcoc;

import java.nio.charset.StandardCharsets;

/** §6 constants. Profile formulas live on {@link Profile}. */
public final class Const {
    public static final int SLOT_BYTES = 16384;
    public static final int WORDS_PER_SLOT = 2048;
    public static final int TERRAIN_BYTES = 268435456;
    public static final int N_SLOTS = 16384;
    public static final int SAMPLE_BYTES = 1024;
    public static final int OFFSET_MODULUS = 268434433;
    public static final int STEPS_PER_LEG = 2_000_000;
    public static final int STAGES = 18;
    public static final int HASH_BYTES = 32;
    public static final int LEG_RESULT_BYTES = 8;
    public static final int PUBLIC_KEY_BYTES = 32;
    public static final int BEACON_BYTES = 32;

    public static final byte[] SEAL_DOMAIN = "pcoc-walk-v1".getBytes(StandardCharsets.US_ASCII);
    public static final String KIND = "v2.2-pcoc-object";
    public static final String FILL = "dag2";

    public static final long MIX_ADD = 0x9E3779B97F4A7C15L;
    public static final long MIX_MUL1 = 0xBF58476D1CE4E5B9L;
    public static final long MIX_MUL2 = 0x94D049BB133111EBL;
    public static final long WORD_XOR = 0xC0FFEE;
    public static final long PARENT_MUL = 0xD1B54A32D192ED03L;
    public static final long SLOT_MUL = 0xA24BAED4963EE407L;

    /** §13.1. The only anchor-window offsets. */
    public static final int[] ANCHOR_WINDOW_OFFSETS = {2, 3, 4};

    /** §13.1. The one Hashcash difficulty. */
    public static final int HASHCASH_ZERO_BITS = 21;

    public static final byte[] HASHCASH_PREFIX = "bari-hc-v2".getBytes(StandardCharsets.US_ASCII);

    /** §14.6. The draw count. Not a section 6 constant. */
    public static final int SIX_LEG = 6;

    private Const() {}

    static {
        if (SEAL_DOMAIN.length != 12) {
            throw new ExceptionInInitializerError("SEAL_DOMAIN");
        }
        if (WORDS_PER_SLOT != SLOT_BYTES / 8) {
            throw new ExceptionInInitializerError("WORDS_PER_SLOT");
        }
        if (N_SLOTS != TERRAIN_BYTES / SLOT_BYTES) {
            throw new ExceptionInInitializerError("N_SLOTS");
        }
        if (OFFSET_MODULUS != TERRAIN_BYTES - SAMPLE_BYTES + 1) {
            throw new ExceptionInInitializerError("OFFSET_MODULUS");
        }
    }
}
