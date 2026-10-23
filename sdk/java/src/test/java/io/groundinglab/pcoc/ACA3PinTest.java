package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;

import org.junit.jupiter.api.Test;

/** AC-A3. sdk/SPEC_PIN vector-sha256 is the SHA-256 of the pack manifest. */
public class ACA3PinTest {
    @Test
    public void pinMatchesManifest() throws Exception {
        Path repo = Repo.root();
        Path sdk = repo.resolve("sdk");
        Path pinFile = sdk.resolve("SPEC_PIN");
        Pin pin = Pin.read(pinFile);
        // The release job owns sdk/SPEC_PIN. Do not hardcode spec-2.2.0 or spec-2.3.0.
        assertFalse(pin.tag.isEmpty(), "SPEC_PIN tag");
        assertTrue(pin.tag.startsWith("spec-"), pin.tag);
        Path manifest = repo.resolve("spec").resolve("conformance").resolve("pack").resolve("manifest.json");
        assertTrue(Files.isRegularFile(manifest), manifest.toString());
        assertTrue(pin.matches(manifest), pin.vectorSha256 + " vs " + Pin.sha256(manifest));
    }
}
