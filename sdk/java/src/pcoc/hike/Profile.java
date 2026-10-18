package pcoc.hike;

/**
 * Section 6 sizes. conformance-small may shrink only the four free integers
 * named in section 16.1. The other three integers are the section 6 formulas.
 * The current production profile uses the locked v2.0 sample and step count.
 */
public final class Profile {
    public static final Profile PRODUCTION = new Profile(
            "production", Hike.KIND, 1024, 16_384, 268_435_456, 2_000_000);
    /**
     * SPEC section 16.1 on the v2.0 kind. The four integers are sample 32,
     * slot 64, terrain 256, and steps 32. The kind stays
     * {@code satoshi-hike-v2.0-object}.
     */
    public static final Profile CONFORMANCE_SMALL_V2 = new Profile(
            "conformance-small", Hike.KIND, 32, 64, 256, 32);

    /** Verifier name selector. {@code conformance-small} is the v2 profile. */
    public static Profile forVerify(String name) {
        return switch (name) {
            case "", "test", "live", "production" -> PRODUCTION;
            case "conformance-small" -> CONFORMANCE_SMALL_V2;
            default -> null;
        };
    }

    /** Produce name selector. {@code conformance-small} is not a produce profile. */
    public static Profile forProduce(String name) {
        return switch (name) {
            case "", "test", "live", "production", "slow-lan" -> PRODUCTION;
            default -> null;
        };
    }

    public final String name;
    public final String kind;
    public final int sampleBytes;
    public final int slotBytes;
    public final int terrainBytes;
    public final int stepsPerLeg;
    public final int wordsPerSlot;
    public final int nSlots;
    public final long offsetModulus;

    private Profile(String name, String kind, int sampleBytes, int slotBytes, int terrainBytes, int stepsPerLeg) {
        if (slotBytes < 8 || slotBytes % 8 != 0) {
            throw new IllegalArgumentException("SLOT_BYTES");
        }
        if (terrainBytes % slotBytes != 0) {
            throw new IllegalArgumentException("TERRAIN_BYTES");
        }
        if (sampleBytes < 1 || terrainBytes < sampleBytes || stepsPerLeg < 1 || kind == null || kind.isEmpty()) {
            throw new IllegalArgumentException("profile");
        }
        this.name = name;
        this.kind = kind;
        this.sampleBytes = sampleBytes;
        this.slotBytes = slotBytes;
        this.terrainBytes = terrainBytes;
        this.stepsPerLeg = stepsPerLeg;
        this.wordsPerSlot = slotBytes / 8;
        this.nSlots = terrainBytes / slotBytes;
        this.offsetModulus = (long) terrainBytes - sampleBytes + 1;
    }
}
