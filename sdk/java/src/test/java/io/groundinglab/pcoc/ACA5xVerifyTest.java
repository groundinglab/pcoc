package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;

import org.junit.jupiter.api.Test;

import io.groundinglab.pcoc.produce.Cases;

/**
 * AC-A5x. The same section 14 checks on each committed object. The CASES synthetic
 * index is served by this harness on loopback. Header reads the index can supply
 * are not {@code not read}.
 */
public class ACA5xVerifyTest {
    @Test
    public void crossVerify() throws Exception {
        Repo.root();
        Path objects = Repo.javaDir().resolve("objects");
        String[] names = {"O1-S1.cbor", "O2-S2.cbor", "O3-PROD.cbor"};
        Profile[] profiles = {Cases.s1(), Cases.s2(), Profile.production()};
        for (String name : names) {
            Path file = objects.resolve(name);
            assertTrue(Files.isRegularFile(file), name);
            assertEquals(Round2Pins.OBJECTS.get(name), ACA5Round2Test.sha256(file), name);
        }
        try (LoopbackIndex index = new LoopbackIndex()) {
            assertTrue(index.base().startsWith("http://127.0.0.1:"), index.base());
            for (int n = 0; n < names.length; n++) {
                int before = index.gets();
                Path file = objects.resolve(names[n]);
                Sha256 sha = new Sha256();
                byte[] b0 = Cases.asciiHash(sha, "pcoc-pack-2.2/draw");
                Verify.Report report = Verify.verifyRig(Files.readAllBytes(file), profiles[n], index,
                        new Draw.HashChain(sha, b0));
                assertTrue(index.gets() > before, names[n] + " did not GET the loopback index");
                assertTrue(report.decoded, names[n]);
                assertEquals("match", report.shape, names[n]);
                assertEquals("match", report.seedValue, names[n]);
                assertTrue(report.checkpointsMatch, names[n]);
                assertTrue(report.waypointsMatch, names[n]);
                assertEquals("match", report.sixLeg, names[n]);
                assertFalse(report.json.contains("\"not read\""), names[n]);
            }
        }
    }
}
