package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import java.nio.file.Path;

import org.junit.jupiter.api.Test;

/** AC-A9. Library constant and pcoc --version. The pin line is whatever sdk/SPEC_PIN names. */
public class ACA9VersionTest {
    @Test
    public void versionEverywhere() throws Exception {
        Path sdk = Repo.root().resolve("sdk");
        // sdk/VERSION is owned by the release job and may still name 2.2.0.
        // This SDK's identity is its own constant.
        assertEquals("2.3.0", BuildInfo.VERSION);
        assertEquals(BuildInfo.VERSION, Version.STRING);
        Pin pin = Pin.read(sdk.resolve("SPEC_PIN"));
        Capture c = Capture.run("--version");
        assertEquals(0, c.code);
        assertEquals(BuildInfo.VERSION + "\n" + pin.tag + "\n" + pin.vectorSha256 + "\n", c.out);
        Capture.assertNoAdmissionWords(c.out);
        Capture.assertNoAdmissionWords(c.err);
        assertFalse(c.out.isEmpty());
    }
}
