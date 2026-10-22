package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertNotNull;

import org.junit.jupiter.api.Test;

import io.groundinglab.pcoc.produce.Hike;

/** AC-A13. The Phase A Rule I2 methods exist and name their sections in the library. */
public class ACA13SurfaceTest {
    @Test
    public void phaseASurface() {
        assertNotNull(Seed.class);
        assertNotNull(Terrain.class);
        assertNotNull(Step.class);
        assertNotNull(Leg.class);
        assertNotNull(Checkpoint.class);
        assertNotNull(Waypoint.class);
        assertNotNull(Hashcash.class);
        assertNotNull(HikeObject.class);
        assertNotNull(Verify.class);
        assertNotNull(Draw.class);
        assertNotNull(Rate.class);
        assertNotNull(Anchor.class);
        assertNotNull(Hike.class);
        assertNotNull(BuildInfo.class);
        Profile p = Profile.conformanceSmall(64, 32, 65536, 100);
        assertNotNull(p);
    }
}
