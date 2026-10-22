package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

/** AC-A8. SHA-256 is the JDK intrinsic. sha-ni only when the JVM flags and the CPU agree. */
public class ACA8ShaTest {
    @Test
    public void backendMatchesHost() throws Exception {
        Repo.root();
        assertEquals(null, BuildInfo.katProblem());
        String backend = BuildInfo.shaBackend();
        if (BuildInfo.cpuHasShaNi()) {
            assertEquals("sha-ni", backend, "CPU has SHA-NI and the backend is software");
            assertTrue(BuildInfo.jvmFlag("UseSHA"));
            assertTrue(BuildInfo.jvmFlag("UseSHA256Intrinsics"));
        } else {
            assertEquals("software", backend);
        }
        Capture c = Capture.run("selftest");
        assertEquals(0, c.code, c.out + c.err);
        assertTrue(c.out.contains("sha " + backend));
        assertTrue(c.out.contains("kernel scalar"));
        Capture.assertNoAdmissionWords(c.out);
        Capture.assertNoAdmissionWords(c.err);
    }
}
