package pcoc.hike;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/**
 * SPEC v2.1 section 6, rules P6.1-P6.25. P6.14 is retired.
 * v2.sh runs this; a changed constant exits 1.
 */
public final class Spec6Pins {
    private static int failures = 0;

    private static void pin(String rule, long got, long want) {
        if (got != want) {
            System.err.println(rule + " = " + got + ", want " + want);
            failures++;
        }
    }

    private static void pin(String rule, boolean ok) {
        if (!ok) {
            System.err.println(rule + " failed");
            failures++;
        }
    }

    /** Section 6 names this port still has. P6.14 {@code SEED_DOMAIN} is retired and is not a key. */
    public static java.util.Map<String, Object> section6() {
        Profile p = Profile.PRODUCTION;
        java.util.Map<String, Object> m = new java.util.LinkedHashMap<>();
        m.put("SLOT_BYTES", (long) p.slotBytes);
        m.put("WORDS_PER_SLOT", (long) p.wordsPerSlot);
        m.put("TERRAIN_BYTES", (long) p.terrainBytes);
        m.put("N_SLOTS", (long) p.nSlots);
        m.put("SAMPLE_BYTES", (long) p.sampleBytes);
        m.put("OFFSET_MODULUS", Long.valueOf(p.offsetModulus));
        m.put("STEPS_PER_LEG", (long) p.stepsPerLeg);
        m.put("STAGES", (long) Hike.STAGES);
        m.put("HASH_BYTES", (long) Hike.HASH_BYTES);
        m.put("LEG_RESULT_BYTES", (long) Hike.LEG_RESULT_BYTES);
        m.put("PUBLIC_KEY_BYTES", (long) Hike.PUBLIC_KEY_BYTES);
        m.put("BEACON_BYTES", (long) Hike.BEACON_BYTES);
        m.put("SEAL_DOMAIN", Bytes.hex(Terrain.SEAL_DOMAIN));
        m.put("KIND", Hike.KIND);
        m.put("FILL", Hike.FILL);
        m.put("MIX_ADD", String.format("0x%016X", Terrain.MIX_ADD));
        m.put("MIX_MUL1", String.format("0x%016X", Terrain.MIX_MUL1));
        m.put("MIX_MUL2", String.format("0x%016X", Terrain.MIX_MUL2));
        m.put("WORD_XOR", String.format("0x%X", Terrain.WORD_XOR));
        m.put("PARENT_MUL", String.format("0x%016X", Terrain.PARENT_MUL));
        m.put("SLOT_MUL", String.format("0x%016X", Terrain.SLOT_MUL));
        return m;
    }

    public static void main(String[] args) {
        Profile p = Profile.PRODUCTION;
        pin("P6.1 SLOT_BYTES", p.slotBytes, 16384);
        pin("P6.2 WORDS_PER_SLOT", p.wordsPerSlot, 2048);
        pin("P6.3 TERRAIN_BYTES", p.terrainBytes, 268435456L);
        pin("P6.4 N_SLOTS", p.nSlots, 16384);
        pin("P6.5 SAMPLE_BYTES", p.sampleBytes, 1024);
        pin("P6.6 OFFSET_MODULUS", p.offsetModulus, 268434433L);
        pin("P6.7 STEPS_PER_LEG", p.stepsPerLeg, 2000000);
        pin("P6.8 STAGES", Hike.STAGES, 18);
        pin("P6.9 HASH_BYTES", Hike.HASH_BYTES, 32);
        pin("P6.10 LEG_RESULT_BYTES", Hike.LEG_RESULT_BYTES, 8);
        pin("P6.11 PUBLIC_KEY_BYTES", Hike.PUBLIC_KEY_BYTES, 32);
        pin("P6.12 BEACON_BYTES", Hike.BEACON_BYTES, 32);
        pin("P6.13 SEAL_DOMAIN", Arrays.equals(Terrain.SEAL_DOMAIN, "pcoc-walk-v1".getBytes(StandardCharsets.US_ASCII)));
        pin("P6.14 RETIRED", retiredSeedDomain());
        pin("P6.15 KIND", "satoshi-hike-v2.0-object".equals(Hike.KIND));
        pin("P6.16 FILL", "dag2".equals(Hike.FILL));
        pin("P6.17 MIX_ADD", Terrain.MIX_ADD, 0x9E3779B97F4A7C15L);
        pin("P6.18 MIX_MUL1", Terrain.MIX_MUL1, 0xBF58476D1CE4E5B9L);
        pin("P6.19 MIX_MUL2", Terrain.MIX_MUL2, 0x94D049BB133111EBL);
        pin("P6.20 WORD_XOR", Terrain.WORD_XOR, 0xC0FFEEL);
        pin("P6.21 PARENT_MUL", Terrain.PARENT_MUL, 0xD1B54A32D192ED03L);
        pin("P6.22 SLOT_MUL", Terrain.SLOT_MUL, 0xA24BAED4963EE407L);
        Profile s = Profile.CONFORMANCE_SMALL_V2;
        pin("16.1 SAMPLE_BYTES", s.sampleBytes, 32);
        pin("16.1 SLOT_BYTES", s.slotBytes, 64);
        pin("16.1 TERRAIN_BYTES", s.terrainBytes, 256);
        pin("16.1 STEPS_PER_LEG", s.stepsPerLeg, 32);
        pin("P6.23 small WORDS_PER_SLOT", s.wordsPerSlot, 8);
        pin("P6.24 small N_SLOTS", s.nSlots, 4);
        pin("P6.25 small OFFSET_MODULUS", s.offsetModulus, 225);
        pin("16.1 KIND", Hike.KIND.equals(s.kind));
        if (failures != 0) {
            System.exit(1);
        }
        System.out.println("spec6 pins ok");
    }

    /** P6.14 is retired. The class must not bring {@code SEED_DOMAIN} back. */
    private static boolean retiredSeedDomain() {
        for (java.lang.reflect.Field field : Terrain.class.getDeclaredFields()) {
            if ("SEED_DOMAIN".equals(field.getName())) {
                return false;
            }
        }
        return true;
    }
}
