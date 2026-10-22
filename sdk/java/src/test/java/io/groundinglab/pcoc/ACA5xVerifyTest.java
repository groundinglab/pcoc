package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;

import org.junit.jupiter.api.Test;

import io.groundinglab.pcoc.produce.Cases;

/**
 * AC-A5x. The same injected draw and the CASES index make the §14 record identical
 * across every round-2 copy and this SDK's objects. Header reads the index can
 * supply are not {@code not read}.
 */
public class ACA5xVerifyTest {
    @Test
    public void crossVerify() throws Exception {
        Path job = Path.of("").toAbsolutePath().getParent().getParent().getParent();
        String[] dirs = {
                "ref-round2/A-sonnet/impl/objects",
                "ref-round2/B-sol/objects",
                "ref-round2/C-grok/objects",
                "ref-round2/D-gemini/objects",
                "ref-round2/E-kimi/objects",
                "ref-round2/F-glm/impl/objects",
                "pcoc/sdk/java/objects"
        };
        String[] names = {"O1-S1.cbor", "O2-S2.cbor", "O3-PROD.cbor"};
        Profile[] profiles = {Cases.s1(), Cases.s2(), Profile.production()};
        Index index = new SyntheticIndex();
        for (int n = 0; n < names.length; n++) {
            String record = null;
            for (String dir : dirs) {
                Path file = job.resolve(dir).resolve(names[n]);
                assertTrue(Files.isRegularFile(file), file.toString());
                Sha256 sha = new Sha256();
                byte[] b0 = Cases.asciiHash(sha, "pcoc-pack-2.2/draw");
                Verify.Report report = Verify.verifyRig(Files.readAllBytes(file), profiles[n], index,
                        new Draw.HashChain(sha, b0));
                assertTrue(report.decoded, names[n]);
                assertEquals("match", report.shape, names[n]);
                assertEquals("match", report.seedValue, names[n]);
                assertTrue(report.checkpointsMatch, names[n]);
                assertTrue(report.waypointsMatch, names[n]);
                assertEquals("match", report.sixLeg, names[n]);
                assertFalse(report.json.contains("\"not read\""), names[n] + " " + dir);
                if (record == null) {
                    record = report.json;
                } else {
                    assertEquals(record, report.json, names[n] + " " + dir);
                }
            }
        }
    }
}
