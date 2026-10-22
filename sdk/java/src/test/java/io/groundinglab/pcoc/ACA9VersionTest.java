package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import java.nio.file.Files;
import java.nio.file.Path;

import org.junit.jupiter.api.Test;

/** AC-A9. Library constant, sdk/VERSION, and pcoc --version. */
public class ACA9VersionTest {
    @Test
    public void versionEverywhere() throws Exception {
        Path sdk = BuildInfo.sdkDir();
        String file = Files.readString(sdk.resolve("VERSION")).trim();
        assertEquals(file, BuildInfo.VERSION);
        assertEquals("2.2.0", BuildInfo.VERSION);
        Pin pin = Pin.read(sdk.resolve("SPEC_PIN"));
        Capture c = Capture.run("--version");
        assertEquals(0, c.code);
        assertEquals("2.2.0\n" + pin.tag + "\n" + pin.vectorSha256 + "\n", c.out);
        Capture.assertNoAdmissionWords(c.out);
        Capture.assertNoAdmissionWords(c.err);
        assertFalse(c.out.isEmpty());
    }
}
