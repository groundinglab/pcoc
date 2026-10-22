package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;

import org.junit.jupiter.api.Test;

import io.groundinglab.pcoc.gen.GenCandidates;
import io.groundinglab.pcoc.produce.Cases;

/**
 * AC-A5. Regenerated candidate files and scripted-rig objects match the committed
 * bytes, and each committed file matches its pinned SHA-256.
 */
public class ACA5Round2Test {
    @Test
    public void candidatesMatchReferenceAndStayStable() throws Exception {
        Repo.root();
        Path committed = Repo.javaDir().resolve("candidates");
        assertPinned(committed, Round2Pins.CANDIDATES);
        String prod = Files.readString(committed.resolve("prod.txt"));
        assertTrueLine(prod, "prod.PROD.stage.0.leg.0.legs_le_hex\t" + Round2Pins.PROD_LEG0);
        Path temp = Files.createTempDirectory("pcoc-cand");
        try {
            GenCandidates.generate(temp);
            for (String name : Round2Pins.CANDIDATES.keySet()) {
                byte[] got = Files.readAllBytes(temp.resolve(name));
                byte[] stored = Files.readAllBytes(committed.resolve(name));
                assertArrayEquals(stored, got, name);
            }
        } finally {
            delete(temp);
        }
        Sha256 sha = new Sha256();
        byte[] header = GenCandidates.feeHeader(sha);
        byte[] once = sha.hash(header);
        byte[] twice = sha.hash(once);
        assertFalse(Arrays.equals(once, twice));
        String hashcash = Files.readString(committed.resolve("hashcash.txt"));
        assertTrueLine(hashcash, "hashcash.header_hash\t" + Bytes.hex(twice));
        assertFalse(hashcash.contains(Bytes.hex(once)));
    }

    @Test
    public void scriptedRigObjects() throws Exception {
        Repo.root();
        Path committed = Repo.javaDir().resolve("objects");
        assertPinned(committed, Round2Pins.OBJECTS);
        Path temp = Files.createTempDirectory("pcoc-obj");
        try {
            Cases.writeObjects(temp);
            for (String name : Round2Pins.OBJECTS.keySet()) {
                byte[] stored = Files.readAllBytes(committed.resolve(name));
                byte[] got = Files.readAllBytes(temp.resolve(name));
                assertArrayEquals(stored, got, name);
                assertEquals(Round2Pins.OBJECTS.get(name), sha256(temp.resolve(name)), name);
            }
        } finally {
            delete(temp);
        }
    }

    private static void assertPinned(Path dir, Map<String, String> pins) throws Exception {
        assertTrue(Files.isDirectory(dir), dir.toString());
        List<String> found = new ArrayList<String>();
        try (Stream<Path> listing = Files.list(dir)) {
            listing.forEach(p -> found.add(p.getFileName().toString()));
        }
        assertEquals(pins.size(), found.size(), found.toString());
        for (Map.Entry<String, String> e : pins.entrySet()) {
            Path file = dir.resolve(e.getKey());
            assertTrue(Files.isRegularFile(file), e.getKey());
            assertEquals(e.getValue(), sha256(file), e.getKey());
        }
    }

    private static void assertTrueLine(String file, String line) {
        for (String row : file.split("\n")) {
            if (row.equals(line)) {
                return;
            }
        }
        throw new AssertionError("missing " + line);
    }

    static String sha256(Path file) throws Exception {
        return Bytes.hex(MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(file)));
    }

    private static void delete(Path dir) throws Exception {
        Files.walk(dir).sorted((a, b) -> b.compareTo(a)).forEach(p -> {
            try {
                Files.deleteIfExists(p);
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        });
    }
}
