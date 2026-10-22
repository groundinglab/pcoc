package io.groundinglab.pcoc;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * SHA-256 of the committed round-2 candidate files and scripted-rig objects.
 * The same bytes are what the independent builds agreed.
 */
public final class Round2Pins {
    /** Stage 0 leg 0 of the production candidate, little-endian hex. */
    public static final String PROD_LEG0 = "88a04ac5beacb8a7";

    public static final Map<String, String> CANDIDATES;
    public static final Map<String, String> OBJECTS;

    static {
        Map<String, String> candidates = new LinkedHashMap<String, String>();
        candidates.put("anchor.txt", "fe58c0b64263d11daa7dcbae7c6daa3e0b0daecb1084b44d47f6f2570da3811a");
        candidates.put("draw.txt", "45821e7ed5d9bbb6b03c686dfc467c623203027a6adf5160414c4e4074a85e0c");
        candidates.put("fold.txt", "2f14b5ef6953e00673c4c7f124f0057cb5498f17d354057639d944e441ffd271");
        candidates.put("hashcash.txt", "92f010b46c15483877343f635164c485500e55527721305bedd0a34def7ca9d5");
        candidates.put("kat-small.txt", "f999097217af06b00a8b879dfc9422263f63cc80b1462d41ff64d09ea866b192");
        candidates.put("negatives.txt", "a236c2966ae46b3e2a9bd83f4414fb4cf8f166106ce0553443aa4bc2e91462d4");
        candidates.put("nl4.txt", "f3fc7424ecd76dbe8ca7976d2fc780bb2a448206e6f116defa4b7cb67fb67a0b");
        candidates.put("prod.txt", "0eb0a567aa73350b80822c86ea3b3ec71ce8f3078bb59012019662cb5850c995");
        candidates.put("rate.txt", "153fbec4acbd3678953f4420d4e5af2f405e866ea16361cc50ab8142cfeef34f");
        candidates.put("seed.txt", "10179736fe1394a3275ad010ad87d5db03214ab3993d94d7cd27ec5c43802002");
        candidates.put("terrain.txt", "f5664d41b9bdc811adc2d7b321f518d33b13808746cf7016837948269019e348");
        candidates.put("test-3leg.txt", "621277600af9a31e40f244ac6010dbe059e602b3b48e9b92a8ad0c0b6d8f7eea");
        candidates.put("unknown-kind.txt", "351fe1fa59d6acd69039ca08704720dba7505155f0d5b26a7bb927321fbcccc2");
        candidates.put("wes.txt", "91c24b4c0e64b5e75bbe5422341cf01e09b25a12833196ac9ed8b898b5f6d768");
        CANDIDATES = Collections.unmodifiableMap(candidates);

        Map<String, String> objects = new LinkedHashMap<String, String>();
        objects.put("O1-S1.cbor", "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246");
        objects.put("O2-S2.cbor", "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0");
        objects.put("O3-PROD.cbor", "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7");
        OBJECTS = Collections.unmodifiableMap(objects);
    }

    private Round2Pins() {}
}
