package pcoc;

/** §11.2 leg result and §11.3 terrain-sample offset. */
public final class Leg {
    private Leg() {}

    /** §11.3. {@code u64_le(previous[0:8]) mod OFFSET_MODULUS}. */
    public static int sampleOffset(byte[] previous, int offsetModulus) {
        return (int) Long.remainderUnsigned(Bytes.getLe64(previous, 0), offsetModulus);
    }

    /**
     * §11.2. {@code STEPS_PER_LEG} steps from {@code S_0}. Each step calls {@link Step#foldSlot}.
     */
    public static long legResult(long[] terrain, Profile profile, Sha256 sha, long s0) {
        byte[] buf = new byte[20];
        System.arraycopy(Const.SEAL_DOMAIN, 0, buf, 0, Const.SEAL_DOMAIN.length);
        long s = s0;
        int words = profile.wordsPerSlot;
        int nSlots = profile.nSlots;
        int steps = profile.stepsPerLeg;
        for (int t = 0; t < steps; t++) {
            s = Step.step(terrain, words, nSlots, sha, buf, s);
        }
        return s;
    }
}
