package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;

import org.junit.jupiter.api.Test;

/**
 * AC-A7. §18 and §7.3 (the decode-failure sentence, SPEC.md line 497):
 * one well-formed data item that is not a map, or a map with no kind text, exits 1
 * with no record. A malformed CBOR value, including an empty input, is a record and exits 0.
 */
public class ACA7CliTest {
    @Test
    public void usageAndExits() throws Exception {
        Path tmp = Files.createTempDirectory("pcoc-cli");
        try {
            Capture none = Capture.run();
            assertEquals(2, none.code);
            Capture.assertNoAdmissionWords(none.out + none.err);

            Capture bare = Capture.run("verify");
            assertEquals(2, bare.code);

            Capture small = Capture.run("verify", "--profile", "conformance-small", tmp.resolve("nope").toString());
            assertEquals(2, small.code);

            Capture partial = Capture.run("verify", "--profile", "conformance-small",
                    "--slot-bytes", "64", tmp.resolve("nope").toString());
            assertEquals(2, partial.code);

            Capture missing = Capture.run("verify", tmp.resolve("missing.cbor").toString());
            assertEquals(1, missing.code);
            assertEquals("", missing.out);
            assertTrue(missing.err.contains("object not read"));
            Capture.assertNoAdmissionWords(missing.out + missing.err);

            Path empty = tmp.resolve("empty.cbor");
            Files.write(empty, new byte[0]);
            Capture emp = Capture.run("verify", empty.toString());
            assertEquals(0, emp.code);
            assertTrue(emp.out.contains("\"malformed cbor\""));
            assertTrue(emp.out.endsWith("\n"));
            assertFalse(emp.out.endsWith("\n\n"));
            assertTrue(emp.out.contains("\"profile\":\"production\""));
            Capture.assertNoAdmissionWords(emp.out + emp.err);

            Path notMap = tmp.resolve("uint.cbor");
            Files.write(notMap, new byte[] {0x00});
            Capture uint = Capture.run("verify", notMap.toString());
            assertEquals(1, uint.code);
            assertEquals("", uint.out);

            Path emptyMap = tmp.resolve("map.cbor");
            Files.write(emptyMap, new byte[] {(byte) 0xa0});
            Capture map = Capture.run("verify", emptyMap.toString());
            assertEquals(1, map.code);
            assertEquals("", map.out);

            // kind present but not a text string: decode failure (§7.3 line 497).
            Path kindInt = tmp.resolve("kind-int.cbor");
            Files.write(kindInt, new byte[] {(byte) 0xa1, 0x64, 0x6b, 0x69, 0x6e, 0x64, 0x01});
            Capture ki = Capture.run("verify", kindInt.toString());
            assertEquals(1, ki.code);
            assertEquals("", ki.out);

            // invalid UTF-8 in a text string is not a CBOR text string (§7.3 line 495): malformed record.
            Path badUtf = tmp.resolve("bad-utf8.cbor");
            Files.write(badUtf, new byte[] {(byte) 0xa1, 0x64, 0x6b, 0x69, 0x6e, 0x64, 0x61, (byte) 0xff});
            Capture utf = Capture.run("verify", badUtf.toString());
            assertEquals(0, utf.code);
            assertTrue(utf.out.contains("\"malformed cbor\""));

            // trailing byte on a foreign-kind map is malformed, not Rule A5.
            Path trail = tmp.resolve("trail.cbor");
            Files.write(trail, new byte[] {(byte) 0xa1, 0x64, 0x6b, 0x69, 0x6e, 0x64, 0x60, 0x00});
            Capture tr = Capture.run("verify", trail.toString());
            assertEquals(0, tr.code);
            assertTrue(tr.out.contains("\"malformed cbor\""));
            assertFalse(tr.out.contains("unknown kind"));

            Path labelled = tmp.resolve("empty2.cbor");
            Files.write(labelled, new byte[0]);
            Capture lab = Capture.run("verify", "--draw-label", "pcoc-pack-2.2/draw", "--profile", "production",
                    labelled.toString());
            assertEquals(0, lab.code);
            assertTrue(lab.err.contains("draw: injected label (rig input)"));
            Capture smallLab = Capture.run("verify", "--profile", "conformance-small",
                    "--slot-bytes", "64", "--sample-bytes", "32", "--terrain-bytes", "65536",
                    "--steps-per-leg", "100", "--draw-label", "pcoc-pack-2.2/draw", labelled.toString());
            assertEquals(0, smallLab.code);
            assertTrue(smallLab.err.contains("draw: injected label (rig input)"));
            assertTrue(smallLab.out.contains("\"sample_bytes\":32"));
            assertTrue(smallLab.out.contains("\"conformance-small\""));
            Capture.assertNoAdmissionWords(lab.out + lab.err);
            Capture.assertNoAdmissionWords(smallLab.out + smallLab.err);
        } finally {
            delete(tmp);
        }
    }

    private static void delete(Path dir) throws Exception {
        if (!Files.exists(dir)) {
            return;
        }
        Files.walk(dir).sorted((a, b) -> b.compareTo(a)).forEach(p -> {
            try {
                Files.deleteIfExists(p);
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        });
    }
}
