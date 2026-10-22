package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Path;

import org.junit.jupiter.api.Test;

/** AC-A3. sdk/SPEC_PIN vector-sha256 is the SHA-256 of the pack manifest. */
public class ACA3PinTest {
    @Test
    public void pinMatchesManifest() throws Exception {
        Path sdk = BuildInfo.sdkDir();
        Pin pin = Pin.read(sdk.resolve("SPEC_PIN"));
        assertEquals("spec-2.2.0", pin.tag);
        Path manifest = sdk.getParent().resolve("spec/conformance/pack/manifest.json");
        assertTrue(manifest.toFile().isFile());
        assertTrue(pin.matches(manifest), pin.vectorSha256 + " vs " + Pin.sha256(manifest));
    }
}
