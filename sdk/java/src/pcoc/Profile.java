package pcoc;

/**
 * The profile in force (§14, §16.1). Production is section 6.
 * {@code conformance-small} may change only slot, sample, terrain, and steps.
 */
public final class Profile {
    public final String name;
    public final int slotBytes;
    public final int sampleBytes;
    public final int terrainBytes;
    public final int stepsPerLeg;
    public final int wordsPerSlot;
    public final int nSlots;
    public final int offsetModulus;

    private Profile(String name, int slotBytes, int sampleBytes, int terrainBytes, int stepsPerLeg) {
        this.name = name;
        this.slotBytes = slotBytes;
        this.sampleBytes = sampleBytes;
        this.terrainBytes = terrainBytes;
        this.stepsPerLeg = stepsPerLeg;
        this.wordsPerSlot = slotBytes / 8;
        this.nSlots = terrainBytes / slotBytes;
        this.offsetModulus = terrainBytes - sampleBytes + 1;
    }

    /** §6 production constants. */
    public static Profile production() {
        return new Profile("production", Const.SLOT_BYTES, Const.SAMPLE_BYTES,
                Const.TERRAIN_BYTES, Const.STEPS_PER_LEG);
    }

    /**
     * §16.1. All four integers are required. This method does not substitute section 6.
     */
    public static Profile conformanceSmall(int slotBytes, int sampleBytes, int terrainBytes, int stepsPerLeg) {
        if (slotBytes < 8 || slotBytes > Const.SLOT_BYTES || slotBytes % 8 != 0) {
            throw new IllegalArgumentException("SLOT_BYTES");
        }
        if (sampleBytes < 1 || sampleBytes > Const.SAMPLE_BYTES) {
            throw new IllegalArgumentException("SAMPLE_BYTES");
        }
        if (stepsPerLeg < 1 || stepsPerLeg > Const.STEPS_PER_LEG) {
            throw new IllegalArgumentException("STEPS_PER_LEG");
        }
        if (terrainBytes > Const.TERRAIN_BYTES || terrainBytes % slotBytes != 0 || terrainBytes < sampleBytes) {
            throw new IllegalArgumentException("TERRAIN_BYTES");
        }
        return new Profile("conformance-small", slotBytes, sampleBytes, terrainBytes, stepsPerLeg);
    }

    public boolean isProduction() {
        return "production".equals(name);
    }
}
