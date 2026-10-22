package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Path;

import org.junit.jupiter.api.Test;

/** AC-A10. sdk/java/check/negatives.sh prints PASS and exits 0. */
public class ACA10ReleaseTest {
    @Test
    public void scriptPasses() throws Exception {
        Path root = Path.of("").toAbsolutePath();
        Process p = new ProcessBuilder(root.resolve("check/negatives.sh").toString())
                .directory(root.toFile())
                .redirectErrorStream(true)
                .start();
        String out = new String(p.getInputStream().readAllBytes());
        int code = p.waitFor();
        assertEquals(0, code, out);
        assertTrue(out.contains("PASS"), out);
    }
}
