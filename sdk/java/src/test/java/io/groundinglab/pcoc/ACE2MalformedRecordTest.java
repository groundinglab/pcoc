package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;

import org.junit.jupiter.api.Test;

/**
 * Erratum 2 (E01, E02). A malformed CBOR value, including an empty input, bytes
 * that end inside an item, and any text string (map key or {@code kind}) that is
 * not valid UTF-8, prints one
 * verify record. An empty path byte string on a decoded stage stays {@code skip},
 * and a decoded empty checkpoint list stays count 0 (SPEC line 961, Rule A4.5).
 */
public class ACE2MalformedRecordTest {
    /** SHA-256 of the one-line record plus its trailing newline. */
    static final String RECORD_SHA256 = "eb7bff4d55ddcc47e47c600276693db8a406a0083a47c2526462651ffa5bdb8c";

    @Test
    public void emptyInputRecord() throws Exception {
        assertMalformedRecord("empty", new byte[0]);
    }

    @Test
    public void truncatedItemRecord() throws Exception {
        assertMalformedRecord("truncated", new byte[] {0x58, 0x05, 0x01});
    }

    @Test
    public void invalidUtf8KindRecord() throws Exception {
        assertMalformedRecord("utf8", new byte[] {
            (byte) 0xa1, 0x64, 0x6b, 0x69, 0x6e, 0x64, 0x61, (byte) 0xff
        });
    }

    @Test
    public void decodedEmptyPathKeepsSkipAndZeroCount() throws Exception {
        byte[] object = decodedEmptyPath();
        Verify.Report report = Verify.verify(object, Profile.production(), Index.EMPTY);
        assertTrue(report.decoded, "decoded");
        String json = report.json;
        assertContains(json, "\"g\":1,\"path_state\":\"skip\"", "path_state");
        assertContains(json, "\"g\":1,\"path_state\":\"skip\",\"plant\":\"n/a\"", "plant");
        assertContains(json, "\"g\":1,\"checkpoint_count\":0,", "checkpoint_count");
        assertContains(json, "\"skip_list\":[1,", "skip_list");
    }

    private static void assertMalformedRecord(String name, byte[] input) throws Exception {
        byte[] expected = expectedRecord();
        assertEquals(RECORD_SHA256, sha256(expected), "fixture");
        Verify.Report report = Verify.verify(input, Profile.production(), Index.EMPTY);
        assertTrue(report.decoded, name);
        assertRecord(name + " library", expected, (report.json + "\n").getBytes(StandardCharsets.UTF_8));

        Repo.root();
        Path tmp = Files.createTempDirectory("pcoc-e2");
        try {
            Path file = tmp.resolve(name + ".cbor");
            Files.write(file, input);
            Capture cap = Capture.run("verify", file.toString());
            assertEquals(0, cap.code, name);
            assertEquals("", cap.err, name);
            assertRecord(name + " cli", expected, cap.out.getBytes(StandardCharsets.UTF_8));
            assertEquals(RECORD_SHA256, sha256(cap.out.getBytes(StandardCharsets.UTF_8)), name);
        } finally {
            delete(tmp);
        }
    }

    private static void assertRecord(String name, byte[] expected, byte[] actual) {
        if (Arrays.equals(expected, actual)) {
            return;
        }
        String field = fieldAt(expected, actual);
        fail(name + ": field " + field + " differs; expected " + window(expected, actual)
                + " actual " + window(actual, expected));
    }

    private static void assertContains(String json, String needle, String field) {
        if (!json.contains(needle)) {
            fail("field " + field + " differs; missing " + needle);
        }
    }

    /** A decoded object: stage 1 stores an empty path and an empty checkpoint list. */
    private static byte[] decodedEmptyPath() {
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        Cbor.writeType(o, 5, 2);
        Cbor.writeText(o, "kind");
        Cbor.writeText(o, Const.KIND);
        Cbor.writeText(o, "stages");
        Cbor.writeType(o, 4, 2);
        Cbor.writeType(o, 5, 1);
        Cbor.writeText(o, "checkpoints");
        Cbor.writeType(o, 4, 0);
        Cbor.writeType(o, 5, 2);
        Cbor.writeText(o, "path");
        Cbor.writeBytes(o, new byte[0]);
        Cbor.writeText(o, "checkpoints");
        Cbor.writeType(o, 4, 0);
        return o.toByteArray();
    }

    private static byte[] expectedRecord() throws Exception {
        InputStream in = ACE2MalformedRecordTest.class.getResourceAsStream("/erratum2-malformed-record.json");
        if (in == null) {
            throw new IllegalStateException("missing erratum2-malformed-record.json");
        }
        try (InputStream stream = in) {
            return stream.readAllBytes();
        }
    }

    private static String fieldAt(byte[] expected, byte[] actual) {
        int n = Math.min(expected.length, actual.length);
        int i = 0;
        while (i < n && expected[i] == actual[i]) {
            i++;
        }
        int colon = -1;
        for (int k = Math.min(i, expected.length - 1); k >= 1; k--) {
            if (expected[k] == ':' && expected[k - 1] == '"') {
                colon = k;
                break;
            }
        }
        if (colon < 2) {
            return "record";
        }
        int keyEnd = colon - 1;
        int keyStart = keyEnd - 1;
        while (keyStart > 0 && expected[keyStart] != '"') {
            keyStart--;
        }
        if (expected[keyStart] != '"') {
            return "record";
        }
        return new String(expected, keyStart + 1, keyEnd - keyStart - 1, StandardCharsets.UTF_8);
    }

    private static String window(byte[] focus, byte[] other) {
        int n = Math.min(focus.length, other.length);
        int i = 0;
        while (i < n && focus[i] == other[i]) {
            i++;
        }
        int from = Math.max(0, i - 48);
        int to = Math.min(focus.length, i + 48);
        return new String(focus, from, to - from, StandardCharsets.UTF_8).replace("\n", "\\n");
    }

    private static String sha256(byte[] bytes) throws Exception {
        return Bytes.hex(MessageDigest.getInstance("SHA-256").digest(bytes));
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
