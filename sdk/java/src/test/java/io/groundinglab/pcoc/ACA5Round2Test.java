package io.groundinglab.pcoc;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;

import org.junit.jupiter.api.Test;

import io.groundinglab.pcoc.gen.GenCandidates;
import io.groundinglab.pcoc.produce.Cases;

/** AC-A5. Round-2 candidate files and scripted-rig objects, byte for byte. */
public class ACA5Round2Test {
    private static final String O1 = "800ad07a87a567c835f539617b61ccddc96a0a984425675a5ece971162dfd246";
    private static final String O2 = "6bfc769b831b62de86165d290b8cb8b4fc872d7759183ebbad4c0ab94c0be5e0";
    private static final String O3 = "b16ecdaa13ad6df28ae62ba84e653b90cbcc3b163af90b62a0db836ff6f3bea7";

    @Test
    public void candidatesMatchReferenceAndStayStable() throws Exception {
        Path job = Path.of("").toAbsolutePath().getParent().getParent().getParent();
        Path ref = job.resolve("ref-round2/C-grok/candidates");
        Path committed = Path.of("candidates").toAbsolutePath();
        Path temp = Files.createTempDirectory("pcoc-cand");
        try {
            GenCandidates.generate(temp);
            String[] names = {"seed", "nl4", "terrain", "fold", "kat-small", "test-3leg", "prod", "wes",
                    "rate", "anchor", "draw", "unknown-kind", "hashcash", "negatives"};
            for (String name : names) {
                byte[] got = Files.readAllBytes(temp.resolve(name + ".txt"));
                byte[] expect = Files.readAllBytes(ref.resolve(name + ".txt"));
                assertArrayEquals(expect, got, name);
                byte[] stored = Files.readAllBytes(committed.resolve(name + ".txt"));
                assertArrayEquals(stored, got, "rerun " + name);
            }
        } finally {
            delete(temp);
        }
        String prod = Files.readString(committed.resolve("prod.txt"));
        assertTrueLine(prod, "prod.PROD.stage.0.leg.0.legs_le_hex\t88a04ac5beacb8a7");
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
        Path temp = Files.createTempDirectory("pcoc-obj");
        try {
            Cases.writeObjects(temp);
            assertEquals(O1, sha256(temp.resolve("O1-S1.cbor")));
            assertEquals(O2, sha256(temp.resolve("O2-S2.cbor")));
            assertEquals(O3, sha256(temp.resolve("O3-PROD.cbor")));
            Path committed = Path.of("objects").toAbsolutePath();
            assertArrayEquals(Files.readAllBytes(temp.resolve("O1-S1.cbor")),
                    Files.readAllBytes(committed.resolve("O1-S1.cbor")));
            assertArrayEquals(Files.readAllBytes(temp.resolve("O2-S2.cbor")),
                    Files.readAllBytes(committed.resolve("O2-S2.cbor")));
            assertArrayEquals(Files.readAllBytes(temp.resolve("O3-PROD.cbor")),
                    Files.readAllBytes(committed.resolve("O3-PROD.cbor")));
        } finally {
            delete(temp);
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
